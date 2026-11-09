# HistoricalAtlas

Use the user-provided repository workflow and Rust rules. Use GitButler for version control.

- Write project documentation and interface text in English. Preserve source titles, place names, and quotations in their original language. Keep prose direct and omit defensive disclaimers.
- Tools are AGPL-3.0-or-later. Curated data is CC-BY-SA-4.0. Reference imagery and elevation that establish control targets must be public domain or under a licence compatible with CC-BY-SA-4.0 (CC0, CC BY, CC BY-SA, or an attribution-only open-data licence); never ODbL, non-commercial, no-derivatives, or proprietary basemaps. Catalogue each reference with its `rights`.
- Every published assertion must cite a source and an exact printed page/sheet plus a one-based scan page where available. Keep original dates, coordinate notation, precision, and unknown values. Never promote OCR drafts to checked records without page review.
- Source observations and derived estimates are separate. Do not invent exact dates, WGS84 coordinates, control points, or surveyed boundaries. Record uncertain interpretation and its basis explicitly.
- Run `scripts/ci.sh` before pushing. It includes native and WASM checks, data validation, and structural gates. Do not change shared CI or force-push without the user's confirmation.
- Keep Rust files under 500 lines, functions under 80 lines, and structs/enums under 30 fields/variants. `lib.rs` contains only crate documentation, module declarations, and re-exports and stays under 100 lines. Use domain names and `foo.rs` with `foo/`; no `mod.rs`, `utils.rs`, `helpers.rs`, or `common.rs`.
- Use typed library errors and `tracing` diagnostics. Monotonic counters use `wrapping_add(1)`. Tests live in domain `tests.rs` modules and exercise behavior. Lint exemptions belong in `clippy.toml`.
- Do not add task histories or status reports to documentation. Update essential usage instructions in place. Keep each published static file strictly smaller than 4,000,000 bytes; never use Git LFS.
