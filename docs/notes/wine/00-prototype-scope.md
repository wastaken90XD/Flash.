# DOSBox-Pure-style Wine core: first prototype slice

## User goal

A single RetroArch-friendly core that boots a Windows game package, offers a launcher, and runs Windows applications without requiring the user to launch a separate desktop Wine environment. The current user's selected first use case is Flash content and Flash Projector `.exe` files.

## What this branch implements now

- `swfr_libretro.info` and `retro_get_system_info` advertise `.swf|.exe`.
- `swfr-player::extract_projector_swf` extracts a projector's appended SWF using the `0xFA123456` trailer and validates fallback overlay candidates by parsing them.
- `Player::new` routes an extracted movie through the existing native SWF runtime.
- ABI end-to-end coverage constructs a Projector-like `.exe`, loads it via `retro_load_game`, runs it, and checks its rendered stage.
- AVM1 compatibility work in this prototype addresses a subset of real Projector/SWF behavior: nested sprite callbacks, `Object.registerClass` registration and attached-symbol initialization, MovieClip method dispatch, Flash-style target-relative frame navigation, and Stage dimensions.

This **does not run a Windows PE program**. It extracts the SWF from a Flash Projector wrapper and runs it in `swfr`; arbitrary `.exe` files remain unsupported. It is not Wine, and no Wine/Win32 emulator has been embedded.

## Why this is staged rather than pretending Wine is done

A DOSBox-Pure-for-Wine core needs a real x86 execution engine plus a substantial Win32/NT API compatibility layer, process/thread and registry/filesystem semantics, a virtual `C:\` filesystem with persistent writable overlay, window/GDI/DirectDraw/Direct3D presentation into libretro video, audio device translation, input/event mapping, and an in-core launcher. Ordinary desktop Wine is not a drop-in libretro library: its normal process model and window-system integration do not fit `retro_run`.

The repository's Part 4 constraints prohibit importing or porting copyleft player code. This prototype therefore does not vendor Boxedwine, Wine, or another player. Before choosing an embeddable Windows runtime, the project needs an owner-approved dependency/license review; an independent clean-room Win32 runtime is a much larger project than Flash Projector extraction.

## Suggested next milestones

1. **Flash Projector compatibility:** add real user-supplied projector test cases (with redistribution permission), verify the trailer format against evidence, implement missing AVM1 behaviors against named tests, and collect timing/profile data for the user's stalled SWF.
2. **Package/launcher contract:** define one archive format, virtual drive layout, content picker, settings, save overlay, and BIOS/runtime provisioning. Keep the initial scope on user-provided Flash Projectors and SWFs.
3. **Runtime decision gate:** compare a permitted embedded Windows runtime or a clean-room x86/Win32 implementation for platform reach, libretro fit, license, maintenance, and testability. Do not claim host Wine is portable in-core.
4. **Actual Windows execution:** only after the runtime decision, implement an in-process prototype that can launch a tiny known PE program and return deterministic video/input/audio through the libretro ABI. Then expand API coverage and test real projectors.

## Verification targets

- `extracts_projector_payload_using_fa123456_footer`
- `finds_swf_in_overlay_without_footer`
- `rejects_bad_footer_extent_and_unrecognised_files`
- `e2e_flash_projector_exe_extracts_and_runs_embedded_swf`
- Existing `cargo test --workspace` and CI ABI/cross-build suites

CI has not run on this working tree yet. Do not treat the prototype as complete until those tests pass and the runtime decision is made.
