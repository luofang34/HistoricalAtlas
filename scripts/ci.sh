#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
RUSTDOCFLAGS="-D missing_docs -D rustdoc::broken_intra_doc_links" cargo doc --locked --no-deps
cargo build --locked --release
sh scripts/check-structure.sh
sh scripts/check-repository.sh
sh scripts/check-ingest.sh
cargo clippy --locked --target wasm32-unknown-unknown -p atlas-core -p atlas-geo -p atlas-ledger -p atlas-boundary --all-targets -- -D warnings
cargo clippy --locked --target wasm32-unknown-unknown -p atlas-app --lib --bins -- -D warnings
cargo run --locked -p atlas-compile -- verify data
