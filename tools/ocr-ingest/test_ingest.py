"""Exercise page extraction through Poppler and the command-line entry point."""

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import ingest


def write_pdf(path, text):
    content = f'BT /F1 12 Tf 40 150 Td ({text}) Tj ET'.encode('ascii')
    objects = [
        b'<< /Type /Catalog /Pages 2 0 R >>',
        b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] '
        b'/Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>',
        b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
        f'<< /Length {len(content)} >>\nstream\n'.encode() + content + b'\nendstream',
    ]
    data = bytearray(b'%PDF-1.4\n')
    offsets = []
    for number, body in enumerate(objects, 1):
        offsets.append(len(data))
        data.extend(f'{number} 0 obj\n'.encode() + body + b'\nendobj\n')
    xref = len(data)
    data.extend(b'xref\n0 6\n0000000000 65535 f \n')
    for offset in offsets:
        data.extend(f'{offset:010d} 00000 n \n'.encode())
    data.extend(f'trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode())
    path.write_bytes(data)


class ExtractionTest(unittest.TestCase):
    def test_extract_resume_and_source_mismatch(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pdf, output = root / 'source.pdf', root / 'drafts'
            write_pdf(pdf, 'Atlas evidence')
            command = [sys.executable, str(Path(ingest.__file__)), '--pdf', str(pdf),
                       '--pages', '1', '--output', str(output)]
            subprocess.run(command, check=True, capture_output=True)
            draft_path = output / 'page-0001.json'
            draft = json.loads(draft_path.read_text())
            self.assertFalse(draft['checked'])
            self.assertEqual(draft['scan_sha256'], ingest.fingerprint(pdf))
            self.assertEqual([word['text'] for word in draft['words']], ['Atlas', 'evidence'])
            self.assertTrue(all(0 <= n <= 1 for word in draft['words'] for n in word['box']))
            timestamp = draft_path.stat().st_mtime_ns
            subprocess.run(command, check=True, capture_output=True)
            self.assertEqual(timestamp, draft_path.stat().st_mtime_ns)
            write_pdf(pdf, 'Different source')
            result = subprocess.run(command, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('another source', result.stderr)
            self.assertEqual(timestamp, draft_path.stat().st_mtime_ns)

    def test_page_selection_rejects_nonexistent_and_reversed_ranges(self):
        self.assertEqual(list(ingest.selected_pages('2:3', 3)), [2, 3])
        for spec in ('0', '4', '3:2', '1:4', '-1'):
            with self.assertRaises(ValueError):
                ingest.selected_pages(spec, 3)


if __name__ == '__main__':
    unittest.main()
