#!/usr/bin/env python3
"""Extract page-local text and bounding boxes for gazetteer transcription review."""

import argparse
import hashlib
import json
import logging
from pathlib import Path
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET

LOG = logging.getLogger('atlas-ingest')


def run(command):
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f'{command[0]} failed ({result.returncode}): {result.stderr.strip()}')
    return result.stdout


def fingerprint(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def page_count(path):
    for line in run(['pdfinfo', str(path)]).splitlines():
        if line.startswith('Pages:'):
            return int(line.split(':', 1)[1])
    raise ValueError(f'pdfinfo did not report a page count for {path}')


def selected_pages(spec, count):
    first, separator, last = spec.partition(':')
    start = int(first)
    end = int(last) if separator else start
    if not 1 <= start <= end <= count:
        raise ValueError(f'page range {spec} is outside 1:{count}')
    return range(start, end + 1)


def text_layer(path, page):
    xml = run(['pdftotext', '-f', str(page), '-l', str(page),
               '-bbox-layout', str(path), '-'])
    document = ET.fromstring(xml)
    element = next(item for item in document.iter() if item.tag.endswith('}page'))
    width, height = float(element.attrib['width']), float(element.attrib['height'])
    words = []
    for item in element.iter():
        if not item.tag.endswith('}word'):
            continue
        box = item.attrib
        words.append({'text': ''.join(item.itertext()), 'confidence': None,
                      'box': [float(box['xMin']) / width, float(box['yMin']) / height,
                              float(box['xMax']) / width, float(box['yMax']) / height]})
    return words


def image_ocr(path, page, output, languages):
    with tempfile.TemporaryDirectory(prefix='.page-', dir=output) as directory:
        image = Path(directory) / 'page'
        run(['pdftoppm', '-f', str(page), '-l', str(page), '-r', '220',
             '-singlefile', '-png', str(path), str(image)])
        image = image.with_suffix('.png')
        import csv
        import io
        text = run(['tesseract', str(image), 'stdout', '-l', languages, 'tsv'])
        rows = list(csv.DictReader(io.StringIO(text), delimiter='\t'))
        page_row = next(row for row in rows if row['level'] == '1')
        width, height = int(page_row['width']), int(page_row['height'])
        words = []
        for row in rows:
            if row['level'] != '5' or not row['text'].strip():
                continue
            x, y, w, h = (int(row[field]) for field in ('left', 'top', 'width', 'height'))
            words.append({'text': row['text'], 'confidence': float(row['conf']) / 100,
                          'box': [x / width, y / height, (x + w) / width, (y + h) / height]})
        return words


def extract(args):
    path, output = args.pdf.resolve(), args.output.resolve()
    count, digest = page_count(path), fingerprint(path)
    pages = selected_pages(args.pages, count)
    output.mkdir(parents=True, exist_ok=True)
    for page in pages:
        destination = output / f'page-{page:04d}.json'
        identity = {'scan_sha256': digest, 'scan_page': page, 'engine': args.engine,
                    'languages': args.languages if args.engine == 'tesseract' else None}
        if destination.exists():
            existing = json.loads(destination.read_text())
            if any(existing.get(key) != value for key, value in identity.items()):
                raise ValueError(f'{destination} belongs to another source or extraction engine')
            LOG.info('Reusing page %s', page)
            continue
        words = text_layer(path, page) if args.engine == 'text' else image_ocr(
            path, page, output, args.languages)
        draft = {**identity, 'checked': False, 'box_origin': 'top_left_normalized', 'words': words}
        temporary = destination.with_suffix('.json.tmp')
        temporary.write_text(json.dumps(draft, ensure_ascii=False, indent=2) + '\n')
        temporary.replace(destination)
        LOG.info('Extracted page %s: %s text regions', page, len(words))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--pdf', type=Path, required=True)
    parser.add_argument('--pages', required=True, help='one-based page or inclusive first:last')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--engine', choices=('text', 'tesseract'), default='text')
    parser.add_argument('--languages', default='chi_sim+eng', help='Tesseract language models')
    args = parser.parse_args()
    commands = ['pdfinfo', 'pdftotext'] if args.engine == 'text' else ['pdfinfo', 'pdftoppm', 'tesseract']
    for command in commands:
        if shutil.which(command) is None:
            parser.error(f'{command} must be installed')
    logging.basicConfig(level=logging.INFO, format='%(levelname)s %(message)s')
    extract(args)


if __name__ == '__main__':
    main()
