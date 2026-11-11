"""Render Bijie commune geometry and its cited locations from a compiled publication."""

import argparse
import json
import hashlib
import math
from pathlib import Path

import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib import colors, font_manager, patheffects
from matplotlib.path import Path as DrawingPath
from matplotlib.patches import PathPatch
from matplotlib.ticker import FuncFormatter, MaxNLocator
import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / 'apps/atlas-app/assets'
RADIUS = 6_378_137.0
CREAM = '#f8f6ed'
INK = '#405b55'
PALETTE = ['#72508b', '#bb5735', '#418b65', '#b58420', '#5b83ad',
           '#b74a91', '#579d36', '#ca7033', '#449aab', '#947322',
           '#8857c0', '#426ba9', '#aa7055', '#847bb2', '#a25183']


def project(longitude, latitude):
    return RADIUS * np.radians(longitude), RADIUS * np.arcsinh(np.tan(np.radians(latitude)))


def polygon_path(vertices, polygons):
    points, codes = [], []
    for polygon in polygons:
        for ring in [polygon['exterior']] + polygon['interiors']:
            points.extend(project(vertices[i]['x'], vertices[i]['y']) for i in ring)
            codes.extend([DrawingPath.MOVETO] + [DrawingPath.LINETO] * (len(ring) - 2)
                         + [DrawingPath.CLOSEPOLY])
    return DrawingPath(points, codes)


def anchor(reference, records, estimates):
    if reference['kind'] == 'record':
        point = records[reference['record_id']]['observation']['coordinate']['point']
    else:
        point = estimates[reference['estimate_id']]['value']['point']
    return project(point['longitude'], point['latitude'])


def load(publication):
    entries = {}
    for path in sorted((publication / 'entities/index').rglob('*.json')):
        entries.update(json.loads(path.read_text()))
    for entry in entries.values():
        if reference := entry.get('geometry'):
            parts = []
            for part in reference['parts']:
                path = publication / part['path']
                if not path.resolve().is_relative_to(publication.resolve()):
                    raise ValueError(f'Invalid geometry path: {path}')
                content = path.read_bytes()
                if len(content) != part['bytes'] or hashlib.sha256(content).hexdigest() != part['sha256']:
                    raise ValueError(f'Invalid geometry part: {path}')
                parts.append(content)
            entry.update(json.loads(b''.join(parts)))
    records = {r['id']: r for e in entries.values() for r in e['records']}
    estimates = {e['id']: e for entry in entries.values() for e in entry.get('estimates', [])}
    recipe = json.loads((publication / 'derived/bijie-communes-terrain.json').read_text())
    county = entries['3598db1c-f453-500e-b412-21cd4953cc68']['boundaries'][0]
    outline = polygon_path(county['vertices'], county['unit']['polygons'])
    units, districts = [], []
    for item in recipe['units']:
        record = records[item['record_id']]
        entry = entries[record['entity_id']]
        boundary = entry['derived_boundaries'][0]
        reference = next((a for a in item['anchors'] if a['kind'] == 'map_location'), None)
        if reference is None:
            direct = next((r for r in entry['records'] if r['source_id'] == recipe['source_id']
                           and r['observation'].get('coordinate')), None)
            reference = ({'kind': 'record', 'record_id': direct['id']}
                         if direct else item['anchors'][0])
        parent = record['observation']['parent_id']
        if parent not in districts:
            districts.append(parent)
        region = boundary['region']
        units.append(dict(id=record['entity_id'], name=record['observation']['name'],
                          key=entry['entity']['key'].split('/')[-1], parent=parent,
                          mapped_anchor=reference['kind'] == 'map_location',
                          reference=anchor(reference, records, estimates),
                          color=PALETTE[districts.index(parent)], boundary=boundary,
                          path=polygon_path(region['vertices'], region['polygons']),
                          seeds=[anchor(a, records, estimates) for a in item['anchors']]))
    parents = {r['entity_id']: r['observation']['parent_id'] for r in records.values()
               if r['source_id'] == 'bijie-gazetteer' and r['observation'].get('coordinate')
               and r['observation'].get('parent_id')}
    owners = {u['id']: u['key'] for u in units}
    points = []
    for entry in entries.values():
        if entry['entity']['kind'] != {'place': 'settlement'}:
            continue
        record = next((r for r in entry['records'] if r['source_id'] == 'bijie-gazetteer'
                       and r['observation'].get('coordinate')), None)
        if record:
            point = anchor({'kind': 'record', 'record_id': record['id']}, records, estimates)
            if outline.contains_point(point):
                subject, visited = entry['entity']['id'], set()
                while subject not in owners and subject in parents and subject not in visited:
                    visited.add(subject)
                    subject = parents[subject]
                points.append((point, record['observation']['name'], owners.get(subject),
                               record['page_or_sheet'], record['scan_page']))
    for unit in units:
        if not all(unit['path'].contains_point(p) for p in unit['seeds']):
            raise ValueError(f"Seed lies outside {unit['key']}")
    return entries, records, units, districts, outline, points


def basemap(ax, precision=1):
    manifest = json.loads((ASSETS / 'basemap/manifest.json').read_text())
    world = 2 * math.pi * RADIUS
    for tile in manifest['tiles']:
        if tile['source'] != 'terrain':
            continue
        width = world / 2 ** tile['z']
        x = -world / 2 + tile['x'] * width
        y = world / 2 - tile['y'] * width
        ax.imshow(Image.open(ASSETS / 'basemap' / tile['file']), extent=(x, x + width, y - width, y),
                  alpha=.54, zorder=0, interpolation='bilinear')
    ax.set_facecolor(CREAM)
    for spine in ax.spines.values():
        spine.set_color('#a3ae98')
    ax.tick_params(length=0, labelcolor='#778578', labelsize=7)
    ax.xaxis.set_major_formatter(FuncFormatter(lambda v, _: f'{math.degrees(v / RADIUS):.{precision}f}°E'))
    ax.yaxis.set_major_formatter(FuncFormatter(lambda v, _: f'{math.degrees(math.atan(math.sinh(v / RADIUS))):.{precision}f}°N'))
    ax.xaxis.set_major_locator(MaxNLocator(6))
    ax.yaxis.set_major_locator(MaxNLocator(6))


def draw_geometry(ax, units, outline, points, selected=None):
    for unit in units:
        active = selected is None or unit['key'] in selected
        color = (PALETTE[selected.index(unit['key'])] if selected else unit['color']) if active else '#a0ae98'
        ax.add_patch(PathPatch(unit['path'], facecolor=colors.to_rgba(color, .27 if active else .10),
                               edgecolor=colors.to_rgba(color, .75), lw=.7, zorder=2))
    ax.add_patch(PathPatch(outline, facecolor='none', edgecolor=INK, lw=1.1, zorder=3))
    ax.scatter([p[0][0] for p in points], [p[0][1] for p in points], s=2, c=INK, alpha=.65, zorder=4)


def extent(ax, vertices, padding):
    lo, hi = vertices.min(axis=0), vertices.max(axis=0)
    margin = (hi - lo) * padding
    ax.set_xlim(lo[0] - margin[0], hi[0] + margin[0])
    ax.set_ylim(lo[1] - margin[1], hi[1] + margin[1])
    ax.set_aspect('equal')
    return lo, hi


def labels(fig, ax, items, size=8):
    fig.canvas.draw()
    renderer = fig.canvas.get_renderer()
    occupied = []
    for point, name, color in items:
        placed = False
        for radius in [5, 12, 20, 29, 39, 50, 62, 76, 92]:
            for angle in [0, 180, 60, 120, 240, 300, 90, 270]:
                dx, dy = radius * math.cos(math.radians(angle)), radius * math.sin(math.radians(angle))
                label = ax.annotate(name, point, xytext=(dx, dy), textcoords='offset points',
                                    ha='left' if dx > 1 else 'right' if dx < -1 else 'center',
                                    va='center', fontsize=size, color=color, zorder=7,
                                    path_effects=[patheffects.withStroke(linewidth=1.8, foreground=CREAM)])
                box = label.get_window_extent(renderer).expanded(1.035, 1.10)
                inside = ax.bbox.contains(box.x0, box.y0) and ax.bbox.contains(box.x1, box.y1)
                if inside and not any(box.overlaps(other) for other in occupied):
                    occupied.append(box)
                    placed = True
                    if radius > 16:
                        ax.annotate('', point, xytext=(dx, dy), textcoords='offset points',
                                    arrowprops={'arrowstyle': '-', 'color': color, 'lw': .35}, zorder=6)
                    break
                label.remove()
            if placed:
                break
        if not placed:
            raise ValueError(f'Could not place label: {name}')


def scale(ax, lo, hi, kilometres, offset=.022):
    latitude = math.atan(math.sinh((lo[1] + hi[1]) / (2 * RADIUS)))
    length = kilometres * 1000 / math.cos(latitude)
    x, y = lo[0], lo[1] - (hi[1] - lo[1]) * offset
    ax.plot([x, x + length], [y, y], color=INK, lw=1.5)
    ax.text(x + length / 2, y + (hi[1] - lo[1]) * .01, f'{kilometres} km', ha='center', fontsize=8)


def overview(data, output):
    entries, records, units, districts, outline, points = data
    fig = plt.figure(figsize=(13, 9.6), dpi=200, facecolor=CREAM)
    ax = fig.add_axes([.055, .105, .72, .76])
    basemap(ax)
    draw_geometry(ax, units, outline, points)
    lo, hi = extent(ax, outline.vertices, .045)
    for unit in units:
        ax.scatter(*unit['reference'], s=7, color=unit['color'], edgecolor=CREAM, lw=.4, zorder=6)
    labels(fig, ax, [(u['reference'], u['name'].removesuffix('公社'), u['color']) for u in units])
    scale(ax, lo, hi, 10)
    fig.text(.055, .95, 'BIJIE / SETTLEMENTS AND RECONSTRUCTED BOUNDARIES', fontsize=16)
    fig.text(.055, .923, 'Checked source locations · terrain partitions · source area constraints', fontsize=10)
    fig.text(.80, .86, '14 districts + Bijie town', fontsize=10)
    for i, district in enumerate(districts):
        record = next(r for r in entries[district]['records'] if r['key'].startswith('bijie-gazetteer/contents-'))
        y = .834 - i * .024
        fig.text(.80, y, '●', color=PALETTE[i], fontsize=11)
        fig.text(.816, y, record['observation']['name'], fontsize=9)
        fig.text(.948, y, str(sum(u['parent'] == district for u in units)), color=PALETTE[i], ha='right')
    fig.text(.80, .46, f'{len(units)} reconstructed communes', fontsize=11)
    fig.text(.80, .435, f'{len(points):,} located settlements in frame', fontsize=9)
    fig.text(.80, .411, f'{sum(len(u["seeds"]) for u in units):,} membership constraints', fontsize=9)
    comparisons = [(u, c) for u in units for c in u['boundary'].get('comparisons', [])
                   if c.get('fitting_tolerance_square_km') is not None]
    heading = 'Area fitted / reported' if len(comparisons) <= 4 else f'Area fits: 4/{len(comparisons)} shown'
    fig.text(.80, .366, heading, fontsize=11)
    pages = []
    for i, (unit, comparison) in enumerate(comparisons[:4]):
        pages.append(records[comparison['record_id']]['page_or_sheet'])
        fig.text(.80, .338 - i * .048, unit['name'], fontsize=9)
        fig.text(.80, .316 - i * .048,
                 f'{comparison["modeled_area_square_km"]:.2f} / {comparison["reported_area_square_km"]:.2f} km²', fontsize=9)
    fig.text(.80, .131, 'Source pages: ' + ', '.join(pages), fontsize=7)
    fig.text(.80, .103, 'Natural Earth II + terrain hillshade', fontsize=8)
    fig.text(.80, .079, 'Historical coordinate datum: unspecified', fontsize=7)
    fig.text(.055, .043, 'Source: 毕节县地名录; county map scan 658. Terrain: Mapzen / USGS.', fontsize=8)
    fig.text(.055, .022, 'Recipe: derived/bijie-communes-terrain.json', fontsize=8)
    fig.savefig(output / 'bijie-terrain-boundaries.png', facecolor=CREAM)
    plt.close(fig)


def detail(data, selected, output):
    _, records, units, _, outline, points = data
    if not 1 <= len(selected) <= 2 or len(set(selected)) != len(selected):
        raise ValueError('A detail view needs one or two distinct commune keys')
    chosen = [u for key in selected for u in units if u['key'] == key]
    if len(chosen) != len(selected):
        raise ValueError('Detail keys must identify distinct published communes')
    fig = plt.figure(figsize=(11, 8), dpi=200, facecolor=CREAM)
    ax = fig.add_axes([.06, .13, .70, .73])
    basemap(ax, precision=3)
    draw_geometry(ax, units, outline, points, selected)
    locations = [p[0] for p in points if p[2] in selected]
    vertices = np.concatenate([u['path'].vertices for u in chosen])
    if locations:
        vertices = np.concatenate([vertices, locations])
    lo, hi = extent(ax, vertices, .15)
    scale(ax, lo, hi, 2, offset=.10)
    names = [(u['reference'], u['name'], INK) for u in chosen]
    citations = set()
    for point, name, owner, page, scan in points:
        if owner in selected and ax.get_xlim()[0] < point[0] < ax.get_xlim()[1] and ax.get_ylim()[0] < point[1] < ax.get_ylim()[1]:
            names.append((point, name, PALETTE[selected.index(owner)]))
            citations.add((int(page), scan))
    labels(fig, ax, names, size=8)
    for unit in chosen:
        ax.scatter(*unit['reference'], s=32, c=PALETTE[selected.index(unit['key'])], edgecolor=CREAM, lw=.8, zorder=6)
    fig.text(.06, .945, 'BIJIE / SETTLEMENTS AND RECONSTRUCTED BOUNDARIES', fontsize=14)
    fig.text(.06, .91, ' · '.join(u['name'] for u in chosen), fontsize=12)
    fig.text(.79, .866, 'Area fitted / reported', fontsize=9)
    y = .83
    for unit in chosen:
        fig.text(.79, y, unit['name'], fontsize=11, color=PALETTE[selected.index(unit['key'])])
        y -= .03
        for comparison in unit['boundary'].get('comparisons', []):
            fig.text(.79, y, f'{comparison["modeled_area_square_km"]:.2f} / {comparison["reported_area_square_km"]:.2f} km²', fontsize=9)
            y -= .027
            source = records[comparison['record_id']]
            fig.text(.79, y, f'Source p. {source["page_or_sheet"]} / scan {source["scan_page"]}', fontsize=8)
            y -= .033
            for neighbor in comparison['neighbors']:
                status = 'shared edge' if neighbor['shares_edge'] else 'not mapped' if neighbor['shares_edge'] is None else 'no shared edge'
                if neighbor.get('unresolved'):
                    status = 'unresolved adjacency'
                fig.text(.79, y, neighbor['name'] + ': ' + status, fontsize=8)
                y -= .027
        y -= .03
    symbol = 'registered commune locations' if all(u['mapped_anchor'] for u in chosen) else 'commune reference locations'
    fig.text(.06, .066, 'Dots: checked settlement coordinates. Large symbols: ' + symbol + '.', fontsize=9)
    table_pages = ', '.join(str(page) for page, _ in sorted(citations))
    scan_pages = ', '.join(str(scan) for _, scan in sorted(citations))
    fig.text(.06, .038, f'Source: 毕节县地名录, tables pp. {table_pages} (scans {scan_pages}); county map scan 658.', fontsize=8)
    fig.text(.06, .016, 'Natural Earth II / Mapzen Terrain Tiles / USGS', fontsize=8)
    fig.savefig(output / ('-'.join(key.removesuffix('-commune') for key in selected) + '-boundaries.png'), facecolor=CREAM)
    plt.close(fig)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('publication', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--detail', nargs='+', default=[])
    args = parser.parse_args()
    families = []
    for filename in ['AtlasCJK.otf', 'AtlasHanSupplement.ttf']:
        font_manager.fontManager.addfont(ASSETS / filename)
        families.append(font_manager.FontProperties(fname=ASSETS / filename).get_name())
    plt.rcParams.update({'font.family': families + ['DejaVu Sans'], 'font.size': 9, 'text.color': INK})
    data = load(args.publication)
    args.output.mkdir(parents=True, exist_ok=True)
    overview(data, args.output)
    if args.detail:
        detail(data, args.detail, args.output)
    print(f'Rendered {len(data[2])} communes and {len(data[5])} settlement locations; all seeds lie within their units')


if __name__ == '__main__':
    main()
