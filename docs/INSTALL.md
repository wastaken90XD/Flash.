# Installing and Using `swfr_libretro` (`docs/INSTALL.md`)

## 1. Downloading Pre-Built Release Packages

Each release (and CI `cross` / `release` workflow run) produces one `.zip` archive per target platform:

| Platform | Archive Name | Core Binary |
| :--- | :--- | :--- |
| Linux (`x86_64`) | `swfr-libretro-x86_64-unknown-linux-gnu.zip` | `swfr_libretro.so` |
| Linux (`x86` 32-bit) | `swfr-libretro-i686-unknown-linux-gnu.zip` | `swfr_libretro.so` |
| Linux (`armv7` hard-float) | `swfr-libretro-armv7-unknown-linux-gnueabihf.zip` | `swfr_libretro.so` |
| Linux (`aarch64`) | `swfr-libretro-aarch64-unknown-linux-gnu.zip` | `swfr_libretro.so` |
| Windows (`x86_64` MSVC / GNU) | `swfr-libretro-x86_64-pc-windows-msvc.zip` | `swfr_libretro.dll` |
| Windows (`x86` 32-bit MSVC / GNU) | `swfr-libretro-i686-pc-windows-msvc.zip` | `swfr_libretro.dll` |
| macOS (`arm64` Apple Silicon) | `swfr-libretro-aarch64-apple-darwin.zip` | `swfr_libretro.dylib` |
| macOS (`x86_64` Intel) | `swfr-libretro-x86_64-apple-darwin.zip` | `swfr_libretro.dylib` |
| Android (`aarch64` API 21+) | `swfr-libretro-aarch64-linux-android.zip` | `swfr_libretro.so` |
| Android (`armv7` NEON API 21+) | `swfr-libretro-thumbv7neon-linux-androideabi.zip` | `swfr_libretro.so` |
| Android (`armv7` baseline API 21+) | `swfr-libretro-armv7-linux-androideabi.zip` | `swfr_libretro.so` |

Each archive includes:
- The compiled core library (`swfr_libretro.so`, `.dll`, or `.dylib`)
- `swfr_libretro.info` (libretro core metadata file)
- `README.md`
- `LICENSE-MIT`, `LICENSE-APACHE`, and `THIRD_PARTY_LICENSES.md`

## 2. Installing into RetroArch

1. Extract the `.zip` archive for your platform.
2. Copy the core library (`swfr_libretro.so` / `.dll` / `.dylib`) into your RetroArch `cores/` directory:
   - **Linux:** `~/.config/retroarch/cores/`
   - **Windows:** `<RetroArch-Install-Dir>\cores\`
   - **macOS:** `~/Library/Application Support/RetroArch/cores/`
   - **Android:** Use **Load Core -> Install or Restore a Core** in RetroArch and select `swfr_libretro.so`.
3. Copy `swfr_libretro.info` into your RetroArch `info/` (Core Info) directory.
4. Launch RetroArch, choose **Load Content**, and select any `.swf` file.

## 3. Core Options (`Quick Menu -> Core Options`)

| Option Key | Default | Values | Description |
| :--- | :--- | :--- | :--- |
| `swfr_scale` | `1x` | `1x`, `2x`, `3x`, `4x` | Internal stage resolution multiplier (`RETRO_ENVIRONMENT_SET_GEOMETRY`). |
| `swfr_quality` | `medium` | `low`, `medium`, `high` | Rendering anti-aliasing and gradient quality level. |
| `swfr_pad_mode` | `cursor` | `cursor`, `keys` | Maps the gamepad D-pad/stick to the virtual stage cursor (`cursor`) or ActionScript `Key` arrows (`keys`). |
| `swfr_cursor_speed` | `5` | `1`..`10` | Virtual cursor movement speed in pixels per frame. |
| `swfr_script_timeout` | `2` | `1`, `2`, `5`, `15` | Per-frame ActionScript watchdog timeout budget in seconds. |
| `swfr_trace_to_log` | `on` | `on`, `off` | Forwards ActionScript `trace()` output to the libretro log interface. |
