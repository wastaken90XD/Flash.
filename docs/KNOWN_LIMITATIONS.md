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

- **Resolution Cap (`MAX_STAGE_PIXELS`):** Internal rendering scale (`swfr_scale`) is clamped so `width * height <= 4,194_304` pixels (`2048x2048`) to bound memory and frame times on Reference B devices.
- **Dynamic Text (`DefineEditText`):** When a `DefineEditText` field references an embedded `DefineFont*` with a populated `codes` table, glyph outlines are rendered directly. When a device font (`_sans`, `_serif`, `_typewriter`) or outline-free font is used, `swfr-render` falls back to built-in vector stroke glyphs.
- **`DefineScalingGrid` (Tag 78, Nine-Slice Scaling — TSD 4.6 item 5):** `DefineScalingGrid` tags are parsed and round-tripped by `swfr-swf`, but nine-slice slice-partitioned scaling of child shapes is not applied during rendering in v1 (uniform matrix scaling is used).
- **Blend Modes Including `erase` (`PlaceObject3` — TSD 4.6 item 6):** `PlaceObject3` blend-mode bytes are parsed cleanly; v1 software rendering composites with standard source-over alpha blending and `CXFORMWITHALPHA` color transforms rather than multi-layer `erase`/`multiply`/`screen`/`overlay` backdrop isolation.
- **Bitmap Smoothing Version Gate (TSD 4.6 item 4):** Per Flash Player behavior, bitmap fill smoothing (`0x40` repeating smoothed, `0x41` clipped smoothed) is enabled only when `swf_version >= 8`; SWF 1–7 files always use nearest-neighbor (`FilterQuality::Nearest`) sampling.

## 3. Audio, Scripting & GC Notes (TSD 4.6 items 3, 9, 11, 13)

- **Nellymoser & Speex Speech Codecs (TSD 4.6 item 3):** `DefineSound` and `SoundStreamHead` tags using Nellymoser (`SoundFormat::Nellymoser16k`, `Nellymoser8k`, `Nellymoser`) or Speex (`SoundFormat::Speex`) log a warning once via `retro_log_printf_t` and output silence (`0i16`), while PCM, SWF IMA ADPCM (2/3/4/5-bit), and MP3 (`symphonia`) are decoded and resampled to 44.1 kHz stereo.
- **Mixed AVM1/AVM2 Files & AS3 Elastic Racetrack Frame Loop (TSD 4.6 items 9 & 11):** If an SWF file sets `FileAttributes.actionscript3 = true` or contains `DoABC` (tags 72/82), `swfr` logs a diagnostic notice once on load/frame 1 and continues executing timeline tags and any AVM1 `DoAction`/`CLIPACTIONRECORD` scripts. The AS3 "elastic racetrack" frame loop and AVM2 bytecode execution are deferred to Milestone M7.
- **Garbage Collector Pauses on Weak CPUs (TSD 4.6 item 13):** `swfr-avm1` uses a deterministic generational arena mark-and-sweep collector (`Heap::collect_garbage`) triggered only after >256 object allocations since the previous collection, avoiding reference-counting cycle leaks while keeping per-frame collection work sub-millisecond on Reference B ARMv7 devices.

## 4. Libretro Frontend Integration Pitfalls (`ruffle_libretro` Issue Review — TSD 4.7 item 7)

Based on reviewing public issues and commit notes from `JesseTG/ruffle_libretro` (specifically Issue #1, `Multiple Build & Run Issues`):

1. **VFS Interface Not Required (`RETRO_ENVIRONMENT_GET_VFS_INTERFACE`):** Frontends such as RetroArch 1.9.x may return a null `vfs_interface_info.interface` pointer when a core demands VFS v3, causing cores that `unwrap()` VFS to panic on load. `swfr` sets `need_fullpath = false` and reads the `.swf` buffer directly from `retro_game_info.data` without requiring the libretro VFS interface.
2. **`.swf` File Extension Association Requires `swfr_libretro.info`:** Setting `valid_extensions = "swf"` in `retro_get_system_info` alone is not enough for RetroArch's "Load Content" browser to associate `.swf` files with the core; `swfr_libretro.info` must be placed in RetroArch's `info/` (`core_info_dir`) directory (documented in `docs/INSTALL.md` and packaged in every release zip).
3. **Zero `bindgen` / `libclang` Build Dependency:** `ruffle_libretro` failed to compile on hosts lacking `clang` (`stddef.h` not found in `rust-libretro-sys`). `swfr` uses a self-contained, test-verified `libretro_sys.rs` (`all_ffi_defines_match_libretro_h`) with zero `bindgen` or `libclang` dependency.
4. **Redundant `mouseMove` Suppression & Keyboard vs Joypad Input:** `Stage::update_pointer` only fires `onClipEvent(mouseMove)` when the stage-space pointer coordinates actually change from the previous poll, and `swfr` supports both `RETRO_ENVIRONMENT_SET_KEYBOARD_CALLBACK` (`retro_keyboard_event_t`) and `swfr_pad_mode = cursor|keys` joypad mapping for handheld devices without a physical keyboard.

