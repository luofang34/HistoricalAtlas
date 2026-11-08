"""Build the atlas glyph subset from an OFL-licensed Noto Sans CJK font."""

import argparse
from pathlib import Path
from fontTools import subset
from fontTools.ttLib import TTFont


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('font', type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    text = ''.join(path.read_text() for path in (root / 'data').rglob('*')
                   if path.suffix in {'.json', '.toml'})
    codepoints = {ord(char) for char in text if ord(char) > 127}
    font = TTFont(args.font, recalcTimestamp=False)
    missing = codepoints - font.getBestCmap().keys()
    if missing:
        raise ValueError(f'Font lacks glyphs: {sorted(missing)}')
    options = subset.Options()
    options.name_IDs = ['*']
    options.name_languages = ['*']
    worker = subset.Subsetter(options)
    worker.populate(unicodes=codepoints)
    worker.subset(font)
    for entry in font['name'].names:
        names = {1: 'Atlas CJK', 2: 'Regular', 3: 'Atlas CJK Regular',
                 4: 'Atlas CJK Regular', 6: 'AtlasCJK-Regular',
                 16: 'Atlas CJK', 17: 'Regular'}
        if entry.nameID in names:
            entry.string = names[entry.nameID].encode(entry.getEncoding())
    if 'CFF ' in font:
        cff = font['CFF '].cff
        cff.fontNames = ['AtlasCJK-Regular']
        top = cff.topDictIndex[0]
        top.FamilyName = 'Atlas CJK'
        top.FullName = 'Atlas CJK Regular'
    destination = root / 'apps/atlas-app/assets/AtlasCJK.otf'
    destination.parent.mkdir(parents=True, exist_ok=True)
    font.save(destination)
    print(f'{len(codepoints)} glyphs, {destination.stat().st_size} bytes')


if __name__ == '__main__':
    main()
