//! M2 & M3 golden rendering tests (`crates/swfr-render/tests/render_goldens.rs`).
//! Enforces TSD 2.13 tolerance (per-channel <= 2, <= 1% outlier pixels) and TSD 3.7
//! provisional golden coordinate sanity checks.

use std::path::Path;
use swfr_reader::{ColorTransform, Matrix, Rect};
use swfr_render::Renderer;
use swfr_scene::Stage;
use swfr_swf::{
    ButtonAction, ButtonDef, ButtonRecord, EditTextDef, FillStyle, GradientStop, LineStyle,
    LosslessBitmapDef, MorphShapeDef, Movie, PlaceObject2Tag, Rgba, ShapeDef, ShapeRecord, Tag,
};
use swfr_testkit::{verify_or_create_golden, SwfBuilder};

fn rect_records(
    x0: i32,
    y0: i32,
    w: i32,
    h: i32,
    fill1: u32,
    line: Option<u32>,
) -> Vec<ShapeRecord> {
    vec![
        ShapeRecord::StyleChange {
            move_to: Some((x0, y0)),
            fill_style_0: None,
            fill_style_1: Some(fill1),
            line_style: line,
            new_fill_styles: None,
            new_line_styles: None,
        },
        ShapeRecord::StraightEdge {
            dx_twips: w,
            dy_twips: 0,
        },
        ShapeRecord::StraightEdge {
            dx_twips: 0,
            dy_twips: h,
        },
        ShapeRecord::StraightEdge {
            dx_twips: -w,
            dy_twips: 0,
        },
        ShapeRecord::StraightEdge {
            dx_twips: 0,
            dy_twips: -h,
        },
    ]
}

fn check_golden(name: &str, note: &str, width: u32, height: u32, xrgb: &[u32]) {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let approved_dir = manifest_dir.join("tests/golden");
    let provisional_dir = manifest_dir.join("tests/golden-provisional");
    verify_or_create_golden(
        &approved_dir,
        &provisional_dir,
        name,
        note,
        width,
        height,
        xrgb,
        false,
        true,
    )
    .expect("golden verification must succeed");
}

#[test]
fn render_m2_solid_rect_and_dirty_rect_skip() {
    let mut b = SwfBuilder::new(8, 64, 64, 60.0, 1);
    b.push_tag(&Tag::SetBackgroundColor {
        r: 0x10,
        g: 0x20,
        b: 0x30,
    });
    b.push_tag(&Tag::DefineShape(ShapeDef {
        id: 1,
        version: 3,
        bounds: Rect {
            x_min: 320,
            x_max: 960,
            y_min: 320,
            y_max: 960,
        },
        fill_styles: vec![FillStyle::Solid(Rgba::rgb(0xE0, 0x20, 0x20))],
        line_styles: vec![LineStyle {
            width_twips: 40,
            color: Rgba::rgb(0xFF, 0xFF, 0xFF),
        }],
        records: rect_records(320, 320, 640, 640, 1, Some(1)),
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
    b.push_tag(&Tag::ShowFrame);
    let movie = Movie::parse(&b.build()).expect("valid SWF");
    let mut stage = Stage::from_movie(&movie);
    let _ = stage.step_frame();

    let mut renderer = Renderer::new(stage.width_px, stage.height_px, 1);
    assert!(renderer.render_stage(&mut stage, 1));
    // Second call without stage mutations must return false (dirty-rect / can_dupe skip)
    assert!(!renderer.render_stage(&mut stage, 1));

    let fb = renderer.xrgb8888();
    // Sanity check: top-left corner (4, 4) is background #102030, center (32, 32) is red #E02020
    assert_eq!(fb[4 * 64 + 4], 0x0010_2030);
    assert_eq!(fb[32 * 64 + 32], 0x00E0_2020);

    check_golden(
        "m2_solid_rect",
        "64x64 dark blue stage (#102030) with centered 32x32 red square (#E02020) and white stroke",
        64,
        64,
        fb,
    );
}

#[test]
fn render_m2_linear_and_radial_gradients() {
    // 1. Linear gradient
    let mut b_lin = SwfBuilder::new(8, 64, 64, 60.0, 1);
    b_lin.push_tag(&Tag::SetBackgroundColor {
        r: 0x18,
        g: 0x18,
        b: 0x20,
    });
    let grad_mat = Matrix {
        scale_x_fp16: 3200,
        scale_y_fp16: 3200,
        rotate_skew0_fp16: 0,
        rotate_skew1_fp16: 0,
        translate_x_twips: 640,
        translate_y_twips: 640,
    };
    b_lin.push_tag(&Tag::DefineShape(ShapeDef {
        id: 1,
        version: 3,
        bounds: Rect {
            x_min: 240,
            x_max: 1040,
            y_min: 240,
            y_max: 1040,
        },
        fill_styles: vec![FillStyle::LinearGradient {
            matrix: grad_mat,
            stops: vec![
                GradientStop {
                    ratio: 0,
                    color: Rgba::rgb(0x00, 0xFF, 0x40),
                },
                GradientStop {
                    ratio: 255,
                    color: Rgba::rgb(0x00, 0x40, 0xFF),
                },
            ],
        }],
        line_styles: Vec::new(),
        records: rect_records(240, 240, 800, 800, 1, None),
    }));
    b_lin.push_tag(&Tag::PlaceObject2(PlaceObject2Tag {
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
    b_lin.push_tag(&Tag::ShowFrame);
    let movie_lin = Movie::parse(&b_lin.build()).expect("valid linear gradient SWF");
    let mut stage_lin = Stage::from_movie(&movie_lin);
    let _ = stage_lin.step_frame();
    let mut renderer = Renderer::new(64, 64, 1);
    renderer.render_stage(&mut stage_lin, 1);
    let fb_lin = renderer.xrgb8888();
    // Sanity check: left side of square (x=14, y=32) has high green, right side (x=50, y=32) has high blue
    let left_px = fb_lin[32 * 64 + 14];
    let right_px = fb_lin[32 * 64 + 50];
    assert!(((left_px >> 8) & 0xFF) > ((right_px >> 8) & 0xFF));
    assert!((right_px & 0xFF) > (left_px & 0xFF));
    check_golden(
        "m2_linear_gradient",
        "64x64 charcoal stage with 40x40 left-to-right green-to-blue linear gradient square",
        64,
        64,
        fb_lin,
    );

    // 2. Radial gradient
    let mut b_rad = SwfBuilder::new(8, 64, 64, 60.0, 1);
    b_rad.push_tag(&Tag::SetBackgroundColor {
        r: 0x10,
        g: 0x10,
        b: 0x18,
    });
    b_rad.push_tag(&Tag::DefineShape(ShapeDef {
        id: 1,
        version: 3,
        bounds: Rect {
            x_min: 200,
            x_max: 1080,
            y_min: 200,
            y_max: 1080,
        },
        fill_styles: vec![FillStyle::RadialGradient {
            matrix: grad_mat,
            stops: vec![
                GradientStop {
                    ratio: 0,
                    color: Rgba::rgb(0xFF, 0xFF, 0x00),
                },
                GradientStop {
                    ratio: 255,
                    color: Rgba::rgb(0xFF, 0x00, 0x80),
                },
            ],
            focal_point_q8: 0,
        }],
        line_styles: Vec::new(),
        records: rect_records(200, 200, 880, 880, 1, None),
    }));
    b_rad.push_tag(&Tag::PlaceObject2(PlaceObject2Tag {
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
    b_rad.push_tag(&Tag::ShowFrame);
    let movie_rad = Movie::parse(&b_rad.build()).expect("valid radial gradient SWF");
    let mut stage_rad = Stage::from_movie(&movie_rad);
    let _ = stage_rad.step_frame();
    renderer.render_stage(&mut stage_rad, 1);
    let fb_rad = renderer.xrgb8888();
    // Center (32, 32) is yellow-ish (high green), edge (12, 32) is magenta-ish (low green)
    assert!(((fb_rad[32 * 64 + 32] >> 8) & 0xFF) > ((fb_rad[32 * 64 + 12] >> 8) & 0xFF));
    check_golden(
        "m2_radial_gradient",
        "64x64 midnight stage with 44x44 yellow-to-magenta radial gradient square",
        64,
        64,
        fb_rad,
    );
}

#[test]
fn render_m2_lossless_bitmap_clip_mask_and_morph() {
    // 1. Lossless bitmap fill
    let mut raw_pixels = Vec::new();
    for y in 0..4 {
        for x in 0..4 {
            let is_red = ((x / 2) + (y / 2)) % 2 == 0;
            if is_red {
                raw_pixels.extend_from_slice(&[255, 220, 40, 40]);
            } else {
                raw_pixels.extend_from_slice(&[255, 40, 220, 80]);
            }
        }
    }
    let zlib_data = miniz_oxide::deflate::compress_to_vec_zlib(&raw_pixels, 6);
    let mut b_bmp = SwfBuilder::new(8, 64, 64, 60.0, 1);
    b_bmp.push_tag(&Tag::SetBackgroundColor {
        r: 0x20,
        g: 0x20,
        b: 0x20,
    });
    b_bmp.push_tag(&Tag::DefineBitsLossless(LosslessBitmapDef {
        id: 10,
        version: 2,
        format: 5,
        width: 4,
        height: 4,
        color_table_size: None,
        zlib_data,
    }));
    let bmp_mat = Matrix {
        scale_x_fp16: 20 * 65536,
        scale_y_fp16: 20 * 65536,
        rotate_skew0_fp16: 0,
        rotate_skew1_fp16: 0,
        translate_x_twips: 240,
        translate_y_twips: 240,
    };
    b_bmp.push_tag(&Tag::DefineShape(ShapeDef {
        id: 1,
        version: 3,
        bounds: Rect {
            x_min: 240,
            x_max: 1040,
            y_min: 240,
            y_max: 1040,
        },
        fill_styles: vec![FillStyle::Bitmap {
            bitmap_id: 10,
            matrix: bmp_mat,
            repeating: true,
            smoothed: false,
        }],
        line_styles: Vec::new(),
        records: rect_records(240, 240, 800, 800, 1, None),
    }));
    b_bmp.push_tag(&Tag::PlaceObject2(PlaceObject2Tag {
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
    b_bmp.push_tag(&Tag::ShowFrame);
    let movie_bmp = Movie::parse(&b_bmp.build()).expect("valid bitmap SWF");
    let mut stage_bmp = Stage::from_movie(&movie_bmp);
    let _ = stage_bmp.step_frame();
    let mut renderer = Renderer::new(64, 64, 1);
    renderer.render_stage(&mut stage_bmp, 1);
    check_golden(
        "m2_lossless_bitmap_fill",
        "64x64 gray stage with 40x40 repeating red/green checkerboard DefineBitsLossless2 fill",
        64,
        64,
        renderer.xrgb8888(),
    );

    // 2. Clip depth mask
    let mut b_clip = SwfBuilder::new(8, 64, 64, 60.0, 1);
    b_clip.push_tag(&Tag::SetBackgroundColor {
        r: 0x30,
        g: 0x30,
        b: 0x30,
    });
    // Shape 1: Narrow vertical mask window (x: 22..42px -> 440..840 twips, y: 8..56px -> 160..1120 twips)
    b_clip.push_tag(&Tag::DefineShape(ShapeDef {
        id: 1,
        version: 3,
        bounds: Rect {
            x_min: 440,
            x_max: 840,
            y_min: 160,
            y_max: 1120,
        },
        fill_styles: vec![FillStyle::Solid(Rgba::rgb(255, 255, 255))],
        line_styles: Vec::new(),
        records: rect_records(440, 160, 400, 960, 1, None),
    }));
    // Shape 2: Wide horizontal green bar (x: 4..60px -> 80..1200 twips, y: 20..44px -> 400..880 twips)
    b_clip.push_tag(&Tag::DefineShape(ShapeDef {
        id: 2,
        version: 3,
        bounds: Rect {
            x_min: 80,
            x_max: 1200,
            y_min: 400,
            y_max: 880,
        },
        fill_styles: vec![FillStyle::Solid(Rgba::rgb(0x00, 0xE0, 0x40))],
        line_styles: Vec::new(),
        records: rect_records(80, 400, 1120, 480, 1, None),
    }));
    b_clip.push_tag(&Tag::PlaceObject2(PlaceObject2Tag {
        is_move: false,
        depth: 1,
        character_id: Some(1),
        matrix: Some(Matrix::IDENTITY),
        color_transform: None,
        ratio: None,
        name: None,
        clip_depth: Some(3),
        clip_actions_raw: None,
    }));
    b_clip.push_tag(&Tag::PlaceObject2(PlaceObject2Tag {
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
    b_clip.push_tag(&Tag::ShowFrame);
    let movie_clip = Movie::parse(&b_clip.build()).expect("valid clip SWF");
    let mut stage_clip = Stage::from_movie(&movie_clip);
    let _ = stage_clip.step_frame();
    renderer.render_stage(&mut stage_clip, 1);
    let fb_clip = renderer.xrgb8888();
    // Outside mask at (10, 32) is background #303030; inside mask at (32, 32) is green #00E040
    assert_eq!(fb_clip[32 * 64 + 10], 0x0030_3030);
    assert_eq!(fb_clip[32 * 64 + 32], 0x0000_E040);
    check_golden(
        "m2_clip_depth_mask",
        "64x64 dark gray stage with green bar at depth 2 clipped by vertical window mask at depth 1",
        64,
        64,
        fb_clip,
    );

    // 3. Morph shape at ratio 32768 (50%)
    let mut b_morph = SwfBuilder::new(8, 64, 64, 60.0, 1);
    b_morph.push_tag(&Tag::SetBackgroundColor {
        r: 0x15,
        g: 0x15,
        b: 0x25,
    });
    b_morph.push_tag(&Tag::DefineMorphShape(MorphShapeDef {
        id: 1,
        version: 1,
        start_bounds: Rect {
            x_min: 160,
            x_max: 640,
            y_min: 320,
            y_max: 960,
        },
        end_bounds: Rect {
            x_min: 640,
            x_max: 1120,
            y_min: 320,
            y_max: 960,
        },
        start_fill_styles: vec![FillStyle::Solid(Rgba::rgb(0xFF, 0x00, 0x00))],
        end_fill_styles: vec![FillStyle::Solid(Rgba::rgb(0x00, 0x00, 0xFF))],
        start_line_styles: Vec::new(),
        end_line_styles: Vec::new(),
        start_records: rect_records(160, 320, 480, 640, 1, None),
        end_records: rect_records(640, 320, 480, 640, 1, None),
    }));
    b_morph.push_tag(&Tag::PlaceObject2(PlaceObject2Tag {
        is_move: false,
        depth: 1,
        character_id: Some(1),
        matrix: Some(Matrix::IDENTITY),
        color_transform: None,
        ratio: Some(32768),
        name: None,
        clip_depth: None,
        clip_actions_raw: None,
    }));
    b_morph.push_tag(&Tag::ShowFrame);
    let movie_morph = Movie::parse(&b_morph.build()).expect("valid morph SWF");
    let mut stage_morph = Stage::from_movie(&movie_morph);
    let _ = stage_morph.step_frame();
    renderer.render_stage(&mut stage_morph, 1);
    let fb_morph = renderer.xrgb8888();
    assert_eq!(fb_morph[32 * 64 + 32], 0x007F_007F);
    check_golden(
        "m2_morph_shape_mid",
        "64x64 navy stage with DefineMorphShape at ratio 32768 producing centered purple rectangle",
        64,
        64,
        fb_morph,
    );
}

#[test]
fn render_m3_button_states_and_dynamic_edit_text() {
    let mut b = SwfBuilder::new(8, 64, 64, 60.0, 1);
    b.push_tag(&Tag::SetBackgroundColor {
        r: 0x1E,
        g: 0x24,
        b: 0x30,
    });
    // Shape 1: Up state button rect (blue)
    b.push_tag(&Tag::DefineShape(ShapeDef {
        id: 1,
        version: 3,
        bounds: Rect {
            x_min: 160,
            x_max: 1120,
            y_min: 120,
            y_max: 440,
        },
        fill_styles: vec![FillStyle::Solid(Rgba::rgb(0x20, 0x60, 0xCC))],
        line_styles: Vec::new(),
        records: rect_records(160, 120, 960, 320, 1, None),
    }));
    // Shape 2: Over state button rect (amber #FFAA00)
    b.push_tag(&Tag::DefineShape(ShapeDef {
        id: 2,
        version: 3,
        bounds: Rect {
            x_min: 160,
            x_max: 1120,
            y_min: 120,
            y_max: 440,
        },
        fill_styles: vec![FillStyle::Solid(Rgba::rgb(0xFF, 0xAA, 0x00))],
        line_styles: Vec::new(),
        records: rect_records(160, 120, 960, 320, 1, None),
    }));
    b.push_tag(&Tag::DefineButton(ButtonDef {
        id: 10,
        version: 2,
        track_as_menu: false,
        records: vec![
            ButtonRecord {
                state_hit_test: true,
                state_down: false,
                state_over: false,
                state_up: true,
                character_id: 1,
                depth: 1,
                matrix: Matrix::IDENTITY,
                color_transform: ColorTransform::IDENTITY,
            },
            ButtonRecord {
                state_hit_test: false,
                state_down: true,
                state_over: true,
                state_up: false,
                character_id: 2,
                depth: 1,
                matrix: Matrix::IDENTITY,
                color_transform: ColorTransform::IDENTITY,
            },
        ],
        actions: vec![ButtonAction {
            condition: 0x0008, // OverDownToOverUp (click release inside)
            actions: vec![0x07, 0x00],
        }],
    }));
    b.push_tag(&Tag::DefineEditText(EditTextDef {
        id: 20,
        bounds: Rect {
            x_min: 160,
            x_max: 1120,
            y_min: 560,
            y_max: 1160,
        },
        font_id: None,
        font_height_twips: 360,
        color: Rgba::rgb(0x40, 0xFF, 0x80),
        variable_name: "score".to_owned(),
        initial_text: Some("00".to_owned()),
        word_wrap: false,
        multiline: false,
        read_only: true,
        html: false,
        use_outlines: false,
    }));
    b.push_tag(&Tag::PlaceObject2(PlaceObject2Tag {
        is_move: false,
        depth: 1,
        character_id: Some(10),
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
        character_id: Some(20),
        matrix: Some(Matrix::IDENTITY),
        color_transform: None,
        ratio: None,
        name: None,
        clip_depth: None,
        clip_actions_raw: None,
    }));
    b.push_tag(&Tag::ShowFrame);

    let movie = Movie::parse(&b.build()).expect("valid button & text SWF");
    let mut stage = Stage::from_movie(&movie);
    let _ = stage.step_frame();

    // Hover pointer over button at (32px, 14px) -> (640, 280) twips
    let hover_ev = stage.update_pointer(640, 280, false);
    assert!(hover_ev.actions.is_empty());
    // Press and release inside button -> triggers OverDownToOverUp (0x0008) action!
    let _ = stage.update_pointer(640, 280, true);
    let click_ev = stage.update_pointer(640, 280, false);
    assert_eq!(click_ev.actions.len(), 1);

    // Update dynamic variable "score" -> "88"
    stage.sync_variable("score", "88");

    let mut renderer = Renderer::new(64, 64, 1);
    renderer.render_stage(&mut stage, 1);
    let fb = renderer.xrgb8888();
    // Button at (32, 14) is in Over state (#FFAA00)
    assert_eq!(fb[14 * 64 + 32], 0x00FF_AA00);

    check_golden(
        "m3_button_and_text",
        "64x64 slate stage with DefineButton2 in Over state (#FFAA00) and DefineEditText '88' in green",
        64,
        64,
        fb,
    );
}
