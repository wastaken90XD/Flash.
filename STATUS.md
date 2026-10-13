# `swfr` Milestone Status

| Milestone | Status | Summary |
| :--- | :--- | :--- |
| **V0** | `COMPLETE` | Frontend baseline & repository bootstrap (`docs/platform-notes.md`). |
| **M0** | `COMPLETE (awaiting V1)` | `swfr-libretro` C ABI skeleton, `libretro.h` constant verification, 320x240 color bars + 440 Hz tone stub player, mock frontend ABI tests, `swfr_libretro.info`, Linux & Windows `cross.yml` builds, `swfr-harness` `AvInfo` CLI. |
| **M1** | `PENDING` | `swfr-reader` & `swfr-swf` container decompression (`FWS`/`CWS`/`ZWS`), header & core control/shape tags. |
| **M2** | `PENDING` | `swfr-scene` display list & timeline, `swfr-render` software vector/bitmap rasterizer (`tiny-skia`, `png`, `zune-jpeg`). |
| **M3** | `PENDING` | Embedded fonts (`DefineFont*`), device-font fallback (`fontdue` + `Arimo.ttf`), static & dynamic text (`DefineText*`, `DefineEditText`). |
| **M4** | `PENDING` | `swfr-avm1` ActionScript 1 & 2 bytecode interpreter, movie clip timeline control, buttons & input routing. |
| **M5** | `PENDING` | `swfr-audio` PCM, ADPCM, and MP3 (`symphonia`) decoding, event & streaming sound mixer. |
| **M6** | `PENDING` | Determinism, performance budgets, release packaging, and real-world SWF corpus validation. |
| **M7** | `PENDING` | `swfr-avm2` ActionScript 3 (`DoABC`) virtual machine foundation. |
