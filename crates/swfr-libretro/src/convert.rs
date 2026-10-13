//! Pixel format conversion and fallback frame helpers for `swfr_libretro` (TSD 2.5).

/// Dark red `XRGB8888` pixel (`0x00800000`) displayed when the core enters a dead state after a panic (TSD 2.5).
pub const DEAD_FRAME_XRGB: u32 = 0x0080_0000;

/// Packs 8-bit `(r, g, b)` channels into a native-endian `XRGB8888` (`0x00RRGGBB`) word (TSD 2.5).
#[must_use]
pub const fn pack_xrgb8888(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Fills `buf` with the solid dark red panic-fallback color (`0x00800000`).
pub fn fill_dead_frame(buf: &mut [u32]) {
    buf.fill(DEAD_FRAME_XRGB);
}
