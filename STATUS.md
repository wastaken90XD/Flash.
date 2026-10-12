# `swfr` Milestone Status

| Milestone | Status | Summary |
| :--- | :--- | :--- |
| **V0** | `READY` | Frontend baseline & repository bootstrap (`docs/platform-notes.md`). |
| **M0** | `PENDING` | `swfr-libretro` C ABI skeleton, header layout verification, test pattern core & headless harness. |
| **M1** | `PENDING` | `swfr-reader` & `swfr-swf` container decompression (`FWS`/`CWS`/`ZWS`), header & core control/shape tags. |
| **M2** | `PENDING` | `swfr-scene` display list & timeline, `swfr-render` software vector/bitmap rasterizer (`tiny-skia`, `png`, `zune-jpeg`). |
| **M3** | `PENDING` | Embedded fonts (`DefineFont*`), device-font fallback (`fontdue` + `Arimo.ttf`), static & dynamic text (`DefineText*`, `DefineEditText`). |
| **M4** | `PENDING` | `swfr-avm1` ActionScript 1 & 2 bytecode interpreter, movie clip timeline control, buttons & input routing. |
| **M5** | `PENDING` | `swfr-audio` PCM, ADPCM, and MP3 (`symphonia`) decoding, event & streaming sound mixer. |
| **M6** | `PENDING` | Save states (`retro_serialize`), determinism, performance budgets, and real-world SWF corpus validation. |
| **M7** | `PENDING` | `swfr-avm2` ActionScript 3 (`DoABC`) virtual machine foundation. |
