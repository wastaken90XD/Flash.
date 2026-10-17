# V0 Frontend & Platform Verification Notes (`docs/platform-notes.md`)

> **STATUS: UNVERIFIED** — These notes record the target platform floor from TSD 1.8 / 2.4. Physical RetroArch frontend and Reference B device verification (`V0` and `V1`–`V7`) have **not** been executed on hardware yet and await owner verification.

## Target Frontend Baseline

| Platform | OS Floor | Target Architecture | RetroArch Baseline | Status |
| :--- | :--- | :--- | :--- | :--- |
| **Linux** | Ubuntu 22.04+ / glibc 2.35+ | `x86_64`, `i686`, `armv7`, `aarch64` | RetroArch 1.19.1 | **UNVERIFIED** (CI ABI & QEMU tests green; awaiting owner RetroArch run) |
| **Windows** | Windows 10 (64-bit / 32-bit) | `x86_64` / `i686` (MSVC & GNU) | RetroArch 1.19.1 | **UNVERIFIED** (CI native MSVC & MinGW builds green; awaiting owner RetroArch run) |
| **macOS** | macOS 13+ | `aarch64-apple-darwin` / `x86_64-apple-darwin` | RetroArch 1.19.1 | **UNVERIFIED** (CI native macOS builds green; awaiting owner RetroArch run) |
| **Android** | Android 5.0 (API 21, Lollipop) | `armv7`, `thumbv7neon`, `aarch64` | RetroArch 1.19.1 (`minSdkVersion = 21`) | **UNVERIFIED** (CI NDK API 21 builds green; awaiting owner Reference B device run) |
| **iOS** | iOS 13+ | `aarch64-apple-ios` | RetroArch iOS | **UNVERIFIED** (CI build-only check green; awaiting owner device run) |

## V0 Hardware Check Checklist (Maintainer — Unverified)

- [ ] **Linux (`x86_64`):** Verify RetroArch 1.19.1 loads `swfr_libretro.so` (`RETRO_PIXEL_FORMAT_XRGB8888`) and renders frames cleanly.
- [ ] **Windows 10 (`x86_64`):** Verify RetroArch 1.19.1 loads `swfr_libretro.dll` (`x86_64-pc-windows-msvc` / `gnu`).
- [ ] **Android 5.0 (API 21, Reference B):** Verify RetroArch on Android 5.0 (Cortex-A7 / API 21) loads `swfr_libretro.so` (`armv7-linux-androideabi` / `thumbv7neon-linux-androideabi`).

## Core Pixel Format & Audio Baseline

- **Pixel Format:** `RETRO_PIXEL_FORMAT_XRGB8888` (`0x00RRGGBB` in native 32-bit words), with `RGB565` conversion available in `swfr-render` / `swfr-libretro::convert`.
- **Audio Sample Rate:** `44100 Hz` stereo signed 16-bit PCM (`i16` interleaved left/right frames).
