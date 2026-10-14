//! Input mapping from libretro devices (pointer, mouse, joypad, keyboard) to [`swfr_player::Input`] (TSD 2.5).

use swfr_player::{Input, KeyEvent, PadMode};

/// Snapshot of raw device inputs sampled during a single `retro_run` call.
#[derive(Debug, Clone, Default)]
pub struct RawInputSnapshot {
    /// Whether the pointer device reported being pressed.
    pub pointer_pressed: bool,
    /// Normalized pointer X in `[-0x7fff, 0x7fff]`.
    pub pointer_x: i16,
    /// Normalized pointer Y in `[-0x7fff, 0x7fff]`.
    pub pointer_y: i16,
    /// Relative mouse delta X in pixels.
    pub mouse_dx: i16,
    /// Relative mouse delta Y in pixels.
    pub mouse_dy: i16,
    /// Mouse left button pressed.
    pub mouse_left: bool,
    /// Mouse right button pressed.
    pub mouse_right: bool,
    /// Mouse middle button pressed.
    pub mouse_middle: bool,
    /// Mouse wheel up pulse.
    pub mouse_wheel_up: bool,
    /// Mouse wheel down pulse.
    pub mouse_wheel_down: bool,
    /// Joypad D-pad up pressed.
    pub joypad_up: bool,
    /// Joypad D-pad down pressed.
    pub joypad_down: bool,
    /// Joypad D-pad left pressed.
    pub joypad_left: bool,
    /// Joypad D-pad right pressed.
    pub joypad_right: bool,
    /// Joypad South (`B`) button pressed.
    pub joypad_south: bool,
    /// Joypad East (`A`) button pressed.
    pub joypad_east: bool,
    /// Joypad Start button pressed.
    pub joypad_start: bool,
    /// Analog left stick X in `[-0x7fff, 0x7fff]`.
    pub analog_left_x: i16,
    /// Analog left stick Y in `[-0x7fff, 0x7fff]`.
    pub analog_left_y: i16,
}

/// Persistent cursor and button state across frames for relative mouse and joypad cursor modes.
#[derive(Debug, Clone)]
pub struct InputTracker {
    cursor_x: f32,
    cursor_y: f32,
    cursor_visible: bool,
    prev_pointer_x: i16,
    prev_pointer_y: i16,
    prev_up: bool,
    prev_down: bool,
    prev_left: bool,
    prev_right: bool,
    prev_south: bool,
    prev_east: bool,
    prev_start: bool,
    queued_keys: Vec<KeyEvent>,
}

impl Default for InputTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl InputTracker {
    /// Creates a new [`InputTracker`] centered on the default 320x240 stage.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            cursor_x: 160.0,
            cursor_y: 120.0,
            cursor_visible: false,
            prev_pointer_x: 0,
            prev_pointer_y: 0,
            prev_up: false,
            prev_down: false,
            prev_left: false,
            prev_right: false,
            prev_south: false,
            prev_east: false,
            prev_start: false,
            queued_keys: Vec::new(),
        }
    }

    /// Returns whether the software cursor reticle should be rendered on the video frame.
    #[must_use]
    pub const fn cursor_visible(&self) -> bool {
        self.cursor_visible
    }

    /// Queues a keyboard event received from `retro_keyboard_event_t`.
    pub fn push_key_event(&mut self, event: KeyEvent) {
        self.queued_keys.push(event);
    }

    /// Converts a [`RawInputSnapshot`] into a [`swfr_player::Input`] for a stage of `(width, height)`.
    #[allow(clippy::arithmetic_side_effects, clippy::too_many_lines)]
    pub fn build_frame_input(
        &mut self,
        raw: &RawInputSnapshot,
        width: u32,
        height: u32,
        pad_mode: PadMode,
        cursor_speed: u8,
    ) -> Input {
        let max_w = (width.max(1) as f32) - 1.0;
        let max_h = (height.max(1) as f32) - 1.0;

        // Apply relative mouse movement.
        let mouse_moved = raw.mouse_dx != 0 || raw.mouse_dy != 0;
        if mouse_moved {
            self.cursor_x = (self.cursor_x + f32::from(raw.mouse_dx)).clamp(0.0, max_w.max(0.0));
            self.cursor_y = (self.cursor_y + f32::from(raw.mouse_dy)).clamp(0.0, max_h.max(0.0));
            self.cursor_visible = true;
        }
        if raw.mouse_left || raw.mouse_right || raw.mouse_middle {
            self.cursor_visible = true;
        }

        // Apply joypad movement when in cursor mode.
        let speed = f32::from(cursor_speed.clamp(1, 10));
        let mut pad_click = false;
        let mut joypad_moved = false;
        if pad_mode == PadMode::Cursor {
            let mut dx = 0.0_f32;
            let mut dy = 0.0_f32;
            if raw.joypad_left {
                dx -= speed;
            }
            if raw.joypad_right {
                dx += speed;
            }
            if raw.joypad_up {
                dy -= speed;
            }
            if raw.joypad_down {
                dy += speed;
            }
            if raw.analog_left_x.abs() > 4096 {
                dx += (f32::from(raw.analog_left_x) / 32767.0) * speed;
            }
            if raw.analog_left_y.abs() > 4096 {
                dy += (f32::from(raw.analog_left_y) / 32767.0) * speed;
            }
            joypad_moved = dx != 0.0 || dy != 0.0;
            if joypad_moved || raw.joypad_south {
                self.cursor_visible = true;
            }
            self.cursor_x = (self.cursor_x + dx).clamp(0.0, max_w.max(0.0));
            self.cursor_y = (self.cursor_y + dy).clamp(0.0, max_h.max(0.0));
            pad_click = raw.joypad_south;
        }

        // Emit Flash virtual key events on joypad button transitions.
        // In Cursor mode, D-Pad and B control the cursor/click, while A (Space) and Start (Enter)
        // emit key events; in Keys mode, all 7 buttons emit Flash virtual key events.
        if pad_mode == PadMode::Keys {
            let dpad_transitions = [
                (raw.joypad_left, &mut self.prev_left, 37_u32),
                (raw.joypad_up, &mut self.prev_up, 38_u32),
                (raw.joypad_right, &mut self.prev_right, 39_u32),
                (raw.joypad_down, &mut self.prev_down, 40_u32),
                (raw.joypad_south, &mut self.prev_south, 32_u32),
            ];
            for (now, prev, keycode) in dpad_transitions {
                if now != *prev {
                    *prev = now;
                    self.queued_keys.push(KeyEvent {
                        keycode,
                        character: 0,
                        modifiers: 0,
                        down: now,
                    });
                }
            }
        }
        let action_transitions = [
            (raw.joypad_east, &mut self.prev_east, 32_u32),
            (raw.joypad_start, &mut self.prev_start, 13_u32),
        ];
        for (now, prev, keycode) in action_transitions {
            if now != *prev {
                *prev = now;
                self.queued_keys.push(KeyEvent {
                    keycode,
                    character: 0,
                    modifiers: 0,
                    down: now,
                });
            }
        }

        // Pointer coordinates update cursor only when pointer coordinates actually changed
        // (or when pointer is pressed/positioned while relative mouse/joypad did not move),
        // preventing frozen post-click pointer coordinates from snapping the cursor back.
        let pointer_changed =
            raw.pointer_x != self.prev_pointer_x || raw.pointer_y != self.prev_pointer_y;
        self.prev_pointer_x = raw.pointer_x;
        self.prev_pointer_y = raw.pointer_y;
        if pointer_changed
            || (!mouse_moved
                && !joypad_moved
                && !self.cursor_visible
                && (raw.pointer_pressed || raw.pointer_x != 0 || raw.pointer_y != 0))
        {
            let norm_x = (f32::from(raw.pointer_x) + 32767.0) / 65534.0;
            let norm_y = (f32::from(raw.pointer_y) + 32767.0) / 65534.0;
            self.cursor_x = (norm_x * (width as f32)).clamp(0.0, max_w.max(0.0));
            self.cursor_y = (norm_y * (height as f32)).clamp(0.0, max_h.max(0.0));
        }

        let mut buttons = 0_u8;
        if raw.mouse_left || raw.pointer_pressed || pad_click {
            buttons |= 1;
        }
        if raw.mouse_right {
            buttons |= 2;
        }
        if raw.mouse_middle {
            buttons |= 4;
        }

        let mut wheel = 0_i32;
        if raw.mouse_wheel_up {
            wheel += 1;
        }
        if raw.mouse_wheel_down {
            wheel -= 1;
        }

        let keys = core::mem::take(&mut self.queued_keys);
        Input {
            pointer: Some((self.cursor_x, self.cursor_y)),
            buttons,
            wheel,
            keys,
        }
    }
}
