#!/usr/bin/env python3
"""Group page text boxes into reviewable rows using declared column layouts."""

import argparse
import hashlib
import json
import logging
from pathlib import Path
import re
import statistics
import unicodedata

PROFILES = {
    'bijie': [('name', .04, .235), ('pinyin', .235, .445), ('population', .445, .515),
              ('ethnicities', .515, .59), ('longitude', .59, .705),
              ('latitude', .705, .81), ('note', .81, .98)],
    'anshun': [('name', .07, .27), ('pinyin', .27, .49), ('population', .49, .555),
               ('ethnicities', .555, .62), ('longitude', .62, .705),
               ('latitude', .705, .785), ('note', .785, .98)],
    'liuzhi': [('name', .08, .26), ('pinyin', .26, .49), ('ethnicities', .49, .615),
               ('population', .615, .705), ('alias', .705, .785), ('note', .785, .97)],
}
LOG = logging.getLogger('atlas-tables')


def centre(word):
    left, top, right, bottom = word['box']
    return ((left + right) / 2, (top + bottom) / 2)


def transcribed_text(words):
    text = ' '.join(word['text'] for word in sorted(words, key=lambda word: word['box'][0]))
    return re.sub(r'(?<=[\u3400-\u9fff])\s+(?=[\u3400-\u9fff])', '', text).strip()


def name_lines(words, left, right, pattern):
    selected = [word for word in words if left <= centre(word)[0] < right
                and re.search(pattern, word['text'])]
    return visual_lines(selected)


def visual_lines(selected):
    if not selected:
        return []
    tolerance = statistics.median(word['box'][3] - word['box'][1] for word in selected) * .55
    lines = []
    for word in sorted(selected, key=lambda word: centre(word)[1]):
        if lines and abs(centre(word)[1] - statistics.median(centre(w)[1] for w in lines[-1])) <= tolerance:
            lines[-1].append(word)
        else:
            lines.append([word])
    return lines


def coordinate(text, maximum):
    value = unicodedata.normalize('NFKC', text)
    match = re.fullmatch(r'\s*(\d{2,3})[°*"?度\s]+(\d{1,2})[′\'"/分.\s]*', value)
    if not match:
        return None
    degrees, minutes = map(int, match.groups())
    if minutes >= 60 or degrees + minutes / 60 > maximum:
        return None
    return degrees + minutes / 60


def is_page_number(word):
    text = unicodedata.normalize('NFKC', word['text'])
    return centre(word)[1] > .85 and re.fullmatch(r'\s*[~–—-]\s*\d+\s*[~–—一-]*\s*', text)


def parse_cells(cells):
    raw = {name: cell['text'] for name, cell in cells.items()}
    warnings = []
    number = unicodedata.normalize('NFKC', raw.get('population', '')).strip()
    population = int(number) if re.fullmatch(r'[0-9]+', number) else None
    if number and population is None:
        warnings.append('population_not_numeric')
    longitude = coordinate(raw.get('longitude', ''), 180)
    latitude = coordinate(raw.get('latitude', ''), 90)
    for field, value in [('longitude', longitude), ('latitude', latitude)]:
        if raw.get(field) and value is None:
            warnings.append(f'{field}_not_parsed')
    if bool(raw.get('longitude')) != bool(raw.get('latitude')):
        warnings.append('incomplete_coordinate')
    confidence = [word['confidence'] for cell in cells.values() for word in cell['regions']
                  if word.get('confidence') is not None]
    if confidence and min(confidence) < .85:
        warnings.append('low_recognition_confidence')
    return {'population': population, 'longitude': longitude, 'latitude': latitude}, warnings


def profile_definition(profile):
    if isinstance(profile, str):
        profile = dict(columns=PROFILES[profile], anchor_pattern=r'[\u3400-\u9fff]',
                       header_pattern=r'标准.*名', row_end=.95)
    columns = profile.get('columns', [])
    if not columns or not 0 < profile.get('row_end', .95) <= 1:
        raise ValueError('Profile requires columns and a row_end in (0, 1]')
    names, previous = set(), 0
    for column in columns:
        if len(column) != 3:
            raise ValueError('Each column requires a name and normalized left/right bounds')
        name, left, right = column
        if not isinstance(name, str) or not name or name in names or not previous <= left < right <= 1:
            raise ValueError('Columns require unique names and ordered, non-overlapping bounds')
        names.add(name)
        previous = right
    result = dict(columns=columns, anchor_pattern=profile.get('anchor_pattern', r'\S'),
                  header_pattern=profile.get('header_pattern', ''), row_end=profile.get('row_end', .95))
    for field in ['anchor_pattern', 'header_pattern']:
        if not isinstance(result[field], str) or not result[field]:
            raise ValueError(f'Profile requires {field}')
        re.compile(result[field])
    return result


def extract_table(page, profile):
    definition = profile_definition(profile)
    columns = definition['columns']
    _, left, right = columns[0]
    words = [word for word in page['words'] if not is_page_number(word)]
    lines = name_lines(words, left, right, definition['anchor_pattern'])
    header = next((line for line in lines if re.search(definition['header_pattern'], transcribed_text(line))), None)
    if header is None:
        return {'layout_detected': False, 'rows': [], 'warnings': ['header_not_detected']}
    header_bottom = max(word['box'][3] for word in header)
    anchors = [line for line in lines if min(word['box'][1] for word in line) > header_bottom
               and centre(line[0])[1] < definition['row_end']]
    if not anchors:
        return {'layout_detected': False, 'rows': [], 'warnings': ['no_name_rows']}
    positions = [statistics.median(centre(word)[1] for word in line) for line in anchors]
    rows = [{name: {'text': '', 'regions': []} for name, _, _ in columns} for _ in anchors]
    for word in words:
        x, y = centre(word)
        if y <= header_bottom or y >= definition['row_end']:
            continue
        row = min(range(len(positions)), key=lambda index: abs(positions[index] - y))
        for name, xmin, xmax in columns:
            if xmin <= x < xmax:
                rows[row][name]['regions'].append(word)
                break
    result = []
    for row, cells in enumerate(rows, 1):
        for cell in cells.values():
            cell['text'] = '\n'.join(transcribed_text(line) for line in visual_lines(cell['regions']))
        parsed, warnings = parse_cells(cells)
        result.append({'row': row, 'checked': False, 'cells': cells, 'parsed': parsed, 'warnings': warnings})
    return {'layout_detected': True, 'rows': result, 'warnings': []}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('input', type=Path, help='Directory of extracted page JSON files')
    parser.add_argument('output', type=Path, help='Destination for table drafts')
    profiles = parser.add_mutually_exclusive_group(required=True)
    profiles.add_argument('--profile', choices=PROFILES)
    profiles.add_argument('--profile-file', type=Path, help='JSON columns, anchor/header patterns, and optional row_end')
    args = parser.parse_args()
    definition = profile_definition(json.loads(args.profile_file.read_text()) if args.profile_file else args.profile)
    profile_name = args.profile_file.name if args.profile_file else args.profile
    logging.basicConfig(level=logging.INFO, format='%(levelname)s %(message)s')
    args.output.mkdir(parents=True, exist_ok=True)
    counts = {'pages': 0, 'tables': 0, 'rows': 0}
    for path in sorted(args.input.glob('page-*.json')):
        raw = path.read_bytes()
        page = json.loads(raw)
        result = extract_table(page, definition)
        result.update({'scan_sha256': page['scan_sha256'], 'scan_page': page['scan_page'],
                       'extraction_sha256': hashlib.sha256(raw).hexdigest(),
                       'profile': profile_name, 'profile_definition': definition, 'columns': definition['columns'], 'checked': False})
        destination = args.output / path.name.replace('.json', '.table.json')
        destination.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
        counts['pages'] += 1
        counts['tables'] += int(result['layout_detected'])
        counts['rows'] += len(result['rows'])
    LOG.info('Parsed %s pages, %s tables, %s draft rows', counts['pages'], counts['tables'], counts['rows'])


if __name__ == '__main__':
    main()
