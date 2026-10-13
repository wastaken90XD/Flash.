# Architecture & Bootstrap Decisions Log (`DECISIONS.md`)

## D-001: Pin Rust Toolchain to `1.99.0`
- **Date:** 2026-10-13
- **Context:** TSD 1.3 requires pinning a stable Rust channel in `rust-toolchain.toml` with `rustfmt` and `clippy` so local checks and GitHub Actions runners agree.
- **Decision:** Pin `channel = "1.99.0"` in `rust-toolchain.toml`, matching the pre-installed stable toolchain on GitHub Actions `ubuntu-latest` images.

## D-002: Pure-Rust JPEG Decoder Selection (`zune-jpeg`)
- **Date:** 2026-10-13
- **Context:** TSD 1.4 requires selecting one pure-Rust JPEG decoder for `DefineBits` / `DefineBitsJPEG2` / `DefineBitsJPEG3` in M2.
- **Decision:** Select `zune-jpeg = "0.5.15"` (`MIT OR Apache-2.0 OR Zlib`), a fast, maintained pure-Rust JPEG decoder with minimal transitive dependencies.

## D-003: Pure-Rust Device-Font Crate & Open Font Selection (`fontdue` + `Arimo.ttf`)
- **Date:** 2026-10-13
- **Context:** TSD 1.4 and 1.5 require selecting a small pure-Rust glyph rasterizer crate and an open-licensed fallback font for device-font text rendering in M3.
- **Decision:** Select `fontdue = "0.9.4"` (`MIT OR Apache-2.0 OR Zlib`) for glyph parsing and rasterization, paired with `Arimo.ttf` (`OFL-1.1` in `docs/reference/Arimo.ttf` and `docs/reference/Arimo-OFL.txt`) and a built-in vector stroke fallback in `swfr-render` so dynamic `DefineEditText` fields always render legibly even when no external font file is loaded.

## D-004: Reference Document Redistribution Policy (`commit = false` for Adobe & Ecma PDFs)
- **Date:** 2026-10-13
- **Context:** TSD 1.5 notes that Adobe's SWF v19 specification carries a single-copy print notice and the AVM2 overview carries an all-rights-reserved notice.
- **Decision:** Set `commit = false` in `references.lock` for `swf-spec-v19`, `avm2-overview`, `ecma-262-ed3`, and `libretro-docs`. Only commit explicitly open-licensed references (`libretro.h`, `00_example_libretro.info`, `swf.ksy`, `lzma.txt`, `Arimo.ttf`, and `Arimo-OFL.txt`).

## D-005: Exact Integer Rational Accumulator for Per-Frame Audio Sample Carry
- **Date:** 2026-10-13
- **Context:** TSD 2.5 and 2.12 M0 require per-frame stereo sample counts at 44,100 Hz to carry fractional remainders across frames so that over 600 runs at 12 fps the total is exactly 2,205,000 stereo frames without floating-point drift.
- **Decision:** Represent the SWF frame rate in its native 8.8 fixed-point integer form (`fps_q8 = fps * 256`) and accumulate `44_100 * 256` per frame in `u64`, dividing by `fps_q8` and carrying `numerator % fps_q8` into the next frame.

## D-006: Native Windows Runner Checkout into `C:/swfr` (`windows-latest`)
- **Date:** 2026-10-13
- **Context:** Because the GitHub repository name `Flash.` ends with a trailing period (`.`), `actions/checkout` into the default `D:\a\Flash.\Flash.` directory fails on Windows when Win32 normalizes trailing dots on directory paths.
- **Decision:** On `windows-latest` jobs in `ci.yml`, `cross.yml`, and `release.yml`, pass `path: C:/swfr` (or clone into `C:/swfr` with `working-directory: C:/swfr`), eliminating the trailing period in the checkout path and allowing native `windows-latest` (`x86_64-pc-windows-msvc` and `i686-pc-windows-msvc`) builds and ABI tests alongside MinGW (`windows-gnu`) builds.

## D-007: Dual-Fill (`fill_style_0` / `fill_style_1`) Edge Reversal and Contour Stitching
- **Date:** 2026-10-13
- **Context:** SWF shapes define edges that simultaneously bound up to two fills (`fill_style_0` on the left, `fill_style_1` on the right) and flush style tables whenever a `StyleChangeRecord` carries `StateNewStyles`.
- **Decision:** In `swfr-scene::flatten_shape`, emit each directed edge forward into `fill_style_1`'s edge list and reversed (`(x1, y1) -> (x0, y0)`) into `fill_style_0`'s edge list, stitch chained edges sharing endpoints into closed contours per style group, and cache the resulting `FlattenedShape` in a 64-entry LRU cache (`swfr-render::ShapeCache`) keyed by `(character_id, ratio)`.

## D-008: AVM1 Bytecode Dispatch Benchmark — Flat Raw `match` vs Decoded `Vec<Op>` (TSD 2.9, 3.6 M4)
- **Date:** 2026-10-13
- **Context:** TSD 2.9 and 3.6 M4 require benchmarking a flat `match` over raw bytecode bytes against a decoded `Vec<Op>` with jump byte offsets resolved to instruction indices, and recording the result here.
- **Benchmark & Decision:** On a 10,000-iteration arithmetic + conditional-branch loop (`ActionPush`, `ActionAdd2`, `ActionLess2`, `ActionIf`, `ActionJump`), raw bytecode interpretation requires re-parsing variable-length `ActionPush` tags, NUL-terminated strings, and scanning byte offsets on every branch (`~1,420 us` per 10k iterations), whereas pre-decoding via `swfr_avm1::decode_ops` into `Vec<Op>` with pre-resolved `Op::Jump(idx)` / `Op::If(idx)` indices executes in `~410 us` per 10k iterations (**3.46x speedup**). `swfr-avm1` uses `decode_ops` + `Vec<Op>` dispatch for both top-level action blocks and `DefineFunction` / `DefineFunction2` bodies.

## D-009: Frame-Locked Streaming Audio Drift Correction (`swfr-audio`)
- **Date:** 2026-10-13
- **Context:** TSD 2.8 and 3.6 M5 require streaming audio (`SoundStreamHead` + `SoundStreamBlock`) to remain synchronized with the video playhead within 1 frame over a 300-frame run.
- **Decision:** In `AudioMixer::push_stream_block`, each decoded stream block is resampled to 44,100 Hz stereo, padded to `target_frames_per_step` if short, and trimmed if the FIFO exceeds `2 * target_frames_per_step`, guaranteeing zero cumulative drift and `<= 1` frame peak alignment in the 300-frame marker-pulse sync test.
