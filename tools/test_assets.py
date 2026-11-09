"""Exercise image transforms, glyph subsets, and browser package integrity."""

import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import numpy as np
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parent.parent


def module(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'tools' / f'{name}.py')
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded


class Assets(unittest.TestCase):
    def test_physical_tiles_preserve_constant_color_and_flat_elevation(self):
        basemap = module('build-basemap')
        color = np.full((8, 16, 3), [12.0, 85.0, 140.0])
        for address in [(0, 0, 0), (8, 243, 134)]:
            sampled = basemap.sample_world(color, *address, 12)
            np.testing.assert_allclose(sampled, np.broadcast_to([12, 85, 140], sampled.shape))
        shade = basemap.shade(np.ones((8, 8)) * 1000, 0, 0)
        np.testing.assert_allclose(shade, shade[0, 0])
        self.assertTrue(np.isfinite(shade).all())

    def test_renderer_preserves_separate_exterior_and_hole(self):
        renderer = module('render-bijie')
        vertices = [dict(x=x, y=y) for x, y in [(0, 0), (4, 0), (4, 4), (0, 4),
                    (1, 1), (1, 2), (2, 2), (2, 1)]]
        path = renderer.polygon_path(vertices, [dict(exterior=[0, 1, 2, 3, 0],
                                                     interiors=[[4, 5, 6, 7, 4]])])
        self.assertEqual(list(path.codes).count(renderer.DrawingPath.MOVETO), 2)
        self.assertEqual(list(path.codes).count(renderer.DrawingPath.CLOSEPOLY), 2)
        self.assertEqual(tuple(renderer.project(0, 0)), (0, 0))
        self.assertTrue(np.isfinite(path.vertices).all())

    def test_font_subset_retains_requested_text_and_renames_family(self):
        subset = module('subset-font')
        with tempfile.TemporaryDirectory() as temporary:
            font = TTFont(ROOT / 'apps/atlas-app/assets/AtlasCJK.otf')
            destination = Path(temporary) / 'subset.otf'
            requested = {ord('毕'), ord('中')}
            subset.save_subset(font, requested, 'Test Atlas', destination)
            with TTFont(destination) as output:
                self.assertEqual(set(output.getBestCmap()), requested)
                self.assertEqual(output['name'].getDebugName(1), 'Test Atlas')

    def test_wasm_parts_round_trip_with_bounded_files_and_checksums(self):
        package = module('package-web')
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            original = bytes(range(256)) * 31000
            (directory / 'atlas_bg.wasm').write_bytes(original)
            (directory / 'atlas.js').write_text('export default function() {}')
            package.split_wasm(directory)
            manifest = json.loads((directory / 'runtime.json').read_text())
            restored = []
            for part in manifest['parts']:
                content = (directory / part['path']).read_bytes()
                self.assertLess(len(content), 4_000_000)
                self.assertEqual(len(content), part['bytes'])
                self.assertEqual(hashlib.sha256(content).hexdigest(), part['sha256'])
                restored.append(content)
            self.assertEqual(b''.join(restored), original)
            self.assertFalse((directory / 'atlas_bg.wasm').exists())

    def test_repository_policy_still_rejects_under_python_optimization(self):
        script = ROOT / 'scripts/check_repository.py'
        command = f"import runpy; runpy.run_path({str(script)!r})['require'](False, 'test-policy')"
        for options in [[], ['-O']]:
            result = subprocess.run([sys.executable, *options, '-c', command], capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Repository policy violation: test-policy', result.stderr)


if __name__ == '__main__':
    unittest.main()
