"""Verify row alignment and preservation of unresolved recognition output."""

import unittest

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


if __name__ == '__main__':
    unittest.main()
