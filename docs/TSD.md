# Technical Specification Document (`swfr`)

## Part 1: Preparing the GitHub Repository

- **1.1 How the AI works here:** CI is the only trusted build; workflow jobs handle all downloads; job summaries and sticky PR comments provide text feedback; goldens and hardware tests go to the human maintainer.
- **1.2 Repository configuration:** Protected `main` (`lints`, `build-test`, `abi`, `cross-build`), `.github/CODEOWNERS`, read-only default token, pinned SHA actions, labels (`human-test`, `goldens-review`, `goldens-approved`, `needs-reference`, `blocked`), issue templates (`human-test.md`, `need.md`).
- **1.3 Bootstrap skeleton:** Workspace with 11 crates (`swfr-reader`, `swfr-swf`, `swfr-scene`, `swfr-render`, `swfr-avm1`, `swfr-avm2`, `swfr-audio`, `swfr-player`, `swfr-libretro`, `swfr-harness`, `swfr-testkit`), `rust-toolchain.toml`, `deny.toml`, `deps-allow.txt`, `references.lock`.
- **1.4 Allowed direct dependencies:** `miniz_oxide`, `lzma-rs`, `tiny-skia`, `png`, `zune-jpeg`, `fontdue`, `symphonia` (`mp3` only).
- **1.5 References:** `references.lock` and `docs/notes/<topic>.md`.
- **1.6 Test corpus:** Hand-derived expectations, programmatic test movies in `swfr-testkit`, and human-approved goldens.
- **1.8 Frontend baseline (V0):** Recorded in `docs/platform-notes.md`.

---

## Part 2: Technical Specification

### 2.1 Goal
`swfr` is a libretro core named `swfr_libretro` that loads `.swf` files and plays them inside any libretro frontend, written from scratch in Rust.

### 2.2 What libretro changes
- Window, GPU, audio device: Owned by the frontend (`no winit`, `no softbuffer`, `no cpal`).
- Threads: Single-threaded on the frontend's thread inside `retro_run`.
- Audio: Pushed once per frame through `retro_audio_sample_batch_t` at 44,100 Hz stereo `i16`.
- Video: CPU-rendered 32-bit `XRGB8888` (`0x00RRGGBB`) frame handed to the frontend each frame.
- Content: Frontend passes SWF bytes at load (`need_fullpath = false`).
- Save states: Not supported in v1 (`retro_serialize_size` returns 0).

### 2.5 Libretro Integration
- Every constant, struct layout, and function signature is verified against `docs/reference/libretro.h` (see `docs/notes/libretro-abi.md`).
- Core options: `swfr_scale` (`1x`, `2x`, `3x`, `4x`), `swfr_quality` (`low`, `medium`, `high`), `swfr_pad_mode` (`cursor`, `keys`), `swfr_cursor_speed` (`1`..`10`), `swfr_script_timeout` (`1`, `2`, `5`, `15`), `swfr_trace_to_log` (`on`, `off`).
- Panic safety: Every exported `extern "C"` function wraps its body in `catch_unwind`. After a panic, the core is marked dead and subsequent `retro_run` calls emit the last good frame (or a solid dark red frame `0x00800000` if none) with silent audio and log the panic once.

### 2.6 Workspace & Player Contract
```rust
Player::new(swf_bytes: &[u8], base_dir: Option<String>, opts: &Options) -> Result<Player, PlayerError>
Player::av_info(&self) -> AvInfo
Player::run_frame(&mut self, input: &Input) -> FrameOut<'_>
Player::reset(&mut self)
```

### 2.11 Safety and Limits
- `Input file size`: 64 MiB
- `Decompressed size`: 64 MiB
- `Tag count`: 2,000,000
- `Edges in one shape`: 1,000,000
- `Movie clip nesting depth`: 256
- `AVM call depth`: 256
- `Bitmap side length`: 8,192 px
- `Total bitmap memory`: 64 MiB
- `String length`: 16 MiB
- `Array length`: 1,048,576
- `Script time per frame`: option, default 2 s
- `Script heap, desktop`: 128 MiB
- `Script heap, Reference B`: 48 MiB

### 2.12 Milestones
- **M0:** Bring-up with a stub player (320x240 color bars, 440 Hz tone, `libretro.h` constant verification test, mock frontend ABI test, `swfr_libretro.info`, Linux & Windows builds in `cross.yml`, `swfr-harness` `AvInfo` CLI, V1 human-test issue).
- **M1:** Container and tags (`swfr-reader`, `swfr-swf`, `swfr-testkit` builder & mutator).
- **M2:** Shapes and rendering (`swfr-scene`, `swfr-render`, caches, dirty rectangles, goldens).
- **M3:** Timeline, input, text, and libretro wiring.
- **M4:** AVM1 (AS1 & AS2 interpreter, generational arena heap & incremental collector).
- **M5:** Audio (PCM, ADPCM, MP3 via `symphonia`, resampler, mixer).
- **M6:** Packaging and platform verification (`release.yml`, V5–V7).
- **M7:** AVM2 (ActionScript 3).
