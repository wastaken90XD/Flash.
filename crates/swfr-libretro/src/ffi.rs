//! Raw `libretro.h` C ABI constants, types, callbacks, and exported symbols (TSD 2.5).
//!
//! All numeric values and struct layouts are verified against `docs/reference/libretro.h`
//! (see `docs/notes/libretro-abi.md` for line numbers) and tested automatically by
//! `crates/swfr-libretro/tests/header_check.rs`.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use crate::input_map::RawInputSnapshot;
use crate::options::{apply_option_kv, ALL_OPTION_KEYS};
use crate::state::CoreState;
use core::ffi::{c_char, c_double, c_float, c_int, c_uint, c_void, CStr};
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::sync::Mutex;
use swfr_player::{KeyEvent, LogLevel};

// ============================================================================
// Constants from docs/reference/libretro.h (see docs/notes/libretro-abi.md)
// ============================================================================

/// `RETRO_API_VERSION` (`libretro.h:97`).
pub const RETRO_API_VERSION: c_uint = 1;

/// `RETRO_DEVICE_NONE` (`libretro.h:153`).
pub const RETRO_DEVICE_NONE: c_uint = 0;
/// `RETRO_DEVICE_JOYPAD` (`libretro.h:176`).
pub const RETRO_DEVICE_JOYPAD: c_uint = 1;
/// `RETRO_DEVICE_MOUSE` (`libretro.h:202`).
pub const RETRO_DEVICE_MOUSE: c_uint = 2;
/// `RETRO_DEVICE_KEYBOARD` (`libretro.h:219`).
pub const RETRO_DEVICE_KEYBOARD: c_uint = 3;
/// `RETRO_DEVICE_LIGHTGUN` (`libretro.h:238`).
pub const RETRO_DEVICE_LIGHTGUN: c_uint = 4;
/// `RETRO_DEVICE_ANALOG` (`libretro.h:260`).
pub const RETRO_DEVICE_ANALOG: c_uint = 5;
/// `RETRO_DEVICE_POINTER` (`libretro.h:305`).
pub const RETRO_DEVICE_POINTER: c_uint = 6;

/// `RETRO_DEVICE_ID_JOYPAD_B` (`libretro.h:320`).
pub const RETRO_DEVICE_ID_JOYPAD_B: c_uint = 0;
/// `RETRO_DEVICE_ID_JOYPAD_Y` (`libretro.h:323`).
pub const RETRO_DEVICE_ID_JOYPAD_Y: c_uint = 1;
/// `RETRO_DEVICE_ID_JOYPAD_SELECT` (`libretro.h:326`).
pub const RETRO_DEVICE_ID_JOYPAD_SELECT: c_uint = 2;
/// `RETRO_DEVICE_ID_JOYPAD_START` (`libretro.h:329`).
pub const RETRO_DEVICE_ID_JOYPAD_START: c_uint = 3;
/// `RETRO_DEVICE_ID_JOYPAD_UP` (`libretro.h:332`).
pub const RETRO_DEVICE_ID_JOYPAD_UP: c_uint = 4;
/// `RETRO_DEVICE_ID_JOYPAD_DOWN` (`libretro.h:335`).
pub const RETRO_DEVICE_ID_JOYPAD_DOWN: c_uint = 5;
/// `RETRO_DEVICE_ID_JOYPAD_LEFT` (`libretro.h:338`).
pub const RETRO_DEVICE_ID_JOYPAD_LEFT: c_uint = 6;
/// `RETRO_DEVICE_ID_JOYPAD_RIGHT` (`libretro.h:341`).
pub const RETRO_DEVICE_ID_JOYPAD_RIGHT: c_uint = 7;
/// `RETRO_DEVICE_ID_JOYPAD_A` (`libretro.h:344`).
pub const RETRO_DEVICE_ID_JOYPAD_A: c_uint = 8;
/// `RETRO_DEVICE_ID_JOYPAD_X` (`libretro.h:347`).
pub const RETRO_DEVICE_ID_JOYPAD_X: c_uint = 9;
/// `RETRO_DEVICE_ID_JOYPAD_L` (`libretro.h:350`).
pub const RETRO_DEVICE_ID_JOYPAD_L: c_uint = 10;
/// `RETRO_DEVICE_ID_JOYPAD_R` (`libretro.h:353`).
pub const RETRO_DEVICE_ID_JOYPAD_R: c_uint = 11;
/// `RETRO_DEVICE_ID_JOYPAD_L2` (`libretro.h:356`).
pub const RETRO_DEVICE_ID_JOYPAD_L2: c_uint = 12;
/// `RETRO_DEVICE_ID_JOYPAD_R2` (`libretro.h:359`).
pub const RETRO_DEVICE_ID_JOYPAD_R2: c_uint = 13;
/// `RETRO_DEVICE_ID_JOYPAD_L3` (`libretro.h:365`).
pub const RETRO_DEVICE_ID_JOYPAD_L3: c_uint = 14;
/// `RETRO_DEVICE_ID_JOYPAD_R3` (`libretro.h:371`).
pub const RETRO_DEVICE_ID_JOYPAD_R3: c_uint = 15;
/// `RETRO_DEVICE_ID_JOYPAD_MASK` (`libretro.h:380`).
pub const RETRO_DEVICE_ID_JOYPAD_MASK: c_uint = 256;

/// `RETRO_DEVICE_INDEX_ANALOG_LEFT` (`libretro.h:389`).
pub const RETRO_DEVICE_INDEX_ANALOG_LEFT: c_uint = 0;
/// `RETRO_DEVICE_INDEX_ANALOG_RIGHT` (`libretro.h:390`).
pub const RETRO_DEVICE_INDEX_ANALOG_RIGHT: c_uint = 1;
/// `RETRO_DEVICE_INDEX_ANALOG_BUTTON` (`libretro.h:391`).
pub const RETRO_DEVICE_INDEX_ANALOG_BUTTON: c_uint = 2;
/// `RETRO_DEVICE_ID_ANALOG_X` (`libretro.h:392`).
pub const RETRO_DEVICE_ID_ANALOG_X: c_uint = 0;
/// `RETRO_DEVICE_ID_ANALOG_Y` (`libretro.h:393`).
pub const RETRO_DEVICE_ID_ANALOG_Y: c_uint = 1;

/// `RETRO_DEVICE_ID_MOUSE_X` (`libretro.h:398`).
pub const RETRO_DEVICE_ID_MOUSE_X: c_uint = 0;
/// `RETRO_DEVICE_ID_MOUSE_Y` (`libretro.h:399`).
pub const RETRO_DEVICE_ID_MOUSE_Y: c_uint = 1;
/// `RETRO_DEVICE_ID_MOUSE_LEFT` (`libretro.h:400`).
pub const RETRO_DEVICE_ID_MOUSE_LEFT: c_uint = 2;
/// `RETRO_DEVICE_ID_MOUSE_RIGHT` (`libretro.h:401`).
pub const RETRO_DEVICE_ID_MOUSE_RIGHT: c_uint = 3;
/// `RETRO_DEVICE_ID_MOUSE_WHEELUP` (`libretro.h:402`).
pub const RETRO_DEVICE_ID_MOUSE_WHEELUP: c_uint = 4;
/// `RETRO_DEVICE_ID_MOUSE_WHEELDOWN` (`libretro.h:403`).
pub const RETRO_DEVICE_ID_MOUSE_WHEELDOWN: c_uint = 5;
/// `RETRO_DEVICE_ID_MOUSE_MIDDLE` (`libretro.h:404`).
pub const RETRO_DEVICE_ID_MOUSE_MIDDLE: c_uint = 6;

/// `RETRO_DEVICE_ID_POINTER_X` (`libretro.h:434`).
pub const RETRO_DEVICE_ID_POINTER_X: c_uint = 0;
/// `RETRO_DEVICE_ID_POINTER_Y` (`libretro.h:435`).
pub const RETRO_DEVICE_ID_POINTER_Y: c_uint = 1;
/// `RETRO_DEVICE_ID_POINTER_PRESSED` (`libretro.h:436`).
pub const RETRO_DEVICE_ID_POINTER_PRESSED: c_uint = 2;
/// `RETRO_DEVICE_ID_POINTER_COUNT` (`libretro.h:437`).
pub const RETRO_DEVICE_ID_POINTER_COUNT: c_uint = 3;

/// `RETRO_REGION_NTSC` (`libretro.h:443`).
pub const RETRO_REGION_NTSC: c_uint = 0;
/// `RETRO_REGION_PAL` (`libretro.h:444`).
pub const RETRO_REGION_PAL: c_uint = 1;
/// `RETRO_MEMORY_SAVE_RAM` (`libretro.h:450`).
pub const RETRO_MEMORY_SAVE_RAM: c_uint = 0;

/// `RETRO_ENVIRONMENT_EXPERIMENTAL` (`libretro.h:730`).
pub const RETRO_ENVIRONMENT_EXPERIMENTAL: c_uint = 0x10000;
/// `RETRO_ENVIRONMENT_GET_CAN_DUPE` (`libretro.h:768`).
pub const RETRO_ENVIRONMENT_GET_CAN_DUPE: c_uint = 3;
/// `RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL` (`libretro.h:837`).
pub const RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL: c_uint = 8;
/// `RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY` (`libretro.h:855`).
pub const RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY: c_uint = 9;
/// `RETRO_ENVIRONMENT_SET_PIXEL_FORMAT` (`libretro.h:870`).
pub const RETRO_ENVIRONMENT_SET_PIXEL_FORMAT: c_uint = 10;
/// `RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS` (`libretro.h:887`).
pub const RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS: c_uint = 11;
/// `RETRO_ENVIRONMENT_SET_KEYBOARD_CALLBACK` (`libretro.h:901`).
pub const RETRO_ENVIRONMENT_SET_KEYBOARD_CALLBACK: c_uint = 12;
/// `RETRO_ENVIRONMENT_GET_VARIABLE` (`libretro.h:971`).
pub const RETRO_ENVIRONMENT_GET_VARIABLE: c_uint = 15;
/// `RETRO_ENVIRONMENT_SET_VARIABLES` (`libretro.h:1021`).
pub const RETRO_ENVIRONMENT_SET_VARIABLES: c_uint = 16;
/// `RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE` (`libretro.h:1039`).
pub const RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE: c_uint = 17;
/// `RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME` (`libretro.h:1056`).
pub const RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME: c_uint = 18;
/// `RETRO_ENVIRONMENT_GET_LOG_INTERFACE` (`libretro.h:1248`).
pub const RETRO_ENVIRONMENT_GET_LOG_INTERFACE: c_uint = 27;
/// `RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY` (`libretro.h:1331`).
pub const RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY: c_uint = 31;
/// `RETRO_ENVIRONMENT_SET_GEOMETRY` (`libretro.h:1559`).
pub const RETRO_ENVIRONMENT_SET_GEOMETRY: c_uint = 37;
/// `RETRO_ENVIRONMENT_GET_VFS_INTERFACE` (`libretro.h:1732`).
pub const RETRO_ENVIRONMENT_GET_VFS_INTERFACE: c_uint = 45 | RETRO_ENVIRONMENT_EXPERIMENTAL;
/// `RETRO_ENVIRONMENT_GET_CORE_OPTIONS_VERSION` (`libretro.h:1871`).
pub const RETRO_ENVIRONMENT_GET_CORE_OPTIONS_VERSION: c_uint = 52;
/// `RETRO_ENVIRONMENT_SET_CORE_OPTIONS_V2` (`libretro.h:2362`).
pub const RETRO_ENVIRONMENT_SET_CORE_OPTIONS_V2: c_uint = 67;
/// `RETRO_NUM_CORE_OPTION_VALUES_MAX` (`libretro.h:7573`).
pub const RETRO_NUM_CORE_OPTION_VALUES_MAX: usize = 128;

/// `RETRO_LOG_DEBUG` (`libretro.h:4994`).
pub const RETRO_LOG_DEBUG: c_int = 0;
/// `RETRO_LOG_INFO` (`libretro.h:4997`).
pub const RETRO_LOG_INFO: c_int = 1;
/// `RETRO_LOG_WARN` (`libretro.h:5000`).
pub const RETRO_LOG_WARN: c_int = 2;
/// `RETRO_LOG_ERROR` (`libretro.h:5003`).
pub const RETRO_LOG_ERROR: c_int = 3;

/// `RETRO_PIXEL_FORMAT_0RGB1555` (`libretro.h:6729`).
pub const RETRO_PIXEL_FORMAT_0RGB1555: c_int = 0;
/// `RETRO_PIXEL_FORMAT_XRGB8888` (`libretro.h:6735`).
pub const RETRO_PIXEL_FORMAT_XRGB8888: c_int = 1;
/// `RETRO_PIXEL_FORMAT_RGB565` (`libretro.h:6742`).
pub const RETRO_PIXEL_FORMAT_RGB565: c_int = 2;

// ============================================================================
// Structs & Callback Typedefs from docs/reference/libretro.h
// ============================================================================

/// `retro_environment_t` (`libretro.h:8605`).
pub type RetroEnvironmentFn = extern "C" fn(cmd: c_uint, data: *mut c_void) -> bool;

/// `retro_video_refresh_t` (`libretro.h:8624`).
pub type RetroVideoRefreshFn =
    extern "C" fn(data: *const c_void, width: c_uint, height: c_uint, pitch: usize);

/// `retro_audio_sample_t` (`libretro.h:8636`).
pub type RetroAudioSampleFn = extern "C" fn(left: i16, right: i16);

/// `retro_audio_sample_batch_t` (`libretro.h:8653`).
pub type RetroAudioSampleBatchFn = extern "C" fn(data: *const i16, frames: usize) -> usize;

/// `retro_input_poll_t` (`libretro.h:8729`).
pub type RetroInputPollFn = extern "C" fn();

/// `retro_input_state_t` (`libretro.h:8758`).
pub type RetroInputStateFn =
    extern "C" fn(port: c_uint, device: c_uint, index: c_uint, id: c_uint) -> i16;

/// `retro_log_printf_t` (`libretro.h:5023`).
pub type RetroLogPrintfFn = unsafe extern "C" fn(level: c_int, fmt: *const c_char, ...);

/// `retro_keyboard_event_t` (`libretro.h:6304`).
pub type RetroKeyboardEventFn =
    unsafe extern "C" fn(down: bool, keycode: c_uint, character: u32, key_modifiers: u16);

/// `struct retro_log_callback` (`libretro.h:5032`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct retro_log_callback {
    /// Frontend printf-style logging callback.
    pub log: Option<RetroLogPrintfFn>,
}

/// `struct retro_keyboard_callback` (`libretro.h:6307`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct retro_keyboard_callback {
    /// Core keyboard event handler registered with the frontend.
    pub callback: Option<RetroKeyboardEventFn>,
}

/// `struct retro_input_descriptor` (`libretro.h:7111`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct retro_input_descriptor {
    /// Controller port index.
    pub port: c_uint,
    /// Device type constant (`RETRO_DEVICE_*`).
    pub device: c_uint,
    /// Sub-device index.
    pub index: c_uint,
    /// Button or axis ID (`RETRO_DEVICE_ID_*`).
    pub id: c_uint,
    /// Null-terminated static description string, or null to terminate the array.
    pub description: *const c_char,
}

// SAFETY: `retro_input_descriptor` only holds static C string literals when stored in static tables.
unsafe impl Sync for retro_input_descriptor {}

/// `struct retro_system_info` (`libretro.h:7132`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct retro_system_info {
    /// Descriptive name of the library (`"swfr"`).
    pub library_name: *const c_char,
    /// Version string from `Cargo.toml`.
    pub library_version: *const c_char,
    /// Pipe-delimited list of valid file extensions (`"swf"`).
    pub valid_extensions: *const c_char,
    /// `false`: frontend loads the SWF bytes into memory for the core.
    pub need_fullpath: bool,
    /// `false`: frontend may extract archives before passing SWF content.
    pub block_extract: bool,
}

/// `struct retro_game_geometry` (`libretro.h:7395`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct retro_game_geometry {
    /// Nominal video width in pixels.
    pub base_width: c_uint,
    /// Nominal video height in pixels.
    pub base_height: c_uint,
    /// Maximum possible video width in pixels.
    pub max_width: c_uint,
    /// Maximum possible video height in pixels.
    pub max_height: c_uint,
    /// Nominal aspect ratio (`stage_width / stage_height`).
    pub aspect_ratio: c_float,
}

/// `struct retro_system_timing` (`libretro.h:7447`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct retro_system_timing {
    /// Video frame rate in frames per second.
    pub fps: c_double,
    /// Audio sample rate in Hz (`44100.0`).
    pub sample_rate: c_double,
}

/// `struct retro_system_av_info` (`libretro.h:7461`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct retro_system_av_info {
    /// Video frame geometry.
    pub geometry: retro_game_geometry,
    /// Video and audio timing.
    pub timing: retro_system_timing,
}

/// `struct retro_variable` (`libretro.h:7483`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct retro_variable {
    /// Option key string.
    pub key: *const c_char,
    /// Option value or legacy definition string.
    pub value: *const c_char,
}

// SAFETY: Static `retro_variable` tables only reference compile-time NUL-terminated string literals.
unsafe impl Sync for retro_variable {}

/// `struct retro_core_option_value` (`libretro.h:7584`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct retro_core_option_value {
    /// Serialized option value string.
    pub value: *const c_char,
    /// Human-readable option value label.
    pub label: *const c_char,
}

/// `struct retro_core_option_v2_category` (`libretro.h:7700`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct retro_core_option_v2_category {
    /// Category key.
    pub key: *const c_char,
    /// Category display name.
    pub desc: *const c_char,
    /// Category help text.
    pub info: *const c_char,
}

/// `struct retro_core_option_v2_definition` (`libretro.h:7747`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct retro_core_option_v2_definition {
    /// Option key (`"swfr_*"`).
    pub key: *const c_char,
    /// Display name.
    pub desc: *const c_char,
    /// Categorized display name.
    pub desc_categorized: *const c_char,
    /// Help description.
    pub info: *const c_char,
    /// Categorized help description.
    pub info_categorized: *const c_char,
    /// Category key.
    pub category_key: *const c_char,
    /// Allowed option values terminated by `{ NULL, NULL }`.
    pub values: [retro_core_option_value; RETRO_NUM_CORE_OPTION_VALUES_MAX],
    /// Default option value string.
    pub default_value: *const c_char,
}

/// `struct retro_core_options_v2` (`libretro.h:7872`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct retro_core_options_v2 {
    /// Optional categories array pointer.
    pub categories: *const retro_core_option_v2_category,
    /// Definitions array pointer.
    pub definitions: *const retro_core_option_v2_definition,
}

/// `struct retro_game_info` (`libretro.h:7981`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct retro_game_info {
    /// Optional UTF-8 path to the loaded content.
    pub path: *const c_char,
    /// Pointer to the in-memory SWF bytes.
    pub data: *const c_void,
    /// Byte length of `data`.
    pub size: usize,
    /// Optional implementation-specific metadata string.
    pub meta: *const c_char,
}

// ============================================================================
// Static Metadata & Option Tables
// ============================================================================

const LIBRARY_NAME: &CStr = c"swfr";
const LIBRARY_VERSION: &CStr = c"0.1.0";
const VALID_EXTENSIONS: &CStr = c"swf";

static LEGACY_VARIABLES: [retro_variable; 7] = [
    retro_variable {
        key: c"swfr_scale".as_ptr(),
        value: c"Resolution Scale; 1x|2x|3x|4x".as_ptr(),
    },
    retro_variable {
        key: c"swfr_quality".as_ptr(),
        value: c"Rendering Quality; medium|low|high".as_ptr(),
    },
    retro_variable {
        key: c"swfr_pad_mode".as_ptr(),
        value: c"Gamepad Mode; cursor|keys".as_ptr(),
    },
    retro_variable {
        key: c"swfr_cursor_speed".as_ptr(),
        value: c"Cursor Speed; 5|1|2|3|4|6|7|8|9|10".as_ptr(),
    },
    retro_variable {
        key: c"swfr_script_timeout".as_ptr(),
        value: c"Script Timeout (seconds); 2|1|5|15".as_ptr(),
    },
    retro_variable {
        key: c"swfr_trace_to_log".as_ptr(),
        value: c"Forward trace() to Log; on|off".as_ptr(),
    },
    retro_variable {
        key: core::ptr::null(),
        value: core::ptr::null(),
    },
];

static INPUT_DESCRIPTORS: [retro_input_descriptor; 8] = [
    retro_input_descriptor {
        port: 0,
        device: RETRO_DEVICE_JOYPAD,
        index: 0,
        id: RETRO_DEVICE_ID_JOYPAD_UP,
        description: c"Cursor / Up".as_ptr(),
    },
    retro_input_descriptor {
        port: 0,
        device: RETRO_DEVICE_JOYPAD,
        index: 0,
        id: RETRO_DEVICE_ID_JOYPAD_DOWN,
        description: c"Cursor / Down".as_ptr(),
    },
    retro_input_descriptor {
        port: 0,
        device: RETRO_DEVICE_JOYPAD,
        index: 0,
        id: RETRO_DEVICE_ID_JOYPAD_LEFT,
        description: c"Cursor / Left".as_ptr(),
    },
    retro_input_descriptor {
        port: 0,
        device: RETRO_DEVICE_JOYPAD,
        index: 0,
        id: RETRO_DEVICE_ID_JOYPAD_RIGHT,
        description: c"Cursor / Right".as_ptr(),
    },
    retro_input_descriptor {
        port: 0,
        device: RETRO_DEVICE_JOYPAD,
        index: 0,
        id: RETRO_DEVICE_ID_JOYPAD_B,
        description: c"Click / Space".as_ptr(),
    },
    retro_input_descriptor {
        port: 0,
        device: RETRO_DEVICE_JOYPAD,
        index: 0,
        id: RETRO_DEVICE_ID_JOYPAD_A,
        description: c"Enter (Keys Mode)".as_ptr(),
    },
    retro_input_descriptor {
        port: 0,
        device: RETRO_DEVICE_JOYPAD,
        index: 0,
        id: RETRO_DEVICE_ID_JOYPAD_START,
        description: c"Enter / Start".as_ptr(),
    },
    retro_input_descriptor {
        port: 0,
        device: 0,
        index: 0,
        id: 0,
        description: core::ptr::null(),
    },
];

// ============================================================================
// Global State & Stored Callbacks
// ============================================================================

#[derive(Default)]
struct FfiCallbacks {
    environ_cb: Option<RetroEnvironmentFn>,
    video_cb: Option<RetroVideoRefreshFn>,
    audio_sample_cb: Option<RetroAudioSampleFn>,
    audio_batch_cb: Option<RetroAudioSampleBatchFn>,
    input_poll_cb: Option<RetroInputPollFn>,
    input_state_cb: Option<RetroInputStateFn>,
    log_cb: Option<RetroLogPrintfFn>,
}

struct GlobalSlot {
    state: CoreState,
    callbacks: FfiCallbacks,
}

impl GlobalSlot {
    const fn new() -> Self {
        Self {
            state: CoreState::new(),
            callbacks: FfiCallbacks {
                environ_cb: None,
                video_cb: None,
                audio_sample_cb: None,
                audio_batch_cb: None,
                input_poll_cb: None,
                input_state_cb: None,
                log_cb: None,
            },
        }
    }
}

static GLOBAL: Mutex<GlobalSlot> = Mutex::new(GlobalSlot::new());

/// Arms a synthetic panic on the next `retro_run` call to test the `catch_unwind` FFI barrier (TSD 2.12 M0).
pub fn arm_fault_injection_on_next_run() {
    if let Ok(mut guard) = GLOBAL.lock() {
        guard.state.inject_fault_next_run = true;
    }
}

/// Copies `width * height` `XRGB8888` pixels from a `retro_video_refresh_t` `data` pointer.
///
/// Returns an empty `Vec` if `data` is null.
#[must_use]
pub fn copy_video_frame_from_callback_ptr(
    data: *const c_void,
    width: c_uint,
    height: c_uint,
) -> Vec<u32> {
    if data.is_null() {
        return Vec::new();
    }
    let len = usize::try_from(width.saturating_mul(height)).unwrap_or(0);
    if len == 0 {
        return Vec::new();
    }
    // SAFETY: `data` was supplied by `retro_run` from a contiguous `Vec<u32>` of length `width * height`.
    let slice = unsafe { core::slice::from_raw_parts(data.cast::<u32>(), len) };
    slice.to_vec()
}

/// Copies `frames * 2` interleaved stereo `i16` samples from a `retro_audio_sample_batch_t` `data` pointer.
#[must_use]
pub fn copy_audio_frames_from_callback_ptr(data: *const i16, frames: usize) -> Vec<i16> {
    if data.is_null() || frames == 0 {
        return Vec::new();
    }
    let len = frames.saturating_mul(2);
    // SAFETY: `data` was supplied by `retro_run` from a contiguous `Vec<i16>` of length `frames * 2`.
    let slice = unsafe { core::slice::from_raw_parts(data, len) };
    slice.to_vec()
}

/// Installs a mock frontend log callback into the `data` pointer passed to
/// `RETRO_ENVIRONMENT_GET_LOG_INTERFACE` and converts C log strings safely for tests.
pub fn install_log_callback_in_environment(data: *mut c_void, cb: RetroLogPrintfFn) -> bool {
    if data.is_null() {
        return false;
    }
    // SAFETY: When `cmd == RETRO_ENVIRONMENT_GET_LOG_INTERFACE`, `data` points to a mutable `retro_log_callback`.
    let out = unsafe { &mut *data.cast::<retro_log_callback>() };
    out.log = Some(cb);
    true
}

/// Extracts the `retro_keyboard_event_t` callback from the `data` pointer passed to
/// `RETRO_ENVIRONMENT_SET_KEYBOARD_CALLBACK` so mock frontends can deliver keyboard events.
#[must_use]
pub fn extract_keyboard_callback_from_environment(
    data: *mut c_void,
) -> Option<RetroKeyboardEventFn> {
    if data.is_null() {
        return None;
    }
    // SAFETY: When `cmd == RETRO_ENVIRONMENT_SET_KEYBOARD_CALLBACK`, `data` points to a `retro_keyboard_callback`.
    let kb = unsafe { &*data.cast::<retro_keyboard_callback>() };
    kb.callback
}

/// Converts a NUL-terminated C string pointer from a callback into an owned `String`.
#[must_use]
pub fn copy_c_str_from_callback_ptr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    // SAFETY: `ptr` was supplied by `emit_log` from a NUL-terminated byte buffer.
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

/// Casts a non-variadic `extern "C" fn(c_int, *const c_char)` test logger to [`RetroLogPrintfFn`].
///
/// Because `emit_log` passes the complete formatted message (with `%` escaped) in the fixed
/// `fmt` parameter without consuming variadic arguments, the fixed parameter ABI matches across
/// `x86_64`, `i686`, `aarch64`, and `armv7`.
#[must_use]
pub fn make_test_log_printf_fn(cb: extern "C" fn(c_int, *const c_char)) -> RetroLogPrintfFn {
    // SAFETY: `emit_log` invokes `cb(level, fmt)` using only the two fixed arguments (`level` and `fmt`).
    unsafe { core::mem::transmute::<extern "C" fn(c_int, *const c_char), RetroLogPrintfFn>(cb) }
}

fn with_global<F, R>(default: R, f: F) -> R
where
    F: FnOnce(&mut GlobalSlot) -> R,
{
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut guard = match GLOBAL.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        f(&mut guard)
    }));

    match result {
        Ok(val) => val,
        Err(payload) => {
            let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                (*s).to_owned()
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic caught across libretro FFI boundary".to_owned()
            };
            let mut guard = match GLOBAL.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            guard.state.mark_dead(msg);
            default
        }
    }
}

fn emit_log(log_cb: Option<RetroLogPrintfFn>, level: LogLevel, message: &str) {
    let retro_level = match level {
        LogLevel::Debug => RETRO_LOG_DEBUG,
        LogLevel::Info => RETRO_LOG_INFO,
        LogLevel::Warn => RETRO_LOG_WARN,
        LogLevel::Error => RETRO_LOG_ERROR,
    };
    if let Some(cb) = log_cb {
        let mut clean = String::with_capacity(message.len().saturating_add(2));
        for ch in message.chars() {
            if ch == '\0' {
                continue;
            }
            if ch == '%' {
                clean.push('%');
            }
            clean.push(ch);
        }
        clean.push('\n');
        let mut bytes = clean.into_bytes();
        bytes.push(0);
        // SAFETY: `bytes` is NUL-terminated and has all `%` escaped as `%%`, so `cb` reads a valid C string from `fmt`.
        unsafe {
            cb(retro_level, bytes.as_ptr().cast::<c_char>());
        }
    }
}

extern "C" fn keyboard_event_trampoline(
    down: bool,
    keycode: c_uint,
    character: u32,
    key_modifiers: u16,
) {
    with_global((), |slot| {
        slot.state.input_tracker.push_key_event(KeyEvent {
            keycode,
            character,
            modifiers: key_modifiers,
            down,
        });
    });
}

fn refresh_options_from_frontend(slot: &mut GlobalSlot) -> bool {
    let Some(env_cb) = slot.callbacks.environ_cb else {
        return false;
    };
    let mut new_opts = slot.state.options.clone();
    for key in ALL_OPTION_KEYS {
        let mut key_bytes = key.as_bytes().to_vec();
        key_bytes.push(0);
        let mut var = retro_variable {
            key: key_bytes.as_ptr().cast::<c_char>(),
            value: core::ptr::null(),
        };
        let ok = env_cb(
            RETRO_ENVIRONMENT_GET_VARIABLE,
            core::ptr::addr_of_mut!(var).cast::<c_void>(),
        );
        if ok && !var.value.is_null() {
            // SAFETY: Frontend sets `var.value` to a valid NUL-terminated string when returning true.
            if let Ok(val_str) = unsafe { CStr::from_ptr(var.value) }.to_str() {
                apply_option_kv(&mut new_opts, key, val_str);
            }
        }
    }
    slot.state.options = new_opts.clone();
    if let Some(player) = slot.state.player.as_mut() {
        player.set_options(&new_opts)
    } else {
        false
    }
}

// ============================================================================
// Exported libretro C ABI Entrypoints (TSD 2.5)
// ============================================================================

/// Returns `RETRO_API_VERSION` (`1`) (`libretro.h:8877`).
#[no_mangle]
pub extern "C" fn retro_api_version() -> c_uint {
    RETRO_API_VERSION
}

/// Registers the frontend environment callback and declares options and metadata (`libretro.h:8770`).
#[no_mangle]
pub extern "C" fn retro_set_environment(cb: Option<RetroEnvironmentFn>) {
    with_global((), |slot| {
        slot.callbacks.environ_cb = cb;
        let Some(env_cb) = cb else {
            return;
        };

        let mut no_game = false;
        let _ = env_cb(
            RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME,
            core::ptr::addr_of_mut!(no_game).cast::<c_void>(),
        );

        let _ = env_cb(
            RETRO_ENVIRONMENT_SET_VARIABLES,
            LEGACY_VARIABLES.as_ptr().cast_mut().cast::<c_void>(),
        );

        let mut log_cb = retro_log_callback { log: None };
        let has_log = env_cb(
            RETRO_ENVIRONMENT_GET_LOG_INTERFACE,
            core::ptr::addr_of_mut!(log_cb).cast::<c_void>(),
        );
        if has_log {
            slot.callbacks.log_cb = log_cb.log;
        }
    });
}

/// Registers the video refresh callback (`libretro.h:8779`).
#[no_mangle]
pub extern "C" fn retro_set_video_refresh(cb: Option<RetroVideoRefreshFn>) {
    with_global((), |slot| {
        slot.callbacks.video_cb = cb;
    });
}

/// Registers the single-sample audio callback (stored and unused per TSD 2.5) (`libretro.h:8788`).
#[no_mangle]
pub extern "C" fn retro_set_audio_sample(cb: Option<RetroAudioSampleFn>) {
    with_global((), |slot| {
        slot.callbacks.audio_sample_cb = cb;
    });
}

/// Registers the batch audio callback (`libretro.h:8797`).
#[no_mangle]
pub extern "C" fn retro_set_audio_sample_batch(cb: Option<RetroAudioSampleBatchFn>) {
    with_global((), |slot| {
        slot.callbacks.audio_batch_cb = cb;
    });
}

/// Registers the input poll callback (`libretro.h:8806`).
#[no_mangle]
pub extern "C" fn retro_set_input_poll(cb: Option<RetroInputPollFn>) {
    with_global((), |slot| {
        slot.callbacks.input_poll_cb = cb;
    });
}

/// Registers the input state callback (`libretro.h:8815`).
#[no_mangle]
pub extern "C" fn retro_set_input_state(cb: Option<RetroInputStateFn>) {
    with_global((), |slot| {
        slot.callbacks.input_state_cb = cb;
    });
}

/// Initializes global core state (`libretro.h:8841`).
#[no_mangle]
pub extern "C" fn retro_init() {
    with_global((), |slot| {
        slot.state.init();
    });
}

/// Deinitializes global core state (`libretro.h:8866`).
#[no_mangle]
pub extern "C" fn retro_deinit() {
    with_global((), |slot| {
        slot.state.deinit();
    });
}

/// Populates `retro_system_info` (`libretro.h:8886`).
#[no_mangle]
pub extern "C" fn retro_get_system_info(info: *mut retro_system_info) {
    with_global((), |_slot| {
        if info.is_null() {
            return;
        }
        // SAFETY: Caller guarantees `info` is a valid non-null pointer to `retro_system_info`.
        unsafe {
            (*info).library_name = LIBRARY_NAME.as_ptr();
            (*info).library_version = LIBRARY_VERSION.as_ptr();
            (*info).valid_extensions = VALID_EXTENSIONS.as_ptr();
            (*info).need_fullpath = false;
            (*info).block_extract = false;
        }
    });
}

/// Populates `retro_system_av_info` from the currently loaded movie (`libretro.h:8901`).
#[no_mangle]
pub extern "C" fn retro_get_system_av_info(info: *mut retro_system_av_info) {
    with_global((), |slot| {
        if info.is_null() {
            return;
        }
        let av = slot.state.av_info();
        // SAFETY: Caller guarantees `info` is a valid non-null pointer to `retro_system_av_info`.
        unsafe {
            (*info).geometry = retro_game_geometry {
                base_width: av.width,
                base_height: av.height,
                max_width: av.max_width,
                max_height: av.max_height,
                aspect_ratio: av.aspect_ratio,
            };
            (*info).timing = retro_system_timing {
                fps: av.fps,
                sample_rate: av.sample_rate,
            };
        }
    });
}

/// Sets the controller device type for `port` (`libretro.h:8932`).
#[no_mangle]
pub extern "C" fn retro_set_controller_port_device(port: c_uint, device: c_uint) {
    with_global((), |slot| {
        if port == 0 {
            slot.state.port0_device = device;
        }
    });
}

/// Resets the current movie back to frame 1 (`libretro.h:8939`).
#[no_mangle]
pub extern "C" fn retro_reset() {
    with_global((), |slot| {
        slot.state.reset();
    });
}

/// Loads a SWF movie from memory (`libretro.h:9020`).
#[no_mangle]
pub extern "C" fn retro_load_game(game: *const retro_game_info) -> bool {
    with_global(false, |slot| {
        if game.is_null() {
            return false;
        }
        // SAFETY: `game` was checked for null above.
        let game_ref = unsafe { &*game };
        if game_ref.data.is_null() || game_ref.size == 0 {
            emit_log(
                slot.callbacks.log_cb,
                LogLevel::Error,
                "retro_load_game failed: null or empty game data",
            );
            return false;
        }

        if let Some(env_cb) = slot.callbacks.environ_cb {
            let mut pixel_format: c_int = RETRO_PIXEL_FORMAT_XRGB8888;
            let fmt_ok = env_cb(
                RETRO_ENVIRONMENT_SET_PIXEL_FORMAT,
                core::ptr::addr_of_mut!(pixel_format).cast::<c_void>(),
            );
            if !fmt_ok {
                emit_log(
                    slot.callbacks.log_cb,
                    LogLevel::Error,
                    "retro_load_game failed: frontend rejected XRGB8888 pixel format",
                );
                return false;
            }

            let mut can_dupe = false;
            if env_cb(
                RETRO_ENVIRONMENT_GET_CAN_DUPE,
                core::ptr::addr_of_mut!(can_dupe).cast::<c_void>(),
            ) {
                slot.state.can_dupe = can_dupe;
            }

            let mut perf_level: c_uint = 4;
            let _ = env_cb(
                RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL,
                core::ptr::addr_of_mut!(perf_level).cast::<c_void>(),
            );

            let _ = env_cb(
                RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS,
                INPUT_DESCRIPTORS.as_ptr().cast_mut().cast::<c_void>(),
            );

            let mut kb_cb = retro_keyboard_callback {
                callback: Some(keyboard_event_trampoline),
            };
            let _ = env_cb(
                RETRO_ENVIRONMENT_SET_KEYBOARD_CALLBACK,
                core::ptr::addr_of_mut!(kb_cb).cast::<c_void>(),
            );
        }

        let _ = refresh_options_from_frontend(slot);

        let base_dir = if !game_ref.path.is_null() {
            // SAFETY: When non-null, `game_ref.path` is a NUL-terminated UTF-8 string from the frontend.
            unsafe { CStr::from_ptr(game_ref.path) }
                .to_str()
                .ok()
                .and_then(|p| std::path::Path::new(p).parent())
                .and_then(|p| p.to_str())
                .map(str::to_owned)
        } else {
            None
        };

        // SAFETY: `game_ref.data` is non-null and `game_ref.size > 0` per check above.
        let swf_bytes =
            unsafe { core::slice::from_raw_parts(game_ref.data.cast::<u8>(), game_ref.size) };

        #[cfg(feature = "fault-inject")]
        if swf_bytes.starts_with(b"SWFR_FAULT_ON_RUN") {
            slot.state.inject_fault_next_run = true;
        }

        match slot.state.load_game(swf_bytes, base_dir) {
            Ok(_) => true,
            Err(err_msg) => {
                emit_log(slot.callbacks.log_cb, LogLevel::Error, &err_msg);
                false
            }
        }
    })
}

/// Subsystem loading is unsupported and always returns `false` (`libretro.h:9044`).
#[no_mangle]
pub extern "C" fn retro_load_game_special(
    _game_type: c_uint,
    _info: *const retro_game_info,
    _num_info: usize,
) -> bool {
    false
}

/// Unloads the active movie while preserving global state (`libretro.h:9057`).
#[no_mangle]
pub extern "C" fn retro_unload_game() {
    with_global((), |slot| {
        slot.state.unload_game();
    });
}

/// Runs the player for a single frame, polling input and submitting video and audio (`libretro.h:8953`).
#[no_mangle]
pub extern "C" fn retro_run() {
    with_global((), |slot| {
        // 1. Poll input exactly once at the start of retro_run (TSD 2.5).
        if let Some(poll_cb) = slot.callbacks.input_poll_cb {
            poll_cb();
        }

        // Check if fault injection was requested for testing panic containment.
        if slot.state.inject_fault_next_run {
            slot.state.inject_fault_next_run = false;
            resume_unwind(Box::new("injected fault in retro_run"));
        }

        // 2. Check if core options were updated by the frontend.
        if let Some(env_cb) = slot.callbacks.environ_cb {
            let mut updated = false;
            let has_update = env_cb(
                RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE,
                core::ptr::addr_of_mut!(updated).cast::<c_void>(),
            );
            if has_update && updated && refresh_options_from_frontend(slot) {
                let av = slot.state.av_info();
                let mut geom = retro_game_geometry {
                    base_width: av.width,
                    base_height: av.height,
                    max_width: av.max_width,
                    max_height: av.max_height,
                    aspect_ratio: av.aspect_ratio,
                };
                let _ = env_cb(
                    RETRO_ENVIRONMENT_SET_GEOMETRY,
                    core::ptr::addr_of_mut!(geom).cast::<c_void>(),
                );
            }
        }

        // 3. Sample input state from the frontend.
        let mut raw_input = RawInputSnapshot::default();
        if let Some(state_cb) = slot.callbacks.input_state_cb {
            raw_input.pointer_pressed =
                state_cb(0, RETRO_DEVICE_POINTER, 0, RETRO_DEVICE_ID_POINTER_PRESSED) != 0;
            raw_input.pointer_x = state_cb(0, RETRO_DEVICE_POINTER, 0, RETRO_DEVICE_ID_POINTER_X);
            raw_input.pointer_y = state_cb(0, RETRO_DEVICE_POINTER, 0, RETRO_DEVICE_ID_POINTER_Y);

            raw_input.mouse_dx = state_cb(0, RETRO_DEVICE_MOUSE, 0, RETRO_DEVICE_ID_MOUSE_X);
            raw_input.mouse_dy = state_cb(0, RETRO_DEVICE_MOUSE, 0, RETRO_DEVICE_ID_MOUSE_Y);
            raw_input.mouse_left =
                state_cb(0, RETRO_DEVICE_MOUSE, 0, RETRO_DEVICE_ID_MOUSE_LEFT) != 0;
            raw_input.mouse_right =
                state_cb(0, RETRO_DEVICE_MOUSE, 0, RETRO_DEVICE_ID_MOUSE_RIGHT) != 0;
            raw_input.mouse_middle =
                state_cb(0, RETRO_DEVICE_MOUSE, 0, RETRO_DEVICE_ID_MOUSE_MIDDLE) != 0;
            raw_input.mouse_wheel_up =
                state_cb(0, RETRO_DEVICE_MOUSE, 0, RETRO_DEVICE_ID_MOUSE_WHEELUP) != 0;
            raw_input.mouse_wheel_down =
                state_cb(0, RETRO_DEVICE_MOUSE, 0, RETRO_DEVICE_ID_MOUSE_WHEELDOWN) != 0;

            raw_input.joypad_up =
                state_cb(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_UP) != 0;
            raw_input.joypad_down =
                state_cb(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_DOWN) != 0;
            raw_input.joypad_left =
                state_cb(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_LEFT) != 0;
            raw_input.joypad_right =
                state_cb(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_RIGHT) != 0;
            raw_input.joypad_south =
                state_cb(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_B) != 0;
            raw_input.joypad_east =
                state_cb(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_A) != 0;
            raw_input.joypad_start =
                state_cb(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_START) != 0;

            raw_input.analog_left_x = state_cb(
                0,
                RETRO_DEVICE_ANALOG,
                RETRO_DEVICE_INDEX_ANALOG_LEFT,
                RETRO_DEVICE_ID_ANALOG_X,
            );
            raw_input.analog_left_y = state_cb(
                0,
                RETRO_DEVICE_ANALOG,
                RETRO_DEVICE_INDEX_ANALOG_LEFT,
                RETRO_DEVICE_ID_ANALOG_Y,
            );
        }

        // 4. Advance the player by one frame and submit logs, video, and audio.
        let video_cb = slot.callbacks.video_cb;
        let audio_batch_cb = slot.callbacks.audio_batch_cb;
        let log_cb = slot.callbacks.log_cb;
        let can_dupe = slot.state.can_dupe;

        let step = slot.state.step_frame(&raw_input);
        for line in step.logs {
            emit_log(log_cb, line.level, &line.message);
        }

        if let Some(v_cb) = video_cb {
            let pitch = (step.width as usize).saturating_mul(4);
            match step.video_pixels {
                Some(pixels) => {
                    v_cb(
                        pixels.as_ptr().cast::<c_void>(),
                        step.width,
                        step.height,
                        pitch,
                    );
                }
                None if can_dupe => {
                    v_cb(core::ptr::null(), step.width, step.height, pitch);
                }
                None => {}
            }
        }

        if let Some(a_cb) = audio_batch_cb {
            let stereo_frames = step.audio_samples.len().checked_div(2).unwrap_or(0);
            let _ = a_cb(step.audio_samples.as_ptr(), stereo_frames);
        }

        if let Some(ref player) = slot.state.player {
            let sram = player.save_ram();
            if sram.len() == 64 * 1024 {
                slot.state.save_ram.clear();
                slot.state.save_ram.extend_from_slice(sram);
            }
        }
    });
}

/// Save states are not supported in v1; returns `0` (`libretro.h:8966`).
#[no_mangle]
pub extern "C" fn retro_serialize_size() -> usize {
    0
}

/// Save states are not supported in v1; returns `false` (`libretro.h:8980`).
#[no_mangle]
pub extern "C" fn retro_serialize(_data: *mut c_void, _len: usize) -> bool {
    false
}

/// Save states are not supported in v1; returns `false` (`libretro.h:8989`).
#[no_mangle]
pub extern "C" fn retro_unserialize(_data: *const c_void, _len: usize) -> bool {
    false
}

/// No-op cheat reset (`libretro.h:8996`).
#[no_mangle]
pub extern "C" fn retro_cheat_reset() {}

/// No-op cheat set (`libretro.h:9007`).
#[no_mangle]
pub extern "C" fn retro_cheat_set(_index: c_uint, _enabled: bool, _code: *const c_char) {}

/// Returns `RETRO_REGION_NTSC` (`0`) (`libretro.h:9071`).
#[no_mangle]
pub extern "C" fn retro_get_region() -> c_uint {
    RETRO_REGION_NTSC
}

/// Returns a pointer to the 64 KiB `SharedObject` `SAVE_RAM` buffer for `RETRO_MEMORY_SAVE_RAM` (`0`), or null (`libretro.h:9085`, TSD 2.5).
#[no_mangle]
pub extern "C" fn retro_get_memory_data(id: c_uint) -> *mut c_void {
    if id != RETRO_MEMORY_SAVE_RAM {
        return core::ptr::null_mut();
    }
    with_global(core::ptr::null_mut(), |slot| {
        if slot.state.save_ram.len() != 64 * 1024 {
            slot.state.save_ram = vec![0_u8; 64 * 1024];
            if let Some(hdr) = slot.state.save_ram.get_mut(0..4) {
                hdr.copy_from_slice(b"SWFS");
            }
        }
        slot.state.save_ram.as_mut_ptr().cast::<c_void>()
    })
}

/// Returns `65536` (`64 KiB`) for `RETRO_MEMORY_SAVE_RAM` (`0`), or `0` for any other memory ID (`libretro.h:9099`, TSD 2.5).
#[no_mangle]
pub extern "C" fn retro_get_memory_size(id: c_uint) -> usize {
    if id == RETRO_MEMORY_SAVE_RAM {
        64 * 1024
    } else {
        0
    }
}

/// Copies a byte slice from a `retro_get_memory_data` pointer for safe integration testing.
#[must_use]
pub fn copy_memory_bytes_from_ptr(data: *const c_void, len: usize) -> Vec<u8> {
    if data.is_null() || len == 0 {
        return Vec::new();
    }
    // SAFETY: Caller passes a valid pointer returned by `retro_get_memory_data` of at least `len` bytes.
    let slice = unsafe { core::slice::from_raw_parts(data.cast::<u8>(), len) };
    slice.to_vec()
}
