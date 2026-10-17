# Third-Party Licenses (`THIRD_PARTY_LICENSES.md`)

`swfr` is dual-licensed under `MIT OR Apache-2.0`. All direct runtime dependencies are pure Rust and permissively licensed (`deny.toml` and `deps-allow.txt`):

| Crate | Version | License | Usage in `swfr` |
| :--- | :--- | :--- | :--- |
| `miniz_oxide` | `0.9.1` | `MIT OR Zlib OR Apache-2.0` | Zlib decompression (`CWS`, `DefineBitsLossless*`, `DefineBitsJPEG3` alpha) |
| `lzma-rs` | `0.3.0` | `MIT` | LZMA decompression (`ZWS` SWF 13+ containers) |
| `tiny-skia` | `0.12.0` | `BSD-3-Clause` | CPU 2D vector path, gradient, pattern, and mask rasterizer |
| `png` | `0.18.1` | `MIT OR Apache-2.0` | PNG decoding inside `DefineBitsJPEG2/3` and golden test verification |
| `zune-jpeg` | `0.5.15` | `MIT OR Apache-2.0 OR Zlib` | JPEG decoding for `DefineBits`, `JPEGTables`, `DefineBitsJPEG2/3/4` |
| `fontdue` | `0.9.4` | `MIT OR Apache-2.0 OR Zlib` | TrueType glyph outline and metric support |
| `symphonia` | `0.5.5` | `MPL-2.0` | MP3 audio decoding (`DefineSound` and `SoundStreamBlock`) |

## Reference Assets (`docs/reference/`)

- `docs/reference/libretro.h`: `MIT` (Copyright (C) 2010-2020 The RetroArch team).
- `docs/reference/Arimo.ttf` & `Arimo-OFL.txt`: `OFL-1.1` (SIL Open Font License 1.1).
