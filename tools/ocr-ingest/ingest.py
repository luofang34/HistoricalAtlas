#!/usr/bin/env python3
"""Extract page-local text and bounding boxes for gazetteer transcription review."""

import argparse
from concurrent.futures import ThreadPoolExecutor
import csv
import hashlib
import json
import io
import logging
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET

LOG = logging.getLogger('atlas-ingest')


def run(command, env=None):
    result = subprocess.run(command, capture_output=True, text=True, env=env)
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


def image_ocr(path, page, output, config):
    with tempfile.TemporaryDirectory(prefix='.page-', dir=output) as directory:
        image = Path(directory) / 'page'
        run(['pdftoppm', '-f', str(page), '-l', str(page), '-r', str(config['dpi']),
             '-singlefile', '-png', str(path), str(image)])
        image = image.with_suffix('.png')
        import os
        command = ['tesseract', str(image), 'stdout', '-l', config['languages'],
                   '--psm', str(config['psm'])]
        if config['tessdata_dir']:
            command.extend(['--tessdata-dir', config['tessdata_dir']])
        text = run([*command, '-c', 'tessedit_create_tsv=1', '-c', 'tessedit_load_sublangs='],
                   env={**os.environ, 'OMP_THREAD_LIMIT': '1'})
        return parse_tsv(text, page)


def parse_tsv(text, page):
    # Tesseract emits raw tab-separated text; quote characters belong to the recognition.
    rows = list(csv.DictReader(io.StringIO(text), delimiter='\t', quoting=csv.QUOTE_NONE))
    page_row = next((row for row in rows if row.get('level') == '1'), None)
    if page_row is None:
        raise ValueError(f'Tesseract returned no TSV page dimensions for scan page {page}')
    width, height = int(page_row['width']), int(page_row['height'])
    words = []
    for row in rows:
        if row['level'] != '5' or not row['text'].strip():
            continue
        x, y, w, h = (int(row[field]) for field in ('left', 'top', 'width', 'height'))
        words.append({'text': row['text'], 'confidence': float(row['conf']) / 100,
                      'box': [x / width, y / height, (x + w) / width, (y + h) / height]})
    return words


def extract_page(path, output, digest, page, engine, config):
    destination = output / f'page-{page:04d}.json'
    identity = {'scan_sha256': digest, 'scan_page': page, 'engine': engine,
                'languages': config['languages'] if config else None}
    if config:
        identity['recognition'] = {key: value for key, value in config.items() if key != 'tessdata_dir'}
    if destination.exists():
        existing = json.loads(destination.read_text())
        if any(existing.get(key) != value for key, value in identity.items()):
            raise ValueError(f'{destination} belongs to another source or extraction engine')
        LOG.info('Reusing page %s', page)
        return
    words = text_layer(path, page) if engine == 'text' else image_ocr(path, page, output, config)
    draft = {**identity, 'checked': False, 'box_origin': 'top_left_normalized', 'words': words}
    temporary = destination.with_suffix('.json.tmp')
    temporary.write_text(json.dumps(draft, ensure_ascii=False, indent=2) + '\n')
    temporary.replace(destination)
    LOG.info('Extracted page %s: %s text regions', page, len(words))


def model_directory(configured):
    if configured:
        return configured.resolve()
    listing = run(['tesseract', '--list-langs'])
    match = re.search(r'^List of available languages in "([^"]+)"', listing, re.MULTILINE)
    if not match:
        raise ValueError('Cannot locate Tesseract models; supply --tessdata-dir')
    return Path(match.group(1)).resolve()


def recognition_config(args):
    if args.engine == 'text':
        return None
    directory = model_directory(args.tessdata_dir)
    models = {}
    for language in args.languages.split('+'):
        models[language] = fingerprint(directory / f'{language}.traineddata')
    return {'languages': args.languages, 'dpi': args.dpi, 'psm': args.psm,
            'tessdata_dir': str(directory), 'models': models,
            'engine_version': run(['tesseract', '--version']).splitlines()[0], 'sublanguages': False,
            'tsv_quoting': 'none'}


def extract(args):
    path, output = args.pdf.resolve(), args.output.resolve()
    count, digest = page_count(path), fingerprint(path)
    pages = selected_pages(args.pages, count)
    config = recognition_config(args)
    output.mkdir(parents=True, exist_ok=True)
    with ThreadPoolExecutor(max_workers=args.jobs, thread_name_prefix='atlas-ocr') as executor:
        results = executor.map(lambda page: extract_page(
            path, output, digest, page, args.engine, config), pages)
        for result in results:
            if result is not None:
                raise ValueError('unexpected extraction result')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--pdf', type=Path, required=True)
    parser.add_argument('--pages', required=True, help='one-based page or inclusive first:last')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--engine', choices=('text', 'tesseract'), default='text')
    parser.add_argument('--languages', default='chi_sim+eng', help='Tesseract language models')
    parser.add_argument('--tessdata-dir', type=Path, help='Directory containing Tesseract models')
    parser.add_argument('--dpi', type=int, default=300)
    parser.add_argument('--psm', type=int, choices=range(3, 14), default=6)
    parser.add_argument('--jobs', type=int, default=1, help='Concurrent page workers')
    args = parser.parse_args()
    if args.jobs < 1 or not 72 <= args.dpi <= 600:
        parser.error('jobs must be positive and dpi must be between 72 and 600')
    commands = ['pdfinfo', 'pdftotext'] if args.engine == 'text' else ['pdfinfo', 'pdftoppm', 'tesseract']
    for command in commands:
        if shutil.which(command) is None:
            parser.error(f'{command} must be installed')
    logging.basicConfig(level=logging.INFO, format='%(levelname)s %(message)s')
    extract(args)


if __name__ == '__main__':
    main()
