# SWF Specification v19 Tag Coverage Matrix (`docs/notes/tag-coverage.md`)

- **Sources:**
  - `docs/reference/swf.ksy` (`tag_type` enumeration)
  - Adobe SWF File Format Specification Version 19 (Chapters 2–11)
  - TSD 2.3 (Scope & Non-goals for v1) and TSD 3.1.5 / 3.6 (M1 Autonomous Requirements)

Every tag code defined in SWF specification v19 either has a programmatic `SwfBuilder` round-trip test in `crates/swfr-testkit/tests/tag_roundtrip.rs` or is preserved as `Tag::Unknown { code, raw }` with its v1 non-goal reason documented below.

| Tag Code | SWF Tag Name | Status in `swfr` v1 | Round-Trip Test / Unsupported Reason |
| :--- | :--- | :--- | :--- |
| `0` | `End` | **Supported (`Tag::End`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `1` | `ShowFrame` | **Supported (`Tag::ShowFrame`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `2` | `DefineShape` | **Supported (`Tag::DefineShape`, v1)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `4` | `PlaceObject` | **Supported (`Tag::PlaceObject`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `5` | `RemoveObject` | **Supported (`Tag::RemoveObject`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `6` | `DefineBits` | **Supported (`Tag::DefineBits`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `7` | `DefineButton` | **Supported (`Tag::DefineButton`, v1)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `8` | `JPEGTables` | **Supported (`Tag::JPEGTables`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `9` | `SetBackgroundColor` | **Supported (`Tag::SetBackgroundColor`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `10` | `DefineFont` | **Supported (`Tag::DefineFont`, v1)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `11` | `DefineText` | **Supported (`Tag::DefineText`, v1)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `12` | `DoAction` | **Supported (`Tag::DoAction`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `13` | `DefineFontInfo` | **Supported (`Tag::DefineFontInfo`, v1)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `14` | `DefineSound` | **Supported (`Tag::DefineSound`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `15` | `StartSound` | **Supported (`Tag::StartSound`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `17` | `DefineButtonSound` | **Supported (`Tag::DefineButtonSound`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `18` | `SoundStreamHead` | **Supported (`Tag::SoundStreamHead`, v1)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `19` | `SoundStreamBlock` | **Supported (`Tag::SoundStreamBlock`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `20` | `DefineBitsLossless` | **Supported (`Tag::DefineBitsLossless`, v1)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `21` | `DefineBitsJPEG2` | **Supported (`Tag::DefineBitsJPEG2`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `22` | `DefineShape2` | **Supported (`Tag::DefineShape`, v2)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `23` | `DefineButtonCxform` | **Supported (`Tag::DefineButtonCxform`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `24` | `Protect` | **Supported (`Tag::Protect`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `26` | `PlaceObject2` | **Supported (`Tag::PlaceObject2`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `28` | `RemoveObject2` | **Supported (`Tag::RemoveObject2`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `32` | `DefineShape3` | **Supported (`Tag::DefineShape`, v3)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `33` | `DefineText2` | **Supported (`Tag::DefineText`, v2)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `34` | `DefineButton2` | **Supported (`Tag::DefineButton`, v2)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `35` | `DefineBitsJPEG3` | **Supported (`Tag::DefineBitsJPEG3`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `36` | `DefineBitsLossless2` | **Supported (`Tag::DefineBitsLossless`, v2)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `37` | `DefineEditText` | **Supported (`Tag::DefineEditText`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `39` | `DefineSprite` | **Supported (`Tag::DefineSprite`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `41` | `ProductInfo` | `Tag::Unknown { code: 41, .. }` | Undocumented Flash authoring metadata tag; non-rendering, preserved as `Tag::Unknown`. |
| `43` | `FrameLabel` | **Supported (`Tag::FrameLabel`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `45` | `SoundStreamHead2` | **Supported (`Tag::SoundStreamHead`, v2)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `46` | `DefineMorphShape` | **Supported (`Tag::DefineMorphShape`, v1)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `48` | `DefineFont2` | **Supported (`Tag::DefineFont`, v2)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `56` | `ExportAssets` | **Supported (`Tag::ExportAssets`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `57` | `ImportAssets` | **Supported (`Tag::ImportAssets`, v1)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `58` | `EnableDebugger` | **Supported (`Tag::EnableDebugger`, v1)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `59` | `DoInitAction` | **Supported (`Tag::DoInitAction`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `60` | `DefineVideoStream` | `Tag::Unknown { code: 60, .. }` | Unsupported in v1 per TSD 2.3 non-goals ("video codecs"). |
| `61` | `VideoFrame` | `Tag::Unknown { code: 61, .. }` | Unsupported in v1 per TSD 2.3 non-goals ("video codecs"). |
| `62` | `DefineFontInfo2` | **Supported (`Tag::DefineFontInfo`, v2)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `63` | `DebugID` | `Tag::Unknown { code: 63, .. }` | Remote debugger UUID metadata; non-goal in v1, preserved as `Tag::Unknown`. |
| `64` | `EnableDebugger2` | **Supported (`Tag::EnableDebugger`, v2)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `65` | `ScriptLimits` | **Supported (`Tag::ScriptLimits`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `66` | `SetTabIndex` | **Supported (`Tag::SetTabIndex`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `69` | `FileAttributes` | **Supported (`Tag::FileAttributes`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `70` | `PlaceObject3` | **Supported (`Tag::PlaceObject3`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `71` | `ImportAssets2` | **Supported (`Tag::ImportAssets`, v2)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `72` | `DoABC` (lazy) | `Tag::Unknown { code: 72, .. }` | AVM2 / ActionScript 3 bytecode (deferred to M7 per TSD 2.9 & 3.13). |
| `73` | `DefineFontAlignZones` | **Supported (`Tag::DefineFontAlignZones`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `74` | `CSMTextSettings` | **Supported (`Tag::CSMTextSettings`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `75` | `DefineFont3` | **Supported (`Tag::DefineFont`, v3)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `76` | `SymbolClass` | **Supported (`Tag::SymbolClass`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `77` | `Metadata` | **Supported (`Tag::Metadata`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `78` | `DefineScalingGrid` | **Supported (`Tag::DefineScalingGrid`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `82` | `DoABC` | `Tag::Unknown { code: 82, .. }` | AVM2 / ActionScript 3 bytecode (deferred to M7 per TSD 2.9 & 3.13). |
| `83` | `DefineShape4` | **Supported (`Tag::DefineShape`, v4)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `84` | `DefineMorphShape2` | **Supported (`Tag::DefineMorphShape`, v2)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `86` | `DefineSceneAndFrameLabelData` | **Supported (`Tag::DefineSceneAndFrameLabelData`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `87` | `DefineBinaryData` | `Tag::Unknown { code: 87, .. }` | AVM2 `ByteArray` asset container (deferred to M7 per TSD 2.9 & 3.13). |
| `88` | `DefineFontName` | **Supported (`Tag::DefineFontName`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `89` | `StartSound2` | **Supported (`Tag::StartSound2`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `90` | `DefineBitsJPEG4` | **Supported (`Tag::DefineBitsJPEG4`)** | `tag_roundtrip::roundtrip_all_supported_swf_v19_tags` |
| `91` | `DefineFont4` | `Tag::Unknown { code: 91, .. }` | Flash Text Engine (FTE) CFF font container for AS3/AVM2 (deferred to M7). |
| `93` | `EnableTelemetry` | `Tag::Unknown { code: 93, .. }` | Adobe Scout profiler telemetry tag; non-goal in v1, preserved as `Tag::Unknown`. |

---

## 2. TSD 4.6 Hard-Areas Checklist Coverage & Test Matrix (`TSD 4.7.2`)

Every item from **TSD 4.6 (`Hard-areas checklist`)** is mapped below to its proving test(s) (for items in M1–M5) or its explicit documentation entry in `docs/KNOWN_LIMITATIONS.md`.

| # | TSD 4.6 Hard Area | Scope / Milestone | Proving Test (`#[test]`) or `docs/KNOWN_LIMITATIONS.md` Entry |
| :--- | :--- | :--- | :--- |
| 1 | SWF 1 and 2 files, with `PlaceObject` (tag 4) and `RemoveObject` (tag 5) instead of the later tags | M1–M3 | `e2e_hard_area_swf1_swf2_morph_mixed_fills_nellymoser_speex_and_skipped_frames_via_abi`, `roundtrip_all_supported_swf_v19_tags` |
| 2 | Morph shapes (`DefineMorphShape`/`2`), including solid, gradient, bitmap, and mixed fill types | M1–M2 | `e2e_hard_area_swf1_swf2_morph_mixed_fills_nellymoser_speex_and_skipped_frames_via_abi`, `e2e_m2_gradients_morphs_and_sprites_via_abi`, `render_m2_lossless_bitmap_clip_mask_and_morph` |
| 3 | ADPCM at every bit depth (2/3/4/5-bit); Nellymoser and Speex out of scope: log warning and play silence | M5 | `adpcm_2_3_4_5_bit_decodes_cleanly`, `e2e_hard_area_swf1_swf2_morph_mixed_fills_nellymoser_speex_and_skipped_frames_via_abi`, and `docs/KNOWN_LIMITATIONS.md` |
| 4 | Bitmap smoothing (`0x40`/`0x41` bilinear filtering) exists only from SWF 8 (`swf_version >= 8`) | M2 | `e2e_hard_area_swf1_swf2_morph_mixed_fills_nellymoser_speex_and_skipped_frames_via_abi` |
| 5 | `DefineScalingGrid` (tag 78, nine-slice scaling) | M1 / v1 limitation | Parsed and round-tripped in `roundtrip_all_supported_swf_v19_tags`; nine-slice layout scaling documented in `docs/KNOWN_LIMITATIONS.md` |
| 6 | Blend modes (`PlaceObject3`), including `erase` | M2 / v1 limitation | Parsed in `PlaceObject3`; non-normal blend modes (including `erase`) documented in `docs/KNOWN_LIMITATIONS.md` |
| 7 | Script order: root timeline vs child clips, playhead changes made by a script (`gotoAndStop`/`gotoAndPlay`), clip constructors before first use, and skipped frames must never run their frame scripts | M3–M4 | `e2e_hard_area_event_order_load_unload_button_mouse_key_via_abi`, `e2e_hard_area_swf1_swf2_morph_mixed_fills_nellymoser_speex_and_skipped_frames_via_abi`, `e2e_real_corpus_manifest_runs_120_frames_without_m0_stub` |
| 8 | AVM1 details: local variable scoping, prototype and class chains, cloning movie clips (`CloneSprite` / `duplicateMovieClip`), `goto` by label, and tracing GC to avoid reference-counting cycles | M4 | `avm1_es3_conformance_160_generated_movies`, `avm1_timers_key_and_gc_work_deterministically`, `e2e_hard_area_swf1_swf2_morph_mixed_fills_nellymoser_speex_and_skipped_frames_via_abi`, `e2e_real_corpus_manifest_runs_120_frames_without_m0_stub` |
| 9 | Mixed AVM1 and AVM2 files and obfuscated AVM1 bytecode (out-of-bounds jumps clamped cleanly) | M4–M5 | `e2e_m5_avm2_diagnostic_notice_and_reset_via_abi`, `avm1_watchdog_aborts_infinite_loop_cleanly`, `mutator_100k_iterations_never_panics`, and `docs/KNOWN_LIMITATIONS.md` |
| 10 | AS1/AS2 event order: button, mouse, and key handlers run immediately when their event happens; `load` runs in the first frame the clip is on stage after parent timeline's code and before the clip's own code and before rendering; `unload` runs in the first frame the clip is gone before parent timeline's code | M3–M4 | `e2e_hard_area_event_order_load_unload_button_mouse_key_via_abi`, `e2e_real_corpus_manifest_runs_120_frames_without_m0_stub` (`ruffle-clip-events.swf`), and `DECISIONS.md` (`D-009`) |
| 11 | AS3-era frame loop ("elastic racetrack": events -> `enterFrame` -> render) | M7 (out of v1 scope) | Documented in `docs/KNOWN_LIMITATIONS.md` and `docs/notes/prior-art.md` |
| 12 | Streaming sound sync (`SoundStreamHead`/`2` + `SoundStreamBlock`) and ActionScript `Sound` object | M4–M5 | `stream_sync_300_frames_marker_pulse_within_one_frame`, `e2e_m4_savestate_roundtrip_and_streaming_audio_via_abi`, `e2e_real_corpus_manifest_runs_120_frames_without_m0_stub` (`ruffle-sound.swf`) |
| 13 | Collector pauses on weak CPUs (bounded generational arena mark-and-sweep GC) | M4 / M6 | `avm1_timers_key_and_gc_work_deterministically`, `harness_formats_full_movie_info_and_runs_honest_bench`, and `docs/KNOWN_LIMITATIONS.md` |
| 14 | Version-gated behavior: string encoding (SWF <= 5 Latin-1/Shift-JIS fallback vs SWF >= 6 UTF-8) and case sensitivity / `undefined` coercion (SWF <= 6 vs SWF >= 7) | M1 / M4 | `rect_matrix_cxform_and_string_parsing`, `avm1_es3_conformance_160_generated_movies` |

