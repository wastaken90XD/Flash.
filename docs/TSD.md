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

---

## Part 3: Autonomous completion directive (for the AI)

Where Part 3 conflicts with Part 2, **Part 3 wins** for the duration of this autonomous run.

### 3.1 Definition of done
1. Every required CI job on `main` (and the active session branch) is green.
2. Milestones **M1 through M6** are implemented as specified in 2.12, with the substitutions in 3.6 where an exit criterion in 2.12 depends on owner hardware or owner approval.
3. `cross.yml` builds the core for **every target in 2.4** (Linux `x86_64`, `x86`, `armv7`, `aarch64`; Android `armv7`, `thumbv7neon`, `aarch64`; Windows `x86_64`, `x86` GNU and MSVC; macOS `arm64`, `x86_64`; iOS `arm64` build-only).
4. `release.yml` produces a draft release (or workflow artifact) with one zip per platform containing the library, `.info`, `README.md`, and licenses.
5. Every tag in the SWF specification v19 either has a builder round-trip test or is listed in `docs/notes/tag-coverage.md` with the reason it is unsupported.
6. The AS1/AS2 conformance suite (M4) has at least 150 generated movies with expected logs derived from ES3 semantics, all passing.
7. Audio tests (M5, including the marker-pulse stream sync test) and rendering tests (M2, M3) pass against approved or provisional goldens (see 3.7).
8. Release core size is under 6 MB and `bench` shows no regression above 30%.
9. `README.md`, `docs/INSTALL.md`, and `docs/KNOWN_LIMITATIONS.md` exist and match the code.
10. All `human-test` issues (V1 through V7) are open with artifact links ready for the owner to run when they return, and a final summary issue titled `v1 engineering-complete report` is open with the checklist from 3.1 and links to every piece of evidence.

### 3.5 Task 0: Fix M0 audit gaps
1. Add a `cross-build` step that lists exported `retro_*` symbols (`nm -D` on Linux, `llvm-nm` or `dumpbin` on other platforms) and fails the job unless all 25 symbols in 2.5 are present.
2. Pin every action by commit SHA.
3. Fix the `ci-report` table rendering and include every job.
4. Apply the `human-test` label to Issue #2 and fix its checklist formatting.
5. Add the missing `windows-latest` and `macos-latest` matrix entries for `build-test` and `abi`, and the native Windows MSVC build.
6. Mark `docs/platform-notes.md` as unverified.
7. Report any environment constraint in a `question` issue.

### 3.6–3.13 Milestone Exit Criteria Substitutions, Provisional Goldens, Audits, and Heartbeat
- **M1:** Tag coverage table in `docs/notes/tag-coverage.md`; 100k-iteration deterministic mutator test.
- **M2:** Provisional goldens in `crates/swfr-render/tests/golden-provisional/` + `GOLDENS.md`; open V2 `human-test` issue.
- **M3:** ABI tests for `retro_input_state_t` (`JOYPAD`, `ANALOG`, `MOUSE`, `POINTER`), keyboard callback, and live `swfr_scale` `SET_GEOMETRY`; open V3 and V4 `human-test` issues.
- **M4:** Benchmark flat `match` vs decoded `Vec<Op>` in `DECISIONS.md`; >= 150 generated ES3 conformance movies in `crates/swfr-avm1/tests/conformance.rs`; open V5 `human-test` issue.
- **M5:** PCM, ADPCM, MP3, resampler, and 300-frame marker-pulse stream sync test; open V6 `human-test` issue.
- **M6:** Full `cross.yml` matrix (including QEMU user-mode ABI test on `armv7`), `release.yml`, `bench` per-frame cost in `ci-report`, `README.md`, `docs/INSTALL.md`, `docs/KNOWN_LIMITATIONS.md`, open V7 `human-test` issue and `v1 engineering-complete report` issue.

3.14 End-to-end acceptance (added after the first run failed it)
The first autonomous run reported completion while the released core still played the M0 stub (color bars and a 440 Hz tone) for every file. Unit tests passed because no test drove a real movie through the finished path. This section overrides 3.1 and 3.10. The run is not complete, and no milestone is audited as complete, until all of the following hold. The status in STATUS.md is reset to not complete until then.
1. The stub is gone. The color-bar and tone generators exist only behind a cargo feature m0-stub, which is off by default. A CI step fails cross.yml and release.yml if that feature is enabled in any shipped build.
2. No silent fallback. If parsing or player construction fails, retro_load_game logs the error and returns false. It never substitutes a placeholder frame or tone.
3. Real dependency chain. A CI step runs cargo tree -p swfr-libretro and fails unless it lists swfr-swf, swfr-scene, swfr-render, swfr-avm1 and swfr-audio.
4. End-to-end ABI tests. In swfr-libretro/tests/e2e.rs, using movies from the swfr-testkit builder and only the exported retro_* symbols:
    ◦ A 1-frame movie with a red 100x80 rectangle centered on a blue stage: after load and one run, the center pixel is red, the corner is blue, and retro_get_system_av_info reports the stage size and the header frame rate.
    ◦ A 10-frame movie where a square moves 10 pixels per frame: successive frames differ and the square is at the expected position in each.
    ◦ A movie with a PCM sound event: the audio buffer contains the sound's exact samples. A silent movie yields all-zero samples. Neither is a 440 Hz sine.
    ◦ An AS2 movie calling trace('hello'): the log callback receives a line containing hello.
    ◦ A button movie with scripted pointer input: the click changes the frame.
    ◦ A malformed file: retro_load_game returns false.
5. Real files. The corpus-real job loads every manifest file through the exported ABI and runs 120 frames. It fails if any output matches the M0 stub's frame hash.
6. Honest benchmarks. bench runs the 10-frame movie and the largest real file through the full path. A benchmark of the stub does not count.
7. Evidence rule. Every audit item must name the test that proves it and the CI run. A criterion with no test is unmet.
After the gates pass, re-audit M1 to M6 against the new end-to-end tests. Fix whatever fails, then rewrite docs/audits/ and the final report.