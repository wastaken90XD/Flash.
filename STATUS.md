# Project Status (`STATUS.md`)

- **Current Milestone:** `M6` — Packaging, cross-platform builds, and v1 release engineering
- **Overall v1 Status:** `engineering-complete (awaiting human verification V1–V7)`
- **Active Branch / PR:** `arena/a4bfe02a-flash` ([PR #1](https://github.com/wastaken90XD/Flash./pull/1))

## Milestone Summary

| Milestone | Scope | Status | Self-Audit | Human-Test Issue |
| :--- | :--- | :--- | :--- | :--- |
| **M0 + Task 0** | Bootstrap, libretro C ABI, stub player, 25 symbol check, SHA-pinned CI | `engineering-complete (awaiting human verification)` | [`docs/audits/m0.md`](docs/audits/m0.md) | V1: [Issue #2](https://github.com/wastaken90XD/Flash./issues/2) |
| **M1** | `swfr-reader`, `swfr-swf` (`FWS`/`CWS`/`ZWS`), SWF v19 tags, `SwfBuilder`, 100k `Mutator` | `engineering-complete` | [`docs/audits/m1.md`](docs/audits/m1.md) | Automated (`tag_roundtrip.rs`) |
| **M2** | `swfr-scene` display list & shape flattener, `swfr-render` (`tiny-skia`), 64-entry LRU, dirty rects | `engineering-complete (awaiting human verification)` | [`docs/audits/m2.md`](docs/audits/m2.md) | V2 (`human-test`) |
| **M3** | Timeline engine, `DefineSprite`, buttons & vector hit-test, input modes, `DefineText*` / `DefineEditText` | `engineering-complete (awaiting human verification)` | [`docs/audits/m3.md`](docs/audits/m3.md) | V3 & V4 (`human-test`) |
| **M4** | `swfr-avm1` (`Value`, ES3 coercions, generational `Heap` + GC, decoded `Vec<Op>`, 160 conformance movies) | `engineering-complete (awaiting human verification)` | [`docs/audits/m4.md`](docs/audits/m4.md) | V5 (`human-test`) |
| **M5** | `swfr-audio` (PCM, SWF ADPCM 2/3/4/5-bit, MP3 via `symphonia`, 44.1 kHz resampler, 300-frame sync test) | `engineering-complete (awaiting human verification)` | [`docs/audits/m5.md`](docs/audits/m5.md) | V6 (`human-test`) |
| **M6** | Full `cross.yml` matrix (Linux/Android/Windows/macOS/iOS + QEMU `armv7`), `release.yml`, `bench`, docs | `engineering-complete (awaiting human verification)` | [`docs/audits/m6.md`](docs/audits/m6.md) | V7 (`human-test`) |
| **M7** | ActionScript 3 (`swfr-avm2`) | Deferred post-v1 (TSD 3.13) | — | — |
