# Historical Atlas

A spatiotemporal atlas and evidence ledger for chronicle applications. Historical names, populations, and coordinates link to their sources, with interpretations recorded as separate estimates.

## Usage

Requires Rust with the `wasm32-unknown-unknown` target, Python 3.12 or later, and Poppler.

```sh
cargo run -p atlas-compile -- verify data
cargo run -p atlas-compile -- build data dist
sh scripts/ci.sh
```

Curated records live in `data/`. `scan_page` is one-based; `page_or_sheet` identifies the printed page. `checked` records transcription review. Dates retain their source convention, and coordinates retain their stated datum and angular precision.

For page extraction, run `python3 tools/ocr-ingest/ingest.py --pdf <path> --pages 1:10 --output scratch/extraction`. The default engine reads the embedded text layer. Use `--engine tesseract --languages chi_sim+eng` for image recognition with installed Tesseract models. Extraction drafts include normalized text boxes and scan fingerprints for page review; rerunning the command resumes the same batch.

Code and tools are licensed under [GNU AGPL v3 or later](LICENSE). Curated data is licensed under [CC BY-SA 4.0](data/LICENSE). Attribute Historical Atlas and the cited sources when reusing the data, and identify your changes.
