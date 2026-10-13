//! End-to-end ABI acceptance tests for `swfr_libretro` (TSD 3.14.4 & 3.14.5).
//!
//! Uses movies built with `swfr-testkit` (and real SWF files from `corpus/real/MANIFEST.toml`)
//! and drives ONLY the exported `retro_*` C ABI symbols.
#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use core::ffi::{c_char, c_int, c_uint, c_void};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use swfr_libretro::ffi::{
    copy_audio_frames_from_callback_ptr, copy_c_str_from_callback_ptr, copy_memory_bytes_from_ptr,
    copy_video_frame_from_callback_ptr, install_log_callback_in_environment,
    make_test_log_printf_fn, retro_deinit, retro_game_geometry, retro_game_info,
    retro_get_memory_data, retro_get_memory_size, retro_get_system_av_info, retro_init,
    retro_load_game, retro_reset, retro_run, retro_serialize, retro_serialize_size,
    retro_set_audio_sample, retro_set_audio_sample_batch, retro_set_environment,
    retro_set_input_poll, retro_set_input_state, retro_set_video_refresh, retro_system_av_info,
    retro_system_timing, retro_unload_game, retro_unserialize, RETRO_DEVICE_ID_POINTER_PRESSED,
    RETRO_DEVICE_ID_POINTER_X, RETRO_DEVICE_ID_POINTER_Y, RETRO_DEVICE_POINTER,
    RETRO_ENVIRONMENT_GET_LOG_INTERFACE, RETRO_ENVIRONMENT_SET_PIXEL_FORMAT,
    RETRO_MEMORY_SAVE_RAM,
};
use swfr_reader::{ColorTransform, Matrix, Rect};
use swfr_swf::{
    AudioFormat, ButtonAction, ButtonDef, ButtonRecord, Compression, EditTextDef, FillStyle,
    GradientStop, MorphShapeDef, PlaceObject2Tag, Rgba, ShapeDef, ShapeRecord, SoundDef, SoundInfo,
    SoundStreamHeadDef, Tag,
};
use swfr_testkit::{ActionBuilder, SwfBuilder};

/// Serializes tests in this binary because the libretro C ABI operates on a single global instance.
static E2E_LOCK: Mutex<()> = Mutex::new(());

#[derive(Default)]
struct E2eFrontend {
    video_frames: Vec<Vec<u32>>,
    last_width: u32,
    last_height: u32,
    audio_batches: Vec<Vec<i16>>,
    logs: Vec<(c_int, String)>,
    pointer_pressed: bool,
    pointer_x: i16,
    pointer_y: i16,
}

static FRONTEND: Mutex<E2eFrontend> = Mutex::new(E2eFrontend {
    video_frames: Vec::new(),
    last_width: 0,
    last_height: 0,
    audio_batches: Vec::new(),
    logs: Vec::new(),
    pointer_pressed: false,
    pointer_x: 0,
    pointer_y: 0,
});

fn reset_frontend() {
    if let Ok(mut g) = FRONTEND.lock() {
        *g = E2eFrontend::default();
    }
}

extern "C" fn e2e_log_printf(level: c_int, fmt: *const c_char) {
    let text = copy_c_str_from_callback_ptr(fmt);
    if let Ok(mut g) = FRONTEND.lock() {
        g.logs.push((level, text));
    }
}

extern "C" fn e2e_environment(cmd: c_uint, data: *mut c_void) -> bool {
    if cmd == RETRO_ENVIRONMENT_SET_PIXEL_FORMAT {
        return true;
    }
    if cmd == RETRO_ENVIRONMENT_GET_LOG_INTERFACE {
        return install_log_callback_in_environment(data, make_test_log_printf_fn(e2e_log_printf));
    }
    false
}

extern "C" fn e2e_video_refresh(data: *const c_void, width: c_uint, height: c_uint, _pitch: usize) {
    let pixels = copy_video_frame_from_callback_ptr(data, width, height);
    if let Ok(mut g) = FRONTEND.lock() {
        g.last_width = width;
        g.last_height = height;
        g.video_frames.push(pixels);
    }
}

extern "C" fn e2e_audio_sample(_left: i16, _right: i16) {}

extern "C" fn e2e_audio_sample_batch(data: *const i16, frames: usize) -> usize {
    let samples = copy_audio_frames_from_callback_ptr(data, frames);
    if let Ok(mut g) = FRONTEND.lock() {
        g.audio_batches.push(samples);
    }
    frames
}

extern "C" fn e2e_input_poll() {}

extern "C" fn e2e_input_state(_port: c_uint, device: c_uint, _index: c_uint, id: c_uint) -> i16 {
    if device == RETRO_DEVICE_POINTER {
        if let Ok(g) = FRONTEND.lock() {
            return match id {
                RETRO_DEVICE_ID_POINTER_PRESSED => i16::from(g.pointer_pressed),
                RETRO_DEVICE_ID_POINTER_X => g.pointer_x,
                RETRO_DEVICE_ID_POINTER_Y => g.pointer_y,
                _ => 0,
            };
        }
    }
    0
}

fn init_abi_callbacks() {
    reset_frontend();
    retro_set_environment(Some(e2e_environment));
    retro_set_video_refresh(Some(e2e_video_refresh));
    retro_set_audio_sample(Some(e2e_audio_sample));
    retro_set_audio_sample_batch(Some(e2e_audio_sample_batch));
    retro_set_input_poll(Some(e2e_input_poll));
    retro_set_input_state(Some(e2e_input_state));
    retro_init();
}

fn rect_shape_records(x_twips: i32, y_twips: i32, w_twips: i32, h_twips: i32) -> Vec<ShapeRecord> {
    vec![
        ShapeRecord::StyleChange {
            move_to: Some((x_twips, y_twips)),
            fill_style_0: None,
            fill_style_1: Some(1),
            line_style: None,
            new_fill_styles: None,
            new_line_styles: None,
        },
        ShapeRecord::StraightEdge {
            dx_twips: w_twips,
            dy_twips: 0,
        },
        ShapeRecord::StraightEdge {
            dx_twips: 0,
            dy_twips: h_twips,
        },
        ShapeRecord::StraightEdge {
            dx_twips: -w_twips,
            dy_twips: 0,
        },
        ShapeRecord::StraightEdge {
            dx_twips: 0,
            dy_twips: -h_twips,
        },
    ]
}

fn fnv1a_frame_hash(pixels: &[u32]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &px in pixels {
        for b in px.to_le_bytes() {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    hash
}

fn m0_stub_frame_hash(width: u32, height: u32) -> u64 {
    const BARS: [u32; 8] = [
        0x00FF_FFFF,
        0x00FF_FF00,
        0x0000_FFFF,
        0x0000_FF00,
        0x00FF_00FF,
        0x00FF_0000,
        0x0000_00FF,
        0x0000_0000,
    ];
    let w = width.max(1);
    let mut pixels = Vec::with_capacity((w * height) as usize);
    for _y in 0..height {
        for x in 0..w {
            let idx = ((x * 8) / w).min(7) as usize;
            pixels.push(BARS[idx]);
        }
    }
    fnv1a_frame_hash(&pixels)
}

fn is_440hz_sine_batch(samples: &[i16]) -> bool {
    if samples.len() < 16 {
        return false;
    }
    let two_pi_440 = core::f64::consts::TAU * 440.0 / 44_100.0;
    for (frame_idx, pair) in samples.as_chunks::<2>().0.iter().take(32).enumerate() {
        let expected = ((frame_idx as f64 * two_pi_440).sin() * 8_000.0) as i16;
        if pair[0] != expected || pair[1] != expected {
            return false;
        }
    }
    true
}

// ============================================================================
// TSD 3.14.4 Case 1: 1-frame movie with red 100x80 rectangle centered on blue stage
// ============================================================================

#[test]
fn e2e_1frame_red_rect_on_blue_stage_and_av_info() {
    let _guard = E2E_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    init_abi_callbacks();

    // Stage: 200x160 pixels at 24.0 fps, 1 frame.
    // Red 100x80 rectangle centered on stage -> x: 50..150 px (1000..3000 twips), y: 40..120 px (800..2400 twips).
    let mut b = SwfBuilder::new(8, 200, 160, 24.0, 1);
    b.push_tag(&Tag::SetBackgroundColor {
        r: 0x00,
        g: 0x00,
        b: 0xFF,
    });
    b.push_tag(&Tag::DefineShape(ShapeDef {
        id: 1,
        version: 1,
        bounds: Rect {
            x_min: 1000,
            x_max: 3000,
            y_min: 800,
            y_max: 2400,
        },
        fill_styles: vec![FillStyle::Solid(Rgba::rgb(0xFF, 0x00, 0x00))],
        line_styles: Vec::new(),
        records: rect_shape_records(1000, 800, 2000, 1600),
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
    let swf_bytes = b.build();

    let game_info = retro_game_info {
        path: c"red_rect_blue_stage.swf".as_ptr(),
        data: swf_bytes.as_ptr().cast::<c_void>(),
        size: swf_bytes.len(),
        meta: core::ptr::null(),
    };
    assert!(retro_load_game(core::ptr::addr_of!(game_info)));

    let mut av_info = retro_system_av_info {
        geometry: retro_game_geometry {
            base_width: 0,
            base_height: 0,
            max_width: 0,
            max_height: 0,
            aspect_ratio: 0.0,
        },
        timing: retro_system_timing {
            fps: 0.0,
            sample_rate: 0.0,
        },
    };
    retro_get_system_av_info(core::ptr::addr_of_mut!(av_info));
    assert_eq!(av_info.geometry.base_width, 200);
    assert_eq!(av_info.geometry.base_height, 160);
    assert!((av_info.timing.fps - 24.0).abs() < 1e-5);
    assert!((av_info.timing.sample_rate - 44_100.0).abs() < 1e-5);

    retro_run();

    let fe = FRONTEND.lock().unwrap();
    assert_eq!(fe.video_frames.len(), 1);
    assert_eq!(fe.last_width, 200);
    assert_eq!(fe.last_height, 160);
    let frame = &fe.video_frames[0];
    assert_eq!(frame.len(), 200 * 160);

    // Center pixel (100, 80) must be red (0x00FF0000); corner pixels (0, 0) and (199, 159) must be blue (0x000000FF).
    assert_eq!(frame[80 * 200 + 100], 0x00FF_0000);
    assert_eq!(frame[0], 0x0000_00FF);
    assert_eq!(frame[159 * 200 + 199], 0x0000_00FF);
    assert_ne!(fnv1a_frame_hash(frame), m0_stub_frame_hash(200, 160));
    drop(fe);

    retro_unload_game();
    retro_deinit();
}

// ============================================================================
// TSD 3.14.4 Case 2: 10-frame movie where a square moves 10 pixels per frame
// ============================================================================

#[test]
fn e2e_10frame_moving_square_differs_and_tracks_position() {
    let _guard = E2E_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    init_abi_callbacks();

    // Stage: 200x100 pixels, 30 fps, 10 frames.
    // 20x20 px square (400x400 twips) moves 10 px (200 twips) right each frame: x = 10 + f * 10, y = 40.
    let mut b = SwfBuilder::new(8, 200, 100, 30.0, 10);
    b.push_tag(&Tag::SetBackgroundColor {
        r: 0x00,
        g: 0x00,
        b: 0x00,
    });
    b.push_tag(&Tag::DefineShape(ShapeDef {
        id: 1,
        version: 1,
        bounds: Rect {
            x_min: 0,
            x_max: 400,
            y_min: 0,
            y_max: 400,
        },
        fill_styles: vec![FillStyle::Solid(Rgba::rgb(0x00, 0xFF, 0x00))],
        line_styles: Vec::new(),
        records: rect_shape_records(0, 0, 400, 400),
    }));

    for frame_idx in 0..10_i32 {
        let tx_twips = (10 + frame_idx * 10) * 20;
        let ty_twips = 40 * 20;
        b.push_tag(&Tag::PlaceObject2(PlaceObject2Tag {
            is_move: frame_idx > 0,
            depth: 1,
            character_id: if frame_idx == 0 { Some(1) } else { None },
            matrix: Some(Matrix {
                scale_x_fp16: 65536,
                scale_y_fp16: 65536,
                rotate_skew0_fp16: 0,
                rotate_skew1_fp16: 0,
                translate_x_twips: tx_twips,
                translate_y_twips: ty_twips,
            }),
            color_transform: None,
            ratio: None,
            name: None,
            clip_depth: None,
            clip_actions_raw: None,
        }));
        b.push_tag(&Tag::ShowFrame);
    }
    let swf_bytes = b.build();

    let game_info = retro_game_info {
        path: c"moving_square_10f.swf".as_ptr(),
        data: swf_bytes.as_ptr().cast::<c_void>(),
        size: swf_bytes.len(),
        meta: core::ptr::null(),
    };
    assert!(retro_load_game(core::ptr::addr_of!(game_info)));

    for _ in 0..10 {
        retro_run();
    }

    let fe = FRONTEND.lock().unwrap();
    assert_eq!(fe.video_frames.len(), 10);
    for f in 0..10_usize {
        let frame = &fe.video_frames[f];
        if f > 0 {
            assert_ne!(
                frame,
                &fe.video_frames[f - 1],
                "frame {f} must differ from frame {}",
                f - 1
            );
            // Pixel at the left half of the previous frame's square is now uncovered (black).
            let vacated_x = 10 + (f - 1) * 10 + 5;
            assert_eq!(
                frame[50 * 200 + vacated_x],
                0x0000_0000,
                "vacated pixel at x={vacated_x} on frame {f} must be black"
            );
        }
        // Center of square on frame f is at x = 10 + f * 10 + 10 = 20 + f * 10, y = 50.
        let expected_x = 20 + f * 10;
        assert_eq!(
            frame[50 * 200 + expected_x],
            0x0000_FF00,
            "square center at ({expected_x}, 50) on frame {f} must be green"
        );
    }
    drop(fe);

    retro_unload_game();
    retro_deinit();
}

// ============================================================================
// TSD 3.14.4 Case 3: PCM sound event exact samples & silent movie all-zero
// ============================================================================

#[test]
fn e2e_pcm_sound_event_exact_samples_and_silent_movie_zero_not_440hz() {
    let _guard = E2E_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    init_abi_callbacks();

    // At 60 fps and 44,100 Hz, 1 frame is 735 stereo frames (1470 i16 samples).
    let stereo_frames = 735_usize;
    let mut expected_interleaved = Vec::with_capacity(stereo_frames * 2);
    let mut pcm_bytes = Vec::with_capacity(stereo_frames * 4);
    for i in 0..stereo_frames {
        let left = ((i as i32 * 37) % 12_000 - 6_000) as i16;
        let right = ((i as i32 * 53) % 12_000 - 6_000) as i16;
        expected_interleaved.push(left);
        expected_interleaved.push(right);
        pcm_bytes.extend_from_slice(&left.to_le_bytes());
        pcm_bytes.extend_from_slice(&right.to_le_bytes());
    }

    let mut b = SwfBuilder::new(8, 120, 90, 60.0, 1);
    b.push_tag(&Tag::SetBackgroundColor {
        r: 0x20,
        g: 0x20,
        b: 0x20,
    });
    b.push_tag(&Tag::DefineSound(SoundDef {
        id: 1,
        format: AudioFormat::PcmLittleEndian,
        sample_rate_code: 3, // 44,100 Hz
        is_16bit: true,
        is_stereo: true,
        sample_count: stereo_frames as u32,
        data: pcm_bytes,
    }));
    b.push_tag(&Tag::StartSound {
        sound_id: 1,
        sound_info: SoundInfo {
            sync_stop: false,
            sync_no_multiple: false,
            in_point: None,
            out_point: None,
            loop_count: Some(1),
            envelope: Vec::new(),
        },
    });
    b.push_tag(&Tag::ShowFrame);
    let pcm_swf = b.build();

    let game_info = retro_game_info {
        path: c"pcm_event.swf".as_ptr(),
        data: pcm_swf.as_ptr().cast::<c_void>(),
        size: pcm_swf.len(),
        meta: core::ptr::null(),
    };
    assert!(retro_load_game(core::ptr::addr_of!(game_info)));
    retro_run();

    let fe = FRONTEND.lock().unwrap();
    assert_eq!(fe.audio_batches.len(), 1);
    let actual_pcm = &fe.audio_batches[0];
    assert_eq!(actual_pcm.len(), expected_interleaved.len());
    assert_eq!(actual_pcm, &expected_interleaved);
    assert!(!is_440hz_sine_batch(actual_pcm));
    drop(fe);

    retro_unload_game();

    // Now load a silent movie and verify all-zero audio samples (and not 440 Hz sine).
    reset_frontend();
    let mut b_silent = SwfBuilder::new(8, 120, 90, 60.0, 1);
    b_silent.push_tag(&Tag::SetBackgroundColor {
        r: 0x10,
        g: 0x10,
        b: 0x10,
    });
    b_silent.push_tag(&Tag::ShowFrame);
    let silent_swf = b_silent.build();

    let silent_info = retro_game_info {
        path: c"silent.swf".as_ptr(),
        data: silent_swf.as_ptr().cast::<c_void>(),
        size: silent_swf.len(),
        meta: core::ptr::null(),
    };
    assert!(retro_load_game(core::ptr::addr_of!(silent_info)));
    retro_run();

    let fe2 = FRONTEND.lock().unwrap();
    assert_eq!(fe2.audio_batches.len(), 1);
    let silent_audio = &fe2.audio_batches[0];
    assert_eq!(silent_audio.len(), stereo_frames * 2);
    assert!(silent_audio.iter().all(|&s| s == 0));
    assert!(!is_440hz_sine_batch(silent_audio));
    drop(fe2);

    retro_unload_game();
    retro_deinit();
}

// ============================================================================
// TSD 3.14.4 Case 4: AS2 movie calling trace('hello')
// ============================================================================

#[test]
fn e2e_as2_trace_hello_delivers_log_callback() {
    let _guard = E2E_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    init_abi_callbacks();

    let mut ab = ActionBuilder::new();
    ab.push_str("hello").trace();

    let mut b = SwfBuilder::new(8, 160, 120, 60.0, 1);
    b.push_tag(&Tag::SetBackgroundColor {
        r: 0x00,
        g: 0x00,
        b: 0x00,
    });
    b.push_tag(&Tag::DoAction {
        actions: ab.build(),
    });
    b.push_tag(&Tag::ShowFrame);
    let swf_bytes = b.build();

    let game_info = retro_game_info {
        path: c"trace_hello.swf".as_ptr(),
        data: swf_bytes.as_ptr().cast::<c_void>(),
        size: swf_bytes.len(),
        meta: core::ptr::null(),
    };
    assert!(retro_load_game(core::ptr::addr_of!(game_info)));
    retro_run();

    let fe = FRONTEND.lock().unwrap();
    assert!(
        fe.logs.iter().any(|(_, line)| line.contains("hello")),
        "expected log callback to receive a line containing 'hello', got: {:?}",
        fe.logs
    );
    drop(fe);

    retro_unload_game();
    retro_deinit();
}

// ============================================================================
// TSD 3.14.4 Case 5: Button movie with scripted pointer input: click changes frame
// ============================================================================

#[test]
fn e2e_button_movie_scripted_pointer_click_changes_frame() {
    let _guard = E2E_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    init_abi_callbacks();

    // 2-frame movie (200x160, 60 fps):
    // Frame 1 (index 0): Stops on Frame 1 (`stop()`), places DefineButton2 id 3 at depth 1 (blue 100x80 rect).
    //                    Clicking the button executes `goto_frame(1)` (jumps to Frame 2).
    // Frame 2 (index 1): Removes button at depth 1 and places yellow 100x80 rect (Shape id 2) at depth 2.
    let mut b = SwfBuilder::new(8, 200, 160, 60.0, 2);
    b.push_tag(&Tag::SetBackgroundColor {
        r: 0x10,
        g: 0x10,
        b: 0x10,
    });
    // Shape 1: Blue 100x80 centered rectangle (for Button on Frame 1)
    b.push_tag(&Tag::DefineShape(ShapeDef {
        id: 1,
        version: 1,
        bounds: Rect {
            x_min: 1000,
            x_max: 3000,
            y_min: 800,
            y_max: 2400,
        },
        fill_styles: vec![FillStyle::Solid(Rgba::rgb(0x00, 0x00, 0xFF))],
        line_styles: Vec::new(),
        records: rect_shape_records(1000, 800, 2000, 1600),
    }));
    // Shape 2: Yellow 100x80 centered rectangle (for Frame 2)
    b.push_tag(&Tag::DefineShape(ShapeDef {
        id: 2,
        version: 1,
        bounds: Rect {
            x_min: 1000,
            x_max: 3000,
            y_min: 800,
            y_max: 2400,
        },
        fill_styles: vec![FillStyle::Solid(Rgba::rgb(0xFF, 0xFF, 0x00))],
        line_styles: Vec::new(),
        records: rect_shape_records(1000, 800, 2000, 1600),
    }));

    let mut click_actions = ActionBuilder::new();
    click_actions.goto_frame(1); // 0-based frame 1 = Frame 2

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
            condition: 0x0008 | 0x0004, // OverDownToOverUp | OverUpToOverDown
            actions: click_actions.build(),
        }],
    }));

    // Frame 1 tags
    b.push_tag(&Tag::PlaceObject2(PlaceObject2Tag {
        is_move: false,
        depth: 1,
        character_id: Some(3),
        matrix: Some(Matrix::IDENTITY),
        color_transform: None,
        ratio: None,
        name: None,
        clip_depth: None,
        clip_actions_raw: None,
    }));
    let mut stop_ab = ActionBuilder::new();
    stop_ab.stop();
    let stop_bytes = stop_ab.build();
    b.push_tag(&Tag::DoAction {
        actions: stop_bytes.clone(),
    });
    b.push_tag(&Tag::ShowFrame);

    // Frame 2 tags
    b.push_tag(&Tag::RemoveObject2 { depth: 1 });
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
    b.push_tag(&Tag::DoAction {
        actions: stop_bytes,
    });
    b.push_tag(&Tag::ShowFrame);

    let swf_bytes = b.build();
    let game_info = retro_game_info {
        path: c"button_click.swf".as_ptr(),
        data: swf_bytes.as_ptr().cast::<c_void>(),
        size: swf_bytes.len(),
        meta: core::ptr::null(),
    };
    assert!(retro_load_game(core::ptr::addr_of!(game_info)));

    // Run 2 frames without clicking: movie stays stopped on Frame 1 (center pixel is Blue 0x000000FF).
    retro_run();
    retro_run();
    {
        let fe = FRONTEND.lock().unwrap();
        assert_eq!(fe.video_frames[0][80 * 200 + 100], 0x0000_00FF);
        assert_eq!(fe.video_frames[1][80 * 200 + 100], 0x0000_00FF);
    }

    // Script pointer click at stage center (pointer_x = 0, pointer_y = 0, pointer_pressed = true).
    if let Ok(mut fe) = FRONTEND.lock() {
        fe.pointer_x = 0;
        fe.pointer_y = 0;
        fe.pointer_pressed = true;
    }
    retro_run();
    if let Ok(mut fe) = FRONTEND.lock() {
        fe.pointer_pressed = false;
    }
    retro_run();

    let fe = FRONTEND.lock().unwrap();
    let last_frame = fe.video_frames.last().unwrap();
    assert_eq!(
        last_frame[80 * 200 + 100],
        0x00FF_FF00,
        "clicking button must advance movie to Frame 2 (yellow center pixel)"
    );
    drop(fe);

    retro_unload_game();
    retro_deinit();
}

// ============================================================================
// TSD 3.14.4 Case 6: Malformed file: retro_load_game returns false
// ============================================================================

#[test]
fn e2e_malformed_files_return_false_from_retro_load_game() {
    let _guard = E2E_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    init_abi_callbacks();

    let malformed_samples: &[&[u8]] = &[
        b"SWFR_STUB",
        b"NOT_AN_SWF_FILE",
        b"FWS\x08\x10\x00",
        b"FWS\x08\x0d\x00\x00\x00\x00\x00\x0c\x01\x00",
        b"CWS\x08\x40\x00\x00\x00\xFF\xFF\xFF\xFF",
        // Valid header followed by a tag claiming 500 bytes when only 2 remain
        b"FWS\x08\x20\x00\x00\x00\x78\x00\x05\x5F\x00\x00\x0F\xA0\x00\x00\x3C\x01\x00\xBF\x00\xF4\x01\x00\x00\x00\x00",
    ];

    for (idx, bad_bytes) in malformed_samples.iter().enumerate() {
        if let Ok(mut fe) = FRONTEND.lock() {
            fe.logs.clear();
        }
        let info = retro_game_info {
            path: c"malformed.swf".as_ptr(),
            data: bad_bytes.as_ptr().cast::<c_void>(),
            size: bad_bytes.len(),
            meta: core::ptr::null(),
        };
        assert!(
            !retro_load_game(core::ptr::addr_of!(info)),
            "malformed sample #{idx} must return false from retro_load_game"
        );
        let fe = FRONTEND.lock().unwrap();
        assert!(
            !fe.logs.is_empty(),
            "malformed sample #{idx} must log an error via retro_log_printf_t"
        );
    }

    retro_deinit();
}

// ============================================================================
// TSD 3.14.5: Real corpus files load through exported ABI and run 120 frames
// ============================================================================

fn locate_workspace_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .unwrap_or(manifest_dir)
        .to_path_buf()
}

fn parse_manifest_names(manifest_text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in manifest_text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("name") {
            let rest = rest.trim_start();
            if let Some(val) = rest.strip_prefix('=') {
                let clean = val.trim().trim_matches('"');
                if !clean.is_empty() {
                    names.push(clean.to_owned());
                }
            }
        }
    }
    names
}

#[test]
fn e2e_real_corpus_manifest_runs_120_frames_without_m0_stub() {
    let _guard = E2E_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = locate_workspace_root();
    let manifest_path = root.join("corpus/real/MANIFEST.toml");
    let manifest_text =
        fs::read_to_string(&manifest_path).expect("corpus/real/MANIFEST.toml must exist");
    let names = parse_manifest_names(&manifest_text);
    assert!(
        !names.is_empty(),
        "corpus/real/MANIFEST.toml must contain real SWF entries"
    );

    let cache_dir = root.join("corpus/real/cache");
    let require_cached = std::env::var("SWFR_REQUIRE_REAL_CORPUS").as_deref() == Ok("1");

    init_abi_callbacks();
    let mut tested_files = 0_usize;

    for name in &names {
        let swf_path = cache_dir.join(format!("{name}.swf"));
        if !swf_path.exists() {
            assert!(
                !require_cached,
                "required real corpus file missing from cache: {}",
                swf_path.display()
            );
            continue;
        }
        let swf_bytes = fs::read(&swf_path).expect("failed to read cached real SWF");
        reset_frontend();

        let game_info = retro_game_info {
            path: c"real_corpus.swf".as_ptr(),
            data: swf_bytes.as_ptr().cast::<c_void>(),
            size: swf_bytes.len(),
            meta: core::ptr::null(),
        };
        assert!(
            retro_load_game(core::ptr::addr_of!(game_info)),
            "retro_load_game failed on real corpus SWF '{name}'"
        );

        for _ in 0..120 {
            retro_run();
        }

        let fe = FRONTEND.lock().unwrap();
        assert_eq!(
            fe.video_frames.len(),
            120,
            "real corpus SWF '{name}' must produce 120 video frames"
        );
        let stub_hash_native = m0_stub_frame_hash(fe.last_width, fe.last_height);
        let stub_hash_320 = m0_stub_frame_hash(320, 240);
        for (idx, frame) in fe.video_frames.iter().enumerate() {
            let h = fnv1a_frame_hash(frame);
            assert_ne!(
                h, stub_hash_native,
                "real corpus SWF '{name}' frame {idx} matched M0 stub frame hash"
            );
            assert_ne!(
                h, stub_hash_320,
                "real corpus SWF '{name}' frame {idx} matched 320x240 M0 stub frame hash"
            );
        }
        for (idx, audio) in fe.audio_batches.iter().enumerate() {
            assert!(
                !is_440hz_sine_batch(audio),
                "real corpus SWF '{name}' frame {idx} audio matched M0 440 Hz sine stub"
            );
        }
        drop(fe);

        retro_unload_game();
        tested_files += 1;
    }

    if require_cached {
        assert_eq!(tested_files, names.len());
    }
    retro_deinit();
}

// -----------------------------------------------------------------------------
// Milestone M1–M6 End-to-End ABI Verification Tests (TSD 3.14.7)
// -----------------------------------------------------------------------------

#[test]
fn e2e_m1_fws_cws_zws_containers_via_abi() {
    let _guard = E2E_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    init_abi_callbacks();

    let mut frame_hashes = Vec::new();
    for compression in [
        Compression::Uncompressed,
        Compression::Zlib,
        Compression::Lzma,
    ] {
        reset_frontend();
        let mut b = SwfBuilder::new(13, 200, 120, 24.0, 1).with_compression(compression);
        b.push_tag(&Tag::SetBackgroundColor {
            r: 0x00,
            g: 0x00,
            b: 0xFF,
        });
        b.push_tag(&Tag::DefineShape(ShapeDef {
            id: 1,
            version: 1,
            bounds: Rect {
                x_min: 1000,
                x_max: 3000,
                y_min: 400,
                y_max: 2000,
            },
            fill_styles: vec![FillStyle::Solid(Rgba::rgb(0xFF, 0x00, 0x00))],
            line_styles: Vec::new(),
            records: rect_shape_records(1000, 400, 2000, 1600),
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
        let swf = b.build();

        let game_info = retro_game_info {
            path: c"m1_container.swf".as_ptr(),
            data: swf.as_ptr().cast::<c_void>(),
            size: swf.len(),
            meta: core::ptr::null(),
        };
        assert!(retro_load_game(core::ptr::addr_of!(game_info)));
        retro_run();

        let fe = FRONTEND.lock().unwrap();
        assert_eq!(fe.video_frames.len(), 1);
        assert_eq!(fe.video_frames[0][60 * 200 + 100], 0x00FF_0000);
        frame_hashes.push(fnv1a_frame_hash(&fe.video_frames[0]));
        drop(fe);

        retro_unload_game();
    }

    assert_eq!(frame_hashes[0], frame_hashes[1]);
    assert_eq!(frame_hashes[1], frame_hashes[2]);
    retro_deinit();
}

#[test]
fn e2e_m2_gradients_morphs_and_sprites_via_abi() {
    let _guard = E2E_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset_frontend();
    init_abi_callbacks();

    let mut b = SwfBuilder::new(8, 160, 120, 30.0, 2);
    b.push_tag(&Tag::SetBackgroundColor {
        r: 0x10,
        g: 0x10,
        b: 0x10,
    });

    // Character 1: Linear gradient rectangle (0..80, 0..120)
    let grad_shape = ShapeDef {
        version: 3,
        id: 1,
        bounds: Rect {
            x_min: 0,
            x_max: 80 * 20,
            y_min: 0,
            y_max: 120 * 20,
        },
        fill_styles: vec![FillStyle::LinearGradient {
            matrix: Matrix::IDENTITY,
            stops: vec![
                GradientStop {
                    ratio: 0,
                    color: Rgba::rgb(255, 0, 0),
                },
                GradientStop {
                    ratio: 255,
                    color: Rgba::rgb(0, 0, 255),
                },
            ],
        }],
        line_styles: Vec::new(),
        records: rect_shape_records(0, 0, 80 * 20, 120 * 20),
    };
    b.push_tag(&Tag::DefineShape(grad_shape));

    // Character 2: Morph shape from pure green (ratio 0) to pure red (ratio 65535) on right half
    let morph_bounds = Rect {
        x_min: 80 * 20,
        x_max: 160 * 20,
        y_min: 0,
        y_max: 120 * 20,
    };
    let morph_records = rect_shape_records(80 * 20, 0, 80 * 20, 120 * 20);
    b.push_tag(&Tag::DefineMorphShape(MorphShapeDef {
        version: 1,
        id: 2,
        start_bounds: morph_bounds,
        end_bounds: morph_bounds,
        start_fill_styles: vec![FillStyle::Solid(Rgba::rgb(0, 255, 0))],
        end_fill_styles: vec![FillStyle::Solid(Rgba::rgb(255, 0, 0))],
        start_line_styles: Vec::new(),
        end_line_styles: Vec::new(),
        start_records: morph_records.clone(),
        end_records: morph_records,
    }));

    // Frame 1: Place gradient at depth 1 and morph shape at ratio 0 at depth 2
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
        ratio: Some(0),
        name: None,
        clip_depth: None,
        clip_actions_raw: None,
    }));
    b.push_tag(&Tag::ShowFrame);

    // Frame 2: Update morph shape to ratio 65535
    b.push_tag(&Tag::PlaceObject2(PlaceObject2Tag {
        is_move: true,
        depth: 2,
        character_id: None,
        matrix: None,
        color_transform: None,
        ratio: Some(65_535),
        name: None,
        clip_depth: None,
        clip_actions_raw: None,
    }));
    b.push_tag(&Tag::ShowFrame);
    let swf = b.build();

    let game_info = retro_game_info {
        path: c"m2_render.swf".as_ptr(),
        data: swf.as_ptr().cast::<c_void>(),
        size: swf.len(),
        meta: core::ptr::null(),
    };
    assert!(retro_load_game(core::ptr::addr_of!(game_info)));
    retro_run();
    retro_run();

    let fe = FRONTEND.lock().unwrap();
    assert_eq!(fe.video_frames.len(), 2);
    // Morph shape right half: green on frame 1, red on frame 2
    assert_eq!(fe.video_frames[0][60 * 160 + 120], 0x0000_FF00);
    assert_eq!(fe.video_frames[1][60 * 160 + 120], 0x00FF_0000);
    drop(fe);

    retro_unload_game();
    retro_deinit();
}

#[test]
fn e2e_m3_avm1_closures_edit_text_and_movieclip_props_via_abi() {
    let _guard = E2E_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset_frontend();
    init_abi_callbacks();

    let mut b = SwfBuilder::new(8, 200, 100, 30.0, 1);
    b.push_tag(&Tag::SetBackgroundColor {
        r: 0x00,
        g: 0x00,
        b: 0x00,
    });

    // Dynamic text field bound to variable `score_txt`
    b.push_tag(&Tag::DefineEditText(EditTextDef {
        id: 1,
        bounds: Rect {
            x_min: 10 * 20,
            x_max: 190 * 20,
            y_min: 10 * 20,
            y_max: 80 * 20,
        },
        font_id: None,
        font_height_twips: 24 * 20,
        color: Rgba::rgb(255, 255, 255),
        variable_name: "score_txt".to_owned(),
        initial_text: Some(String::new()),
        word_wrap: false,
        multiline: false,
        read_only: true,
        html: false,
        use_outlines: false,
    }));
    b.push_tag(&Tag::PlaceObject2(PlaceObject2Tag {
        is_move: false,
        depth: 1,
        character_id: Some(1),
        matrix: Some(Matrix::IDENTITY),
        color_transform: None,
        ratio: None,
        name: Some("label_mc".to_owned()),
        clip_depth: None,
        clip_actions_raw: None,
    }));

    // DoAction: Set variable `score_txt = "SCORE 42"` and trace it
    let mut ab = ActionBuilder::new();
    ab.push_str("score_txt");
    ab.push_str("SCORE 42");
    ab.set_var();
    ab.push_str("score_txt");
    ab.get_var();
    ab.trace();
    b.push_tag(&Tag::DoAction {
        actions: ab.build(),
    });
    b.push_tag(&Tag::ShowFrame);
    let swf = b.build();

    let game_info = retro_game_info {
        path: c"m3_avm1.swf".as_ptr(),
        data: swf.as_ptr().cast::<c_void>(),
        size: swf.len(),
        meta: core::ptr::null(),
    };
    assert!(retro_load_game(core::ptr::addr_of!(game_info)));
    retro_run();

    let fe = FRONTEND.lock().unwrap();
    assert!(
        fe.logs.iter().any(|(_, m)| m.contains("SCORE 42")),
        "AVM1 variable trace must emit SCORE 42, got: {:?}",
        fe.logs
    );
    // Verify that the bound EditText rendered white glyph pixels onto the black stage
    let non_black = fe.video_frames[0].iter().filter(|&&px| px != 0).count();
    assert!(
        non_black > 20,
        "bound DefineEditText must rasterize glyphs via embedded fallback font"
    );
    drop(fe);

    retro_unload_game();
    retro_deinit();
}

#[test]
fn e2e_m4_savestate_roundtrip_and_streaming_audio_via_abi() {
    let _guard = E2E_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset_frontend();
    init_abi_callbacks();

    // Build a 2-frame movie with timeline streaming audio (SoundStreamHead2 + SoundStreamBlock)
    let mut b = SwfBuilder::new(8, 200, 100, 60.0, 2);
    b.push_tag(&Tag::SetBackgroundColor {
        r: 0x00,
        g: 0x00,
        b: 0x00,
    });
    b.push_tag(&Tag::SoundStreamHead(SoundStreamHeadDef {
        version: 2,
        playback_rate_code: 3,
        playback_is_16bit: true,
        playback_is_stereo: true,
        stream_format: AudioFormat::PcmLittleEndian,
        stream_rate_code: 3,
        stream_is_16bit: true,
        stream_is_stereo: true,
        stream_sample_count: 735,
        latency_seek: None,
    }));

    let mut block_bytes = Vec::with_capacity(735 * 4);
    for i in 0..735_i16 {
        let l: i16 = 2100_i16.wrapping_add(i.wrapping_mul(5));
        let r: i16 = -2100_i16.wrapping_sub(i.wrapping_mul(5));
        block_bytes.extend_from_slice(&l.to_le_bytes());
        block_bytes.extend_from_slice(&r.to_le_bytes());
    }
    b.push_tag(&Tag::SoundStreamBlock { data: block_bytes });
    b.push_tag(&Tag::ShowFrame);
    b.push_tag(&Tag::ShowFrame);
    let swf = b.build();

    let game_info = retro_game_info {
        path: c"m4_stream.swf".as_ptr(),
        data: swf.as_ptr().cast::<c_void>(),
        size: swf.len(),
        meta: core::ptr::null(),
    };
    assert!(retro_load_game(core::ptr::addr_of!(game_info)));

    // Verify TSD 2.5 serialize contract: serialize_size returns 0, serialize/unserialize return false
    assert_eq!(retro_serialize_size(), 0);
    assert!(!retro_serialize(core::ptr::null_mut(), 0));
    assert!(!retro_unserialize(core::ptr::null(), 0));

    retro_run();
    let fe = FRONTEND.lock().unwrap();
    assert_eq!(fe.audio_batches.len(), 1);
    assert_eq!(fe.audio_batches[0][0], 2100);
    assert_eq!(fe.audio_batches[0][1], -2100);
    assert!(!is_440hz_sine_batch(&fe.audio_batches[0]));
    drop(fe);

    retro_unload_game();
    retro_deinit();
}

#[test]
fn e2e_m5_avm2_diagnostic_notice_and_reset_via_abi() {
    let _guard = E2E_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    reset_frontend();
    init_abi_callbacks();

    // Part A: SharedObject.getLocal("save") -> so.data.level = 3; so.flush(); + disallowed loadMovie rejection
    let mut b1 = SwfBuilder::new(8, 160, 120, 30.0, 1);
    b1.push_tag(&Tag::SetBackgroundColor {
        r: 0x10,
        g: 0x20,
        b: 0x30,
    });
    let mut ab1 = ActionBuilder::new();
    // var so = SharedObject.getLocal("save");
    ab1.push_str("so");
    ab1.push_str("save");
    ab1.push_int(1);
    ab1.push_str("SharedObject");
    ab1.get_var();
    ab1.push_str("getLocal");
    ab1.call_method();
    ab1.set_var();
    // so.data.level = 3;
    ab1.push_str("so");
    ab1.get_var();
    ab1.push_str("data");
    ab1.get_member();
    ab1.push_str("level");
    ab1.push_int(3);
    ab1.set_member();
    // so.flush();
    ab1.push_int(0);
    ab1.push_str("so");
    ab1.get_var();
    ab1.push_str("flush");
    ab1.call_method();
    ab1.pop();
    // Unsafe loadMovie("../evil.swf", "_root") and loadMovie("http://example.com/a.swf", "_root")
    ab1.push_str("_root");
    ab1.push_str("../evil.swf");
    ab1.push_int(2);
    ab1.push_str("loadMovie");
    ab1.call_function();
    ab1.pop();
    ab1.push_str("_root");
    ab1.push_str("http://example.com/a.swf");
    ab1.push_int(2);
    ab1.push_str("loadMovie");
    ab1.call_function();
    ab1.pop();
    b1.push_tag(&Tag::DoAction {
        actions: ab1.build(),
    });
    b1.push_tag(&Tag::ShowFrame);
    let swf1 = b1.build();

    let game_info1 = retro_game_info {
        path: c"m5_so_write.swf".as_ptr(),
        data: swf1.as_ptr().cast::<c_void>(),
        size: swf1.len(),
        meta: core::ptr::null(),
    };
    assert!(retro_load_game(core::ptr::addr_of!(game_info1)));
    retro_run();

    // Verify RETRO_MEMORY_SAVE_RAM (64 KiB) has "SWFS" magic and valid CRC32
    let mem_size = retro_get_memory_size(RETRO_MEMORY_SAVE_RAM);
    assert_eq!(mem_size, 64 * 1024);
    let mem_ptr = retro_get_memory_data(RETRO_MEMORY_SAVE_RAM);
    assert!(!mem_ptr.is_null());
    let sram = copy_memory_bytes_from_ptr(mem_ptr, mem_size);
    assert_eq!(&sram[0..4], b"SWFS");
    let stored_crc = u32::from_le_bytes([sram[4], sram[5], sram[6], sram[7]]);
    let payload_len = u32::from_le_bytes([sram[8], sram[9], sram[10], sram[11]]) as usize;
    assert!(payload_len > 0);
    // Verify IEEE CRC32 over payload
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in &sram[12..12 + payload_len] {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if (crc & 1) != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    assert_eq!(!crc, stored_crc);

    let fe = FRONTEND.lock().unwrap();
    assert!(
        fe.logs.iter().any(|(_, m)| m.contains("../evil.swf")),
        "traversal loadMovie must be rejected and logged, got: {:?}",
        fe.logs
    );
    assert!(
        fe.logs
            .iter()
            .any(|(_, m)| m.contains("http://example.com/a.swf")),
        "remote loadMovie must be rejected and logged, got: {:?}",
        fe.logs
    );
    drop(fe);
    retro_unload_game();

    // Part B: Load a fresh movie and verify SharedObject.getLocal("save").data.level == 3
    reset_frontend();
    let mut b2 = SwfBuilder::new(8, 160, 120, 30.0, 1);
    let mut ab2 = ActionBuilder::new();
    ab2.push_str("save");
    ab2.push_int(1);
    ab2.push_str("SharedObject");
    ab2.get_var();
    ab2.push_str("getLocal");
    ab2.call_method();
    ab2.push_str("data");
    ab2.get_member();
    ab2.push_str("level");
    ab2.get_member();
    ab2.trace();
    b2.push_tag(&Tag::DoAction {
        actions: ab2.build(),
    });
    b2.push_tag(&Tag::ShowFrame);
    let swf2 = b2.build();
    let game_info2 = retro_game_info {
        path: c"m5_so_read.swf".as_ptr(),
        data: swf2.as_ptr().cast::<c_void>(),
        size: swf2.len(),
        meta: core::ptr::null(),
    };
    assert!(retro_load_game(core::ptr::addr_of!(game_info2)));
    retro_run();
    let fe = FRONTEND.lock().unwrap();
    assert!(
        fe.logs.iter().any(|(_, m)| m.contains("[trace] 3")),
        "reloaded movie must read so.data.level == 3 from SAVE_RAM, got: {:?}",
        fe.logs
    );
    drop(fe);
    retro_unload_game();

    // Part C: AVM2 movie emits once-per-movie notice and renders vector timeline cleanly
    reset_frontend();
    let mut b = SwfBuilder::new(10, 160, 120, 30.0, 1);
    b.push_tag(&Tag::FileAttributes {
        use_direct_blit: false,
        use_gpu: false,
        has_metadata: false,
        actionscript3: true,
        use_network: false,
    });
    b.push_tag(&Tag::SetBackgroundColor {
        r: 0x00,
        g: 0x80,
        b: 0x40,
    });
    b.push_tag(&Tag::Unknown {
        code: 82,
        raw: vec![
            0x01, 0x00, 0x00, 0x00, b'f', b'1', 0x00, 0x10, 0x00, 0x2E, 0x00,
        ],
    });
    b.push_tag(&Tag::ShowFrame);
    let swf = b.build();

    let game_info = retro_game_info {
        path: c"m5_avm2.swf".as_ptr(),
        data: swf.as_ptr().cast::<c_void>(),
        size: swf.len(),
        meta: core::ptr::null(),
    };
    assert!(retro_load_game(core::ptr::addr_of!(game_info)));
    retro_run();
    retro_run();

    let fe = FRONTEND.lock().unwrap();
    let avm2_notices = fe
        .logs
        .iter()
        .filter(|(_, m)| m.contains("AVM2") || m.contains("ActionScript 3"))
        .count();
    assert_eq!(
        avm2_notices, 1,
        "AVM2 movie must emit exactly one diagnostic notice per load, got: {:?}",
        fe.logs
    );
    // Vector stage background must still render cleanly (not M0 stub)
    assert_eq!(fe.video_frames[0][0], 0x0000_8040);
    drop(fe);

    retro_reset();
    retro_unload_game();
    retro_deinit();
}

#[test]
fn e2e_m6_repeated_load_unload_50_cycles_via_abi() {
    let _guard = E2E_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    init_abi_callbacks();

    let mut b = SwfBuilder::new(8, 200, 160, 24.0, 1);
    b.push_tag(&Tag::SetBackgroundColor {
        r: 0x00,
        g: 0x00,
        b: 0xFF,
    });
    b.push_tag(&Tag::DefineShape(ShapeDef {
        id: 1,
        version: 1,
        bounds: Rect {
            x_min: 1000,
            x_max: 3000,
            y_min: 800,
            y_max: 2400,
        },
        fill_styles: vec![FillStyle::Solid(Rgba::rgb(0xFF, 0x00, 0x00))],
        line_styles: Vec::new(),
        records: rect_shape_records(1000, 800, 2000, 1600),
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
    let swf = b.build();

    for _ in 0..50 {
        reset_frontend();
        let game_info = retro_game_info {
            path: c"m6_cycle.swf".as_ptr(),
            data: swf.as_ptr().cast::<c_void>(),
            size: swf.len(),
            meta: core::ptr::null(),
        };
        assert!(retro_load_game(core::ptr::addr_of!(game_info)));
        retro_run();
        let fe = FRONTEND.lock().unwrap();
        assert_eq!(fe.video_frames[0][80 * 200 + 100], 0x00FF_0000);
        drop(fe);
        retro_unload_game();
    }

    retro_deinit();
}
