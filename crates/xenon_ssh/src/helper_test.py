"""Deterministic helper checks; all filesystem/process access is mocked."""
import io
import os
import unittest
from unittest.mock import patch
import helper


class HelperChecks(unittest.TestCase):
    def test_read_returns_content_and_revision(self):
        with patch('builtins.open', return_value=io.BytesIO(b'hello')):
            snapshot = helper.snapshot('/virtual/file')
        self.assertEqual(snapshot['text'], 'hello')
        self.assertEqual(snapshot['revision'], '2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824')

    def test_missing_is_distinct_from_connection_failure(self):
        with patch('builtins.open', side_effect=FileNotFoundError):
            self.assertIsNone(helper.snapshot('/virtual/file'))
        with patch('builtins.open', side_effect=PermissionError):
            with self.assertRaises(PermissionError):
                helper.snapshot('/virtual/file')

    def test_binary_is_not_opened_as_text(self):
        with patch('builtins.open', return_value=io.BytesIO(b'hello\0world')):
            with self.assertRaisesRegex(ValueError, 'binary'):
                helper.snapshot('/virtual/file')

    def test_conflict_does_not_write(self):
        with patch.object(helper, 'snapshot', return_value={'revision': 'new'}), \
             patch.object(helper.tempfile, 'mkstemp') as create:
            with self.assertRaisesRegex(ValueError, 'changed remotely'):
                helper.write('/virtual/file', {'text': 'draft', 'expected': 'old'})
            create.assert_not_called()

    def test_creating_does_not_overwrite_an_existing_file(self):
        with patch.object(helper, 'snapshot', return_value={'revision': 'new'}), \
             patch.object(helper.tempfile, 'mkstemp') as create:
            with self.assertRaisesRegex(ValueError, 'changed remotely'):
                helper.write('/virtual/file', {'text': 'draft', 'expected': ''})
            create.assert_not_called()

    def test_paths_cannot_escape_the_workspace(self):
        with patch.object(helper.os.path, 'realpath', return_value='/outside/file'):
            with self.assertRaisesRegex(ValueError, 'leaves'):
                helper.file_path('/workspace', '../outside/file')
        with self.assertRaisesRegex(ValueError, 'relative'):
            helper.file_path('/workspace', '/outside/file')


TREE = {
    '/home/me': ['src', 'docs', '.config', 'Library'],
    '/home/me/src': ['xenon', 'xen-old', 'notes', 'node_modules', 'link'],
    '/home/me/src/xenon': ['crates', '.git', 'target'],
    '/home/me/src/xen-old': [],
    '/home/me/src/notes': ['deep'],
    '/home/me/src/notes/deep': ['deeper'],
    '/home/me/src/notes/deep/deeper': ['mid'],
    '/home/me/src/notes/deep/deeper/mid': ['xenial'],
    '/home/me/docs': ['xenon-spec'],
}
GIT = {'/home/me/src/xenon'}


def fake_children(path, follow_links):
    names = TREE.get(path, [])
    return sorted(n for n in names if follow_links or n != 'link')


class HostChecks(unittest.TestCase):
    def setUp(self):
        for target, value in [('home_dir', '/home/me'), ('child_dirs', fake_children)]:
            patcher = patch.object(helper, target, **(
                {'side_effect': value} if callable(value) else {'return_value': value}))
            patcher.start()
            self.addCleanup(patcher.stop)
        patcher = patch.object(helper, 'has_git', side_effect=lambda p: p in GIT)
        patcher.start()
        self.addCleanup(patcher.stop)
        patcher = patch.object(helper.os.path, 'realpath', side_effect=os.path.normpath)
        patcher.start()
        self.addCleanup(patcher.stop)

    def paths(self, result):
        return [d['path'] for d in result['dirs']]

    def test_name_search_uses_default_roots_and_ranks_like_local(self):
        result = helper.discover({'needle': 'xenon'})
        self.assertEqual(result['home'], '/home/me')
        self.assertEqual(self.paths(result), ['/home/me/src/xenon'])
        self.assertTrue(result['dirs'][0]['git'])
        result = helper.discover({'needle': 'xen'})
        self.assertEqual(self.paths(result)[:2], ['/home/me/src/xenon', '/home/me/src/xen-old'])

    def test_search_is_limited_by_depth_and_skips_junk(self):
        self.assertEqual(helper.discover({'needle': 'xenial'})['dirs'], [])
        self.assertEqual(helper.discover({'needle': 'node'})['dirs'], [])
        self.assertEqual(helper.discover({'needle': 'target', 'scope': '~/src/xenon'})['dirs'], [])

    def test_scoped_search_stays_under_its_root(self):
        result = helper.discover({'needle': 'xen', 'scope': '~/docs'})
        self.assertEqual(self.paths(result), ['/home/me/docs/xenon-spec'])

    def test_search_never_follows_symlinks_and_stops_on_budget(self):
        self.assertEqual(helper.discover({'needle': 'link'})['dirs'], [])
        self.assertEqual(self.paths(helper.discover({'needle': 'deep'})), ['/home/me/src/notes/deep', '/home/me/src/notes/deep/deeper'])
        with patch.object(helper, 'MAX_VISITS', 1):
            self.assertEqual(helper.discover({'needle': 'deep'})['dirs'], [])

    def test_results_are_capped(self):
        many = {'/home/me/src': ['app%02d' % i for i in range(40)]}
        with patch.dict(TREE, many):
            self.assertEqual(len(helper.discover({'needle': 'app'})['dirs']), helper.MAX_RESULTS)

    def test_list_dirs_sorts_filters_and_hides_dotted(self):
        result = helper.list_dirs({'path': '~/src', 'prefix': ''})
        self.assertEqual([os.path.basename(p) for p in self.paths(result)],
                         ['link', 'notes', 'xen-old', 'xenon'])
        result = helper.list_dirs({'path': '/home/me/src/', 'prefix': 'XEN'})
        self.assertEqual(self.paths(result), ['/home/me/src/xen-old', '/home/me/src/xenon'])
        self.assertEqual(helper.list_dirs({'path': 'src/xenon', 'prefix': '.g'})['dirs'], [])
        self.assertEqual(self.paths(helper.list_dirs({'path': '~', 'prefix': '.c'})),
                         ['/home/me/.config'])

    def test_list_dirs_relative_to_home_and_missing_is_empty(self):
        self.assertEqual(self.paths(helper.list_dirs({'path': 'docs'})), ['/home/me/docs/xenon-spec'])
        self.assertEqual(helper.list_dirs({'path': '/nope', 'prefix': ''})['dirs'], [])

    def test_host_ops_do_not_need_a_workspace_root(self):
        value = helper.dispatch({'op': 'list_dirs', 'path': '~', 'prefix': 'sr'})
        self.assertEqual(self.paths(value), ['/home/me/src'])
        with self.assertRaisesRegex(ValueError, 'Invalid'):
            helper.resolve_dir('~/a\0b', '/home/me')


if __name__ == '__main__':
    unittest.main()
