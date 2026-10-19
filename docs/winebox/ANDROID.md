# Winebox on Android 5 (API 21+)

Winebox Android support is an NDK-built **libretro shared core**, not an Android APK. RetroArch owns the Android activity, display surface, audio device, controller polling, and touch overlay. The BoxedWine runtime is built for Android and embedded into `winebox_libretro.so`; its own SDL window and audio devices are not initialized.

## Build targets

The Android workflow builds against Android API 21 for:

- `armeabi-v7a` baseline (interpreter-only; upstream BoxedWine has no ARMv7 JIT)
- `armeabi-v7a` with NEON enabled
- `arm64-v8a` (upstream ARMv8 JIT enabled)

The build pins BoxedWine commit `6c70c03c6a611a0993ccc922a4f10e658af02f49`, applies `winebox/patches/boxedwine-libretro-hooks.patch` and the Android API 21 addon patch, builds SDL2 as a PIC static library, and links BoxedWine plus the libretro bridge into `winebox_libretro.so`. The Android linker emits 16 KB-aligned ELF load segments for newer Android devices while retaining API 21 as the minimum OS target.

The API 21 build matrix passed in GitHub Actions run `39184774948` for ARMv7 baseline, ARMv7 NEON, and ARM64. CI verifies the Android ABI, libretro exports, and ELF layout. It does **not** prove that RetroArch or Winebox runs on a physical Android 5 device; the first Cortex-A7 / Android 5 smoke test is still required. ARMv7 interpreter performance is also unmeasured.

## Required user-provided files

Winebox does not download Windows or Wine files at runtime. Place these files under RetroArch's system directory in a `winebox/` subdirectory:

- `TinyCore15Wine11.0.zip` (or choose another leaf name using the `winebox_root` core option)
- `FlashPlayer.exe` (or choose another leaf name using the `winebox_projector` core option)

The core's writable guest root, copied content when RetroArch supplies it in memory, and controller mapping profiles are stored in RetroArch's save directory under `winebox/`. On Android, use RetroArch's own system/save directory settings and copy the two prerequisite files there with the Android file picker/file manager before launching content.

The core accepts SWFs as frontend-provided bytes as well as accessible filesystem paths. Its libretro system info and content override both advertise `swf` with `need_fullpath = false`; the core caches the supplied bytes before returning from `retro_load_game()`. This supports Android frontends that grant access to a selected document but do not expose a normal readable path to the core.

On Android, do not try to copy files into RetroArch's app-private Core Info directory with a file manager; modern Android blocks that without elevated access. The current core registers `swf` and `need_fullpath = false` through libretro at runtime, so a separate `.info` file is not required for content browsing once this updated core is loaded. The artifact still includes the `.info` file for frontends that use external core metadata. After updating the core, restart/reload it, then use **Load Content** to browse to the SWF. If the file remains hidden, check RetroArch's file-browser extension filter or choose **All Files**; this Android path has not yet been smoke-tested on-device.

## Android controls

- L3 opens the Winebox controls overlay on a controller.
- A persistent MENU touch target opens it without a mouse or physical controller.
- The overlay can be operated by touch or controller focus navigation.
- RetroArch's touch pointer is forwarded to BoxedWine as pointer/mouse input; the libretro frontend remains responsible for its Android touch overlay configuration.

## Validation

After each Android workflow run, download the ABI-specific artifact `winebox-android-{armv7,armv7-neon,arm64}-api21`. For Android 5 ARMv7, test the baseline core first; use the NEON build only on a device that supports NEON. Record the device, OS/API, RetroArch version, chosen core ABI, load result, and a short SWF playback/touch test in the human hardware-test issue. Do not describe Android 5 runtime as verified on the strength of cross-compilation alone.
