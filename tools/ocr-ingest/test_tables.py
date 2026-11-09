"""Verify row alignment and preservation of unresolved recognition output."""

import unittest
import json
import subprocess
import sys
import tempfile
from pathlib import Path

import tables


def word(text, x, y, confidence=.99):
    return {'text': text, 'box': [x, y, x + .04, y + .012], 'confidence': confidence}


class TableTest(unittest.TestCase):
    def test_row_alignment_and_coordinate_parsing(self):
        page = {'words': [word('标准名称', .1, .08), word('翠屏公社', .1, .12),
                          word('15515', .45, .12), word('105°16′', .62, .12),
                          word('公社', .85, .12), word('驻地', .83, .145),
                          word('27°18′', .73, .12), word('双树大队', .1, .18),
                          word('3845', .45, .18), word('105*17/', .62, .18),
                          word('27?18/', .73, .18), word('～１２一', .1, .94)]}
        table = tables.extract_table(page, 'bijie')
        self.assertTrue(table['layout_detected'])
        self.assertEqual(len(table['rows']), 2)
        first, second = table['rows']
        self.assertEqual(first['parsed']['population'], 15515)
        self.assertAlmostEqual(second['parsed']['longitude'], 105 + 17 / 60)
        self.assertEqual(first['cells']['longitude']['text'], '105°16′')
        self.assertEqual(first['cells']['note']['text'], '公社\n驻地')
        self.assertEqual(second['cells']['name']['text'], '双树大队')
        self.assertFalse(first['checked'])

    def test_unresolved_values_remain_visible(self):
        page = {'words': [word('标准名称', .1, .08), word('村名', .1, .12),
                          word('I00', .45, .12, .4), word('105°61′', .62, .12)]}
        row = tables.extract_table(page, 'bijie')['rows'][0]
        self.assertEqual(row['cells']['population']['text'], 'I00')
        self.assertIsNone(row['parsed']['population'])
        self.assertIsNone(row['parsed']['longitude'])
        self.assertIn('low_recognition_confidence', row['warnings'])
        self.assertIn('incomplete_coordinate', row['warnings'])
        self.assertFalse(tables.extract_table({'words': [word('村名', .1, .12)]}, 'bijie')['layout_detected'])

    def test_custom_layout_runs_through_cli_with_non_chinese_names(self):
        definition = dict(columns=[['feature', .05, .4], ['length', .4, .9]],
                          header_pattern='Feature', anchor_pattern=r'\S')
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / 'input'
            source.mkdir()
            profile = root / 'layout.json'
            profile.write_text(json.dumps(definition))
            page = dict(scan_sha256='a' * 64, scan_page=1,
                        words=[word('Feature', .1, .08), word('Runway A', .1, .12), word('1200 m', .5, .12)])
            (source / 'page-1.json').write_text(json.dumps(page))
            subprocess.run([sys.executable, str(Path(tables.__file__)), str(source), str(root / 'output'),
                            '--profile-file', str(profile)], check=True, capture_output=True)
            result = json.loads((root / 'output/page-1.table.json').read_text())
            self.assertTrue(result['layout_detected'])
            self.assertEqual(result['rows'][0]['cells']['feature']['text'], 'Runway A')
            self.assertEqual(result['rows'][0]['cells']['length']['text'], '1200 m')
            self.assertFalse(result['rows'][0]['checked'])
            self.assertEqual(result['profile_definition']['header_pattern'], 'Feature')
        with self.assertRaises(ValueError):
            tables.profile_definition(dict(columns=[['a', .1, .6], ['a', .5, .9]], header_pattern='Name'))


if __name__ == '__main__':
    unittest.main()
