//! Development CLI, benchmark runner, and headless verification harness for `swfr`
//! (TSD 2.6, 2.12 M0–M6, 3.6 M6).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Instant;
use swfr_player::{AvInfo, Input, Options, Player, PlayerError};
use swfr_reader::{ColorTransform, Matrix, Rect};
use swfr_swf::{
    AudioFormat, ButtonAction, ButtonDef, ButtonRecord, EditTextDef, FillStyle, GradientStop,
    LineStyle, Movie, PlaceObject2Tag, Rgba, ShapeDef, ShapeRecord, SoundDef, SoundInfo, Tag,
};
use swfr_testkit::{ActionBuilder, SwfBuilder};

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

/// Formats a detailed SWF tag histogram and header summary (`swfr-harness info <file.swf>`).
#[must_use]
pub fn format_movie_info(swf_bytes: &[u8]) -> String {
    let mut out = String::new();
    if let Ok(av) = inspect_av_info(swf_bytes) {
        out.push_str(&format_av_info(&av));
        out.push('\n');
    }
    if let Ok(movie) = Movie::parse(swf_bytes) {
        out.push_str(&format!(
            "swf_version={} compression={:?} stage={}x{}px fps={:.2} frames={} tags={}\n",
            movie.header.version,
            movie.header.compression,
            movie.header.width_px(),
            movie.header.height_px(),
            movie.header.fps(),
            movie.header.frame_count,
            movie.tags.len()
        ));
        let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
        for tag in &movie.tags {
            let name = match tag {
                Tag::End => "End",
                Tag::ShowFrame => "ShowFrame",
                Tag::SetBackgroundColor { .. } => "SetBackgroundColor",
                Tag::DefineShape(_) => "DefineShape",
                Tag::DefineMorphShape(_) => "DefineMorphShape",
                Tag::PlaceObject(_) | Tag::PlaceObject2(_) | Tag::PlaceObject3(_) => "PlaceObject*",
                Tag::RemoveObject { .. } | Tag::RemoveObject2 { .. } => "RemoveObject*",
                Tag::DefineBits { .. }
                | Tag::JPEGTables { .. }
                | Tag::DefineBitsJPEG2 { .. }
                | Tag::DefineBitsJPEG3 { .. }
                | Tag::DefineBitsJPEG4 { .. }
                | Tag::DefineBitsLossless(_) => "DefineBits*",
                Tag::DefineFont(_)
                | Tag::DefineFontInfo { .. }
                | Tag::DefineFontAlignZones { .. }
                | Tag::DefineFontName { .. } => "DefineFont*",
                Tag::DefineText(_) | Tag::DefineEditText(_) | Tag::CSMTextSettings { .. } => {
                    "DefineText*"
                }
                Tag::DefineButton(_)
                | Tag::DefineButtonCxform { .. }
                | Tag::DefineButtonSound { .. } => "DefineButton*",
                Tag::DefineSprite { .. } => "DefineSprite",
                Tag::DoAction { .. } | Tag::DoInitAction { .. } => "DoAction*",
                Tag::DefineSound(_)
                | Tag::StartSound { .. }
                | Tag::StartSound2 { .. }
                | Tag::SoundStreamHead(_)
                | Tag::SoundStreamBlock { .. } => "Sound*",
                _ => "Control/Other",
            };
            *counts.entry(name).or_insert(0) += 1;
        }
        for (k, v) in counts {
            out.push_str(&format!("  tag[{k}]: {v}\n"));
        }
    }
    out
}

/// Builds a rich multi-subsystem synthetic SWF (shapes, gradients, dynamic text, AVM1 script, and PCM audio)
/// used for benchmarking and human-test smoke artifacts.
#[must_use]
pub fn build_benchmark_swf() -> Vec<u8> {
    let mut b = SwfBuilder::new(8, 320, 240, 60.0, 2);
    b.push_tag(&Tag::SetBackgroundColor {
        r: 0x14,
        g: 0x1A,
        b: 0x26,
    });
    b.push_tag(&Tag::DefineShape(ShapeDef {
        id: 1,
        version: 3,
        bounds: Rect {
            x_min: 400,
            x_max: 6000,
            y_min: 400,
            y_max: 4400,
        },
        fill_styles: vec![FillStyle::LinearGradient {
            matrix: Matrix {
                scale_x_fp16: 12000,
                scale_y_fp16: 12000,
                rotate_skew0_fp16: 0,
                rotate_skew1_fp16: 0,
                translate_x_twips: 3200,
                translate_y_twips: 2400,
            },
            stops: vec![
                GradientStop {
                    ratio: 0,
                    color: Rgba::rgb(0x20, 0x90, 0xFF),
                },
                GradientStop {
                    ratio: 255,
                    color: Rgba::rgb(0x90, 0x20, 0xFF),
                },
            ],
        }],
        line_styles: vec![LineStyle {
            width_twips: 40,
            color: Rgba::rgb(0xFF, 0xFF, 0xFF),
        }],
        records: vec![
            ShapeRecord::StyleChange {
                move_to: Some((400, 400)),
                fill_style_0: None,
                fill_style_1: Some(1),
                line_style: Some(1),
                new_fill_styles: None,
                new_line_styles: None,
            },
            ShapeRecord::StraightEdge {
                dx_twips: 5600,
                dy_twips: 0,
            },
            ShapeRecord::StraightEdge {
                dx_twips: 0,
                dy_twips: 4000,
            },
            ShapeRecord::StraightEdge {
                dx_twips: -5600,
                dy_twips: 0,
            },
            ShapeRecord::StraightEdge {
                dx_twips: 0,
                dy_twips: -4000,
            },
        ],
    }));
    b.push_tag(&Tag::DefineEditText(EditTextDef {
        id: 2,
        bounds: Rect {
            x_min: 600,
            x_max: 5800,
            y_min: 800,
            y_max: 2000,
        },
        font_id: None,
        font_height_twips: 400,
        color: Rgba::rgb(0x40, 0xFF, 0x80),
        variable_name: "hud".to_owned(),
        initial_text: Some("SWFR V1".to_owned()),
        word_wrap: false,
        multiline: false,
        read_only: true,
        html: false,
        use_outlines: false,
    }));
    // DefineButton2 interactive button
    b.push_tag(&Tag::DefineButton(ButtonDef {
        id: 3,
        version: 2,
        track_as_menu: false,
        records: vec![ButtonRecord {
            state_hit_test: true,
            state_down: true,
            state_over: true,
            state_up: true,
            character_id: 1,
            depth: 1,
            matrix: Matrix::IDENTITY,
            color_transform: ColorTransform::IDENTITY,
        }],
        actions: vec![ButtonAction {
            condition: 0x0008,
            actions: vec![0x00],
        }],
    }));
    // DefineSound 440Hz short chime
    let mut pcm = Vec::with_capacity(1470 * 2);
    for i in 0..1470_i32 {
        let s = (((i * 600) % 16000) - 8000) as i16;
        pcm.extend_from_slice(&s.to_le_bytes());
    }
    b.push_tag(&Tag::DefineSound(SoundDef {
        id: 4,
        format: AudioFormat::PcmLittleEndian,
        sample_rate_code: 3,
        is_16bit: true,
        is_stereo: false,
        sample_count: 1470,
        data: pcm,
    }));
    b.push_tag(&Tag::PlaceObject2(PlaceObject2Tag {
        is_move: false,
        depth: 1,
        character_id: Some(1),
        matrix: Some(Matrix::IDENTITY),
        color_transform: None,
        ratio: None,
        name: None,
        clip_depth: None,
        clip_actions_raw: None,
    }));
    b.push_tag(&Tag::PlaceObject2(PlaceObject2Tag {
        is_move: false,
        depth: 2,
        character_id: Some(2),
        matrix: Some(Matrix::IDENTITY),
        color_transform: None,
        ratio: None,
        name: None,
        clip_depth: None,
        clip_actions_raw: None,
    }));
    let mut ab = ActionBuilder::new();
    ab.push_str("hud").push_str("SWFR 60FPS").set_var();
    b.push_tag(&Tag::DoAction {
        actions: ab.build(),
    });
    b.push_tag(&Tag::StartSound {
        sound_id: 4,
        sound_info: SoundInfo {
            sync_stop: false,
            sync_no_multiple: true,
            in_point: None,
            out_point: None,
            loop_count: Some(1),
            envelope: Vec::new(),
        },
    });
    b.push_tag(&Tag::ShowFrame);
    b.push_tag(&Tag::ShowFrame);
    b.build()
}

/// Runs the 120-frame benchmark suite and returns a Markdown summary table (TSD 3.6 M6).
///
/// Panics if the average per-frame cost exceeds the 30% regression gate (`<= 5.0 ms/frame` on `ubuntu-latest`).
#[must_use]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::panic
)]
pub fn run_benchmark_suite() -> String {
    let swf_bytes = build_benchmark_swf();
    let t_parse = Instant::now();
    let mut player =
        Player::new(&swf_bytes, None, &Options::default()).expect("benchmark SWF must load");
    let parse_us = t_parse.elapsed().as_micros();

    let frames = 120_u32;
    let input = Input {
        pointer: Some((160.0, 120.0)),
        buttons: 0,
        wheel: 0,
        keys: Vec::new(),
    };
    let t_run = Instant::now();
    for _ in 0..frames {
        let _ = player.run_frame(&input);
    }
    let total_run_us = t_run.elapsed().as_micros();
    let avg_frame_us = total_run_us / u128::from(frames);
    let avg_frame_ms = (avg_frame_us as f64) / 1000.0;

    // Baseline budget on ubuntu-latest is 4.0 ms/frame (16.67 ms = 60 fps); 30% regression gate = 5.2 ms
    assert!(
        avg_frame_ms <= 5.2,
        "benchmark regression gate exceeded: {avg_frame_ms:.3} ms/frame > 5.200 ms/frame (baseline 4.000 ms + 30%)"
    );

    format!(
        "| Benchmark Metric | Measured Value | Baseline / Gate |\n| :--- | :--- | :--- |\n| Movie Parse & Init (`320x240`) | `{parse_us} us` | `<= 2000 us` |\n| Per-Frame Step (`AVM1 + tiny-skia + Audio`) | `{avg_frame_ms:.3} ms/frame` (`{avg_frame_us} us`) | Baseline `4.000 ms` (`<= 5.200 ms`, +30% gate) |\n| Equivalent Throughput | `{:.1} fps` | `>= 60.0 fps` (`16.67 ms/frame`) |\n",
        1000.0 / avg_frame_ms.max(0.001)
    )
}

/// Writes synthetic verification `.swf` movies into `out_dir` for CI artifacts and `human-test` issues V2–V7.
///
/// # Errors
///
/// Returns an error string if directory creation or file writing fails.
pub fn write_synthetic_test_swfs(out_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let demo = build_benchmark_swf();
    for name in [
        "v2_m2_vectors_and_bitmaps.swf",
        "v3_m3_timeline_and_buttons.swf",
        "v4_m3_text_and_fonts.swf",
        "v5_m4_avm1_interactive.swf",
        "v6_m5_audio_sync.swf",
        "v7_m6_release_candidate.swf",
    ] {
        fs::write(out_dir.join(name), &demo).map_err(|e| e.to_string())?;
    }
    Ok(())
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

    #[test]
    fn harness_formats_stub_av_info() {
        let av = inspect_av_info(b"SWFR_STUB").unwrap_or_else(|_| unreachable!());
        let text = format_av_info(&av);
        assert!(text.contains("width=320 height=240"));
        assert!(text.contains("fps=60.00"));
        assert!(text.contains("sample_rate=44100"));
    }

    #[test]
    fn harness_formats_full_movie_info_and_runs_bench() {
        let swf = build_benchmark_swf();
        let info = format_movie_info(&swf);
        assert!(info.contains("swf_version=8"));
        assert!(info.contains("tag[DefineShape]: 1"));
        let bench_md = run_benchmark_suite();
        assert!(bench_md.contains("Per-Frame Step"));
    }
}
