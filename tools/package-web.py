"""Package the Trunk output with published evidence and bounded WASM parts."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess


def split_wasm(staging):
    wasm, = staging.glob('*_bg.wasm')
    module = wasm.name.removesuffix('_bg.wasm') + '.js'
    if not (staging / module).is_file():
        raise ValueError(f'Missing WASM module: {module}')
    parts = []
    with wasm.open('rb') as source:
        while chunk := source.read(3_900_000):
            name = f'atlas-{len(parts):03}.part'
            (staging / name).write_bytes(chunk)
            parts.append({'path': name, 'bytes': len(chunk), 'sha256': hashlib.sha256(chunk).hexdigest()})
    (staging / 'runtime.json').write_text(json.dumps({'module': module, 'parts': parts}) + '\n')
    wasm.unlink()
    return parts


def package(staging):
    root = Path(__file__).resolve().parent.parent
    parts = split_wasm(staging)
    shutil.copytree(root / 'apps/atlas-app/assets', staging / 'assets', dirs_exist_ok=True)
    subprocess.run(['cargo', 'run', '--locked', '-p', 'atlas-compile', '--',
                    'build', 'data', str(staging / 'data')], cwd=root, check=True)
    shutil.copyfile(root / 'apps/atlas-app/assets/OFL.txt', staging / 'FONT-LICENSE.txt')
    shutil.copyfile(root / 'apps/atlas-app/assets/ARPHICPL.TXT', staging / 'ARPHICPL.TXT')
    shutil.copyfile(root / 'LICENSE', staging / 'LICENSE.txt')
    shutil.copyfile(root / 'data/LICENSE', staging / 'data/LICENSE.txt')
    for path in staging.rglob('*'):
        if path.is_file() and path.stat().st_size >= 4_000_000:
            raise ValueError(f'Published file exceeds size limit: {path}')
    print(f'Packaged {len(parts)} WASM parts with published evidence')


if __name__ == '__main__':
    package(Path(os.environ['TRUNK_STAGING_DIR']).resolve())
