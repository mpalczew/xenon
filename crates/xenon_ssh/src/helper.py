"""Xenon SSH file helper, protocol 1. Standard library only; runs as the SSH user."""
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile

MAX_FILE = 16 * 1024 * 1024
SKIP = {'.git', '.svn', '.hg', 'node_modules', 'target', 'vendor', 'Pods',
        'dist', 'build', 'out', 'coverage', '.next', '.cache', '.cargo', '.rustup',
        '.venv', 'venv', '__pycache__', '.npm', '.yarn', '.gradle'}


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


def dispatch(request):
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
