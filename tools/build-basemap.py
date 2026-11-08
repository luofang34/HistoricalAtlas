"""Build the atlas's bundled physical tiles from Natural Earth and Terrain Tiles."""

import argparse
import hashlib
import json
from pathlib import Path

import numpy as np
from PIL import Image


def fingerprint(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sample_world(world, z, x, y, size):
    axis = (np.arange(size) + 0.5) / size
    longitude = (x + axis) / 2**z * 360 - 180
    latitude = np.degrees(np.arctan(np.sinh(np.pi * (1 - 2 * (y + axis) / 2**z))))
    sx = (longitude + 180) / 360 * world.shape[1] - 0.5
    sy = (90 - latitude) / 180 * world.shape[0] - 0.5
    ix, iy = np.floor(sx).astype(int), np.floor(sy).astype(int)
    fx, fy = sx - ix, sy - iy
    ix = np.clip(ix, 0, world.shape[1] - 2)
    iy = np.clip(iy, 0, world.shape[0] - 2)
    top = world[iy[:, None], ix] * (1 - fx)[None, :, None]
    top += world[iy[:, None], ix + 1] * fx[None, :, None]
    bottom = world[iy[:, None] + 1, ix] * (1 - fx)[None, :, None]
    bottom += world[iy[:, None] + 1, ix + 1] * fx[None, :, None]
    return top * (1 - fy)[:, None, None] + bottom * fy[:, None, None]


def shade(elevation, y, z):
    size = elevation.shape[0]
    latitude = np.arctan(np.sinh(np.pi * (1 - 2 * (y + 0.5) / 2**z)))
    spacing = 40075016.68557849 * np.cos(latitude) / (2**z * size)
    south, east = np.gradient(elevation, spacing)
    illumination = (-east + south + 1.4) / np.sqrt(east**2 + south**2 + 1) / np.sqrt(3.96)
    return np.clip(0.6 + 0.5 * illumination, 0.5, 1.1)


def build(world_path, terrain_dir, output):
    output.mkdir(parents=True, exist_ok=True)
    world = np.asarray(Image.open(world_path).convert('RGB'), dtype=float)
    tiles = []

    def save(z, x, y, pixels, source, input_path):
        name = f'{source}-{z}-{x}-{y}.jpg'
        Image.fromarray(np.clip(pixels, 0, 255).astype('uint8')).save(
            output / name, quality=88, subsampling=0, optimize=True)
        tiles.append(dict(source=source, z=z, x=x, y=y, file=name,
                          sha256=fingerprint(output / name), input_sha256=fingerprint(input_path)))

    save(0, 0, 0, sample_world(world, 0, 0, 0, 2048), 'natural-earth', world_path)
    for path in sorted(terrain_dir.glob('*.tif')):
        image = Image.open(path)
        if image.size != (512, 512):
            raise ValueError(f'Expected 512-pixel Terrain Tile: {path}')
        step = image.tag_v2[33550][0]
        west, north = image.tag_v2[33922][3:5]
        z = round(np.log2(40075016.68557849 / (step * 512)))
        x = round((west + 20037508.342789244) / (step * 512))
        y = round((20037508.342789244 - north) / (step * 512))
        elevation = np.asarray(image, dtype=float)
        if np.any(elevation <= -32768):
            raise ValueError(f'Elevation tile contains voids: {path}')
        colors = sample_world(world, z, x, y, 512)
        colors = colors * 0.4 + np.array([229, 226, 207]) * 0.6
        save(z, x, y, colors * shade(elevation, y, z)[:, :, None], 'terrain', path)
    manifest = dict(
        natural_earth='https://www.naturalearthdata.com/downloads/50m-raster-data/50m-natural-earth-2/',
        terrain='https://elevation-tiles-prod.s3.amazonaws.com/geotiff/{z}/{x}/{y}.tif',
        attribution='Natural Earth II; Mapzen Terrain Tiles; SRTM and GMTED2010 courtesy of USGS',
        processing='Web Mercator resampling; regional elevation hillshade with Natural Earth color tint',
        tiles=tiles)
    (output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print(f'Built {len(tiles)} physical basemap tiles')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('natural_earth', type=Path)
    parser.add_argument('terrain_dir', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    build(args.natural_earth, args.terrain_dir, args.output)
