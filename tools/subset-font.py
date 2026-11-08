"""Build atlas glyph subsets from Noto Sans CJK and BabelStone Han."""

import argparse
from datetime import date
from pathlib import Path
from fontTools import subset
from fontTools.ttLib import TTFont


def save_subset(font, codepoints, family, destination):
    options = subset.Options()
    options.name_IDs = ['*']
    options.name_languages = ['*']
    worker = subset.Subsetter(options)
    worker.populate(unicodes=codepoints)
    worker.subset(font)
    if family == 'Atlas Han Supplement':
        notice = (f'Subset and renamed for HistoricalAtlas on {date.today().isoformat()}. '
                  'Derived from BabelStone Han by Andrew West; '
                  'distributed under the Arphic Public License.')
        font['name'].setName(notice, 10, 3, 1, 0x409)
    for entry in font['name'].names:
        names = {1: family, 2: 'Regular', 3: f'{family} Regular',
                 4: f'{family} Regular', 6: family.replace(' ', '') + '-Regular',
                 16: family, 17: 'Regular', 18: f'{family} Regular'}
        if entry.nameID in names:
            entry.string = names[entry.nameID].encode(entry.getEncoding())
    if 'CFF ' in font:
        cff = font['CFF '].cff
        cff.fontNames = [family.replace(' ', '') + '-Regular']
        top = cff.topDictIndex[0]
        top.FamilyName = family
        top.FullName = f'{family} Regular'
    destination.parent.mkdir(parents=True, exist_ok=True)
    font.save(destination)
    print(f'{len(codepoints)} glyphs, {destination.stat().st_size} bytes')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('font', type=Path)
    parser.add_argument('--fallback-font', type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    text = ''.join(path.read_text() for path in (root / 'data').rglob('*')
                   if path.suffix in {'.json', '.toml'})
    codepoints = {ord(char) for char in text if ord(char) > 127}
    font = TTFont(args.font, recalcTimestamp=False)
    fallback = TTFont(args.fallback_font, recalcTimestamp=False)
    missing = codepoints - font.getBestCmap().keys()
    uncovered = missing - fallback.getBestCmap().keys()
    if uncovered:
        raise ValueError(f'Fonts lack glyphs: {sorted(uncovered)}')
    assets = root / 'apps/atlas-app/assets'
    save_subset(font, codepoints - missing, 'Atlas CJK', assets / 'AtlasCJK.otf')
    save_subset(fallback, missing, 'Atlas Han Supplement', assets / 'AtlasHanSupplement.ttf')


if __name__ == '__main__':
    main()
