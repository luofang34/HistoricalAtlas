"""Validate manifest policy and distributable source files."""

from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parent.parent
SKIP = {'.git', 'target', 'dist', 'scratch', 'tmp', 'node_modules'}
FORBIDDEN = ('unwrap_used', 'expect_used', 'panic', 'todo', 'unimplemented',
             'unreachable', 'exit', 'mem_forget', 'get_unwrap')


def check():
    manifest = tomllib.loads((ROOT / 'Cargo.toml').read_text())
    policy = manifest['workspace']['lints']
    assert policy['rust']['unsafe_code'] == 'forbid'
    for lint in FORBIDDEN:
        assert policy['clippy'][lint] == 'forbid', lint
    for lint in ('let_underscore_must_use', 'let_underscore_future', 'await_holding_lock'):
        assert policy['clippy'][lint] == 'deny', lint
    for directory, dirs, files in ROOT.walk():
        dirs[:] = [name for name in dirs if name not in SKIP]
        for name in files:
            path = directory / name
            if name == 'Cargo.toml' and path != ROOT / 'Cargo.toml':
                crate = tomllib.loads(path.read_text())
                assert crate['lints']['workspace'] is True, path
            assert not name.endswith(('.bak', ' 2.rs')), path
            assert path.suffix.lower() not in {'.pdf', '.tif', '.tiff'}, path
            assert path.stat().st_size < 4_000_000, path
            if name == '.gitattributes':
                assert 'filter=lfs' not in path.read_text(), path
    print('Repository policy verified')


if __name__ == '__main__':
    check()
