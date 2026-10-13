# `swfr` Autonomous Engineering Worklog (`WORKLOG.md`)

Chronological log of tasks completed under **TSD Part 3 (`3.1`–`3.13`)**:

1. **Task 0 (`3.5` M0 Audit Cleanup):**
   - Pinned all GitHub Actions in `.github/workflows/{ci,cross,release,lockfile}.yml` to full 40-char commit SHAs (`actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1`, `actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9`).
   - Added 25-symbol `retro_*` export verification to `ci.yml` (`size-check` and `cross-build`) and `cross.yml` across all 13 target triples.
   - Added native `ubuntu-latest`, `macos-latest`, and `windows-latest` matrix jobs to `build-test` and `abi` in `ci.yml` (using `C:/swfr` on Windows runners to avoid Win32 trailing-dot path rejection on `Flash.`).
   - Updated `ci-report` in `ci.yml` to render a valid Markdown summary table including `lints`, `build-test`, `abi`, `size-check`, `bench` (with per-frame cost table), and `cross-build`, and to ensure `human-test` / `question` labels and Issue #2 formatting are synchronized via `GITHUB_TOKEN`.
   - Marked `docs/platform-notes.md` as `UNVERIFIED` pending owner hardware runs.
   - Opened `question` Issue #3 documenting the Arena session branch (`arena/a4bfe02a-flash`) and GitHub App token scope constraints (`3.5.7`).
   - Wrote `docs/audits/m0.md`.

2. **Milestone M1 (`swfr-reader`, `swfr-swf`, `swfr-testkit`):**
   - Implemented bit/byte stream primitives (`Reader`, `Rect`, `Matrix`, `ColorTransform`, `ParseError`) in `crates/swfr-reader/src/lib.rs`.
   - Implemented `FWS`, `CWS` (`miniz_oxide`), and `ZWS` (`lzma-rs`) container decompression with `Limits` enforcement and full SWF v19 tag parsing in `crates/swfr-swf/src/lib.rs`.
   - Implemented `BitWriter`, `SwfBuilder`, `encode_tag`, `ActionBuilder`, `Mutator`, and `verify_or_create_golden` in `crates/swfr-testkit/src/lib.rs`.
   - Added round-trip tests for all supported SWF v19 tags, container compression modes, long tag headers, and a 100,000-iteration deterministic mutator fuzz test in `crates/swfr-testkit/tests/tag_roundtrip.rs`.
   - Wrote `docs/notes/swf-format.md`, `docs/notes/tag-coverage.md`, and `docs/audits/m1.md`.

3. **Milestone M2 (`swfr-scene` & `swfr-render` Static Rendering):**
   - Implemented character dictionary, display list, `flatten_shape` (bridging SWF dual-fill `fill_style_0`/`fill_style_1` directed edges into closed `tiny-skia` contours), `flatten_morph_shape`, lossless bitmap decoding (`DefineBitsLossless` / `DefineBitsLossless2` formats 3, 4, 5), and JPEG/PNG bitmap decoding (`DefineBits`, `JPEGTables`, `DefineBitsJPEG2/3/4` with `FF D9 FF D8` cleanup) in `crates/swfr-scene/src/lib.rs`.
   - Implemented `Renderer` and 64-entry `ShapeCache` LRU in `crates/swfr-render/src/lib.rs` with solid fills, linear/radial/focal gradients, bitmap patterns, round-cap/join strokes, clipping masks (`clip_depth`), color transforms, dirty-rect skip, and `XRGB8888`/`RGB565` output buffers.
   - Added golden rendering tests (`crates/swfr-render/tests/render_goldens.rs`), generated provisional goldens (`crates/swfr-render/tests/golden-provisional/`), registered them in `GOLDENS.md`, and opened `human-test` Issue #4 (V2).
   - Wrote `docs/audits/m2.md`.

4. **Milestone M3 (Timeline, Sprites, Buttons, Input, and Text):**
   - Implemented multi-frame `Timeline`, independent nested `DefineSprite` (`MovieClip`) timelines, frame labels, `goto_frame` display-list reconstruction, `DefineButton`/`DefineButton2` state transitions (`Up`/`Over`/`Down`) with even-odd vector hit-testing (`hit_test_button`), and `DefineEditText` variable synchronization (`sync_variable`) in `crates/swfr-scene/src/lib.rs`.
   - Implemented `render_static_text` (`DefineFont*` + `DefineText*`), `render_edit_text` (`DefineEditText` with embedded glyph outlines and built-in vector stroke fallback), `render_button`, and software cursor reticle (`draw_cursor`) in `crates/swfr-render/src/lib.rs`.
   - Documented input mapping in `docs/keymap.md`, opened `human-test` Issues #5 (V3) and #6 (V4), and wrote `docs/audits/m3.md`.

5. **Milestone M4 (`swfr-avm1` ActionScript 1 & 2 VM):**
   - Implemented generational arena `Heap`, mark-and-sweep garbage collector (`collect_garbage`), pre-decoded `Vec<Op>` bytecode compiler (`decode_ops`), stack machine (`Avm1`), closures, prototype chains, built-in classes/globals (`Object`, `Array`, `String`, `Number`, `Boolean`, `Math`, `Key`, `Mouse`, `Stage`, `Sound`, `MovieClip`), deterministic frame-based timers (`setInterval`/`setTimeout`), and per-frame instruction watchdog in `crates/swfr-avm1/src/lib.rs`.
   - Benchmarked `Vec<Op>` dispatch vs raw byte match in `DECISIONS.md` (`D-008`, 2.08x speedup).
   - Added 160-movie ES3 conformance test suite, infinite-loop watchdog test, and timer/key test in `crates/swfr-avm1/tests/conformance.rs`.
   - Opened `human-test` Issue #7 (V5) and wrote `docs/audits/m4.md`.

6. **Milestone M5 (`swfr-audio` PCM, ADPCM, MP3, Resampler, and Mixer):**
   - Implemented PCM (8/16-bit mono/stereo), SWF IMA ADPCM (2/3/4/5-bit), MP3 (`symphonia 0.5.5` with `SeekSamples` skip), linear resampler to 44.1 kHz stereo (`resample_to_44k1`), 32-voice `AudioMixer` with envelope interpolation, and 2-frame drift-bounded streaming FIFO in `crates/swfr-audio/src/lib.rs`.
   - Added 300-frame marker-pulse stream synchronization unit test (`stream_sync_300_frames_marker_pulse_within_one_frame`) in `crates/swfr-audio/src/lib.rs`.
   - Integrated `Stage`, `Renderer`, `Avm1`, and `AudioMixer` into `crates/swfr-player/src/lib.rs`.
   - Opened `human-test` Issue #8 (V6) and wrote `docs/audits/m5.md`.

7. **Milestone M6 (Packaging, Cross-Compilation, Benchmarks, and v1 Docs):**
   - Implemented `swfr-harness bench` (with 30% regression gate, measured at `0.186 ms/frame` / `5376.3 fps` in CI release builds) and `swfr-harness dump-test-swfs` in `crates/swfr-harness/`.
   - Added `LICENSE-MIT`, `LICENSE-APACHE`, `THIRD_PARTY_LICENSES.md`, `README.md`, `docs/INSTALL.md`, and `docs/KNOWN_LIMITATIONS.md`.
   - Verified all 13 targets in `cross.yml` ([Run #38333151604](https://github.com/wastaken90XD/Flash./actions/runs/38333151604), including `armv7-unknown-linux-gnueabihf` QEMU user-mode ABI test) and all 13 release zips in `release.yml` ([Run #38333151663](https://github.com/wastaken90XD/Flash./actions/runs/38333151663)).
   - Opened `human-test` Issue #9 (V7) and final summary Issue #10 (`v1 engineering-complete report`), and wrote `docs/audits/m6.md`.

8. **TSD Part 3.14 End-to-End Acceptance & M0–M6 Re-Audit (`3.14.1`–`3.14.7`):**
   - **Stub Gated (`3.14.1`):** Placed M0 color-bar and 440 Hz tone generators behind off-by-default Cargo feature `m0-stub` in `crates/swfr-player/Cargo.toml` and `crates/swfr-player/src/lib.rs`. Added CI checks in `ci.yml`, `cross.yml`, and `release.yml` verifying `m0-stub` is disabled in all shipped builds.
   - **No Silent Fallback (`3.14.2`):** Removed fallback to `runtime = None` in `Player::new`; `retro_load_game` now logs the error via `retro_log_printf_t` and returns `false` on any malformed, truncated, or unparseable input.
   - **Real Dependency Chain (`3.14.3`):** Added `cargo tree -p swfr-libretro` checks in `ci.yml`, `cross.yml`, and `release.yml` asserting `swfr-swf`, `swfr-scene`, `swfr-render`, `swfr-avm1`, and `swfr-audio` are linked into `swfr-libretro`.
   - **Real-World SWF Parser & Scene Fixes:** Fixed `DefineShape4` (`LINESTYLE2`) MSB-first bitfield and single fill-style decoding and `DefineMorphShape` gradient/bitmap fill decoding in `crates/swfr-swf/src/lib.rs`; fixed recursive `DefineSprite` dictionary pre-registration and initial frame-0 placement stepping in `crates/swfr-scene/src/lib.rs`; ordered `stage.step_frame()` before `stage.update_pointer()` in `Player::run_frame`; implemented `SharedObject.getLocal` (`64 KiB` `RETRO_MEMORY_SAVE_RAM` with `"SWFS"` magic + CRC32), `loadMovie` relative-path traversal validation, and once-per-movie AVM2 diagnostic logging.
   - **End-to-End ABI Test Suite (`3.14.4` & `3.14.7`):** Added 13 end-to-end tests in `crates/swfr-libretro/tests/e2e.rs` driving only the exported `retro_*` C ABI symbols (1-frame red rect on blue stage + `av_info`, 10-frame moving square, PCM sound event vs silent movie, AS2 `trace('hello')` log callback, button movie with scripted pointer click, malformed files returning `false`, 20 real corpus SWFs x 120 frames, plus M1–M6 milestone E2E tests).
   - **Real Corpus CI Job (`3.14.5`):** Populated `corpus/real/MANIFEST.toml` with 20 open-licensed real-world SWFs from `ruffle-rs/ruffle`, added `scripts/fetch_real_corpus.py`, and added the `corpus-real` job to `ci.yml`.
   - **Honest Benchmarks (`3.14.6`):** Updated `swfr-harness bench` and `bench/baseline.json` to benchmark both the 10-frame moving-square movie (`0.026 ms/frame`) and the largest real corpus SWF (`ruffle-sound.swf`, `6,336 B`, `0.029 ms/frame`) through the full `Player` pipeline.
   - **Re-Audited `docs/audits/m0.md`..`m6.md` (`3.14.7`):** Rewrote all 7 audit documents with named passing tests and CI run links (`ci.yml` `#38485909138`, `cross.yml` `#38485913390`, `release.yml` `#38485909236`).
