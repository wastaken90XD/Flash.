# `swfr` — Pure-Rust SWF Player & `libretro` Core

`swfr` is a clean-room Adobe Flash (SWF) player implemented from scratch in Rust and packaged as a `libretro` core (`swfr_libretro`).

## Workspace Crates

- **`crates/swfr-reader`** — Bitstream and bytestream parsing primitives.
- **`crates/swfr-swf`** — SWF decompression (`FWS`, `CWS`, `ZWS`), headers, and tag decoders.
- **`crates/swfr-scene`** — Character dictionary, display list, and timeline management.
- **`crates/swfr-render`** — Software 2D vector, bitmap, and text renderer.
- **`crates/swfr-avm1`** — ActionScript 1 & ActionScript 2 (AVM1) interpreter.
- **`crates/swfr-avm2`** — ActionScript 3 (AVM2) placeholder (M7).
- **`crates/swfr-audio`** — PCM, ADPCM, and MP3 audio decoding and mixer.
- **`crates/swfr-player`** — Deterministic frame-stepping player coordinator.
- **`crates/swfr-libretro`** — `libretro` C ABI frontend (`cdylib` and `rlib`; the only crate permitted a scoped `#[allow(unsafe_code)]` on `ffi`).
- **`crates/swfr-harness`** — Headless development and verification CLI.
- **`crates/swfr-testkit`** — Byte-level SWF generator, AS1/AS2 bytecode assembler, and golden test helpers.

See [`AGENTS.md`](AGENTS.md) for engineering and CI workflow rules, and [`STATUS.md`](STATUS.md) for milestone progress.
