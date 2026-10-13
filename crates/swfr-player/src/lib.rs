//! Frame-stepping SWF player coordinator integrating `swfr-swf`, `swfr-scene`, `swfr-render`,
//! `swfr-avm1`, and `swfr-audio` (TSD 2.6, 2.11, 2.12 M0–M5).
#![allow(
    clippy::arithmetic_side_effects,
    clippy::collapsible_if,
    clippy::collapsible_match
)]

use swfr_audio::AudioMixer;
use swfr_avm1::Avm1;
use swfr_render::Renderer;
use swfr_scene::{FrameEvents, Stage};
use swfr_swf::{Movie, Tag};

/// Fixed output audio sample rate in Hz (TSD 2.5, 2.8).
pub const SAMPLE_RATE_HZ: u32 = 44_100;

/// Default stub stage width in pixels (TSD 2.12 M0).
pub const DEFAULT_STAGE_WIDTH: u32 = 320;

/// Default stub stage height in pixels (TSD 2.12 M0).
pub const DEFAULT_STAGE_HEIGHT: u32 = 240;

/// Default stub frame rate in frames per second.
pub const DEFAULT_FPS: f64 = 60.0;

/// Maximum total pixel count (`width * height`) allowed for scaled output (TSD 2.5).
pub const MAX_STAGE_PIXELS: u32 = 4_194_304;

/// Safety and resource limits enforced during parsing and execution (TSD 2.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Maximum compressed/raw input file size in bytes (default: 64 MiB).
    pub max_input_file_size: usize,
    /// Maximum decompressed SWF payload size in bytes (default: 64 MiB).
    pub max_decompressed_size: usize,
    /// Maximum number of tags in a movie (default: 2,000,000).
    pub max_tag_count: u32,
    /// Maximum number of edges in a single shape (default: 1,000,000).
    pub max_shape_edges: u32,
    /// Maximum movie clip nesting depth (default: 256).
    pub max_clip_nesting_depth: u32,
    /// Maximum AVM call stack depth (default: 256).
    pub max_avm_call_depth: u32,
    /// Maximum bitmap width or height in pixels (default: 8,192).
    pub max_bitmap_side: u32,
    /// Maximum total decoded bitmap memory in bytes (default: 64 MiB).
    pub max_bitmap_memory: usize,
    /// Maximum script string length in bytes (default: 16 MiB).
    pub max_string_len: usize,
    /// Maximum script array length in elements (default: 1,048,576).
    pub max_array_len: u32,
    /// Default script execution timeout per frame in seconds (default: 2 s).
    pub script_timeout_secs: u8,
    /// Maximum script heap size on desktop targets in bytes (default: 128 MiB).
    pub script_heap_desktop: usize,
    /// Maximum script heap size on Reference B targets in bytes (default: 48 MiB).
    pub script_heap_reference_b: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_file_size: 64 * 1024 * 1024,
            max_decompressed_size: 64 * 1024 * 1024,
            max_tag_count: 2_000_000,
            max_shape_edges: 1_000_000,
            max_clip_nesting_depth: 256,
            max_avm_call_depth: 256,
            max_bitmap_side: 8_192,
            max_bitmap_memory: 64 * 1024 * 1024,
            max_string_len: 16 * 1024 * 1024,
            max_array_len: 1_048_576,
            script_timeout_secs: 2,
            script_heap_desktop: 128 * 1024 * 1024,
            script_heap_reference_b: 48 * 1024 * 1024,
        }
    }
}

/// Resolution scale factor (`swfr_scale` core option, TSD 2.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleOption {
    /// 1x native stage resolution.
    Scale1x = 1,
    /// 2x stage resolution.
    Scale2x = 2,
    /// 3x stage resolution.
    Scale3x = 3,
    /// 4x stage resolution.
    Scale4x = 4,
}

impl ScaleOption {
    /// Returns the numeric multiplier (`1`, `2`, `3`, or `4`).
    #[must_use]
    pub const fn factor(self) -> u32 {
        self as u32
    }
}

/// Rendering quality level (`swfr_quality` core option, TSD 2.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityOption {
    /// Low quality (anti-aliasing disabled, halved gradient steps).
    Low,
    /// Medium quality (default).
    Medium,
    /// High quality (full anti-aliasing and gradient steps).
    High,
}

/// Gamepad mapping mode (`swfr_pad_mode` core option, TSD 2.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadMode {
    /// Left stick and D-pad move the virtual cursor; South button clicks.
    Cursor,
    /// D-pad sends arrow keys; South sends Space; East/Start send Enter.
    Keys,
}

/// Runtime player options corresponding to `swfr_*` core options (TSD 2.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Requested resolution scale factor (`swfr_scale`, default `1x`).
    pub scale: ScaleOption,
    /// Rendering quality (`swfr_quality`, default `medium`).
    pub quality: QualityOption,
    /// Joypad input mode (`swfr_pad_mode`, default `cursor`).
    pub pad_mode: PadMode,
    /// Cursor speed from `1` to `10` (`swfr_cursor_speed`, default `5`).
    pub cursor_speed: u8,
    /// Script timeout in seconds: `1`, `2`, `5`, or `15` (`swfr_script_timeout`, default `2`).
    pub script_timeout_secs: u8,
    /// Whether `trace()` output is forwarded to the log interface (`swfr_trace_to_log`, default `true`).
    pub trace_to_log: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            scale: ScaleOption::Scale1x,
            quality: QualityOption::Medium,
            pad_mode: PadMode::Cursor,
            cursor_speed: 5,
            script_timeout_secs: 2,
            trace_to_log: true,
        }
    }
}

/// Audio/video geometry and timing parameters reported by [`Player::av_info`] (TSD 2.5, 2.6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AvInfo {
    /// Current scaled frame width in pixels.
    pub width: u32,
    /// Current scaled frame height in pixels.
    pub height: u32,
    /// Maximum possible scaled width in pixels (capped within `MAX_STAGE_PIXELS`).
    pub max_width: u32,
    /// Maximum possible scaled height in pixels (capped within `MAX_STAGE_PIXELS`).
    pub max_height: u32,
    /// Stage aspect ratio (`stage_width / stage_height`), ignoring scale.
    pub aspect_ratio: f32,
    /// Frame rate in frames per second (`1.0..=120.0`).
    pub fps: f64,
    /// Audio sample rate in Hz (`44100.0`).
    pub sample_rate: f64,
}

/// CPU-rendered 32-bit `XRGB8888` (`0x00RRGGBB`) video frame (TSD 2.5, 2.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Packed `0x00RRGGBB` pixel buffer of length `width * height`.
    pub xrgb: Vec<u32>,
}

/// Severity level for diagnostic and `trace()` log lines (TSD 2.5, 2.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// Verbose debugging message.
    Debug,
    /// Normal informational or `[trace]` message.
    Info,
    /// Recoverable warning.
    Warn,
    /// Error message.
    Error,
}

/// A single diagnostic or `[trace]` log entry emitted during a frame (TSD 2.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    /// Severity level.
    pub level: LogLevel,
    /// Formatted message text.
    pub message: String,
}

/// Keyboard event delivered from the frontend (TSD 2.5, 2.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    /// Frontend key code (`retro_key`).
    pub keycode: u32,
    /// UTF-32 character code, or `0` if non-printable.
    pub character: u32,
    /// Key modifier bitmask (`retro_mod`).
    pub modifiers: u16,
    /// `true` if pressed, `false` if released.
    pub down: bool,
}

/// Per-frame input state passed to [`Player::run_frame`] (TSD 2.6).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Input {
    /// Stage-space pointer coordinates `(x, y)` in pixels if active.
    pub pointer: Option<(f32, f32)>,
    /// Mouse/pointer button bitmask (bit 0 = left, bit 1 = right, bit 2 = middle).
    pub buttons: u8,
    /// Mouse wheel delta for this frame.
    pub wheel: i32,
    /// Keyboard events queued since the previous frame.
    pub keys: Vec<KeyEvent>,
}

/// Borrowed output of a single [`Player::run_frame`] step (TSD 2.6).
#[derive(Debug)]
pub struct FrameOut<'a> {
    /// Rendered video frame, or `None` if unchanged since the previous frame.
    pub video: Option<&'a Frame>,
    /// Interleaved stereo 16-bit PCM samples (`frames * 2` elements).
    pub audio: &'a [i16],
    /// Log lines produced during the frame.
    pub logs: &'a [LogLine],
}

/// Errors returned when constructing or loading a [`Player`] (TSD 2.6, 2.11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerError {
    /// The input byte slice was empty.
    EmptyInput,
    /// The input byte slice exceeded [`Limits::max_input_file_size`].
    InputTooLarge {
        /// Actual byte length.
        size: usize,
        /// Configured limit in bytes.
        limit: usize,
    },
    /// The input bytes were explicitly marked as malformed.
    MalformedHeader(&'static str),
}

impl core::fmt::Display for PlayerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "SWF input buffer is empty"),
            Self::InputTooLarge { size, limit } => {
                write!(
                    f,
                    "SWF input size ({size} bytes) exceeds limit ({limit} bytes)"
                )
            }
            Self::MalformedHeader(msg) => write!(f, "malformed SWF header: {msg}"),
        }
    }
}

impl std::error::Error for PlayerError {}

/// SMPTE 8-bar color palette in `0x00RRGGBB` format (White, Yellow, Cyan, Green, Magenta, Red, Blue, Black).
const COLOR_BARS_XRGB: [u32; 8] = [
    0x00FF_FFFF,
    0x00FF_FF00,
    0x0000_FFFF,
    0x0000_FF00,
    0x00FF_00FF,
    0x00FF_0000,
    0x0000_00FF,
    0x0000_0000,
];

#[derive(Debug)]
struct ActiveRuntime {
    movie: Movie,
    stage: Stage,
    renderer: Renderer,
    vm: Avm1,
    audio: AudioMixer,
}

/// Frame-stepping SWF player instance (TSD 2.6).
#[derive(Debug)]
pub struct Player {
    stage_width: u32,
    stage_height: u32,
    effective_scale: u32,
    max_scale: u32,
    fps_q8: u64,
    fps: f64,
    audio_rem_q8: u64,
    tone_sample_idx: u64,
    frame_count: u64,
    options: Options,
    base_dir: Option<String>,
    frame: Frame,
    audio_buf: Vec<i16>,
    logs: Vec<LogLine>,
    runtime: Option<ActiveRuntime>,
}

impl Player {
    /// Parses `swf_bytes` and constructs a new [`Player`] instance (TSD 2.6).
    ///
    /// # Errors
    ///
    /// Returns [`PlayerError`] if `swf_bytes` is empty, exceeds [`Limits::max_input_file_size`],
    /// or contains an explicitly truncated header.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn new(
        swf_bytes: &[u8],
        base_dir: Option<String>,
        opts: &Options,
    ) -> Result<Self, PlayerError> {
        let limits = Limits::default();
        if swf_bytes.is_empty() {
            return Err(PlayerError::EmptyInput);
        }
        if swf_bytes.len() > limits.max_input_file_size {
            return Err(PlayerError::InputTooLarge {
                size: swf_bytes.len(),
                limit: limits.max_input_file_size,
            });
        }

        let mut fps_q8: u64 = 60 * 256;
        let mut parsed_movie: Option<Movie> = None;

        if let Some(sig) = swf_bytes.get(0..3) {
            if sig == b"FWS" || sig == b"CWS" || sig == b"ZWS" {
                if swf_bytes.len() < 8 {
                    return Err(PlayerError::MalformedHeader(
                        "SWF header shorter than 8-byte preamble",
                    ));
                }
                if sig == b"FWS" && swf_bytes.get(8) == Some(&0) {
                    if let (Some(&lo), Some(&hi)) = (swf_bytes.get(9), swf_bytes.get(10)) {
                        let raw_q8 = u64::from(u16::from_le_bytes([lo, hi]));
                        fps_q8 = raw_q8.clamp(256, 120 * 256);
                    }
                }
                if let Ok(m) = Movie::parse(swf_bytes) {
                    if m.header.frame_size.width_twips() > 0 && !m.tags.is_empty() {
                        fps_q8 = u64::from(m.header.frame_rate_q8).clamp(256, 120 * 256);
                        parsed_movie = Some(m);
                    }
                }
            }
        }

        let (stage_width, stage_height, runtime) = if let Some(movie) = parsed_movie {
            let sw = movie.header.width_px();
            let sh = movie.header.height_px();
            let stage = Stage::from_movie(&movie);
            let renderer = Renderer::new(sw, sh, opts.scale.factor().clamp(1, 2));
            let mut vm = Avm1::new(movie.header.version);
            vm.max_ops_per_run = u64::from(opts.script_timeout_secs)
                .saturating_mul(250_000)
                .max(10_000);
            let mut audio = AudioMixer::new();
            for tag in &movie.tags {
                if let Tag::DefineSound(snd) = tag {
                    audio.register_sound(snd);
                }
            }
            (
                sw,
                sh,
                Some(ActiveRuntime {
                    movie,
                    stage,
                    renderer,
                    vm,
                    audio,
                }),
            )
        } else {
            (DEFAULT_STAGE_WIDTH, DEFAULT_STAGE_HEIGHT, None)
        };

        let max_scale = Self::compute_max_scale(stage_width, stage_height);
        let effective_scale = opts.scale.factor().min(max_scale);
        let width = stage_width.saturating_mul(effective_scale);
        let height = stage_height.saturating_mul(effective_scale);
        let pixel_count = usize::try_from(width.saturating_mul(height)).unwrap_or(0);

        let fps = (fps_q8 as f64) / 256.0;
        let max_samples_per_frame = (SAMPLE_RATE_HZ as usize).saturating_mul(2);

        let mut player = Self {
            stage_width,
            stage_height,
            effective_scale,
            max_scale,
            fps_q8,
            fps,
            audio_rem_q8: 0,
            tone_sample_idx: 0,
            frame_count: 0,
            options: opts.clone(),
            base_dir,
            frame: Frame {
                width,
                height,
                xrgb: vec![0; pixel_count],
            },
            audio_buf: Vec::with_capacity(max_samples_per_frame),
            logs: Vec::new(),
            runtime,
        };
        if player.runtime.is_none() {
            player.render_color_bars();
        }
        Ok(player)
    }

    /// Returns the loaded movie's geometry and timing metadata (TSD 2.5, 2.6).
    #[must_use]
    #[allow(clippy::arithmetic_side_effects)]
    pub fn av_info(&self) -> AvInfo {
        let width = self.stage_width.saturating_mul(self.effective_scale);
        let height = self.stage_height.saturating_mul(self.effective_scale);
        let max_width = self.stage_width.saturating_mul(self.max_scale);
        let max_height = self.stage_height.saturating_mul(self.max_scale);
        let aspect_ratio = if self.stage_height > 0 {
            (self.stage_width as f32) / (self.stage_height as f32)
        } else {
            4.0 / 3.0
        };
        AvInfo {
            width,
            height,
            max_width,
            max_height,
            aspect_ratio,
            fps: self.fps,
            sample_rate: f64::from(SAMPLE_RATE_HZ),
        }
    }

    /// Updates runtime options and resizes the frame buffer if `swfr_scale` changed.
    ///
    /// Returns `true` if the output frame dimensions changed (requiring `RETRO_ENVIRONMENT_SET_GEOMETRY`).
    pub fn set_options(&mut self, opts: &Options) -> bool {
        let new_scale = opts.scale.factor().min(self.max_scale);
        let geometry_changed = new_scale != self.effective_scale;
        self.options = opts.clone();
        if let Some(ref mut rt) = self.runtime {
            rt.vm.max_ops_per_run = u64::from(opts.script_timeout_secs)
                .saturating_mul(250_000)
                .max(10_000);
        }
        if geometry_changed {
            self.effective_scale = new_scale;
            let width = self.stage_width.saturating_mul(self.effective_scale);
            let height = self.stage_height.saturating_mul(self.effective_scale);
            let pixel_count = usize::try_from(width.saturating_mul(height)).unwrap_or(0);
            self.frame.width = width;
            self.frame.height = height;
            self.frame.xrgb.resize(pixel_count, 0);
            if let Some(ref mut rt) = self.runtime {
                rt.stage.full_redraw = true;
            } else {
                self.render_color_bars();
            }
        }
        geometry_changed
    }

    /// Returns the base directory associated with the loaded content, if any.
    #[must_use]
    pub fn base_dir(&self) -> Option<&str> {
        self.base_dir.as_deref()
    }

    /// Returns the number of frames executed since load or last [`Player::reset`].
    #[must_use]
    pub const fn frame_index(&self) -> u64 {
        self.frame_count
    }

    /// Advances the player by one frame and produces video, stereo audio, and logs (TSD 2.6).
    #[allow(clippy::arithmetic_side_effects)]
    pub fn run_frame(&mut self, input: &Input) -> FrameOut<'_> {
        self.logs.clear();
        self.frame_count = self.frame_count.saturating_add(1);

        let numerator = self
            .audio_rem_q8
            .saturating_add(u64::from(SAMPLE_RATE_HZ).saturating_mul(256));
        let stereo_frames_u64 = numerator.checked_div(self.fps_q8).unwrap_or(735);
        self.audio_rem_q8 = numerator.checked_rem(self.fps_q8).unwrap_or(0);
        let stereo_frames = usize::try_from(stereo_frames_u64).unwrap_or(735);

        if let Some(ref mut rt) = self.runtime {
            // 1. Deliver keyboard input to AVM1
            for kev in &input.keys {
                rt.vm.set_key_state(kev.keycode, kev.down);
            }

            // 2. Deliver pointer input to Stage and execute any triggered button actions
            if let Some((px, py)) = input.pointer {
                let x_twips = (px * 20.0).round() as i32;
                let y_twips = (py * 20.0).round() as i32;
                let pressed = (input.buttons & 1) != 0;
                let btn_events = rt.stage.update_pointer(x_twips, y_twips, pressed);
                Self::dispatch_events(
                    btn_events,
                    &mut rt.stage,
                    &mut rt.vm,
                    &mut rt.audio,
                    stereo_frames,
                    self.options.trace_to_log,
                    &mut self.logs,
                );
            }

            // 3. Advance timeline by 1 frame and execute frame actions + sound tags
            let frame_events = rt.stage.step_frame();
            Self::dispatch_events(
                frame_events,
                &mut rt.stage,
                &mut rt.vm,
                &mut rt.audio,
                stereo_frames,
                self.options.trace_to_log,
                &mut self.logs,
            );

            // 4. Advance deterministic timers
            let delta_ms = (1000.0 / self.fps.max(1.0)).round() as u64;
            rt.vm.step_timers(&mut rt.stage, delta_ms);
            Self::drain_vm_logs(
                &mut rt.vm,
                &mut rt.audio,
                self.options.trace_to_log,
                &mut self.logs,
            );

            // 5. Render stage at effective scale
            let render_scale = self.effective_scale.clamp(1, 2);
            let _ = rt.renderer.render_stage(&mut rt.stage, render_scale);
            self.frame.width = rt.renderer.width();
            self.frame.height = rt.renderer.height();
            self.frame.xrgb.clear();
            self.frame.xrgb.extend_from_slice(rt.renderer.xrgb8888());

            // 6. Mix 44.1 kHz stereo audio for this frame
            self.audio_buf = rt.audio.mix_interleaved(stereo_frames);

            return FrameOut {
                video: Some(&self.frame),
                audio: &self.audio_buf,
                logs: &self.logs,
            };
        }

        // Stub fallback (SMPTE color bars + 440 Hz sine tone)
        let sample_len = stereo_frames.saturating_mul(2);
        self.audio_buf.clear();
        self.audio_buf.resize(sample_len, 0);

        let two_pi_440 = core::f64::consts::TAU * 440.0 / f64::from(SAMPLE_RATE_HZ);
        for frame_idx in 0..stereo_frames {
            let phase = (self.tone_sample_idx as f64) * two_pi_440;
            let sample = (phase.sin() * 8_000.0) as i16;
            let base = frame_idx.saturating_mul(2);
            if let Some(left) = self.audio_buf.get_mut(base) {
                *left = sample;
            }
            if let Some(right) = self.audio_buf.get_mut(base.saturating_add(1)) {
                *right = sample;
            }
            self.tone_sample_idx = self.tone_sample_idx.wrapping_add(1);
        }

        FrameOut {
            video: Some(&self.frame),
            audio: &self.audio_buf,
            logs: &self.logs,
        }
    }

    /// Resets the player back to frame 1 and clears runtime state (TSD 2.5, 2.6).
    pub fn reset(&mut self) {
        self.audio_rem_q8 = 0;
        self.tone_sample_idx = 0;
        self.frame_count = 0;
        self.logs.clear();
        if let Some(ref mut rt) = self.runtime {
            rt.stage = Stage::from_movie(&rt.movie);
            rt.vm = Avm1::new(rt.movie.header.version);
            rt.vm.max_ops_per_run = u64::from(self.options.script_timeout_secs)
                .saturating_mul(250_000)
                .max(10_000);
            rt.audio = AudioMixer::new();
            for tag in &rt.movie.tags {
                if let Tag::DefineSound(snd) = tag {
                    rt.audio.register_sound(snd);
                }
            }
        } else {
            self.render_color_bars();
        }
    }

    fn dispatch_events(
        events: FrameEvents,
        stage: &mut Stage,
        vm: &mut Avm1,
        audio: &mut AudioMixer,
        stereo_frames: usize,
        trace_to_log: bool,
        logs: &mut Vec<LogLine>,
    ) {
        for head in events.stream_heads {
            audio.set_stream_head(head);
        }
        for block in events.stream_blocks {
            audio.push_stream_block(&block, stereo_frames);
        }
        for (sid, info) in events.start_sounds {
            audio.start_sound(sid, &info);
        }
        for (_, code) in events.actions {
            vm.execute_bytecode(&code, stage);
            Self::drain_vm_logs(vm, audio, trace_to_log, logs);
        }
    }

    fn drain_vm_logs(
        vm: &mut Avm1,
        audio: &mut AudioMixer,
        trace_to_log: bool,
        logs: &mut Vec<LogLine>,
    ) {
        if trace_to_log {
            for t in vm.trace_log.drain(..) {
                logs.push(LogLine {
                    level: LogLevel::Info,
                    message: format!("[trace] {t}"),
                });
            }
        } else {
            vm.trace_log.clear();
        }
        if vm.watchdog_tripped {
            vm.watchdog_tripped = false;
            logs.push(LogLine {
                level: LogLevel::Warn,
                message: "AVM1 instruction watchdog tripped; aborting action block".to_owned(),
            });
        }
        audio.volume_percent = vm.sound_volume;
        if vm.stop_sounds_requested {
            vm.stop_sounds_requested = false;
            audio.stop_all_sounds();
        }
    }

    fn compute_max_scale(stage_width: u32, stage_height: u32) -> u32 {
        let mut best = 1_u32;
        for candidate in 1_u32..=4_u32 {
            let w = stage_width.saturating_mul(candidate);
            let h = stage_height.saturating_mul(candidate);
            if w.saturating_mul(h) <= MAX_STAGE_PIXELS {
                best = candidate;
            }
        }
        best
    }

    fn render_color_bars(&mut self) {
        let width = self.frame.width.max(1);
        let height = self.frame.height;
        for y in 0..height {
            let row_offset = y.saturating_mul(width);
            for x in 0..width {
                let bar_idx = usize::try_from(x.saturating_mul(8).checked_div(width).unwrap_or(0))
                    .unwrap_or(0)
                    .min(7);
                let color = COLOR_BARS_XRGB.get(bar_idx).copied().unwrap_or(0);
                let idx = usize::try_from(row_offset.saturating_add(x)).unwrap_or(0);
                if let Some(pixel) = self.frame.xrgb.get_mut(idx) {
                    *pixel = color;
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]
mod tests {
    use super::*;
    use swfr_testkit::{ActionBuilder, SwfBuilder};

    #[test]
    fn stub_player_renders_320x240_color_bars_and_440hz_tone() {
        let opts = Options::default();
        let mut player = Player::new(b"SWFR_STUB", None, &opts).unwrap_or_else(|_| unreachable!());
        let av = player.av_info();
        assert_eq!(av.width, 320);
        assert_eq!(av.height, 240);
        assert_eq!(av.max_width, 1280);
        assert_eq!(av.max_height, 960);
        assert!((av.fps - 60.0).abs() < f64::EPSILON);
        assert!((av.sample_rate - 44_100.0).abs() < f64::EPSILON);

        let out = player.run_frame(&Input::default());
        let video = out.video.unwrap_or_else(|| unreachable!());
        assert_eq!(video.width, 320);
        assert_eq!(video.height, 240);
        assert_eq!(video.xrgb.len(), 320 * 240);
        assert_eq!(video.xrgb.first().copied(), Some(0x00FF_FFFF));
        assert_eq!(video.xrgb.last().copied(), Some(0x0000_0000));
        assert_eq!(out.audio.len(), 735 * 2);
        assert!(out.audio.iter().any(|&s| s != 0));
    }

    #[test]
    fn fractional_audio_carry_600_runs_at_12fps_equals_2205000_frames() {
        let swf_12fps = b"FWS\x08\x0d\x00\x00\x00\x00\x00\x0c\x01\x00";
        let mut player =
            Player::new(swf_12fps, None, &Options::default()).unwrap_or_else(|_| unreachable!());
        assert!((player.av_info().fps - 12.0).abs() < f64::EPSILON);

        let mut total_stereo_frames: usize = 0;
        let input = Input::default();
        for _ in 0..600 {
            let out = player.run_frame(&input);
            total_stereo_frames = total_stereo_frames.saturating_add(out.audio.len() / 2);
        }
        assert_eq!(total_stereo_frames, 2_205_000);
    }

    #[test]
    fn end_to_end_swf_movie_executes_scene_render_avm1_and_audio() {
        let mut ab = ActionBuilder::new();
        ab.push_str("hello_from_player").trace();
        let mut b = SwfBuilder::new(8, 128, 96, 60.0, 1);
        b.push_tag(&Tag::SetBackgroundColor {
            r: 0x33,
            g: 0x66,
            b: 0x99,
        });
        b.push_tag(&Tag::DoAction {
            actions: ab.build(),
        });
        b.push_tag(&Tag::ShowFrame);
        let swf_bytes = b.build();

        let mut player = Player::new(&swf_bytes, None, &Options::default()).unwrap();
        assert_eq!(player.av_info().width, 128);
        assert_eq!(player.av_info().height, 96);

        let out = player.run_frame(&Input::default());
        let v = out.video.unwrap();
        assert_eq!(v.xrgb[0], 0x0033_6699);
        assert_eq!(out.logs.len(), 1);
        assert_eq!(out.logs[0].message, "[trace] hello_from_player");
    }
}
