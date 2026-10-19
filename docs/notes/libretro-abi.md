# `libretro.h` ABI Reference Notes (`docs/notes/libretro-abi.md`)

- **Primary Source:** `docs/reference/libretro.h` (fetched from `https://raw.githubusercontent.com/libretro/libretro-common/master/include/libretro.h`, SHA-256 `036ddffc995ad116c3081ca0a7515d09c2a116f3483e4f5012fe0c29a885da76`)
- **Secondary Source:** `docs/reference/00_example_libretro.info` (fetched from `https://raw.githubusercontent.com/libretro/libretro-super/master/dist/info/00_example_libretro.info`, SHA-256 `80d512323d2b80a10f66f95304c06a1c21e6101026220f8c8e50a4031113a086`)

Every numeric constant, enum variant, struct layout, callback signature, and exported function signature used by `swfr-libretro` is listed below with its exact line number in `docs/reference/libretro.h`.

---

## 1. Core API Version, Region & Device Constants

| Constant | Value | Line in `docs/reference/libretro.h` |
| :--- | :--- | :--- |
| `RETRO_API_VERSION` | `1` | Line 97 |
| `RETRO_DEVICE_NONE` | `0` | Line 153 |
| `RETRO_DEVICE_JOYPAD` | `1` | Line 176 |
| `RETRO_DEVICE_MOUSE` | `2` | Line 202 |
| `RETRO_DEVICE_KEYBOARD` | `3` | Line 219 |
| `RETRO_DEVICE_LIGHTGUN` | `4` | Line 238 |
| `RETRO_DEVICE_ANALOG` | `5` | Line 260 |
| `RETRO_DEVICE_POINTER` | `6` | Line 305 |
| `RETRO_REGION_NTSC` | `0` | Line 443 |
| `RETRO_REGION_PAL` | `1` | Line 444 |

### Joypad Button IDs (`RETRO_DEVICE_ID_JOYPAD_*`)

| Constant | Value | Line in `docs/reference/libretro.h` |
| :--- | :--- | :--- |
| `RETRO_DEVICE_ID_JOYPAD_B` | `0` | Line 320 |
| `RETRO_DEVICE_ID_JOYPAD_Y` | `1` | Line 323 |
| `RETRO_DEVICE_ID_JOYPAD_SELECT` | `2` | Line 326 |
| `RETRO_DEVICE_ID_JOYPAD_START` | `3` | Line 329 |
| `RETRO_DEVICE_ID_JOYPAD_UP` | `4` | Line 332 |
| `RETRO_DEVICE_ID_JOYPAD_DOWN` | `5` | Line 335 |
| `RETRO_DEVICE_ID_JOYPAD_LEFT` | `6` | Line 338 |
| `RETRO_DEVICE_ID_JOYPAD_RIGHT` | `7` | Line 341 |
| `RETRO_DEVICE_ID_JOYPAD_A` | `8` | Line 344 |
| `RETRO_DEVICE_ID_JOYPAD_X` | `9` | Line 347 |
| `RETRO_DEVICE_ID_JOYPAD_L` | `10` | Line 350 |
| `RETRO_DEVICE_ID_JOYPAD_R` | `11` | Line 353 |
| `RETRO_DEVICE_ID_JOYPAD_L2` | `12` | Line 356 |
| `RETRO_DEVICE_ID_JOYPAD_R2` | `13` | Line 359 |
| `RETRO_DEVICE_ID_JOYPAD_L3` | `14` | Line 365 |
| `RETRO_DEVICE_ID_JOYPAD_R3` | `15` | Line 371 |
| `RETRO_DEVICE_ID_JOYPAD_MASK` | `256` | Line 380 |

### Analog Stick & Axis IDs

| Constant | Value | Line in `docs/reference/libretro.h` |
| :--- | :--- | :--- |
| `RETRO_DEVICE_INDEX_ANALOG_LEFT` | `0` | Line 389 |
| `RETRO_DEVICE_INDEX_ANALOG_RIGHT` | `1` | Line 390 |
| `RETRO_DEVICE_INDEX_ANALOG_BUTTON` | `2` | Line 391 |
| `RETRO_DEVICE_ID_ANALOG_X` | `0` | Line 392 |
| `RETRO_DEVICE_ID_ANALOG_Y` | `1` | Line 393 |

### Mouse IDs (`RETRO_DEVICE_ID_MOUSE_*`)

| Constant | Value | Line in `docs/reference/libretro.h` |
| :--- | :--- | :--- |
| `RETRO_DEVICE_ID_MOUSE_X` | `0` | Line 398 |
| `RETRO_DEVICE_ID_MOUSE_Y` | `1` | Line 399 |
| `RETRO_DEVICE_ID_MOUSE_LEFT` | `2` | Line 400 |
| `RETRO_DEVICE_ID_MOUSE_RIGHT` | `3` | Line 401 |
| `RETRO_DEVICE_ID_MOUSE_WHEELUP` | `4` | Line 402 |
| `RETRO_DEVICE_ID_MOUSE_WHEELDOWN` | `5` | Line 403 |
| `RETRO_DEVICE_ID_MOUSE_MIDDLE` | `6` | Line 404 |

### Pointer IDs (`RETRO_DEVICE_ID_POINTER_*`)

| Constant | Value | Line in `docs/reference/libretro.h` |
| :--- | :--- | :--- |
| `RETRO_DEVICE_ID_POINTER_X` | `0` | Line 434 |
| `RETRO_DEVICE_ID_POINTER_Y` | `1` | Line 435 |
| `RETRO_DEVICE_ID_POINTER_PRESSED` | `2` | Line 436 |
| `RETRO_DEVICE_ID_POINTER_COUNT` | `3` | Line 437 |

---

## 2. Environment Command Constants (`RETRO_ENVIRONMENT_*`)

| Constant | Value | Line in `docs/reference/libretro.h` |
| :--- | :--- | :--- |
| `RETRO_ENVIRONMENT_EXPERIMENTAL` | `0x10000` (`65536`) | Line 730 |
| `RETRO_ENVIRONMENT_GET_CAN_DUPE` | `3` | Line 768 |
| `RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL` | `8` | Line 837 |
| `RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY` | `9` | Line 855 |
| `RETRO_ENVIRONMENT_SET_PIXEL_FORMAT` | `10` | Line 870 |
| `RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS` | `11` | Line 887 |
| `RETRO_ENVIRONMENT_SET_KEYBOARD_CALLBACK` | `12` | Line 901 |
| `RETRO_ENVIRONMENT_GET_VARIABLE` | `15` | Line 971 |
| `RETRO_ENVIRONMENT_SET_VARIABLES` | `16` | Line 1021 |
| `RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE` | `17` | Line 1039 |
| `RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME` | `18` | Line 1056 |
| `RETRO_ENVIRONMENT_GET_LOG_INTERFACE` | `27` | Line 1248 |
| `RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY` | `31` | Line 1331 |
| `RETRO_ENVIRONMENT_SET_GEOMETRY` | `37` | Line 1559 |
| `RETRO_ENVIRONMENT_GET_VFS_INTERFACE` | `45 \| RETRO_ENVIRONMENT_EXPERIMENTAL` (`65581`) | Line 1732 |
| `RETRO_ENVIRONMENT_GET_CORE_OPTIONS_VERSION` | `52` | Line 1871 |
| `RETRO_ENVIRONMENT_SET_CORE_OPTIONS_V2` | `67` | Line 2362 |
| `RETRO_NUM_CORE_OPTION_VALUES_MAX` | `128` | Line 7573 |

---

## 3. Enums and Struct Layouts

1. **`enum retro_log_level`** (Lines 4991–5007):
   - `RETRO_LOG_DEBUG = 0` (Line 4994)
   - `RETRO_LOG_INFO = 1` (Line 4997)
   - `RETRO_LOG_WARN = 2` (Line 5000)
   - `RETRO_LOG_ERROR = 3` (Line 5003)
   - `RETRO_LOG_DUMMY = INT_MAX` (Line 5006; ensures `sizeof(enum retro_log_level) == sizeof(int)`).
2. **`struct retro_log_callback`** (Lines 5032–5040):
   - `log: retro_log_printf_t` (`extern "C" fn(level: c_uint, fmt: *const c_char, ...)`).
3. **`struct retro_keyboard_callback`** (Lines 6304–6310):
   - `callback: retro_keyboard_event_t` (`extern "C" fn(down: bool, keycode: c_uint, character: u32, key_modifiers: u16)`).
4. **`enum retro_pixel_format`** (Lines 6719–6761):
   - `RETRO_PIXEL_FORMAT_0RGB1555 = 0` (Line 6729)
   - `RETRO_PIXEL_FORMAT_XRGB8888 = 1` (Line 6735)
   - `RETRO_PIXEL_FORMAT_RGB565 = 2` (Line 6742)
   - `RETRO_PIXEL_FORMAT_UNKNOWN = INT_MAX` (Line 6760).
5. **`struct retro_input_descriptor`** (Lines 7111–7123):
   - `port: c_uint`, `device: c_uint`, `index: c_uint`, `id: c_uint`, `description: *const c_char`. Terminated by a zeroed entry (`description == NULL`).
6. **`struct retro_system_info`** (Lines 7132–7179):
   - `library_name: *const c_char`
   - `library_version: *const c_char`
   - `valid_extensions: *const c_char`
   - `need_fullpath: bool`
   - `block_extract: bool`
7. **`struct retro_game_geometry`** (Lines 7395–7439):
   - `base_width: c_uint`
   - `base_height: c_uint`
   - `max_width: c_uint`
   - `max_height: c_uint`
   - `aspect_ratio: c_float`
8. **`struct retro_system_timing`** (Lines 7447–7454):
   - `fps: c_double`
   - `sample_rate: c_double`
9. **`struct retro_system_av_info`** (Lines 7461–7468):
   - `geometry: retro_game_geometry`
   - `timing: retro_system_timing`
10. **`struct retro_variable`** (Lines 7483–7522):
    - `key: *const c_char`
    - `value: *const c_char`
11. **`struct retro_core_option_value`** (Lines 7584–7628):
    - `value: *const c_char`
    - `label: *const c_char`
12. **`struct retro_core_option_v2_category`** (Lines 7700–7735):
    - `key: *const c_char`
    - `desc: *const c_char`
    - `info: *const c_char`
13. **`struct retro_core_option_v2_definition`** (Lines 7747–7864):
    - `key: *const c_char`
    - `desc: *const c_char`
    - `desc_categorized: *const c_char`
    - `info: *const c_char`
    - `info_categorized: *const c_char`
    - `category_key: *const c_char`
    - `values: [retro_core_option_value; 128]`
    - `default_value: *const c_char`
14. **`struct retro_core_options_v2`** (Lines 7872–7893):
    - `categories: *const retro_core_option_v2_category`
    - `definitions: *const retro_core_option_v2_definition`
15. **`struct retro_game_info`** (Lines 7981–7995):
    - `path: *const c_char`
    - `data: *const c_void`
    - `size: usize`
    - `meta: *const c_char`

---

## 4. Callback Typedefs & Exported Entrypoints

### Callback Typedefs (Lines 8605–8759)
- `retro_environment_t` (Line 8605): `extern "C" fn(cmd: c_uint, data: *mut c_void) -> bool`
- `retro_video_refresh_t` (Line 8624): `extern "C" fn(data: *const c_void, width: c_uint, height: c_uint, pitch: usize)`
- `retro_audio_sample_t` (Line 8636): `extern "C" fn(left: i16, right: i16)`
- `retro_audio_sample_batch_t` (Line 8653): `extern "C" fn(data: *const i16, frames: usize) -> usize`
- `retro_input_poll_t` (Line 8729): `extern "C" fn()`
- `retro_input_state_t` (Line 8758): `extern "C" fn(port: c_uint, device: c_uint, index: c_uint, id: c_uint) -> i16`

### Required Exported Functions (Lines 8770–9099)
- `retro_set_environment` (Line 8770)
- `retro_set_video_refresh` (Line 8779)
- `retro_set_audio_sample` (Line 8788)
- `retro_set_audio_sample_batch` (Line 8797)
- `retro_set_input_poll` (Line 8806)
- `retro_set_input_state` (Line 8815)
- `retro_init` (Line 8841)
- `retro_deinit` (Line 8866)
- `retro_api_version` (Line 8877)
- `retro_get_system_info` (Line 8886)
- `retro_get_system_av_info` (Line 8901)
- `retro_set_controller_port_device` (Line 8932)
- `retro_reset` (Line 8939)
- `retro_run` (Line 8953)
- `retro_serialize_size` (Line 8966)
- `retro_serialize` (Line 8980)
- `retro_unserialize` (Line 8989)
- `retro_cheat_reset` (Line 8996)
- `retro_cheat_set` (Line 9007)
- `retro_load_game` (Line 9020)
- `retro_load_game_special` (Line 9044)
- `retro_unload_game` (Line 9057)
- `retro_get_region` (Line 9071)
- `retro_get_memory_data` (Line 9085)
- `retro_get_memory_size` (Line 9099)
