//! Automated verification of `swfr_libretro::ffi` constants against `docs/reference/libretro.h`
//! and `swfr_libretro.info` field names against `docs/reference/00_example_libretro.info` (TSD 2.5, 2.12 M0).

use std::collections::HashMap;
use std::path::PathBuf;
use swfr_libretro::ffi;

fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .map_or_else(|| manifest_dir.clone(), PathBuf::from)
}

fn parse_header_defines(header_text: &str) -> HashMap<String, u64> {
    let mut map = HashMap::new();
    for raw_line in header_text.lines() {
        let line = raw_line.trim();
        let Some(rest) = line.strip_prefix("#define ") else {
            continue;
        };
        let rest = rest.trim();
        let mut parts = rest.splitn(2, char::is_whitespace);
        let Some(name) = parts.next() else {
            continue;
        };
        if name.contains('(') {
            continue;
        }
        let Some(val_part) = parts.next() else {
            continue;
        };
        // Strip trailing comments (`/* ... */` or `// ...`).
        let val_clean = val_part
            .split("/*")
            .next()
            .unwrap_or("")
            .split("//")
            .next()
            .unwrap_or("")
            .trim();
        if let Some(hex) = val_clean.strip_prefix("0x") {
            if let Ok(v) = u64::from_str_radix(hex, 16) {
                map.insert(name.to_owned(), v);
            }
        } else if val_clean == "(45 | RETRO_ENVIRONMENT_EXPERIMENTAL)" {
            if let Some(&exp) = map.get("RETRO_ENVIRONMENT_EXPERIMENTAL") {
                map.insert(name.to_owned(), 45 | exp);
            }
        } else if let Ok(v) = val_clean.parse::<u64>() {
            map.insert(name.to_owned(), v);
        }
    }
    map
}

#[test]
fn all_ffi_defines_match_libretro_h() {
    let header_path = repo_root().join("docs/reference/libretro.h");
    let header_text = std::fs::read_to_string(&header_path).unwrap_or_default();
    assert!(
        !header_text.is_empty(),
        "docs/reference/libretro.h must exist and be non-empty"
    );

    let defines = parse_header_defines(&header_text);

    let expected: &[(&str, u64)] = &[
        ("RETRO_API_VERSION", u64::from(ffi::RETRO_API_VERSION)),
        ("RETRO_DEVICE_NONE", u64::from(ffi::RETRO_DEVICE_NONE)),
        ("RETRO_DEVICE_JOYPAD", u64::from(ffi::RETRO_DEVICE_JOYPAD)),
        ("RETRO_DEVICE_MOUSE", u64::from(ffi::RETRO_DEVICE_MOUSE)),
        (
            "RETRO_DEVICE_KEYBOARD",
            u64::from(ffi::RETRO_DEVICE_KEYBOARD),
        ),
        (
            "RETRO_DEVICE_LIGHTGUN",
            u64::from(ffi::RETRO_DEVICE_LIGHTGUN),
        ),
        ("RETRO_DEVICE_ANALOG", u64::from(ffi::RETRO_DEVICE_ANALOG)),
        ("RETRO_DEVICE_POINTER", u64::from(ffi::RETRO_DEVICE_POINTER)),
        (
            "RETRO_DEVICE_ID_JOYPAD_B",
            u64::from(ffi::RETRO_DEVICE_ID_JOYPAD_B),
        ),
        (
            "RETRO_DEVICE_ID_JOYPAD_Y",
            u64::from(ffi::RETRO_DEVICE_ID_JOYPAD_Y),
        ),
        (
            "RETRO_DEVICE_ID_JOYPAD_SELECT",
            u64::from(ffi::RETRO_DEVICE_ID_JOYPAD_SELECT),
        ),
        (
            "RETRO_DEVICE_ID_JOYPAD_START",
            u64::from(ffi::RETRO_DEVICE_ID_JOYPAD_START),
        ),
        (
            "RETRO_DEVICE_ID_JOYPAD_UP",
            u64::from(ffi::RETRO_DEVICE_ID_JOYPAD_UP),
        ),
        (
            "RETRO_DEVICE_ID_JOYPAD_DOWN",
            u64::from(ffi::RETRO_DEVICE_ID_JOYPAD_DOWN),
        ),
        (
            "RETRO_DEVICE_ID_JOYPAD_LEFT",
            u64::from(ffi::RETRO_DEVICE_ID_JOYPAD_LEFT),
        ),
        (
            "RETRO_DEVICE_ID_JOYPAD_RIGHT",
            u64::from(ffi::RETRO_DEVICE_ID_JOYPAD_RIGHT),
        ),
        (
            "RETRO_DEVICE_ID_JOYPAD_A",
            u64::from(ffi::RETRO_DEVICE_ID_JOYPAD_A),
        ),
        (
            "RETRO_DEVICE_ID_JOYPAD_X",
            u64::from(ffi::RETRO_DEVICE_ID_JOYPAD_X),
        ),
        (
            "RETRO_DEVICE_ID_JOYPAD_L",
            u64::from(ffi::RETRO_DEVICE_ID_JOYPAD_L),
        ),
        (
            "RETRO_DEVICE_ID_JOYPAD_R",
            u64::from(ffi::RETRO_DEVICE_ID_JOYPAD_R),
        ),
        (
            "RETRO_DEVICE_ID_JOYPAD_L2",
            u64::from(ffi::RETRO_DEVICE_ID_JOYPAD_L2),
        ),
        (
            "RETRO_DEVICE_ID_JOYPAD_R2",
            u64::from(ffi::RETRO_DEVICE_ID_JOYPAD_R2),
        ),
        (
            "RETRO_DEVICE_ID_JOYPAD_L3",
            u64::from(ffi::RETRO_DEVICE_ID_JOYPAD_L3),
        ),
        (
            "RETRO_DEVICE_ID_JOYPAD_R3",
            u64::from(ffi::RETRO_DEVICE_ID_JOYPAD_R3),
        ),
        (
            "RETRO_DEVICE_ID_JOYPAD_MASK",
            u64::from(ffi::RETRO_DEVICE_ID_JOYPAD_MASK),
        ),
        (
            "RETRO_DEVICE_INDEX_ANALOG_LEFT",
            u64::from(ffi::RETRO_DEVICE_INDEX_ANALOG_LEFT),
        ),
        (
            "RETRO_DEVICE_INDEX_ANALOG_RIGHT",
            u64::from(ffi::RETRO_DEVICE_INDEX_ANALOG_RIGHT),
        ),
        (
            "RETRO_DEVICE_INDEX_ANALOG_BUTTON",
            u64::from(ffi::RETRO_DEVICE_INDEX_ANALOG_BUTTON),
        ),
        (
            "RETRO_DEVICE_ID_ANALOG_X",
            u64::from(ffi::RETRO_DEVICE_ID_ANALOG_X),
        ),
        (
            "RETRO_DEVICE_ID_ANALOG_Y",
            u64::from(ffi::RETRO_DEVICE_ID_ANALOG_Y),
        ),
        (
            "RETRO_DEVICE_ID_MOUSE_X",
            u64::from(ffi::RETRO_DEVICE_ID_MOUSE_X),
        ),
        (
            "RETRO_DEVICE_ID_MOUSE_Y",
            u64::from(ffi::RETRO_DEVICE_ID_MOUSE_Y),
        ),
        (
            "RETRO_DEVICE_ID_MOUSE_LEFT",
            u64::from(ffi::RETRO_DEVICE_ID_MOUSE_LEFT),
        ),
        (
            "RETRO_DEVICE_ID_MOUSE_RIGHT",
            u64::from(ffi::RETRO_DEVICE_ID_MOUSE_RIGHT),
        ),
        (
            "RETRO_DEVICE_ID_MOUSE_WHEELUP",
            u64::from(ffi::RETRO_DEVICE_ID_MOUSE_WHEELUP),
        ),
        (
            "RETRO_DEVICE_ID_MOUSE_WHEELDOWN",
            u64::from(ffi::RETRO_DEVICE_ID_MOUSE_WHEELDOWN),
        ),
        (
            "RETRO_DEVICE_ID_MOUSE_MIDDLE",
            u64::from(ffi::RETRO_DEVICE_ID_MOUSE_MIDDLE),
        ),
        (
            "RETRO_DEVICE_ID_POINTER_X",
            u64::from(ffi::RETRO_DEVICE_ID_POINTER_X),
        ),
        (
            "RETRO_DEVICE_ID_POINTER_Y",
            u64::from(ffi::RETRO_DEVICE_ID_POINTER_Y),
        ),
        (
            "RETRO_DEVICE_ID_POINTER_PRESSED",
            u64::from(ffi::RETRO_DEVICE_ID_POINTER_PRESSED),
        ),
        (
            "RETRO_DEVICE_ID_POINTER_COUNT",
            u64::from(ffi::RETRO_DEVICE_ID_POINTER_COUNT),
        ),
        ("RETRO_REGION_NTSC", u64::from(ffi::RETRO_REGION_NTSC)),
        ("RETRO_REGION_PAL", u64::from(ffi::RETRO_REGION_PAL)),
        (
            "RETRO_ENVIRONMENT_EXPERIMENTAL",
            u64::from(ffi::RETRO_ENVIRONMENT_EXPERIMENTAL),
        ),
        (
            "RETRO_ENVIRONMENT_GET_CAN_DUPE",
            u64::from(ffi::RETRO_ENVIRONMENT_GET_CAN_DUPE),
        ),
        (
            "RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL",
            u64::from(ffi::RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL),
        ),
        (
            "RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY",
            u64::from(ffi::RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY),
        ),
        (
            "RETRO_ENVIRONMENT_SET_PIXEL_FORMAT",
            u64::from(ffi::RETRO_ENVIRONMENT_SET_PIXEL_FORMAT),
        ),
        (
            "RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS",
            u64::from(ffi::RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS),
        ),
        (
            "RETRO_ENVIRONMENT_SET_KEYBOARD_CALLBACK",
            u64::from(ffi::RETRO_ENVIRONMENT_SET_KEYBOARD_CALLBACK),
        ),
        (
            "RETRO_ENVIRONMENT_GET_VARIABLE",
            u64::from(ffi::RETRO_ENVIRONMENT_GET_VARIABLE),
        ),
        (
            "RETRO_ENVIRONMENT_SET_VARIABLES",
            u64::from(ffi::RETRO_ENVIRONMENT_SET_VARIABLES),
        ),
        (
            "RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE",
            u64::from(ffi::RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE),
        ),
        (
            "RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME",
            u64::from(ffi::RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME),
        ),
        (
            "RETRO_ENVIRONMENT_GET_LOG_INTERFACE",
            u64::from(ffi::RETRO_ENVIRONMENT_GET_LOG_INTERFACE),
        ),
        (
            "RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY",
            u64::from(ffi::RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY),
        ),
        (
            "RETRO_ENVIRONMENT_SET_GEOMETRY",
            u64::from(ffi::RETRO_ENVIRONMENT_SET_GEOMETRY),
        ),
        (
            "RETRO_ENVIRONMENT_GET_VFS_INTERFACE",
            u64::from(ffi::RETRO_ENVIRONMENT_GET_VFS_INTERFACE),
        ),
        (
            "RETRO_ENVIRONMENT_GET_CORE_OPTIONS_VERSION",
            u64::from(ffi::RETRO_ENVIRONMENT_GET_CORE_OPTIONS_VERSION),
        ),
        (
            "RETRO_ENVIRONMENT_SET_CORE_OPTIONS_V2",
            u64::from(ffi::RETRO_ENVIRONMENT_SET_CORE_OPTIONS_V2),
        ),
        (
            "RETRO_NUM_CORE_OPTION_VALUES_MAX",
            ffi::RETRO_NUM_CORE_OPTION_VALUES_MAX as u64,
        ),
    ];

    for &(name, rust_val) in expected {
        let header_val = defines.get(name).copied();
        assert_eq!(
            header_val,
            Some(rust_val),
            "constant {name} in ffi ({rust_val}) did not match docs/reference/libretro.h ({header_val:?})"
        );
    }

    // Verify enum variants in docs/reference/libretro.h text.
    assert!(header_text.contains("RETRO_PIXEL_FORMAT_0RGB1555 = 0"));
    assert!(header_text.contains("RETRO_PIXEL_FORMAT_XRGB8888 = 1"));
    assert!(header_text.contains("RETRO_PIXEL_FORMAT_RGB565   = 2"));
    assert!(header_text.contains("RETRO_LOG_DEBUG = 0"));
}

#[test]
fn core_info_fields_match_example_info() {
    let example_path = repo_root().join("docs/reference/00_example_libretro.info");
    let example_text = std::fs::read_to_string(example_path).unwrap_or_default();
    let info_path = repo_root().join("swfr_libretro.info");
    let info_text = std::fs::read_to_string(info_path).unwrap_or_default();
    assert!(!info_text.is_empty(), "swfr_libretro.info must exist");

    for raw_line in info_text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, _val)) = line.split_once('=') {
            let key = key.trim();
            let needle = format!("{key} =");
            assert!(
                example_text.contains(&needle),
                "field `{key}` in swfr_libretro.info is not documented in docs/reference/00_example_libretro.info"
            );
        }
    }
}
