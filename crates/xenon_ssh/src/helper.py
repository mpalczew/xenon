"""Xenon SSH file helper, protocol 1. Standard library only; runs as the SSH user."""
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

MAX_FILE = 16 * 1024 * 1024
SKIP = {'.git', '.svn', '.hg', 'node_modules', 'target', 'vendor', 'Pods',
        'dist', 'build', 'out', 'coverage', '.next', '.cache', '.cargo', '.rustup',
        '.venv', 'venv', '__pycache__', '.npm', '.yarn', '.gradle'}

# Host-level discovery mirrors the local picker's caps and skip list.
DEFAULT_ROOTS = ('src', 'dev', 'code', 'Projects', 'Developer')
SKIP_DISCOVER = {'node_modules', 'target', '.git', '.svn', '.hg', 'dist', 'build',
                 '.next', '.cache', 'vendor', '__pycache__', '.tox', '.venv', 'venv',
                 'shows', 'Library', 'Applications'}
MAX_DEPTH = 3
MAX_RESULTS = 20
MAX_COLLECT = 64
MAX_VISITS = 2500
MAX_SECONDS = 4.0
MAX_LISTED = 200


def snapshot(path):
    try:
        with open(path, 'rb') as source:
            data = source.read(MAX_FILE + 1)
    except FileNotFoundError:
        return None
    if len(data) > MAX_FILE:
        raise ValueError('Remote file exceeds 16 MiB')
    if b'\0' in data[:8000]:
        raise ValueError('Remote binary files cannot be edited')
    return {'text': data.decode('utf-8'),
            'revision': hashlib.sha256(data).hexdigest()}


def file_path(root, relative):
    if os.path.isabs(relative):
        raise ValueError('Expected a workspace-relative path')
    path = os.path.realpath(os.path.join(root, relative))
    if os.path.commonpath([root, path]) != root:
        raise ValueError('Path leaves the remote workspace')
    return path


def index(root):
    ignored = set()
    try:
        result = subprocess.run(['git', '-C', root, 'ls-files', '--others',
                                 '--ignored', '--exclude-standard', '-z', '--directory'],
                                stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        if result.returncode == 0:
            ignored = set(result.stdout.decode('utf-8').split('\0'))
    except FileNotFoundError:
        pass
    entries = []
    for directory, dirs, files in os.walk(root, followlinks=False):
        relative = os.path.relpath(directory, root)
        prefix = '' if relative == '.' else relative + '/'
        dirs[:] = sorted(d for d in dirs if d not in SKIP and prefix + d + '/' not in ignored)
        for name in dirs + sorted(files):
            path = prefix + name
            if path in ignored:
                continue
            absolute = os.path.join(directory, name)
            entries.append({'path': path, 'is_dir': os.path.isdir(absolute),
                            'is_symlink': os.path.islink(absolute)})
            if len(entries) > 200000:
                raise ValueError('Remote workspace exceeds 200,000 entries')
    return entries


def write(path, request):
    previous = snapshot(path)
    expected = request.get('expected')
    revision = '' if previous is None else previous['revision']
    if expected is not None and revision != expected:
        raise ValueError('File changed remotely; reload before saving')
    data = request['text'].encode('utf-8')
    if len(data) > MAX_FILE:
        raise ValueError('Remote file exceeds 16 MiB')
    parent = os.path.dirname(path)
    os.makedirs(parent, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix='.xenon-save-', dir=parent)
    try:
        if os.path.exists(path):
            os.fchmod(fd, os.stat(path).st_mode & 0o777)
        with os.fdopen(fd, 'wb') as destination:
            destination.write(data)
            destination.flush()
            os.fsync(destination.fileno())
        current = snapshot(path)
        revision = '' if current is None else current['revision']
        if expected is not None and revision != expected:
            raise ValueError('File changed remotely; reload before saving')
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)
    return {'text': request['text'], 'revision': hashlib.sha256(data).hexdigest()}


def home_dir():
    return os.path.realpath(os.path.expanduser('~'))


def resolve_dir(path, home):
    """Absolute real path for `~`, `~/x`, `/x`, or a path relative to home."""
    if '\0' in path:
        raise ValueError('Invalid remote path')
    if path == '~':
        path = home
    elif path.startswith('~/'):
        path = os.path.join(home, path[2:])
    elif not os.path.isabs(path):
        path = os.path.join(home, path)
    return os.path.realpath(path)


def child_dirs(path, follow_links):
    """Sorted subdirectory names; the seam tests replace instead of the disk."""
    try:
        with os.scandir(path) as entries:
            return sorted(e.name for e in entries if e.is_dir(follow_symlinks=follow_links))
    except OSError:
        return []


def has_git(path):
    return os.path.exists(os.path.join(path, '.git'))


def skipped(name):
    return name in SKIP_DISCOVER or name.startswith('.')


def match_quality(name, needle):
    """0 exact, 1 prefix, 2 contains, None no match (case-insensitive)."""
    name, needle = name.lower(), needle.lower()
    if not needle:
        return None
    if name == needle:
        return 0
    if name.startswith(needle):
        return 1
    return 2 if needle in name else None


class Budget:
    def __init__(self):
        self.visits = 0
        self.deadline = time.monotonic() + MAX_SECONDS

    def spent(self):
        return self.visits >= MAX_VISITS or time.monotonic() > self.deadline


def keep_found(found, entry):
    if len(found) < MAX_COLLECT:
        found.append(entry)
        return
    weaker = [i for i, other in enumerate(found) if other['quality'] > entry['quality']]
    if weaker:
        found[max(weaker, key=lambda i: found[i]['quality'])] = entry


def exact_count(found):
    return sum(1 for entry in found if entry['quality'] == 0)


def walk_for_name(root, needle, found, seen, budget):
    """Breadth-first, never following symlinks, so it stays under `root`."""
    queue = [(root, 0)]
    cursor = 0
    while cursor < len(queue):
        if budget.spent() or exact_count(found) >= MAX_RESULTS:
            return
        directory, depth = queue[cursor]
        cursor += 1
        budget.visits += 1
        for name in child_dirs(directory, False):
            if skipped(name):
                continue
            path = os.path.join(directory, name)
            quality = match_quality(name, needle)
            if quality is not None and path not in seen:
                seen.add(path)
                keep_found(found, {'path': path, 'git': has_git(path), 'quality': quality})
            if depth < MAX_DEPTH:
                queue.append((path, depth + 1))


def found_order(entry):
    path = entry['path']
    return (entry['quality'], path.count('/'), len(path), not entry['git'], path)


def discover(request):
    home = home_dir()
    needle = request['needle'].strip()
    scope = request.get('scope')
    if scope:
        roots = [resolve_dir(scope, home)]
    else:
        roots = [os.path.join(home, name) for name in DEFAULT_ROOTS]
    found, seen, budget = [], set(), Budget()
    if needle:
        for root in roots:
            walk_for_name(root, needle, found, seen, budget)
            if exact_count(found) >= MAX_RESULTS:
                break
    found.sort(key=found_order)
    return {'home': home, 'dirs': found[:MAX_RESULTS]}


def listable(name, prefix):
    if name.startswith('.'):
        return prefix.startswith('.') and name not in SKIP_DISCOVER
    return name not in SKIP_DISCOVER


def list_dirs(request):
    home = home_dir()
    directory = resolve_dir(request['path'], home)
    prefix = request.get('prefix', '')
    names = [n for n in child_dirs(directory, True)
             if listable(n, prefix) and n.lower().startswith(prefix.lower())]
    names.sort(key=lambda n: (n.startswith('.'), n.lower(), n))
    dirs = [{'path': os.path.join(directory, n), 'git': has_git(os.path.join(directory, n)),
             'quality': 1} for n in names[:MAX_LISTED]]
    return {'home': home, 'dirs': dirs}


HOST_OPERATIONS = {'discover': discover, 'list_dirs': list_dirs}


def dispatch(request):
    if request['op'] in HOST_OPERATIONS:
        return HOST_OPERATIONS[request['op']](request)
    root = os.path.realpath(os.path.expanduser(request['root']))
    if not os.path.isdir(root):
        raise ValueError('Remote workspace directory does not exist')
    operation = request['op']
    if operation == 'probe':
        if not shutil.which('tmux'):
            raise ValueError('Install tmux on the SSH host for persistent terminals')
        return root
    if operation == 'index':
        return index(root)
    path = file_path(root, request['path'])
    if operation == 'read':
        current = snapshot(path)
        if current is None:
            return {'state': 'deleted'}
        if current['revision'] == request.get('revision'):
            return {'state': 'unchanged'}
        return {'state': 'file', 'snapshot': current}
    if operation == 'write':
        return write(path, request)
    raise ValueError('Unknown helper operation')


if __name__ == '__main__':
    try:
        print(json.dumps({'value': dispatch(json.load(sys.stdin))}))
    except Exception as error:
        print(json.dumps({'error': str(error)}))
