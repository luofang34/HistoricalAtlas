"""Validate manifest policy and distributable source files."""

from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parent.parent
SKIP = {'.git', 'target', 'dist', 'scratch', 'tmp', 'node_modules'}
FORBIDDEN = ('unwrap_used', 'expect_used', 'panic', 'todo', 'unimplemented',
             'unreachable', 'exit', 'mem_forget', 'get_unwrap')


def require(condition, context):
    if not condition:
        raise ValueError(f'Repository policy violation: {context}')


def check():
    manifest = tomllib.loads((ROOT / 'Cargo.toml').read_text())
    policy = manifest['workspace']['lints']
    require(policy['rust']['unsafe_code'] == 'forbid', 'unsafe_code')
    for lint in FORBIDDEN:
        require(policy['clippy'][lint] == 'forbid', lint)
    for lint in ('let_underscore_must_use', 'let_underscore_future', 'await_holding_lock'):
        require(policy['clippy'][lint] == 'deny', lint)
    for directory, dirs, files in ROOT.walk():
        dirs[:] = [name for name in dirs if name not in SKIP]
        for name in files:
            path = directory / name
            if name == 'Cargo.toml' and path != ROOT / 'Cargo.toml':
                crate = tomllib.loads(path.read_text())
                require(crate['lints']['workspace'] is True, path)
            require(not name.endswith(('.bak', ' 2.rs')), path)
            require(path.suffix.lower() not in {'.pdf', '.tif', '.tiff'}, path)
            require(path.stat().st_size < 4_000_000, path)
            if name == '.gitattributes':
                require('filter=lfs' not in path.read_text(), path)
    print('Repository policy verified')


if __name__ == '__main__':
    check()
