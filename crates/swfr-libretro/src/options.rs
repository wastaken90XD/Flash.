//! Core option definitions and value parsing for `swfr_libretro` (TSD 2.5).

use swfr_player::{Options, PadMode, QualityOption, ScaleOption};

/// Key for the resolution scale option.
pub const OPT_SCALE_KEY: &str = "swfr_scale";
/// Key for the rendering quality option.
pub const OPT_QUALITY_KEY: &str = "swfr_quality";
/// Key for the joypad mapping mode option.
pub const OPT_PAD_MODE_KEY: &str = "swfr_pad_mode";
/// Key for the cursor speed option.
pub const OPT_CURSOR_SPEED_KEY: &str = "swfr_cursor_speed";
/// Key for the script timeout option.
pub const OPT_SCRIPT_TIMEOUT_KEY: &str = "swfr_script_timeout";
/// Key for the trace-to-log option.
pub const OPT_TRACE_TO_LOG_KEY: &str = "swfr_trace_to_log";

/// All option keys queried by the core in declaration order.
pub const ALL_OPTION_KEYS: [&str; 6] = [
    OPT_SCALE_KEY,
    OPT_QUALITY_KEY,
    OPT_PAD_MODE_KEY,
    OPT_CURSOR_SPEED_KEY,
    OPT_SCRIPT_TIMEOUT_KEY,
    OPT_TRACE_TO_LOG_KEY,
];

/// Applies a single `(key, value)` pair returned by `RETRO_ENVIRONMENT_GET_VARIABLE` to `opts`.
pub fn apply_option_kv(opts: &mut Options, key: &str, value: &str) {
    match key {
        OPT_SCALE_KEY => {
            opts.scale = match value {
                "2x" => ScaleOption::Scale2x,
                "3x" => ScaleOption::Scale3x,
                "4x" => ScaleOption::Scale4x,
                _ => ScaleOption::Scale1x,
            };
        }
        OPT_QUALITY_KEY => {
            opts.quality = match value {
                "low" => QualityOption::Low,
                "high" => QualityOption::High,
                _ => QualityOption::Medium,
            };
        }
        OPT_PAD_MODE_KEY => {
            opts.pad_mode = match value {
                "keys" => PadMode::Keys,
                _ => PadMode::Cursor,
            };
        }
        OPT_CURSOR_SPEED_KEY => {
            if let Ok(speed) = value.parse::<u8>() {
                opts.cursor_speed = speed.clamp(1, 10);
            }
        }
        OPT_SCRIPT_TIMEOUT_KEY => {
            opts.script_timeout_secs = match value {
                "1" => 1,
                "5" => 5,
                "15" => 15,
                _ => 2,
            };
        }
        OPT_TRACE_TO_LOG_KEY => {
            opts.trace_to_log = !matches!(value, "off" | "false" | "0");
        }
        _ => {}
    }
}
