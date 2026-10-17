# `swfr` Test Corpus (`corpus/`)

There is no reference Flash Player binary in CI to compare against. Test expectations in `swfr` come from three sources (TSD 1.6):

1. **Hand-derived values for simple shapes and records:**
   - Exact pixel values, bounding boxes, matrix transforms, and color transforms derived analytically from known inputs.
2. **Expected `trace()` and state output from programmatic test movies:**
   - `crates/swfr-testkit` builds SWF movies byte by byte in Rust code (headers, tags, bit-packed shape records, AS1/AS2 bytecode via the built-in assembler, PCM/ADPCM sounds, and deliberately malformed variants).
   - Because each test movie is constructed programmatically in Rust, its exact expected `trace()` log, display list state, and audio sample properties sit right beside the generator.
3. **Human-approved golden frames (`golden/`):**
   - Rendered frame PNGs that require visual inspection are proposed in pull requests with the `goldens-review` label and approved by `@wastaken90XD` with the `goldens-approved` label.

## Real-World SWF Corpus (`corpus/real/MANIFEST.toml`)

- Real-world SWF files are never committed to the repository.
- `corpus/real/MANIFEST.toml` lists downloadable files with `name`, `url`, `sha256`, `license`, `source_page`, `swf_version`, and `frames_to_capture`.
- The `lints` CI check rejects any entry in `corpus/real/MANIFEST.toml` that is missing a non-empty `license`, `url`, or `sha256`.
- `@wastaken90XD` reviews each license before merging changes to `corpus/real/MANIFEST.toml`.
