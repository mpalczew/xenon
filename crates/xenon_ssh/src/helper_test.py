"""Deterministic helper checks; all filesystem/process access is mocked."""
import io
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


if __name__ == '__main__':
    unittest.main()
