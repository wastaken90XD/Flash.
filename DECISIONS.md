# Architecture & Bootstrap Decisions Log

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
- **Decision:** Select `fontdue = "0.9.4"` (`MIT OR Apache-2.0 OR Zlib`) for glyph parsing and rasterization, paired with `Arimo.ttf` (`OFL-1.1` in `docs/reference/Arimo.ttf` and `docs/reference/Arimo-OFL.txt`) as the metrically compatible sans-serif fallback font.

## D-004: Reference Document Redistribution Policy (`commit = false` for Adobe & Ecma PDFs)
- **Date:** 2026-10-13
- **Context:** TSD 1.5 notes that Adobe's SWF v19 specification carries a single-copy print notice and the AVM2 overview carries an all-rights-reserved notice.
- **Decision:** Set `commit = false` in `references.lock` for `swf-spec-v19`, `avm2-overview`, `ecma-262-ed3`, and `libretro-docs`. Only commit explicitly open-licensed references (`libretro.h`, `00_example_libretro.info`, `swf.ksy`, `lzma.txt`, `Arimo.ttf`, and `Arimo-OFL.txt`).

## D-005: Exact Integer Rational Accumulator for Per-Frame Audio Sample Carry
- **Date:** 2026-10-13
- **Context:** TSD 2.5 and 2.12 M0 require per-frame stereo sample counts at 44,100 Hz to carry fractional remainders across frames so that over 600 runs at 12 fps the total is exactly 2,205,000 stereo frames without floating-point drift.
- **Decision:** Represent the SWF frame rate in its native 8.8 fixed-point integer form (`fps_q8 = fps * 256`) and accumulate `44_100 * 256` per frame in `u64`, dividing by `fps_q8` and carrying `numerator % fps_q8` into the next frame.

## D-006: Cross-Compile Windows `swfr_libretro.dll` via `x86_64-pc-windows-gnu` on `ubuntu-latest`
- **Date:** 2026-10-13
- **Context:** Because the GitHub repository name `Flash.` ends with a trailing period (`.`), `actions/checkout` on `windows-latest` fails when Win32 strips trailing dots from `D:\a\Flash.\Flash.`.
- **Decision:** Build the Windows 10 `x86_64` core (`swfr_libretro.dll`) in `.github/workflows/cross.yml` on `ubuntu-latest` using `x86_64-pc-windows-gnu` (`gcc-mingw-w64-x86-64`).
