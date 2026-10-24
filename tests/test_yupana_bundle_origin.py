"""Pinned whole-bundle refresh; host registration is a separate delivery step."""
import hashlib
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class YupanaBundleTests(unittest.TestCase):
    def test_release_bytes_match_reviewed_origin(self):
        raw = (ROOT / 'hook-bundles/yupana.json').read_bytes()
        origin = json.loads((ROOT / 'hook-bundles/yupana.origin.json').read_text())
        self.assertEqual(hashlib.sha256(raw).hexdigest(), origin['sha256'])
        self.assertEqual(origin['tag'], 'v0.15.4')
        bundle = json.loads(raw)
        self.assertEqual((bundle['name'], bundle['version'], bundle['owner']),
                         ('yupana', '0.15.4', 'yupana'))

    def test_refresh_keeps_failure_guard_and_existing_advisory_routes(self):
        hooks = json.loads((ROOT / 'hook-bundles/yupana.json').read_text())['hooks']
        for event, matcher, command in [
            ('PostToolUse', '.*', 'yupana hook post-edit'),
            ('PostToolUse', 'Bash', 'yupana hook post-bash'),
            ('PostToolUseFailure', 'Bash', 'yupana hook post-bash'),
            ('PreToolUse', 'apply_patch', 'yupana hook pre-edit'),
        ]:
            found = [h for h in hooks if h['event'] == event and h.get('matcher') == matcher]
            self.assertEqual(len(found), 1)
            self.assertIn(command, found[0]['command'])
        self.assertFalse(any('codex_notify' in h for h in hooks))

    def test_read_legend_is_opt_in_and_has_no_codex_enrollment(self):
        bundle = json.loads((ROOT / 'hook-bundles/yupana.json').read_text())
        reads = [h for h in bundle['hooks'] if 'hook post-read' in h['command']]
        self.assertEqual(len(reads), 1)
        self.assertEqual(reads[0]['harnesses'], ['claude'])
        self.assertEqual(reads[0]['matcher'], 'Read|Bash')
        self.assertNotIn('YUPANA_READ_LEGEND=', json.dumps(bundle))
        self.assertNotIn('DP_LEGEND=', json.dumps(bundle))
        self.assertNotIn('codex_notify', bundle)


if __name__ == '__main__':
    unittest.main()
