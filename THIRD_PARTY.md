# Third-Party Crates and Reference Assets (`THIRD_PARTY.md`)

## Direct Rust Crate Dependencies

All direct external dependencies are declared in the root `Cargo.toml` and allowlisted in `deps-allow.txt`.

| Crate | Version | Purpose | First Used | License (SPDX) |
| :--- | :--- | :--- | :--- | :--- |
| `miniz_oxide` | `0.9.1` | Zlib/Deflate decompression (`CWS` SWF files, lossless bitmaps) | M1 | `MIT OR Zlib OR Apache-2.0` |
| `lzma-rs` | `0.3.0` | LZMA decompression (`ZWS` SWF files) | M1 | `MIT` |
| `tiny-skia` | `0.12.0` | Software 2D vector path rasterizer and compositing | M2 | `BSD-3-Clause` |
| `png` | `0.18.1` | PNG decoding and golden frame PNG encoding | M2 | `MIT OR Apache-2.0` |
| `zune-jpeg` | `0.5.15` | Pure-Rust JPEG decoding (`DefineBitsJPEG*`) | M2 | `MIT OR Apache-2.0 OR Zlib` |
| `fontdue` | `0.9.4` | Pure-Rust TTF/OTF glyph rasterizer for device fonts | M3 | `MIT OR Apache-2.0 OR Zlib` |
| `symphonia` | `0.6.1` | MP3 audio decoding (`default-features = false, features = ["mp3"]`) | M5 | `MPL-2.0` |

## Committed Reference Files (`docs/reference/`)

| File | Source | License |
| :--- | :--- | :--- |
| `docs/reference/libretro.h` | `libretro/libretro-common` (`include/libretro.h`) | `MIT` |
| `docs/reference/00_example_libretro.info` | `libretro/libretro-super` (`dist/info/00_example_libretro.info`) | `MIT` |
| `docs/reference/swf.ksy` | `kaitai-io/kaitai_struct_formats` (`executable/swf.ksy`) | `CC0-1.0` |
| `docs/reference/lzma.txt` | `ip7z/7zip` (`DOC/lzma.txt`) | `Public Domain` |
| `docs/reference/Arimo.ttf` | `google/fonts` (`ofl/arimo/Arimo[wght].ttf`) | `OFL-1.1` (see `docs/reference/Arimo-OFL.txt`) |
| `docs/reference/Arimo-OFL.txt` | `google/fonts` (`ofl/arimo/OFL.txt`) | `OFL-1.1` |
