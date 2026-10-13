# `swfr` Default Input Mapping (`docs/keymap.md`)

`swfr` maps libretro input devices (`RETRO_DEVICE_JOYPAD`, `RETRO_DEVICE_ANALOG`, `RETRO_DEVICE_MOUSE`, `RETRO_DEVICE_POINTER`, and `RETRO_DEVICE_KEYBOARD`) to Flash stage cursor coordinates and ActionScript `Key` / button events according to the **`swfr_input_mode`** core option (`auto`, `cursor`, `gamepad`).

## Core Option: `swfr_input_mode`

| Value | Description |
| :--- | :--- |
| `auto` (default) | Combines pointer/mouse coordinates, analog-stick cursor movement, and joypad button/key bindings simultaneously. |
| `cursor` | Treats the left analog stick and D-pad as a virtual stage cursor with `A` / `B` acting as left/right mouse buttons. |
| `gamepad` | Maps the D-pad and face buttons directly to Flash ActionScript keyboard codes (`Key.LEFT`, `Key.RIGHT`, `Key.UP`, `Key.DOWN`, `Key.SPACE`, `Key.CONTROL`, `Key.ENTER`, `Key.SHIFT`). |

## Default RetroPad (`RETRO_DEVICE_JOYPAD`) Mapping

| RetroPad Control | `cursor` Mode Action | `gamepad` / `auto` Key Code (`Key.*`) |
| :--- | :--- | :--- |
| D-Pad Up (`RETRO_DEVICE_ID_JOYPAD_UP`) | Move virtual cursor up (`-4 px/frame`) | `Key.UP` (`38`) |
| D-Pad Down (`RETRO_DEVICE_ID_JOYPAD_DOWN`) | Move virtual cursor down (`+4 px/frame`) | `Key.DOWN` (`40`) |
| D-Pad Left (`RETRO_DEVICE_ID_JOYPAD_LEFT`) | Move virtual cursor left (`-4 px/frame`) | `Key.LEFT` (`37`) |
| D-Pad Right (`RETRO_DEVICE_ID_JOYPAD_RIGHT`) | Move virtual cursor right (`+4 px/frame`) | `Key.RIGHT` (`39`) |
| `A` (`RETRO_DEVICE_ID_JOYPAD_A`) | Left Mouse Button (Primary Click) | `Key.SPACE` (`32`) + Primary Click |
| `B` (`RETRO_DEVICE_ID_JOYPAD_B`) | Right Mouse Button / Secondary | `Key.CONTROL` (`17`) |
| `X` (`RETRO_DEVICE_ID_JOYPAD_X`) | Action 1 (`Z` key) | `'Z'` (`90`) |
| `Y` (`RETRO_DEVICE_ID_JOYPAD_Y`) | Action 2 (`X` key) | `'X'` (`88`) |
| `Start` (`RETRO_DEVICE_ID_JOYPAD_START`) | `Enter` (`13`) | `Key.ENTER` (`13`) |
| `Select` (`RETRO_DEVICE_ID_JOYPAD_SELECT`) | `Escape` (`27`) | `Key.ESCAPE` (`27`) |
| `L` (`RETRO_DEVICE_ID_JOYPAD_L`) | `Shift` (`16`) | `Key.SHIFT` (`16`) |
| `R` (`RETRO_DEVICE_ID_JOYPAD_R`) | `Tab` (`9`) | `Key.TAB` (`9`) |

## Analog Stick, Mouse, and Touch Pointer

- **Left Analog Stick (`RETRO_DEVICE_INDEX_ANALOG_LEFT`):** Moves the virtual stage cursor proportionally (`[-0x7fff, 0x7fff]` scaled to `±8 px/frame`), clamped to the stage bounds `[0..width_px, 0..height_px]`.
- **Mouse (`RETRO_DEVICE_MOUSE`):** Relative deltas `MOUSE_X` and `MOUSE_Y` move the stage cursor; `MOUSE_LEFT` triggers primary button press/release (`DefineButton` / `DefineButton2` hit-test and `Mouse.onMouseDown` / `Mouse.onMouseUp`).
- **Touch / Pointer (`RETRO_DEVICE_POINTER`):** Normalized coordinates `[-0x7fff, 0x7fff]` map linearly to `[0..stage_width_twips, 0..stage_height_twips]`; `POINTER_PRESSED` triggers primary button press/release at the touch location.
- **Keyboard (`RETRO_ENVIRONMENT_SET_KEYBOARD_CALLBACK`):** Hardware key-down and key-up events are forwarded directly to ActionScript `Key.isDown(code)` and `Key.getCode()`.
