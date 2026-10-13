# Autonomous Engineering Worklog (`WORKLOG.md`)

One entry per completed task/milestone (TSD 3.10):

- `2026-10-13T00:05:00Z` | `M0` | PR `#1` | `651d9ef` | Bootstrapped 11-crate workspace, `swfr-libretro` C ABI (25 `retro_*` functions with `catch_unwind`), stub `Player`, `header_check` & `abi` tests, and V1 `human-test` Issue `#2`.
- `2026-10-13T00:25:00Z` | `Task-0` | PR `#1` | `HEAD` | Fixed M0 audit gaps: pinned all actions by SHA, added 25-symbol `nm -D`/`objdump` check in `cross-build`, added native `windows-latest` & `macos-latest` CI jobs, marked `docs/platform-notes.md` unverified, and opened `[question]` Issue `#3`.
- `2026-10-13T00:35:00Z` | `M1` | PR `#1` | `HEAD` | Implemented `docs/notes/swf-format.md`, `docs/notes/tag-coverage.md`, `swfr-reader`, `swfr-swf` (`FWS`/`CWS`/`ZWS` + all v19 tags), and `swfr-testkit` (`SwfBuilder`, tag round-trip tests, and 100k-iteration `Mutator` test).
- `2026-10-13T00:45:00Z` | `M2` | PR `#1` | `HEAD` | Implemented `swfr-scene` character dictionary, display list, dual-fill shape flattener, morph interpolation, and `swfr-render` (`tiny-skia`, 64-entry LRU cache, dirty rects, clipping masks, and provisional goldens + `GOLDENS.md`).
- `2026-10-13T00:50:00Z` | `M3` | PR `#1` | `HEAD` | Implemented multi-frame root and nested `DefineSprite` timelines, `DefineButton2` vector hit-testing, input modes (`docs/keymap.md`), static/dynamic text rendering, and live `swfr_scale` `SET_GEOMETRY` updates.
- `2026-10-13T00:55:00Z` | `M4` | PR `#1` | `HEAD` | Implemented `swfr-avm1` (`Value`, ES3 coercions, generational arena `Heap` + mark-and-sweep GC, pre-decoded `Vec<Op>` interpreter, built-ins, deterministic timers, watchdog, and 160 generated ES3 conformance movies).
- `2026-10-13T01:00:00Z` | `M5` | PR `#1` | `HEAD` | Implemented `swfr-audio` (PCM, SWF IMA ADPCM 2/3/4/5-bit, MP3 via `symphonia`, linear resampler to 44.1 kHz stereo, 32-voice mixer, and 300-frame marker-pulse stream sync test) and wired `swfr-player` + `swfr-harness`.
- `2026-10-13T01:05:00Z` | `M6` | PR `#1` | `HEAD` | Completed full `cross.yml` target matrix (Linux, Android API 21, Windows GNU/MSVC, macOS, iOS + QEMU `armv7` ABI test), `release.yml`, `bench` per-frame cost report, `README.md`, `docs/INSTALL.md`, `docs/KNOWN_LIMITATIONS.md`, and milestone audits `docs/audits/m0.md`..`m6.md`.
