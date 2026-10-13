//! Mock frontend ABI integration test for `swfr_libretro` (TSD 2.12 M0, 2.13).
//!
//! Drives the exported `extern "C"` libretro entrypoints and asserts:
//! - `retro_api_version` matches `RETRO_API_VERSION`.
//! - `retro_load_game` with `NULL` returns `false`.
//! - `retro_input_poll_t` is called exactly once per `retro_run`.
//! - Video callback receives `pitch >= width * 4` and 320x240 color bars.
//! - Over 600 runs at 12 fps, total stereo audio frames equal `2,205,000`.
//! - `retro_reset`, `retro_unload_game`, reload, and `retro_deinit` work cleanly.
//! - Injected panics do not cross the C ABI boundary and place the core in the
//!   dark-red (`0x00800000`) + silent-audio dead fallback state.

use core::ffi::{c_uint, c_void};
use std::sync::Mutex;
use swfr_libretro::convert::DEAD_FRAME_XRGB;
use swfr_libretro::ffi::{
    self, arm_fault_injection_on_next_run, copy_audio_frames_from_callback_ptr,
    copy_video_frame_from_callback_ptr, retro_api_version, retro_cheat_reset, retro_cheat_set,
    retro_deinit, retro_game_geometry, retro_game_info, retro_get_memory_data,
    retro_get_memory_size, retro_get_region, retro_get_system_av_info, retro_get_system_info,
    retro_init, retro_load_game, retro_load_game_special, retro_reset, retro_run, retro_serialize,
    retro_serialize_size, retro_set_audio_sample, retro_set_audio_sample_batch,
    retro_set_controller_port_device, retro_set_environment, retro_set_input_poll,
    retro_set_input_state, retro_set_video_refresh, retro_system_av_info, retro_system_info,
    retro_system_timing, retro_unload_game, retro_unserialize, RETRO_API_VERSION,
    RETRO_ENVIRONMENT_SET_PIXEL_FORMAT, RETRO_REGION_NTSC,
};

#[derive(Default)]
struct MockFrontendState {
    poll_calls: usize,
    video_calls: usize,
    last_width: u32,
    last_height: u32,
    last_pitch: usize,
    last_video_pixels: Vec<u32>,
    total_audio_frames: usize,
    last_audio_samples: Vec<i16>,
    pixel_format_set: bool,
}

static FRONTEND: Mutex<MockFrontendState> = Mutex::new(MockFrontendState {
    poll_calls: 0,
    video_calls: 0,
    last_width: 0,
    last_height: 0,
    last_pitch: 0,
    last_video_pixels: Vec::new(),
    total_audio_frames: 0,
    last_audio_samples: Vec::new(),
    pixel_format_set: false,
});

fn reset_frontend_state() {
    if let Ok(mut guard) = FRONTEND.lock() {
        *guard = MockFrontendState::default();
    }
}

extern "C" fn mock_environment(cmd: c_uint, _data: *mut c_void) -> bool {
    if cmd == RETRO_ENVIRONMENT_SET_PIXEL_FORMAT {
        if let Ok(mut guard) = FRONTEND.lock() {
            guard.pixel_format_set = true;
        }
        return true;
    }
    false
}

extern "C" fn mock_video_refresh(
    data: *const c_void,
    width: c_uint,
    height: c_uint,
    pitch: usize,
) {
    let pixels = copy_video_frame_from_callback_ptr(data, width, height);
    if let Ok(mut guard) = FRONTEND.lock() {
        guard.video_calls = guard.video_calls.saturating_add(1);
        guard.last_width = width;
        guard.last_height = height;
        guard.last_pitch = pitch;
        guard.last_video_pixels = pixels;
    }
}

extern "C" fn mock_audio_sample(_left: i16, _right: i16) {}

extern "C" fn mock_audio_sample_batch(data: *const i16, frames: usize) -> usize {
    let samples = copy_audio_frames_from_callback_ptr(data, frames);
    if let Ok(mut guard) = FRONTEND.lock() {
        guard.total_audio_frames = guard.total_audio_frames.saturating_add(frames);
        guard.last_audio_samples = samples;
    }
    frames
}

extern "C" fn mock_input_poll() {
    if let Ok(mut guard) = FRONTEND.lock() {
        guard.poll_calls = guard.poll_calls.saturating_add(1);
    }
}

extern "C" fn mock_input_state(
    _port: c_uint,
    _device: c_uint,
    _index: c_uint,
    _id: c_uint,
) -> i16 {
    0
}

#[test]
fn full_libretro_abi_lifecycle_and_fault_containment() {
    reset_frontend_state();

    // 1. Verify API version and system info before init.
    assert_eq!(retro_api_version(), RETRO_API_VERSION);
    let mut sys_info = retro_system_info {
        library_name: core::ptr::null(),
        library_version: core::ptr::null(),
        valid_extensions: core::ptr::null(),
        need_fullpath: true,
        block_extract: true,
    };
    retro_get_system_info(core::ptr::addr_of_mut!(sys_info));
    assert!(!sys_info.library_name.is_null());
    assert!(!sys_info.library_version.is_null());
    assert!(!sys_info.valid_extensions.is_null());
    assert!(!sys_info.need_fullpath);
    assert!(!sys_info.block_extract);

    // 2. Register callbacks and initialize core.
    retro_set_environment(Some(mock_environment));
    retro_set_video_refresh(Some(mock_video_refresh));
    retro_set_audio_sample(Some(mock_audio_sample));
    retro_set_audio_sample_batch(Some(mock_audio_sample_batch));
    retro_set_input_poll(Some(mock_input_poll));
    retro_set_input_state(Some(mock_input_state));
    retro_init();
    retro_set_controller_port_device(0, ffi::RETRO_DEVICE_JOYPAD);

    // 3. Null game pointer must return false.
    assert!(!retro_load_game(core::ptr::null()));
    assert!(!retro_load_game_special(0, core::ptr::null(), 0));

    // 4. Load a 12 fps minimal SWF header and run 600 frames.
    let swf_12fps = b"FWS\x08\x0d\x00\x00\x00\x00\x00\x0c\x01\x00";
    let game_info = retro_game_info {
        path: c"test_12fps.swf".as_ptr(),
        data: swf_12fps.as_ptr().cast::<c_void>(),
        size: swf_12fps.len(),
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
    assert_eq!(av_info.geometry.base_width, 320);
    assert_eq!(av_info.geometry.base_height, 240);
    assert!((av_info.timing.fps - 12.0).abs() < f64::EPSILON);
    assert!((av_info.timing.sample_rate - 44_100.0).abs() < f64::EPSILON);

    for _ in 0..600 {
        retro_run();
    }

    if let Ok(guard) = FRONTEND.lock() {
        assert!(guard.pixel_format_set);
        assert_eq!(guard.poll_calls, 600, "input_poll must be called once per retro_run");
        assert_eq!(guard.video_calls, 600);
        assert_eq!(guard.last_width, 320);
        assert_eq!(guard.last_height, 240);
        assert!(
            guard.last_pitch >= (guard.last_width as usize) * 4,
            "pitch must be at least width * 4"
        );
        assert_eq!(
            guard.total_audio_frames, 2_205_000,
            "600 runs at 12 fps must yield exactly 2,205,000 stereo audio frames"
        );
        assert_eq!(guard.last_video_pixels.first().copied(), Some(0x00FF_FFFF));
    }

    // 5. Verify serialize / memory / region / reset / unload / reload.
    assert_eq!(retro_serialize_size(), 0);
    assert!(!retro_serialize(core::ptr::null_mut(), 0));
    assert!(!retro_unserialize(core::ptr::null(), 0));
    retro_cheat_reset();
    retro_cheat_set(0, false, core::ptr::null());
    assert_eq!(retro_get_region(), RETRO_REGION_NTSC);
    assert!(retro_get_memory_data(0).is_null());
    assert_eq!(retro_get_memory_size(0), 0);

    retro_reset();
    retro_unload_game();

    // Reload and test fault injection across the C ABI boundary.
    reset_frontend_state();
    assert!(retro_load_game(core::ptr::addr_of!(game_info)));
    arm_fault_injection_on_next_run();
    // This call catches the injected panic inside `retro_run` and marks the core dead.
    retro_run();
    // Subsequent `retro_run` outputs the solid dark-red fallback frame and silent audio.
    retro_run();

    if let Ok(guard) = FRONTEND.lock() {
        assert_eq!(guard.poll_calls, 2);
        assert_eq!(guard.last_video_pixels.len(), 320 * 240);
        assert!(
            guard
                .last_video_pixels
                .iter()
                .all(|&px| px == DEAD_FRAME_XRGB),
            "dead core must output solid dark red frame (0x00800000)"
        );
        assert!(
            guard.last_audio_samples.iter().all(|&s| s == 0),
            "dead core must output silent audio"
        );
    }

    retro_unload_game();
    retro_deinit();
}
