# Historical Atlas

A spatiotemporal atlas and evidence ledger for chronicle applications. Historical names, populations, and coordinates link to their sources, with interpretations recorded as separate estimates.

## Usage

Requires Rust with the `wasm32-unknown-unknown` target, Python 3.12 or later, Poppler, and Tesseract with the English model for extraction tests. Image recognition of Chinese sources also uses the `chi_sim` model.

```sh
cargo run -p atlas-compile -- verify data
cargo run -p atlas-compile -- build data dist
cargo run -p atlas-compile -- register data controls.json registered.json
cargo run -p atlas-compile -- slice data <tt-ms> nominal <new-output-directory>
sh scripts/ci.sh
```

Curated records live in `data/`. `scan_page` is one-based; `page_or_sheet` identifies the printed page. `checked` records transcription review. Dates retain their source convention, and coordinates retain their stated datum and angular precision.

Time slices select dated name, hierarchy, and demographic estimates. Supply a signed decimal TT millisecond offset from J2000 and choose `nominal`, `possible`, or `certain`. The output includes `slice.json`, UUID shards under `slice/index/`, and the complete evidence index. Each selected value retains its interval, source-record links, uncertainty, and source disagreements. Competing values appear together with a conflict flag; different name languages occupy separate slots.

For page extraction, run `python3 tools/ocr-ingest/ingest.py --pdf <path> --pages 1:10 --output scratch/extraction`. The default engine reads the embedded text layer. Use `--engine tesseract --languages chi_sim --tessdata-dir <models>` for image recognition; `--jobs 4` processes four pages concurrently. Extraction drafts include normalized text boxes, scan fingerprints, and recognition settings for page review; rerunning the command resumes the same batch.

Group extracted table cells with `python3 tools/ocr-ingest/tables.py scratch/extraction scratch/tables --profile bijie`. Profiles are available for Bijie, Anshun, and Liuzhi. Table drafts retain the text boxes and mark fields that need review.

Sheet registration accepts a `RegistrationJob` JSON document with a catalogued source, scan page, full-page raster dimensions, target CRS and units, referenced control points, independent check points, and traced vertices. Source coordinates use image pixels measured from the top-left corner. Choose `{"kind":"affine"}` or `{"kind":"thin_plate","smoothing":0.0}`. The result includes the scan fingerprint, transformed vertices, and residuals in the declared target units. See `atlas_compile::registration` in the generated Rust API documentation for the input fields.

Code and tools are licensed under [GNU AGPL v3 or later](LICENSE). Curated data is licensed under [CC BY-SA 4.0](data/LICENSE). Attribute Historical Atlas and the cited sources when reusing the data, and identify your changes.
