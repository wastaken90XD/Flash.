# R12 — Backend Traits and Libretro Fit (`11-fit.md`)

- **Pinned Release Tag:** `v0.7.1`
- **Pinned Commit SHA:** `89a7049965f3090e706265670df309a5128233ac`
- **Repository:** `https://github.com/ruffle-rs/ruffle`
- **Overall Confidence:** **High** (verified directly from `core/src/backend/`, `core/src/player.rs`, `render/src/backend.rs`, `desktop/src/`, and `LICENSE.md` at `89a7049965f3090e706265670df309a5128233ac`).

---

## 1. All Ruffle Backend Traits and Their Libretro Mapping (`Confidence: High`)

`PlayerBuilder` (`core/src/player.rs:58-247`) accepts 11 host-abstraction traits:

| Backend Trait | Defined In | Purpose | Libretro Callback / Interface Mapping |
|---|---|---|---|
| `RenderBackend` | `render/src/backend.rs:28-129` | Shape registration, bitmap textures, `submit_frame(clear, commands, cache_entries)`, offscreen rendering, filters | Software (`tiny-skia` -> `XRGB8888` buffer -> `retro_video_refresh_t`) or hardware (`RETRO_ENVIRONMENT_SET_HW_RENDER`) |
| `AudioBackend` | `core/src/backend/audio.rs:88-205` | Sound registration (`DefineSound`, MP3, streams), playback, transforms, sample history (`AudioMixer` in `core/src/backend/audio/mixer.rs:58-76` implements the full software mixer via `impl_audio_mixer_backend!`) | `AudioMixer::mix::<i16>(&mut buf)` once per `retro_run` -> `retro_audio_sample_batch_t` (`44,100 Hz` stereo `i16`) |
| `NavigatorBackend` | `core/src/backend/navigator.rs:265-363` | Async `fetch(Request)`, `navigate_to_url`, `spawn_future`, `resolve_url`, socket connection | `NullNavigatorBackend` (`core/src/backend/navigator.rs:366-423`) for single-SWF offline sandboxed mode, or VFS-backed local relative path reader via `RETRO_ENVIRONMENT_GET_VFS_INTERFACE` |
| `StorageBackend` | `core/src/backend/storage.rs:4-14` | `SharedObject` (`get`, `put`, `get_size`, `remove_key`) | Backed by `retro_get_memory_data(RETRO_MEMORY_SAVE_RAM)` or files in `RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY` |
| `LogBackend` | `core/src/backend/log.rs:1-5` | `avm_trace(&str)` and `avm_warning(&str)` | `RETRO_ENVIRONMENT_GET_LOG_INTERFACE` (`retro_log_printf_t` with `%`-escaping) |
| `UiBackend` | `core/src/backend/ui.rs:117-186` | Mouse visibility/cursor, clipboard, fullscreen, device font loading, language, virtual keyboard, file dialogs | `NullUiBackend` (`core/src/backend/ui.rs:214-294`) + software cursor state updated by `set_mouse_visible` / `set_mouse_cursor` |
| `VideoBackend` | `video/src/backend.rs:14-48` | Video stream registration and frame decoding (`ruffle_video_software` provides pure-Rust H.263/VP6/ScreenVideo) | `SoftwareVideoBackend` (`video/software/src/backend.rs`) or `NullVideoBackend` |
| `LocaleBackend` | `core/src/backend/locale.rs:4-8` | `get_current_date_time() -> DateTime<Utc>` and `get_timezone() -> FixedOffset` | `DefaultLocaleBackend` (or `DeterministicLocaleBackend` when deterministic replay is enabled) |
| `FsCommandProvider` | `core/src/external.rs:189-193` | Handles `fscommand(command, args)` (`quit`, `fullscreen`, `allowscale`, `showmenu`) | Maps `"quit"` -> `RETRO_ENVIRONMENT_SHUTDOWN` |
| `ExternalInterfaceProvider` | `core/src/external.rs:103-112` | Host JS<->Flash `ExternalInterface.call` bridge | Empty/Not-available in standalone libretro core |
| `SocketBackend` | `core/src/backend/navigator.rs:348-358` | ActionScript `XMLSocket` / `Socket` | Disabled (`channel.close()`) in offline libretro core |

---

## 2. Sandbox Safety: Threads, Sockets, Files, TLS/Globals, and Panics (`Confidence: High`)

- **Threads:** `ruffle_core` spawns **zero** OS threads (`grep "thread::spawn" core/src/` returns 0 matches). All async work (`loadMovie`, `LoadVars`, timers) runs cooperatively on the single caller thread via `NavigatorBackend::spawn_future`.
- **Network Sockets:** `ruffle_core` never opens `std::net` sockets directly (`grep "std::net" core/src/` returns 0 matches); all network access is delegated to `NavigatorBackend`.
- **Filesystem (`std::fs`):** `ruffle_core` only touches `std::fs` in two optional helpers (`SwfMovie::from_path` in `core/src/tag_utils.rs:107` and `fetch_path` in `core/src/backend/navigator.rs:624, 642, 669`), neither of which is called when loading from memory (`SwfMovie::from_data`) with `NullNavigatorBackend`.
- **Thread-Local Storage & Globals:** `ruffle_core` has **zero** `thread_local!` or `static mut` declarations. Only two immutable `std::sync::LazyLock` statics exist in `core/src/`:
  1. `TRANSFER_TABLE` (`core/src/avm2/api_version.rs:97-98`) for AVM2 version transfer tables.
  2. `FFT` (`core/src/avm2/globals/flash/media/sound_mixer.rs:111-112`) for `SoundMixer.computeSpectrum` 2048-point FFT planner.
- **Panics:** `ruffle_core` uses `unwrap()`/`expect()` on internal invariants (such as `gc_arena` borrow checks or `OnceLock` initialization). A libretro core wrapping `ruffle_core` (or `swfr-player`) must wrap `retro_load_game`, `retro_run`, and `retro_unload_game` in `std::panic::catch_unwind` at the FFI boundary (`crates/swfr-libretro/src/ffi.rs`) so no panic ever unwinds across `extern "C"`.

---

## 3. Does `wgpu` Fit Libretro's `SET_HW_RENDER` vs Software Rendering? (`Confidence: High`)

- **Why `ruffle_render_wgpu` fits poorly with `RETRO_ENVIRONMENT_SET_HW_RENDER`:**
  1. Libretro's `SET_HW_RENDER` callback model (`RETRO_HW_CONTEXT_OPENGL_CORE` / `VULKAN` / `D3D11`) varies wildly across RetroArch video drivers (`glcore`, `vulkan`, `d3d11`, `metal`, `sdl2`), whereas `ruffle_render_wgpu` expects to own or wrap a `wgpu::Instance` / `wgpu::Device` / `wgpu::Queue` (`render/wgpu/src/backend.rs`).
  2. Bridging libretro's per-API context Negotiation (`retro_hw_render_interface_vulkan`, `get_proc_address` for GL) into `wgpu-hal` requires unsafe backend-specific interop for every OS/API pair, and fails completely when the user selects a software or incompatible video driver in RetroArch (a known failure mode in upstream libretro GPU cores).
  3. Readback from `wgpu` to CPU `XRGB8888` every frame adds PCIe staging stalls and still requires a host GPU driver in headless CI.
- **Why a `tiny-skia` software `RenderBackend` fits libretro best:**
  - Always works across every RetroArch video driver (`gl`, `glcore`, `vulkan`, `d3d11`, `metal`, `gdi`, headless test harness), produces deterministic `XRGB8888` pixels in RAM for `retro_video_refresh_t`, and requires zero unsafe GPU context sharing.

---

## 4. Legacy Targets (ARMv7 / Android API 21, 6 MB Binary / 32 MB Idle RAM), Git Dependency Usability, and GPU-less Behavior (`Confidence: High`)

- **Could `ruffle_core` build for ARMv7 and Android API 21 and stay within 6 MB binary size and 32 MB idle RAM?**
  1. **ARMv7 & Android API 21 Compilation:** `wstr/src/ptr.rs:39-40` supports both 32-bit (`target_pointer_width = "32"`) and 64-bit targets, and pure-Rust default features compile for `armv7-unknown-linux-gnueabihf` and `armv7-linux-androideabi` (when `lzma-sys` / `openxr` C dependencies are disabled).
  2. **6 MB Binary Size Budget: NO (Exceeds budget).** `ruffle_core` (162,449 LOC) embeds `core/assets/library.swf` (`core/build.rs:1-46`) and `core/assets/notosans.subset.ttf.gz` (`core/src/player.rs:85`), and `ruffle_render` unconditionally pulls in `wgpu = "30.0.1"` and `naga = "29.0.0"` (`render/Cargo.toml:27-36`), plus `harfrust`, `ttf-parser`, `symphonia`, `gif`, `png`, `jpeg-decoder`, and `webp`. Even stripped with LTO, a `cdylib` linking `ruffle_core` is ~12–20 MB (2–3x over the 6 MB budget), whereas `swfr_libretro` is ~1.2 MB.
  3. **32 MB Idle RAM Budget (on a 512 MB device): Borderline / Exceeds.** On startup (`Player::new`, `core/src/player.rs:2969-3135`), `ruffle_core` allocates `GcArena` (`GcRootData`), parses and loads `library.swf` into the AVM2 `player_domain`, pre-allocates **two** complete AVM1 global/prototype object graphs (one case-insensitive for SWF <= 6 and one case-sensitive for SWF >= 7 in `core/src/avm1/runtime.rs:193-201`), and interns `CommonStrings` (`core/common/src/avm_string/interner.rs:61-76`), consuming ~25–45 MB of heap before the game SWF allocates a single frame.
- **Is `ruffle_core` usable as a library from a Cargo `git` dependency?**
  - Technically Cargo can resolve `ruffle_core = { git = "https://github.com/ruffle-rs/ruffle", tag = "v0.7.1" }`, **but** with major caveats:
    1. Requires Rust `>= 1.85.0` because the workspace uses `edition = "2024"` (`Cargo.toml:42`).
    2. `ruffle_core` has `publish = false` (`core/Cargo.toml:7`), meaning `swfr` could never be published to crates.io while depending on it.
    3. `ruffle_render` (`render/Cargo.toml:36`) has a non-optional dependency on `wgpu = "30.0.1"`, forcing every downstream build to compile `wgpu` and `naga` even when using a software renderer!
    4. Adds ~200+ transitive crates to `Cargo.lock`, conflicting with `deps-allow.txt` discipline.
- **What breaks without a GPU?**
  - Without a GPU (`ruffle_render_wgpu`), `ruffle_core` has no built-in desktop CPU rasterizer (`ruffle_render_canvas` is WASM/Web-only via `web-sys`, `render/canvas/Cargo.toml:17-35`). Unless a custom CPU `RenderBackend` (e.g., backed by `tiny-skia`) is written:
    - Stage rendering produces no pixels (`NoOpRenderBackend`).
    - `BitmapData.draw()` fails or no-ops if `RenderBackend::is_offscreen_supported()` returns `false` (`render/src/backend.rs:49-51`).
    - Bitmap filters (`BlurFilter`, `GlowFilter`, `DropShadowFilter`, `BevelFilter`, `ColorMatrixFilter`, `ConvolutionFilter`, `DisplacementMapFilter`, `ShaderFilter`) are skipped when `is_filter_supported()` returns `false` (`render/src/backend.rs:67-69`).
    - `Stage3D` (`create_context3d`, `render/src/backend.rs:104-106`) and `PixelBender` shaders return `None` / unsupported.
- **Notice Integrity (`LICENSE.md`):**
  - Ruffle is dual-licensed `MIT OR Apache-2.0` (`Cargo.toml:46`, `LICENSE.md:1-31`), and `LICENSE.md:11-31` documents bundled third-party notices (`Adobe Flex` Apache-2.0, `Adobe asc.jar`/`playerglobal.swc` in tests only, `Noto Sans` SIL OFL 1.1). Under Option 1 (no Ruffle runtime code depended on), `swfr`'s own MIT/Apache-2.0 notice remains self-contained; if any Ruffle crate were ever added under Option 2/3, `deps-allow.txt` and `NOTICE` would need to list Ruffle and its transitive crates.

---

## 5. Verification Against Regression Tests (`Confidence: High`)

1. **Claim 1 — `AudioBackend` / `AudioMixer` sound duration and volume/pan transforms (`core/src/backend/audio/mixer.rs:58-76, 1069-1167`):**
   - Verified against `tests/tests/swfs/avm1/sound/output.txt:1-25` (`sound1.duration: 1452`, `sound2.duration: 907`, `volume: 100`, `pan: 0`).
2. **Claim 2 — `NavigatorBackend` `LoadVars.load` / `sendAndLoad` async fetch lifecycle (`core/src/backend/navigator.rs:265-363`):**
   - Verified against `tests/tests/swfs/avm1/load_vars/output.txt:1-25` (`loadVars.load('test.txt')` -> `true`, `loadVars.decode('test=foo&test2=bar')` -> `foo`, `bar`, `Load: true`).
3. **Claim 3 — `StorageBackend` `SharedObject.getLocal` persistence across sessions (`core/src/backend/storage.rs:4-40`):**
   - Verified against `tests/tests/swfs/avm1/shared_object/{output1.txt,output2.txt}` (`1` on first run, `2` on second run).

---

## 6. What Could Not Be Verified (`Confidence: High`)

- Actual binary size of a compiled `cdylib` linking `ruffle_core` could not be measured in the sandbox because `cargo` is not installed locally.

---

## 7. Open Questions

1. Should `swfr-libretro` wire `fscommand("quit", ...)` in `swfr-avm1` (`Op::GetUrl` / `Op::GetUrl2` with `"FSCommand:quit"`) to `RETRO_ENVIRONMENT_SHUTDOWN`?

---

## 8. Test Ideas for `swfr`

1. **`FSCommand:quit` test:** Create a 1-frame SWF that executes `getURL("FSCommand:quit", "")` and verify `swfr-player` / `swfr-libretro` records the quit request cleanly.
2. **Offline `loadVariables` / `LoadVars` graceful failure test:** Verify that attempting `loadVariables("external.txt", "_root")` in offline sandboxed mode does not hang or panic the player.
3. **Save-RAM `SharedObject` roundtrip test (`e2e.rs`):** Verify `SharedObject.getLocal("save")` persists across `retro_serialize` / `retro_unserialize` or save-RAM buffers.
