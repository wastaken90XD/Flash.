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
            queued_keys: Vec::new(),
        }
    }

    /// Queues a keyboard event received from `retro_keyboard_event_t`.
    pub fn push_key_event(&mut self, event: KeyEvent) {
        self.queued_keys.push(event);
    }

    /// Converts a [`RawInputSnapshot`] into a [`swfr_player::Input`] for a stage of `(width, height)`.
    #[allow(clippy::arithmetic_side_effects)]
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
        if raw.mouse_dx != 0 || raw.mouse_dy != 0 {
            self.cursor_x = (self.cursor_x + f32::from(raw.mouse_dx)).clamp(0.0, max_w.max(0.0));
            self.cursor_y = (self.cursor_y + f32::from(raw.mouse_dy)).clamp(0.0, max_h.max(0.0));
        }

        // Apply joypad movement when in cursor mode.
        let speed = f32::from(cursor_speed.clamp(1, 10));
        let mut pad_click = false;
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
            self.cursor_x = (self.cursor_x + dx).clamp(0.0, max_w.max(0.0));
            self.cursor_y = (self.cursor_y + dy).clamp(0.0, max_h.max(0.0));
            pad_click = raw.joypad_south;
        }

        // Pointer touch overrides cursor coordinates when pressed or inside normalized bounds.
        if raw.pointer_pressed {
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
