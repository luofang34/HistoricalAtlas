# Historical Atlas

A spatiotemporal atlas and evidence ledger for chronicle applications. Historical names, populations, and coordinates link to their sources, with interpretations recorded as separate estimates.

## Usage

Requires Rust with the `wasm32-unknown-unknown` target, Python 3.12 or later, Poppler, and Tesseract with the English model for extraction tests. Image recognition of Chinese sources also uses the `chi_sim` model.

```sh
cargo run -p atlas-compile -- verify data
cargo run -p atlas-compile -- build data dist
cargo run -p atlas-compile -- register data controls.json registered.json
cargo run -p atlas-compile -- slice data <tt-ms> nominal <new-output-directory>
cargo run -p atlas-compile -- reconstruct data data/paleogeography/panxian-240ma.reconstruction.json <new-output-directory>
cargo run -p atlas-compile -- tiles data https://example.org/atlas/ 14 <new-output-directory>
sh scripts/ci.sh
```

Curated records live in `data/`. `scan_page` is one-based; `page_or_sheet` identifies the printed page. Online sources carry a URL and records identify the cited section or table in `locator`. `checked` records transcription review. Dates retain their source convention, and coordinates retain their stated datum and angular precision.

The `tiles` command publishes reviewed coordinate observations as body-separated MVT layers, TileJSON manifests, map styles, and the complete evidence index. Set `base-url` to the directory's final HTTP(S) address. `tiles/index.json` lists the available bodies; each tileset includes a sparse tile inventory. Points retain their record and entity UUIDs, original coordinate notation, source datum, reporting precision, and evidence URL. Dates remain source expressions; the time-slice indexes supply dated interpretations. The display projection is spherical Mercator, with buffered tile edges and an explicit error for coordinates outside its latitude domain.

Time slices select dated name, hierarchy, and demographic estimates. Supply a signed decimal TT millisecond offset from J2000 and choose `nominal`, `possible`, or `certain`. The output includes `slice.json`, UUID shards under `slice/index/`, and the complete evidence index. Each selected value retains its interval, source-record links, uncertainty, and source disagreements. Competing values appear together with a conflict flag; different name languages occupy separate slots.

Entity slices also contain `origins`: evidence that formation or establishment had occurred by the selected time. Origin dates distinguish bounded estimates, latest-formation limits, and approximate ages with unquantified uncertainty. Nominal mode uses the representative date; possible and certain modes evaluate the supplied constraints. Later survival is described by separate temporal properties. Lunar records retain the geological age convention and the dated UTC-to-TT conversion used for Apollo 11.

Plate reconstruction requests select a TT epoch, a model, an anchor plate, and pairs of coordinate records and plate-assignment estimates. Model excerpts in `*.plates.json` retain the source fingerprint, attribution, reference frame, and original rotation-file line numbers. The output includes `reconstruction.json`, the model under `plates/`, and the evidence index. Each derived position retains its original coordinate notation and datum alongside the resolved rotation circuit. The Panxian example uses the Merdith model's South China circuit at 240 Ma; its excerpt covers 0–250 Ma. Geological query ages use Julian million years before the model's declared present epoch.

For page extraction, run `python3 tools/ocr-ingest/ingest.py --pdf <path> --pages 1:10 --output scratch/extraction`. The default engine reads the embedded text layer. Use `--engine tesseract --languages chi_sim --tessdata-dir <models>` for image recognition; `--jobs 4` processes four pages concurrently. Extraction drafts include normalized text boxes, scan fingerprints, and recognition settings for page review; rerunning the command resumes the same batch.

Group extracted table cells with `python3 tools/ocr-ingest/tables.py scratch/extraction scratch/tables --profile bijie`. Profiles are available for Bijie, Anshun, and Liuzhi. Table drafts retain the text boxes and mark fields that need review.

Sheet registration accepts a `RegistrationJob` JSON document with a catalogued source, scan page, full-page raster dimensions, target CRS and units, referenced control points, independent check points, and traced vertices. Source coordinates use image pixels measured from the top-left corner. Choose `{"kind":"affine"}` or `{"kind":"thin_plate","smoothing":0.0}`. The result includes the scan fingerprint, transformed vertices, and residuals in the declared target units. See `atlas_compile::registration` in the generated Rust API documentation for the input fields.

Code and tools are licensed under [GNU AGPL v3 or later](LICENSE). Curated data is licensed under [CC BY-SA 4.0](data/LICENSE). Attribute Historical Atlas and the cited sources when reusing the data, and identify your changes.
