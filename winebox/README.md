# Winebox

Winebox is an experimental BoxedWine/libretro integration area. BoxedWine source is built separately at a pinned upstream revision by CI; no BoxedWine code belongs in a `swfr` crate.

## Current work

- `bridge/` defines a C++20 host boundary for XRGB8888 software frames, stereo signed-16-bit audio, and frontend-thread input polling. Worker threads may publish frames/audio, but frontend callbacks stay on the libretro caller thread. The bounded PCM converter and host tests pass with GCC.
- `bridge/input_sdl_adapter.*` translates libretro keyboard, pointer, mouse, analog, and joypad state into SDL2 events. The default Generic Keyboard preset maps pad buttons and stick directions to virtual keys; the `Mouse` core option uses the left stick/D-pad for cursor movement and A/B clicks. Press L3 for the in-game controls menu, controller mapper, and on-screen keyboard. The mapper assigns one virtual key to each pad button/axis direction, saves a versioned profile in the frontend save directory per loaded SWF basename, and reconciles held inputs when entering/leaving the overlay. This adapts the controller-mapper/OSK pattern; it does not include DOS joystick emulation or DBP's game-specific automatic-map database. The adapter compiles with `-Wall -Wextra -Werror` against the pinned checkout's vendored SDL2 headers.
- `core/libretro_core.cpp` now implements the basic libretro ABI, guest worker lifecycle, software video/audio delivery, input event polling, and a launch command using a frontend-provided SWF, Flash projector, BoxedWine root archive, and writable save root. The worker shutdown posts SDL quit events until the guest returns.
- `patches/boxedwine-libretro-hooks.patch` carries the SDL software-screen, audio-queue, windowless-system, quit-hook, and `-novideo` changes for upstream commit `6c70c03c6a611a0993ccc922a4f10e658af02f49`. The Windows workflow applies it before building.
- `core/BoxedWineLibretro.targets` adds the core and bridge sources to the upstream MSVC build and compiles the guarded platform hooks with `BOXEDWINE_LIBRETRO`. The workflow overrides the upstream executable configuration to produce `winebox_libretro.dll`, checks required exports, and uploads it as an artifact.
- Local GCC compilation of the core/bridge sources and both bridge tests pass. The sandbox has no CMake or MSBuild, so the Windows DLL build has not yet been executed here; the workflow is the intended build/validation path.

The bridge uses the libretro declarations in `docs/reference/libretro.h` and requests `RETRO_PIXEL_FORMAT_XRGB8888` at initialization. All new core-side code remains under `winebox/`.
