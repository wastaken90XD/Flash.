# V0 Frontend & Platform Verification Notes (`docs/platform-notes.md`)

This document records the target platform floor and frontend verification (TSD 1.8, Test V0).

## Target Frontend Baseline

| Platform | OS Floor | Target Architecture | RetroArch Baseline | Status |
| :--- | :--- | :--- | :--- | :--- |
| **Linux** | Ubuntu 22.04+ / glibc 2.35+ | `x86_64-unknown-linux-gnu` | RetroArch 1.19.1 (`x86_64`) | Verified supported |
| **Windows** | Windows 10 (64-bit) | `x86_64-pc-windows-msvc` | RetroArch 1.19.1 (`x86_64`) | Verified supported |
| **macOS** | macOS 13+ | `aarch64-apple-darwin` / `x86_64-apple-darwin` | RetroArch 1.19.1 (Metal / OpenGL) | Verified supported |
| **Android** | Android 5.0 (API 21, Lollipop) | `aarch64-linux-android` / `armv7-linux-androideabi` | RetroArch 1.19.1 (`RetroArch_aarch64.apk`, `minSdkVersion = 21`) | Standard APK supports API 21+ |

## V0 Hardware Check Checklist (Maintainer)

- [x] **Linux (`x86_64`):** RetroArch 1.19.1 loads software-rendered `XRGB8888` (`RETRO_PIXEL_FORMAT_XRGB8888`) `cdylib` cores without OpenGL context requirements.
- [x] **Windows 10 (`x86_64`):** RetroArch 1.19.1 loads `.dll` software cores built with `x86_64-pc-windows-msvc`.
- [x] **Android 5.0 (API 21):** RetroArch's standard Android build targets `minSdkVersion 21` (Android 5.0 Lollipop). Core builds targeting NDK API 21 (`aarch64-linux-android` / `armv7-linux-androideabi`) remain compatible with the Android 5 floor.

## Core Pixel Format & Audio Baseline

- **Pixel Format:** `RETRO_PIXEL_FORMAT_XRGB8888` (`0x00RRGGBB` in native 32-bit words), falling back to `RGB565` only if explicitly requested by a constrained frontend.
- **Audio Sample Rate:** `44100 Hz` stereo signed 16-bit PCM (`i16` interleaved left/right frames).
