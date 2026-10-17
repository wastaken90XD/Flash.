# `swfr` Verification & Acceptance Test SWFs

This directory (`test-swfs/`) is committed directly in the repository branch so you can load `.swf` files straight from the checkout (as well as from every platform build zip in `cross.yml` / `release.yml`) to verify `swfr_libretro` in RetroArch.

---

## 1. Milestone & Acceptance Test SWFs (`test-swfs/*.swf`)

| File | Stage & Rate | Expected Behavior in RetroArch |
| :--- | :--- | :--- |
| `1frame_red_rect_on_blue_stage.swf` | `200×160` @ `60 fps` | Static 1-frame movie: centered `100×80` red rectangle (`0x00FF0000`) on a blue stage (`0x000000FF`). Silent audio (`0`). |
| `10frame_moving_square.swf` | `200×100` @ `60 fps` | 10-frame looping animation: a `20×20` bright-green square moves horizontally across a dark stage (`0x00102030`) by 10 pixels per frame (`x = 10..100`). |
| `v2_m2_vectors_and_bitmaps.swf` | `320×240` @ `30 fps` | Interactive showcase: dark-slate stage (`0x00141A26`), animated cyan indicator bar moving smoothly across the top, centered interactive button, and dynamic HUD text (`"SWFR READY"`). |
| `v3_m3_timeline_and_buttons.swf` | `320×240` @ `30 fps` | Timeline + `DefineButton2`: moving the cursor (mouse or D-pad/analog stick) over the center button changes it from **blue/purple (`Up`)** to **gold (`Over`)**; clicking (Left Mouse or RetroPad `B` / `Z`) changes it to **emerald green (`Down`)** and updates the HUD to **`"BTN CLICKED"`**. |
| `v4_m3_text_and_fonts.swf` | `320×240` @ `30 fps` | `DefineFont2` + `DefineEditText` HUD (`"SWFR READY"`) with live variable binding (`"hud"`). |
| `v5_m4_avm1_interactive.swf` | `320×240` @ `30 fps` | AVM1 interactive scripting: clicking the center button sets `"hud"` to **`"BTN CLICKED"`** and emits `[trace] button_clicked`; pressing any keyboard key or RetroPad D-pad/button triggers `onClipEvent(keyDown)` and sets `"hud"` to **`"KEY PRESSED"`**. |
| `v6_m5_audio_sync.swf` | `320×240` @ `30 fps` | Audio + timeline + input: plays a clean **one-shot 0.20s two-note C5→E5 chime** on Frame 1 load, then remains silent while the 20-frame indicator bar loops (`gotoAndPlay(2)`). |
| `v7_m6_release_candidate.swf` | `320×240` @ `30 fps` | Full M6 release-candidate verification movie combining vector rendering, 20-frame timeline animation, interactive `DefineButton2`, keyboard/gamepad clip events, `DefineEditText` HUD, and one-shot Frame 1 chime audio. |

---

## 2. Open-Licensed Real-World Corpus SWFs (`test-swfs/real/*.swf`)

Copied from the pinned, SHA-256-verified Ruffle test suite (`MIT OR Apache-2.0`):

- `ruffle-avm1-array-prototyping.swf`
- `ruffle-avm1-button-order.swf`
- `ruffle-avm1-clip-events.swf`
- `ruffle-avm1-color.swf`
- `ruffle-avm1-duplicate-movie-clip.swf`
- `ruffle-avm1-edittext-tag-indent.swf`
- `ruffle-avm1-function-base-clip.swf`
- `ruffle-avm1-goto-frame.swf`
- `ruffle-avm1-goto-frame2.swf`
- `ruffle-avm1-mouse-pos.swf`
- `ruffle-avm1-movieclip-depth-methods.swf`
- `ruffle-avm1-register-class.swf`
- `ruffle-avm1-remove-movie-clip.swf`
- `ruffle-avm1-set-interval.swf`
- `ruffle-avm1-shared-object.swf`
- `ruffle-avm1-sound.swf`
- `ruffle-avm1-string-methods.swf`
- `ruffle-avm1-tell-target.swf`
- `ruffle-avm1-try-catch-finally.swf`
- `ruffle-avm1-with.swf`

---

## 3. Loading in RetroArch

1. Copy `swfr_libretro.{dll,so,dylib}` into your RetroArch `cores/` directory and `swfr_libretro.info` into your RetroArch `info/` directory.
2. Run:
   ```sh
   retroarch -L path/to/swfr_libretro.dll test-swfs/v7_m6_release_candidate.swf -v
   ```
3. **Controls in RetroArch:**
   - **Mouse / Touch Pointer:** Move and left-click directly on stage buttons.
   - **Gamepad / Keyboard (RetroPad):** D-Pad / Left Stick moves the on-screen crosshair cursor AND sends Flash arrow keys (`Key.LEFT/UP/RIGHT/DOWN`); RetroPad `B` (`Z`) left-clicks at the cursor AND sends `Key.SPACE`; RetroPad `A` (`X`) and `Start` (`Enter`) send `Key.ENTER`.
