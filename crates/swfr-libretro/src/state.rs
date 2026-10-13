//! Safe global core state and lifecycle management for `swfr_libretro` (TSD 2.5).

use crate::convert::{fill_dead_frame, DEAD_FRAME_XRGB};
use crate::input_map::{InputTracker, RawInputSnapshot};
use swfr_player::{
    AvInfo, LogLine, Options, Player, DEFAULT_FPS, DEFAULT_STAGE_HEIGHT, DEFAULT_STAGE_WIDTH,
    SAMPLE_RATE_HZ,
};

/// Output produced by [`CoreState::step_frame`] for the FFI layer to hand to the frontend.
#[derive(Debug)]
pub struct CoreStepOutput<'a> {
    /// Video frame pixels (`XRGB8888`), or `None` if frame duping is used.
    pub video_pixels: Option<&'a [u32]>,
    /// Video frame width in pixels.
    pub width: u32,
    /// Video frame height in pixels.
    pub height: u32,
    /// Interleaved stereo `i16` audio samples (`stereo_frames * 2` elements).
    pub audio_samples: &'a [i16],
    /// Log lines emitted during this step.
    pub logs: &'a [LogLine],
}

/// Safe Rust representation of the core's runtime state, protected by a single `Mutex`.
#[derive(Debug)]
pub struct CoreState {
    /// Whether `retro_init` has been called.
    pub initialized: bool,
    /// Whether the core caught a panic and entered the dead fallback state (TSD 2.5).
    pub dead: bool,
    /// Whether the panic message has already been logged once.
    pub panic_logged: bool,
    /// Recorded panic message if `dead == true`.
    pub panic_message: Option<String>,
    /// Whether the frontend supports passing `NULL` to `retro_video_refresh_t` for unchanged frames.
    pub can_dupe: bool,
    /// Controller device ID assigned to port 0 (`RETRO_DEVICE_JOYPAD` = 1 by default).
    pub port0_device: u32,
    /// Optional frontend system directory path.
    pub system_dir: Option<String>,
    /// Optional frontend save directory path.
    pub save_dir: Option<String>,
    /// Current core options (`swfr_*`).
    pub options: Options,
    /// Persistent input tracker across frames.
    pub input_tracker: InputTracker,
    /// Active player instance when a game is loaded.
    pub player: Option<Player>,
    /// Last known frame width for dead-state fallback rendering.
    pub fallback_width: u32,
    /// Last known frame height for dead-state fallback rendering.
    pub fallback_height: u32,
    /// Fallback dark-red video buffer used when `dead == true`.
    pub dead_frame_buf: Vec<u32>,
    /// Fallback silent stereo audio buffer used when `dead == true`.
    pub dead_audio_buf: Vec<i16>,
    /// Scratch log buffer for dead-state single-shot error reporting.
    pub dead_logs: Vec<LogLine>,
    /// 64 KiB `RETRO_MEMORY_SAVE_RAM` buffer backing `SharedObject.getLocal` (TSD 2.5, M5.5).
    pub save_ram: Vec<u8>,
    /// Whether the next `retro_run` should inject a test panic (`fault-inject` testing).
    pub inject_fault_next_run: bool,
}

impl Default for CoreState {
    fn default() -> Self {
        Self::new()
    }
}

impl CoreState {
    /// Creates a new uninitialized [`CoreState`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            initialized: false,
            dead: false,
            panic_logged: false,
            panic_message: None,
            can_dupe: false,
            port0_device: 1,
            system_dir: None,
            save_dir: None,
            options: Options {
                scale: swfr_player::ScaleOption::Scale1x,
                quality: swfr_player::QualityOption::Medium,
                pad_mode: swfr_player::PadMode::Cursor,
                cursor_speed: 5,
                script_timeout_secs: 2,
                trace_to_log: true,
            },
            input_tracker: InputTracker::new(),
            player: None,
            fallback_width: DEFAULT_STAGE_WIDTH,
            fallback_height: DEFAULT_STAGE_HEIGHT,
            dead_frame_buf: Vec::new(),
            dead_audio_buf: Vec::new(),
            dead_logs: Vec::new(),
            save_ram: Vec::new(),
            inject_fault_next_run: false,
        }
    }

    /// Initializes the core (`retro_init`).
    pub fn init(&mut self) {
        self.initialized = true;
        self.dead = false;
        self.panic_logged = false;
        self.panic_message = None;
        self.inject_fault_next_run = false;
        if self.save_ram.len() != 64 * 1024 {
            self.save_ram = vec![0_u8; 64 * 1024];
            self.save_ram[0..4].copy_from_slice(b"SWFS");
        }
    }

    /// Tears down the core (`retro_deinit`).
    pub fn deinit(&mut self) {
        self.player = None;
        self.initialized = false;
        self.dead = false;
        self.panic_logged = false;
        self.panic_message = None;
        self.dead_frame_buf.clear();
        self.dead_audio_buf.clear();
        self.dead_logs.clear();
        self.save_ram.clear();
        self.inject_fault_next_run = false;
    }

    /// Loads SWF bytes into a new [`Player`] instance (`retro_load_game`).
    ///
    /// # Errors
    ///
    /// Returns an error string if the input is empty, exceeds limits, or has a malformed header.
    pub fn load_game(
        &mut self,
        swf_bytes: &[u8],
        base_dir: Option<String>,
    ) -> Result<AvInfo, String> {
        self.dead = false;
        self.panic_logged = false;
        self.panic_message = None;
        let mut player =
            Player::new(swf_bytes, base_dir, &self.options).map_err(|err| err.to_string())?;
        if self.save_ram.len() == 64 * 1024 && &self.save_ram[0..4] == b"SWFS" {
            let _ = player.load_save_ram(&self.save_ram);
        }
        let sram = player.save_ram();
        if sram.len() == 64 * 1024 {
            self.save_ram.clear();
            self.save_ram.extend_from_slice(sram);
        }
        let av = player.av_info();
        self.fallback_width = av.width;
        self.fallback_height = av.height;
        self.player = Some(player);
        Ok(av)
    }

    /// Unloads the current game while keeping global core state (`retro_unload_game`).
    pub fn unload_game(&mut self) {
        self.player = None;
    }

    /// Resets the loaded player back to frame 1 (`retro_reset`).
    pub fn reset(&mut self) {
        if let Some(player) = self.player.as_mut() {
            player.reset();
        }
    }

    /// Returns the current [`AvInfo`], or default stub geometry if no game is loaded.
    #[must_use]
    pub fn av_info(&self) -> AvInfo {
        if let Some(player) = self.player.as_ref() {
            player.av_info()
        } else {
            AvInfo {
                width: DEFAULT_STAGE_WIDTH,
                height: DEFAULT_STAGE_HEIGHT,
                max_width: DEFAULT_STAGE_WIDTH.saturating_mul(4),
                max_height: DEFAULT_STAGE_HEIGHT.saturating_mul(4),
                aspect_ratio: 4.0 / 3.0,
                fps: DEFAULT_FPS,
                sample_rate: f64::from(SAMPLE_RATE_HZ),
            }
        }
    }

    /// Marks the core as dead after catching a panic at the FFI boundary (TSD 2.5).
    pub fn mark_dead(&mut self, msg: String) {
        self.dead = true;
        self.panic_message = Some(msg);
        let pixel_count = usize::try_from(
            self.fallback_width
                .max(1)
                .saturating_mul(self.fallback_height.max(1)),
        )
        .unwrap_or(320 * 240);
        self.dead_frame_buf.resize(pixel_count, DEAD_FRAME_XRGB);
        fill_dead_frame(&mut self.dead_frame_buf);
        // 735 stereo frames (44100 / 60) = 1470 silent i16 samples.
        self.dead_audio_buf.resize(735 * 2, 0);
        self.dead_audio_buf.fill(0);
    }

    /// Runs one frame (`retro_run`), returning video, audio, and logs.
    pub fn step_frame(&mut self, raw_input: &RawInputSnapshot) -> CoreStepOutput<'_> {
        if self.dead {
            self.dead_logs.clear();
            if !self.panic_logged {
                self.panic_logged = true;
                let msg = self
                    .panic_message
                    .as_deref()
                    .unwrap_or("unknown panic caught at FFI boundary");
                self.dead_logs.push(LogLine {
                    level: swfr_player::LogLevel::Error,
                    message: format!("swfr core marked dead after panic: {msg}"),
                });
            }
            return CoreStepOutput {
                video_pixels: Some(&self.dead_frame_buf),
                width: self.fallback_width.max(1),
                height: self.fallback_height.max(1),
                audio_samples: &self.dead_audio_buf,
                logs: &self.dead_logs,
            };
        }

        let av = self.av_info();
        let input = self.input_tracker.build_frame_input(
            raw_input,
            av.width,
            av.height,
            self.options.pad_mode,
            self.options.cursor_speed,
        );

        if let Some(player) = self.player.as_mut() {
            let out = player.run_frame(&input);
            let sram = out.logs; // borrow check: copy save_ram after borrowing out
            let _ = sram;
            let (video_pixels, width, height) = match out.video {
                Some(frame) => (
                    Some(frame.xrgb.as_slice()),
                    frame.width.max(1),
                    frame.height.max(1),
                ),
                None => (None, av.width.max(1), av.height.max(1)),
            };
            CoreStepOutput {
                video_pixels,
                width,
                height,
                audio_samples: out.audio,
                logs: out.logs,
            }
        } else {
            self.dead_logs.clear();
            self.dead_audio_buf.resize(735 * 2, 0);
            self.dead_audio_buf.fill(0);
            CoreStepOutput {
                video_pixels: None,
                width: self.fallback_width.max(1),
                height: self.fallback_height.max(1),
                audio_samples: &self.dead_audio_buf,
                logs: &self.dead_logs,
            }
        }
    }
}
