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
