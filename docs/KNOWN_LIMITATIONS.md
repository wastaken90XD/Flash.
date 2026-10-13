# Known Limitations in `swfr` v1 (`docs/KNOWN_LIMITATIONS.md`)

This document lists intentional v1 non-goals (per TSD 2.3 and `docs/notes/tag-coverage.md`) and known behavioral limitations.

## 1. Intentional v1 Scope Non-Goals (TSD 2.3)

- **ActionScript 3 / AVM2 (`DoABC`, `DefineBinaryData`, `DefineFont4`):** `swfr` v1 implements ActionScript 1 & 2 (`swfr-avm1`). SWF files that require AVM2 (`FileAttributes.actionscript3 == true` or `DoABC` tags 72/82) parse and render their timeline vector/bitmap content, while AVM2 bytecode execution is deferred to Milestone M7 (`crates/swfr-avm2`).
- **Video Codecs (`DefineVideoStream`, `VideoFrame`):** Embedded Sorenson Spark, On2 VP6, and H.264 video streams (tags 60 and 61) are preserved as `Tag::Unknown` and not decoded in v1.
- **Voice Speech Codecs (`Nellymoser`, `Speex`):** Event and streaming audio support uncompressed PCM, SWF IMA ADPCM (2/3/4/5-bit), and MP3 (`symphonia`). Nellymoser Asao and Speex voice codecs are muted in v1.
- **Pixel Shaders, Blend Modes & Bitmap Filters:** `PlaceObject3` surface filters (DropShadow, Blur, Glow, Bevel) and Pixel Bender shaders are parsed and skipped during software rendering; standard alpha blending and `CXFORMWITHALPHA` color transforms are supported.
- **Networking & External Loading (`GetURL`, `LoadVars`, `XMLSocket`):** `swfr` runs completely offline inside the libretro sandbox (`need_fullpath = false`) and does not make outbound HTTP or socket connections.
- **Libretro Save States (`retro_serialize`):** `retro_serialize_size()` returns `0` in v1 (TSD 2.2, 2.5).

## 2. Rendering & Text Notes

- **Resolution Cap (`MAX_STAGE_PIXELS`):** Internal rendering scale (`swfr_scale`) is clamped so `width * height <= 4,194,304` pixels (`2048x2048`) to bound memory and frame times on Reference B devices.
- **Dynamic Text (`DefineEditText`):** When a `DefineEditText` field references an embedded `DefineFont*` with a populated `codes` table, glyph outlines are rendered directly. When a device font (`_sans`, `_serif`, `_typewriter`) or outline-free font is used, `swfr-render` falls back to built-in vector stroke glyphs.
