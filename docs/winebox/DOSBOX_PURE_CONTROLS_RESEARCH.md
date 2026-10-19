# DOSBox Pure controls: source-level research for Winebox

**Research date:** 2026-10-18

**Scope:** How DOSBox Pure (DBP) turns frontend controller/keyboard/mouse state into guest keyboard, mouse, and joystick actions; how its presets, mapper, on-screen keyboard (OSK), automatic mappings, and persistence work; and what is suitable to adapt to Winebox. This is research and a proposed design, **not an implementation**.

## Source provenance and confidence

The official GitHub repository `schellingb/dosbox-pure` is a move notice pointing to Codeberg. The Codeberg host could not be fetched in this environment, so the implementation details below were inspected in the unofficial GitHub mirror `anydream/dosbox-pure`, checkout `6f28498450314e5b168b25d0f90359b0d1bdb747`. Treat source-specific statements as findings from that mirror, not as a verified comparison against the current Codeberg tip. User-facing behaviors are cross-checked against the mirror's README. Source paths and line numbers below refer to that checkout.

Files central to the trace:

- `dosbox_pure_pad.h`: port modes, bind table, presets, mapping load/save, mapper-facing bind operations.
- `dosbox_pure_libretro.cpp`: input polling, event queue, event delivery to DOSBox keyboard/mouse/joystick APIs, mapping detection.
- `dosbox_pure_osd.h`: menu/input interception, mapper UI, OSK UI and event handling.
- `dosbox_pure_run.h`, `keyb2joypad.cpp`, `keyb2joypad.yml`: program launch/mapping data support. The YAML is human-readable source data; the inspected runtime embeds compressed mapping/index tables rather than parsing this YAML at runtime.
- `README.md`: documented controls, presets, OSD, and save location.

## Executive summary

DOSBox Pure is **not** just forwarding physical keyboard events. It has a translation pipeline:

`RetroArch input state (port/device/index/id) → DBP_InputBind mapping → typed DBP event → DOSBox guest input API`

The frontend-facing source can be a joypad button, analog axis, keyboard key, mouse, or pointer. A bind maps it to a guest keyboard key, mouse action, DOS joystick action, or DBP UI action. DBP polls state each `retro_run()`, edge-detects changes, and queues normalized events; the event consumer then calls `KEYBOARD_AddKey`, mouse APIs, and joystick APIs. Customizable controller maps, default generic keyboard maps, game-specific maps, mouse presets, joystick presets, an OSK, and menu/mapper interception all sit above that event path.

That separation is the core lesson for Winebox: **make a controller-to-virtual-input mapping layer**, then translate its actions to SDL/BoxedWine input. Keep physical keyboard input as one source among several, not as the controller design itself.

## 1. Frontend device model and defaults

### Port selection and modes

`DBP_PadMapping::SetPortMode` converts Libretro controller selections into internal modes (`dosbox_pure_pad.h:210-223`). It recognizes:

- a special automatic-mapping controller subclass (`subclass == 99`) when auto mapping is available;
- gamepad subclasses for named presets;
- ordinary joypad/analog devices as the custom mapper mode;
- keyboard, plus keyboard-with-left-stick-mouse and keyboard-with-right-stick-mouse subclasses.

The core publishes controller choices and input descriptors through Libretro environment calls (`dosbox_pure_pad.h:225-304`). That lets a frontend choose among "Use Gamepad Mapper", automatic/game-specific mappings, generic keyboard, mouse, and DOS joystick presets, as well as keyboard-based modes. It is not just a fixed assumption that every port is a joypad.

`DefaultPreset(port)` selects game auto-mapping for port 0 if one exists; otherwise it selects Generic Keyboard. Other ports default to Generic Keyboard (`dosbox_pure_pad.h:37`). The README also documents port 0 auto mapping when available and generic keyboard as fallback.

### Bind representation

Each `DBP_InputBind` stores a Libretro source tuple (`port`, `device`, `index`, `id`), a destination event (`evt`), metadata (`meta`, such as key/button/direction), and `lastval` (`dosbox_pure_libretro.cpp:~100`; `dosbox_pure_pad.h`). This is a data-driven mapping table, not a collection of hard-coded checks for individual controller buttons.

The special destination table covers:

- keyboard keys;
- mouse move directions, left/right/middle click, speed up/slow down;
- DOS joystick 1 and 2 axes/buttons/hats;
- OSD toggle, action wheel, and temporary port shift.

These are enumerated as `DBP_SpecialMappings` in `dosbox_pure_libretro.cpp:~130-180`. A mapper binding can therefore target more than a keyboard key.

### Generic keyboard layers

The four Generic Keyboard presets provide extended key coverage across controller ports (port modulo four chooses the layer). The first maps D-pad and left stick to arrows, Select/Escape, Start/Enter, face buttons to Space/Shift/Ctrl/Alt, shoulders/triggers to number keys, L3/R3 to F1/F2, and right stick to Home/End/Page Up/Page Down (`dosbox_pure_pad.h:758-813`). Other layers map keypad keys, WASD-like keys, and additional punctuation/navigation keys.

This is a useful detail for Winebox: a controller can operate keyboard-centric games without pretending a controller button *is* a physical keyboard device. It produces a virtual key action at the guest boundary.

### Mouse and joystick presets

Mouse presets bind an analog stick to mouse movement and buttons/triggers to click/speed adjustment (`dosbox_pure_pad.h:692-707`). Joystick presets expose DOS joystick 1/2 axes, buttons, and hats, including Gravis GamePad, basic 2-button joysticks, Thrustmaster Flight Stick, and both DOS joysticks (`dosbox_pure_pad.h:708-757`; README Keyboard/Joystick Emulation sections).

The implementation enables DOS joystick devices only when a mapping actually targets them (`dosbox_pure_pad.h:306-324`). This avoids games accidentally reacting to a joystick that the user did not configure.

## 2. Polling, normalization, and guest event delivery

### Per-frame source polling

`retro_run()` invokes `input_poll_cb()` before querying inputs (`dosbox_pure_libretro.cpp:3434-3485`). It reads mouse/pointer state separately (relative mouse and normalized pointer) and then checks each bind's current state. Changed values call `DBP_InputBind::Update` (`dosbox_pure_libretro.cpp:3483-3535`). Analog-button support has a fallback from analog-button state to digital joypad state for older frontends (`3520-3526`).

When DBP's OSD is active, normal bindings are intercepted. A fixed port-0 control map is polled instead, so navigation remains available even if gameplay bindings were customized (`dosbox_pure_osd.h:2058-2138`). On entering/leaving the interceptor it snapshots input state and releases keys to avoid stale transitions (`2100-2105`, `2140-2148`).

### Bind conversion and edge semantics

`DBP_InputBind::Update` distinguishes three cases (`dosbox_pure_libretro.cpp:438-~480`):

1. Analog source to analog destination: forward a signed value, with optional sign inversion.
2. Digital source to a key/special action: emit destination down on press and up on release.
3. Analog source to key/special action: treat each axis direction independently and use a threshold (12,000 in the inspected source) for digital activation; analog destinations remain continuous.

Analog dead-zone normalization is explicit: `dbp_joy_analog_deadzone` is 15% of the signed range, with rescaling outside the dead zone (`dosbox_pure_libretro.cpp:~125`). This prevents small stick drift from becoming guest movement.

### Event queue and delivery

The bind layer emits typed `DBP_Event`s into a bounded ring queue (`dosbox_pure_libretro.cpp:~190-220, 326-417`). Repeated key/button sources are reference-counted so a key remains down until all contributing sources release it (`326-355`). If the queue fills, accumulated motion-like events can be collapsed rather than blindly losing all event history (`382-416`).

The consumer drains events and dispatches them to DOSBox's own input APIs: `KEYBOARD_AddKey` for key down/up; `Mouse_CursorMoved`, `Mouse_ButtonPressed/Released` for mouse; `JOYSTICK_Move_X/Y`, `JOYSTICK_Button` for joystick; and dedicated handling for OSD, wheel, and port-shift events (`dosbox_pure_libretro.cpp:1831-1898`). Thus the host-side mapping is translated into guest device semantics rather than simply synthesizing host OS keyboard events.

### Physical keyboard and stuck-key defenses

Physical Libretro keyboard state is represented in the same input system, with key identity mapping between DOS key codes and Libretro key IDs (`dosbox_pure_libretro.cpp:~100, 419-435`). It is a valid source, not the only source. The core tracks held inputs and periodically releases keys it detects as stale, including a special path for physical keyboard keys (`419-435`, `3541-3544`). The OSD warns about keyboard/controller focus conflicts and suggests frontend Game Focus (`dosbox_pure_osd.h:2108-2119`).

## 3. Mapper UI and mapping capabilities

### Mapper contents

The mapper lists every standard joypad button/direction and both axes of both analog sticks (`dosbox_pure_osd.h:696-722`). It shows current bindings, allows creating/editing/deleting a binding, offers preset selection, lets the user switch controller ports, and displays an automatic-map action label when applicable (`699-831`). The README says up to four functions can be mapped per controller button/direction and each function can target keyboard, mouse, or joystick.

Axis mapping stores separate negative/positive functions (encoded as a pair) and supports analog-to-analog mappings as well as axis-to-digital-key/special-function mappings (`dosbox_pure_pad.h:129-147, 631-663`). This is more expressive than a flat button-to-key table. The mapper also has wheel/action-wheel options (`dosbox_pure_osd.h:793-818`; mapping serialization includes wheel items at `dosbox_pure_pad.h:101-109`).

When a preset is applied, DBP tracks which source controls are already occupied, allowing bindings from a preset/automatic map to be distributed without blindly colliding with already-used buttons; the L3 OSD binding is managed specially (`dosbox_pure_pad.h:561-609`).

### OSD navigation and input isolation

The OSD is an in-core rendered overlay and input interceptor, not an external window. `DBP_MenuInterceptor` supplies fixed navigation bindings for port 0: D-pad, left/right analog sticks, face buttons, shoulders, Select/Start, L2/R2, and L3 (`dosbox_pure_osd.h:2058-2093`). The menu therefore remains operable even while game inputs are mapped differently. Its start menu offers the mapper, and the OSK exposes a PAD MAPPER button (README Gamepad Mapper section).

By default L3 opens the menu/keyboard; OSD configuration can choose whether it opens the menu or OSK, and an OSK-only in-game mode is handled by `SetMode` (`dosbox_pure_osd.h:2162-2185`). Note the important design distinction: L3 is a *DBP UI action* in the control layer, not inherently a game key.

### On-screen keyboard behavior

The OSK draws a complete keyboard layout into an internal framebuffer overlay (`dosbox_pure_osd.h:1155-1253`). It includes function/navigation keys, alphanumeric rows, modifiers, numpad, and a PAD MAPPER affordance. The visual cursor can come from pointer/mouse and menu controls; selecting a key feeds a key event into DOSBox's keyboard API rather than injecting text into the host (`1255-1329`).

Input semantics include:

- short press: key down, then key up on release;
- holding a key past 500 ms: turn it into a latched key until selected/toggled off;
- modifier keys: toggle/hold state;
- controller face buttons: backspace and shift conveniences;
- Enter/Space and mouse left click: activate a selected key;
- Escape or OSD toggle: close OSK;
- selecting its mapper key opens the controller mapper.

These behaviors are visible in `dosbox_pure_osd.h:1194-1198, 1255-1327`. The OSK is therefore a virtual input UI and key-state producer, not a text-entry widget.

## 4. Automatic game mappings and controller labels

DBP has per-program automatic maps. Its runtime scans loaded content, hashes executable filename and size, looks up the hash in an index, decompresses the corresponding identity/mapping bucket, and exposes the map and action-name strings (`dosbox_pure_libretro.cpp:2608-2650`). The map can carry human-readable game title/action labels; the mapper can display these labels beside controls (`dosbox_pure_pad.h:826-832`; `dosbox_pure_osd.h:786-790`). The README describes detection by executable, game metadata, and a curated mapping collection.

`keyb2joypad.yml` is human-readable mapping source data (game identifier, year, pad button-to-key/chord, label, optional wheel entries). In the inspected build, the runtime index/compressed arrays are in `keyb2joypad.cpp` / `keyb2joypad.h`; do not assume DBP parses the YAML for each game at runtime. This is a data-pack + lookup approach, separate from the manual mapper.

## 5. Persistence and configuration

Custom mappings are loaded from the emulated DOS C: drive as `PADMAP.DBP` (`dosbox_pure_pad.h:52-75`). Saves happen when the mapper is closed and changed (`dosbox_pure_osd.h:712-715`). The format is versioned (version byte currently 0) and serializes per-port button mappings, up to four functions per control, axis direction pairs, and action-wheel options (`dosbox_pure_pad.h:77-127`). If no port remains customized, it removes `PADMAP.DBP` rather than storing redundant defaults (`79-85`). The README says the file lives in the frontend save file or beside the loaded program when running without ZIP/DOSZ.

This persistence choice is tightly integrated with DOS content/save semantics. Winebox does not have DOS's C: drive abstraction, so it should preserve the same *concept* (versioned custom bindings, applied immediately, saved reliably) but use an appropriate host-side per-game/core config path rather than copying `C:\PADMAP.DBP` literally.

## 6. What to adapt to Winebox (proposal, not implementation)

Winebox has a Libretro-to-SDL input adapter (`winebox/bridge/input_sdl_adapter.cpp/.hpp`) that polls physical keyboard, mouse/pointer, and controller state; physical keyboard input remains a supported source. After this research, Winebox's DBP-inspired adapter was expanded toward full behavior parity: Generic Keyboard and left/right-stick Mouse presets; an L3-opened menu, mapper, and OSK; four action slots for each joypad button/axis direction; keyboard, mouse, and virtual SDL joystick action targets; and versioned per-content profile persistence with version-1 migration. Guest joystick output uses a synthetic SDL joystick driver added to the BoxedWine patch because the pinned SDL does not provide SDL virtual-joystick APIs. This is still partial: automatic executable/game-specific maps, explicit chord presets and edge-ordering, configurable dead-zone/sensitivity and action-wheel controls, additional controller ports/device profiles, and broader OSD behavior are not implemented. A user can currently bind a modifier and key into separate slots on one source for a basic held chord.

Recommended translation of DBP's architecture to Winebox:

1. **Define a virtual-action layer**, e.g. `KeyDown/KeyUp(SDL key)`, `MouseMove`, `MouseButtonDown/Up`, optional text input, and `UIAction`. Keep game-specific SDL/BoxedWine details at the final backend. DOS joystick actions are not a first requirement for the current Flash/projector workload; do not import DOS-specific joystick presets unless Winebox's scope later needs them.
2. **Normalize physical sources into bind identities**: Libretro joypad button, analog axis + direction, keyboard key, pointer, mouse, with source port/index/id. Keep keyboard callbacks and polled keyboard as physical source providers, not as a control mapping system.
3. **Use data-driven mappings** from a physical control to one or more virtual actions. Support button-to-key, axis-to-key with configurable threshold/deadzone, left/right stick-to-cursor, click buttons, and modifier/chord actions. Define deterministic conflict semantics and key/button reference counts so multiple sources cannot prematurely release a held action.
4. **Separate gameplay input from overlay input**. An in-game overlay needs a fixed, reserved navigation profile that continues working even when the gameplay profile is customized, with a user-visible way to open it. Opening/closing must snapshot current physical states and release or reconcile held actions to prevent stuck keys.
5. **Start with useful presets, not auto-detection complexity**: keyboard-centric generic preset, pointer/mouse preset, and editable custom profile. A game-specific mapping database could come later once Winebox has stable game identity (for example, normalized executable/content hash) and labeling conventions.
6. **Provide an overlay mapper and virtual keyboard**. Mapper UI should show physical control → virtual action; allow add/edit/remove, axis direction pairs, modifiers/chords, deadzone/sensitivity, and reset-to-preset. OSK should render in Winebox's video output and emit virtual key press/release events; it must not be implemented as host text entry.
7. **Persist versioned mappings outside guest content**: core-wide defaults plus per-content overrides, using Libretro save/config directory conventions where available and a safe fallback. Write atomically, validate version/length, and make save/reset behavior explicit. Avoid writing mapping data into a guest filesystem unless user-visible guest portability is an intentional product requirement.
8. **Add state-machine tests before UI integration**: press/release symmetry; repeated poll/no duplicate edges; axis deadzone and thresholds; opposite directions; chord/modifier release; two physical sources mapped to one key; UI open while keys held; device callback changes; config round trip/corrupt config; pointer normalization and cursor bounds.

### Critical difference from DOSBox Pure

DBP owns the emulated DOS keyboard/mouse/joystick APIs and can dispatch normalized events directly to them. Winebox runs a Windows guest and its existing integration surfaces SDL events through BoxedWine hooks. Therefore, the reusable part is the **mapping/event architecture and UX model**, not DBP's DOS event enums, guest C: save file, event queue implementation, exact presets, or its game database. Winebox should emit virtual input into its own supported SDL/BoxedWine path, and should not assume that injecting SDL events at one point necessarily reaches every Windows input API. That final backend path must be traced and validated separately.

## 7. Bottom line

The key DOSBox Pure pattern is a controller mapper plus a virtual input abstraction, persistent presets, an OSK, and a dedicated menu-input mode. Winebox now adapts more of that model through its SDL/BoxedWine path, including multiple action slots and a synthetic guest joystick, rather than copying DOS-specific code. It remains incomplete: no automatic per-game mapping database, compound key chords, full analog dead-zone/sensitivity UI, action wheel, or broader device/port model. The patched SDL C source and C++ adapter/core passed local syntax checks, and the complete BoxedWine patch applied cleanly to the pinned upstream commit; no end-to-end Windows core build, mapping-state tests, or in-frontend behavior tests have run on these uncommitted changes. Before calling it production-ready, build and test the Windows core, exercise the L3 menu/mapper/OSK and guest joystick in a real frontend, and add focused tests for mapping round-trip/corruption/migration, multiple sources holding the same key, axis thresholds/dead-zone behavior, and mode changes while inputs are held.
