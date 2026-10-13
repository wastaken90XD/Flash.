# `swfr` (`swfr_libretro`)

`swfr` is a clean-room, pure-Rust **libretro core** (`swfr_libretro`) for playing Adobe Flash (`.swf`) animations and ActionScript 1 & 2 (AVM1) games inside any libretro frontend (such as RetroArch) across Linux, Windows, macOS, Android (API 21+), and iOS.

## Features (v1)

- **SWF Containers & Tags (`swfr-reader`, `swfr-swf`):** Uncompressed (`FWS`), zlib-compressed (`CWS`), and LZMA-compressed (`ZWS`) SWF files up to specification v19, with strict decompression ratio (`10x`) and resource limits.
- **2D Vector & Raster Rendering (`swfr-scene`, `swfr-render`):** Software rasterization via `tiny-skia` (`XRGB8888` and `RGB565`), solid/linear/radial/focal gradients, repeating and clipped bitmap fills (`DefineBits*`, `DefineBitsLossless*`), morph shapes (`DefineMorphShape*`), clipping masks (`clip_depth`), 64-entry flattened-shape LRU cache, and dirty-rectangle tracking (`RETRO_ENVIRONMENT_GET_CAN_DUPE`).
- **Timeline, Buttons & Text (`swfr-scene`, `swfr-render`):** Multi-frame root and nested `DefineSprite` timelines, frame labels, interactive buttons (`DefineButton`, `DefineButton2`) with vector point-in-path hit-testing, static text (`DefineText*`), and variable-bound dynamic text (`DefineEditText`).
- **ActionScript 1 & 2 VM (`swfr-avm1`):** Pre-decoded `Vec<Op>` bytecode interpreter, ECMA-262 3rd edition coercions (with SWF <= 6 vs SWF >= 7 semantics), generational arena `Heap` with incremental mark-and-sweep GC, built-in classes (`Object`, `Array`, `String`, `Number`, `Boolean`, `Math`, `Key`, `Mouse`, `Stage`, `Sound`, `MovieClip`), deterministic frame-counted timers (`setInterval`, `setTimeout`), and per-frame instruction watchdog.
- **Audio Engine (`swfr-audio`):** 8/16-bit PCM, SWF IMA ADPCM (2, 3, 4, and 5-bit), and MP3 decoding (via `symphonia`), linear resampling to `44,100 Hz` stereo `i16`, up to 32 simultaneous event voices, and frame-locked streaming audio (`SoundStreamHead*`, `SoundStreamBlock`).
- **Panic-Safe Libretro C ABI (`swfr-libretro`):** Exports all 25 `retro_*` entrypoints verified against `docs/reference/libretro.h`, wrapped in `catch_unwind` with a dead-frame fallback.

## Workspace Layout

| Crate | Responsibility |
| :--- | :--- |
| `crates/swfr-reader` | Bit-aligned and byte-aligned SWF primitive stream reader (`RECT`, `MATRIX`, `CXFORM`). |
| `crates/swfr-swf` | `FWS`/`CWS`/`ZWS` container decompression and SWF v19 tag parser. |
| `crates/swfr-scene` | Character dictionary, display list, dual-fill shape flattener, timeline engine, and button hit-testing. |
| `crates/swfr-render` | `tiny-skia` software renderer, 64-entry shape LRU cache, text & button rendering. |
| `crates/swfr-avm1` | ActionScript 1 & 2 interpreter, generational arena heap, mark-and-sweep GC, and built-ins. |
| `crates/swfr-avm2` | Reserved stub for ActionScript 3 (AVM2, M7). |
| `crates/swfr-audio` | PCM, SWF ADPCM, and MP3 decoders, 44.1 kHz linear resampler, and 32-voice mixer. |
| `crates/swfr-player` | Frame-stepping coordinator (`Player::new`, `av_info`, `run_frame`, `reset`). |
| `crates/swfr-libretro` | Libretro C ABI (`cdylib` / `rlib`) and core options (`swfr_*`). |
| `crates/swfr-harness` | Headless CLI inspector (`info`), benchmark runner (`bench`), and test SWF generator. |
| `crates/swfr-testkit` | Programmatic `SwfBuilder`, `ActionBuilder`, deterministic `Mutator`, and golden verifier. |

## Documentation

- [Installation & RetroArch Setup (`docs/INSTALL.md`)](docs/INSTALL.md)
- [Default Input & Controller Mapping (`docs/keymap.md`)](docs/keymap.md)
- [Known Limitations in v1 (`docs/KNOWN_LIMITATIONS.md`)](docs/KNOWN_LIMITATIONS.md)
- [Technical Specification Document (`docs/TSD.md`)](docs/TSD.md)
- [SWF v19 Tag Coverage Matrix (`docs/notes/tag-coverage.md`)](docs/notes/tag-coverage.md)

## License

Dual-licensed under **MIT** (`LICENSE-MIT`) or **Apache-2.0** (`LICENSE-APACHE`) at your option.
