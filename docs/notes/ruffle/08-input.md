# R9 — Input, Events, Focus, and Drag (`08-input.md`)

- **Pinned Release Tag:** `v0.7.1`
- **Pinned Commit SHA:** `89a7049965f3090e706265670df309a5128233ac`
- **Repository:** `https://github.com/ruffle-rs/ruffle`
- **Overall Confidence:** **High** (verified directly from `core/src/{events,input,focus_tracker,player}.rs`, `core/src/display_object/stage.rs`, and `core/src/avm1/globals/{key,mouse,selection}.rs` at `89a7049965f3090e706265670df309a5128233ac`).

---

## 1. `PlayerEvent`, `InputEvent`, and `ClipEvent` (`Confidence: High`)

- **Two-Stage Input Pipeline (`core/src/events.rs`, `core/src/input.rs:21-150`):**
  1. Frontends submit `PlayerEvent` (`KeyDown`, `KeyUp`, `MouseMove`, `MouseDown`, `MouseUp`, `MouseLeave`, `MouseWheel`, `TextInput`, `TextControl`, `GamepadButtonDown`/`Up`, `Ime`) to `Player::handle_event`.
  2. `InputManager::handle_event` (`core/src/input.rs:75-180`) updates physical/logical key state (`keys_down`, `keys_toggled`, `last_key_code`, `last_key_char`, mouse position/buttons, double-click tracking, and gamepad-to-key remapping) and returns a normalized `InputEvent`.
- **Propagating vs Targeted `ClipEvent`s (`core/src/events.rs:200-425`):**
  - **Propagating ("anycast") events (`ClipEvent::propagates() == true`, `core/src/events.rs:365-375`):** `MouseUp`, `MouseDown`, `MouseMove`, `KeyPress`, `KeyDown`, `KeyUp` are broadcast to **every** listening MovieClip/Button on the stage.
  - **Targeted button events (`core/src/events.rs:317-336`):** `Press`, `Release`, `ReleaseOutside`, `RollOver`, `RollOut`, `DragOver`, `DragOut` are dispatched only to the hovered/pressed `InteractiveObject`!

---

## 2. Viewport-to-Stage Coordinate Mapping (`StageScaleMode` & Letterbox) (`Confidence: High`)

- **`Stage::build_matrices` (`core/src/display_object/stage.rs:500-607`):**
  - Computes `scale_x` and `scale_y` from `StageScaleMode` (`ShowAll` uses `min(scale_x, scale_y)`, `NoBorder` uses `max(scale_x, scale_y)`, `ExactFit` uses independent `scale_x`/`scale_y`, `NoScale` uses `1.0`), applies `StageAlign` (`TOP`, `BOTTOM`, `LEFT`, `RIGHT`) to compute letterbox offsets `(tx, ty)`, and multiplies by `stage_tx.inverse()` (where `stage_tx` translates by the SWF header's `stage_size.x_min, y_min`) to form `viewport_matrix` (`core/src/display_object/stage.rs:575-577`).
  - Viewport mouse coordinates `(x_px, y_px)` are converted to stage `Twips` by multiplying by `stage.inverse_view_matrix()` (`core/src/player.rs:1440-1460`).
  - Inside each MovieClip (`_xmouse`, `_ymouse`), stage `Twips` are transformed via `display_object.global_to_local(mouse_pos)` (`core/src/display_object.rs:640-675`).

---

## 3. Mouse Picking (`update_mouse_state`) and `startDrag` / `stopDrag` (`Confidence: High`)

- **Mouse Picking (`Player::update_mouse_state`, `core/src/player.rs:1551-1920`):**
  - Calls `run_mouse_pick(context, true)` (`core/src/player.rs:1569`), which traverses `Stage` children **top-to-bottom** (highest depth first) calling `InteractiveObject::mouse_pick_avm1`.
  - Tracks `MouseData.hovered` and `MouseData.pressed` (`Left`, `Right`, `Middle`, `core/src/player.rs:125-132`):
    - If `hovered` was removed from stage (`avm1_removed()`), Ruffle searches for a replacement character at the same depth/ID (`find_first_character_instance`, `core/src/player.rs:1622-1640`) and transfers the button state!
    - When the left mouse button is **up** and `hovered` changes (`cur != new`), dispatches `RollOut` to `cur` and `RollOver` to `new` (`core/src/player.rs:1761-1781`).
    - When the left mouse button is **down** and `hovered` changes, dispatches `DragOut` / `DragOver` to `pressed` (and in AVM1 when `startDrag` is **not** active, also dispatches `DragOut` / `DragOver` to other buttons with `trackAsMenu`, `core/src/player.rs:1673-1760`).
    - On button release (`core/src/player.rs:1818-1865`), compares `pressed == hovered`: if equal, dispatches `Release`; otherwise dispatches `ReleaseOutside` to `pressed`.
- **`startDrag` / `stopDrag` (`Player::update_drag`, `core/src/player.rs:1498-1549`):**
  - `DragObject` stores `display_object`, `last_mouse_position`, `lock_center: bool`, and `constraint: Rectangle<Twips>`.
  - On every `Player::update`, `update_drag` transforms `mouse_position` into the dragged clip's parent coordinate space (`global_to_local_matrix`), applies either `lock_center` or delta-tracking (`position + global_to_local_matrix * mouse_delta`), clamps to `constraint`, and updates `display_object.set_x` / `set_y`.
  - To compute `_droptarget` (`core/src/player.rs:1532-1547`), `update_drag` temporarily sets `display_object.set_visible(context, false)`, calls `run_mouse_pick(context, false)` (guarded by `updating_drop_target` to prevent infinite recursion if a `hitArea` getter calls `stopDrag()`), sets `movie_clip.set_drop_target(...)`, and restores `visible`!

---

## 4. `Key`, `Mouse`, `Selection`, and `FocusTracker` (`Confidence: High`)

- **`Key` (`core/src/avm1/globals/key.rs:1-92`):**
  - Initialized as an `AsBroadcaster` (`broadcaster_functions.initialize`, line 40) so `Key.addListener(obj)` receives `onKeyDown` and `onKeyUp` via `broadcastMessage`.
  - Exposes `Key.isDown(code)`, `Key.isToggled(code)`, `Key.getCode()`, `Key.getAscii()`, and 19 key constants (`SPACE = 32`, `ENTER = 13`, `UP = 38`, `DOWN = 40`, `LEFT = 37`, `RIGHT = 39`, etc.).
- **`Mouse` (`core/src/avm1/globals/mouse.rs:1-72`):**
  - Initialized as an `AsBroadcaster` (`onMouseDown`, `onMouseUp`, `onMouseMove`, `onMouseWheel`).
  - `Mouse.hide()` and `Mouse.show()` return `0` if the cursor was previously visible and `1` if previously hidden (`core/src/avm1/globals/mouse.rs:32-44`)!
- **`Selection` and `FocusTracker` (`core/src/focus_tracker.rs:1-540`, `core/src/avm1/globals/selection.rs`):**
  - `FocusTracker` tracks the currently focused `InteractiveObject<'gc>` (`EditText`, `Avm1Button`, or `MovieClip` with `focusEnabled`).
  - Changing focus fires `old.onKillFocus(new)` -> `new.onSetFocus(old)` -> `Selection.broadcastMessage("onSetFocus", old, new)`.
  - Tab ordering (`FocusTracker::navigate_tab`, `core/src/focus_tracker.rs`) sorts tabbable objects either by explicit `tabIndex` or by geometric reading order (`y` then `x`) and renders the yellow highlight rectangle (`render_highlight`, `core/src/display_object/stage.rs:639`) when `_focusrect` is enabled.

---

## 5. Verification Against Regression Tests (`Confidence: High`)

1. **Claim 1 — `_xmouse` and `_ymouse` across nested scaled and rotated MovieClips (`core/src/display_object.rs:640-675`):**
   - Verified against `tests/tests/swfs/avm1/mouse_pos/output.txt:1-25` (`_root 308 340`, `zs 15.4 17`, `s133fpr15 -0.4 216.5`).
2. **Claim 2 — Focus change event order (`onKillFocus` -> `onSetFocus` -> `Selection.onSetFocus`, `core/src/focus_tracker.rs:144-240`):**
   - Verified against `tests/tests/swfs/avm1/tab_ordering_events/output.txt:1-25` (`_level0.clip.onKillFocus` -> `_level0.button.onSetFocus` -> `Focus changed: _level0.clip -> _level0.button`).
3. **Claim 3 — Button and clip event ordering (`core/src/events.rs:315-362`):**
   - Verified against `tests/tests/swfs/avm1/button_order/output.txt:1-2` (`enterFrame instance3`, `enterFrame instance2`).

---

## 6. What Could Not Be Verified (`Confidence: High`)

- IME composition (`ImePreedit` / `ImeCommit`) with host OS input methods was not verified at runtime.

---

## 7. Open Questions

1. Should `swfr-avm1` return `0` (if previously visible) or `1` (if previously hidden) from `Mouse.show()` and `Mouse.hide()`, matching `core/src/avm1/globals/mouse.rs:34, 44`?
2. Should `swfr-scene` update `_droptarget` during `startDrag` by picking the topmost visible object underneath the dragged MovieClip (`core/src/player.rs:1532-1547`)?

---

## 8. Test Ideas for `swfr`

1. **`Mouse.hide()` / `Mouse.show()` return value test:** Assert `Mouse.hide()` returns `0` when cursor was visible and `1` when called a second time while already hidden (`core/src/avm1/globals/mouse.rs:27-45`).
2. **`startDrag` with bounding rectangle and `_droptarget` test:** Start dragging a clip with `(lockCenter = true, left, top, right, bottom)`, move pointer outside the constraint box, and assert `_x`/`_y` clamp to the box and `_droptarget` reflects the clip underneath.
3. **`onReleaseOutside` button transition test:** Press mouse button inside a button/clip, move pointer outside its hit area, release mouse button, and verify `onDragOut` followed by `onReleaseOutside` fires.
