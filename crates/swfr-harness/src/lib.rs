//! Development CLI and headless verification harness for `swfr` (TSD 2.6, 2.12 M0).

use swfr_player::{AvInfo, Options, Player, PlayerError};

/// Formats an [`AvInfo`] structure into a human-readable summary string.
#[must_use]
pub fn format_av_info(av: &AvInfo) -> String {
    format!(
        "width={} height={} max_width={} max_height={} aspect_ratio={:.4} fps={:.2} sample_rate={:.0}",
        av.width, av.height, av.max_width, av.max_height, av.aspect_ratio, av.fps, av.sample_rate
    )
}

/// Loads `swf_bytes` into a [`Player`] and returns its [`AvInfo`].
///
/// # Errors
///
/// Returns [`PlayerError`] if `Player::new` fails.
pub fn inspect_av_info(swf_bytes: &[u8]) -> Result<AvInfo, PlayerError> {
    let player = Player::new(swf_bytes, None, &Options::default())?;
    Ok(player.av_info())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harness_formats_stub_av_info() {
        let av = inspect_av_info(b"SWFR_STUB").unwrap_or_else(|_| unreachable!());
        let text = format_av_info(&av);
        assert!(text.contains("width=320 height=240"));
        assert!(text.contains("fps=60.00"));
        assert!(text.contains("sample_rate=44100"));
    }
}
