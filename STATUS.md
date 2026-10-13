# `swfr` Milestone & Autonomous Execution Status (`STATUS.md`)

**Overall Run Status:** `not complete` (Reopened under TSD 3.14 End-to-End Acceptance)
**Branch:** `arena/a4bfe02a-flash` ([PR #1](https://github.com/wastaken90XD/Flash./pull/1))

| Milestone | Scope | Implementation Status | CI / Verification Status | Self-Audit |
| :--- | :--- | :--- | :--- | :--- |
| **Task 0 (M0 Cleanup)** | 25-symbol `nm`/`objdump` gate, SHA-pinned actions, `ci-report` table, Issue #2 `human-test` label & checklist fix, `windows-latest`/`macos-latest` CI jobs, `docs/platform-notes.md` marked `UNVERIFIED` | `not complete` (pending 3.14 re-audit) | In progress | [`docs/audits/m0.md`](docs/audits/m0.md) |
| **M1** | `swfr-reader`, `swfr-swf` (`FWS`/`CWS`/`ZWS`, v19 tags), `swfr-testkit` (`SwfBuilder`, 100k-iteration `Mutator`, round-trip suite), `docs/notes/swf-format.md`, `docs/notes/tag-coverage.md` | `not complete` (pending 3.14 re-audit) | In progress | [`docs/audits/m1.md`](docs/audits/m1.md) |
| **M2** | `swfr-scene` (dictionary, display list, shape/morph flattener, lossless/JPEG/PNG bitmaps), `swfr-render` (`tiny-skia`, 64-entry LRU cache, dirty-rect skip, masks, `XRGB8888`/`RGB565`), `GOLDENS.md` | `not complete` (pending 3.14 re-audit) | In progress | [`docs/audits/m2.md`](docs/audits/m2.md) |
| **M3** | Multi-frame `Timeline`, nested `DefineSprite` clips, `DefineButton`/`DefineButton2` state machine & vector hit-testing, `DefineFont*`/`DefineText*`/`DefineEditText`, cursor reticle, `docs/keymap.md` | `not complete` (pending 3.14 re-audit) | In progress | [`docs/audits/m3.md`](docs/audits/m3.md) |
| **M4** | `swfr-avm1` (AS1/AS2 interpreter, generational `Heap`, incremental mark-and-sweep GC, `Vec<Op>` dispatch, built-in classes, deterministic timers, watchdog), 160-movie ES3 conformance suite | `not complete` (pending 3.14 re-audit) | In progress | [`docs/audits/m4.md`](docs/audits/m4.md) |
| **M5** | `swfr-audio` (PCM, SWF IMA ADPCM 2/3/4/5-bit, MP3 via `symphonia 0.5.5`, 44.1 kHz stereo resampler, 32-voice `AudioMixer`, 300-frame marker-pulse stream sync test) | `not complete` (pending 3.14 re-audit) | In progress | [`docs/audits/m5.md`](docs/audits/m5.md) |
| **M6** | 13-target `cross.yml`, `release.yml`, `swfr-libretro/tests/e2e.rs`, `corpus-real`, `swfr-harness bench` (real 10-frame movie + largest corpus file), `README.md`, `docs/INSTALL.md`, `docs/KNOWN_LIMITATIONS.md` | `not complete` (pending 3.14 re-audit) | In progress | [`docs/audits/m6.md`](docs/audits/m6.md) |
| **M7** | `swfr-avm2` (ActionScript 3 / AVM2) | Deferred (v2 per TSD) | Stub crate | N/A |
