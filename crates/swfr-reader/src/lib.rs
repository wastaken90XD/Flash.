//! Zero-copy bounds-checked bit and byte stream reader primitives for `swfr` (TSD 2.6, M1).
//!
//! Layout and bit-packing rules follow `docs/notes/swf-format.md` and `docs/reference/swf.ksy`.

/// Error kind returned by [`Reader`] methods when parsing fails (TSD 2.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseErrorKind {
    /// Unexpected end of input buffer.
    UnexpectedEof,
    /// Invalid bit count requested (`n > 32`).
    InvalidBitCount(u8),
    /// Invalid UTF-8 or NUL-terminated string.
    InvalidString,
    /// Invalid container signature or header field.
    InvalidHeader(&'static str),
    /// Decompression failed or exceeded configured limits.
    DecompressionError(&'static str),
    /// A safety limit from `Limits` was exceeded.
    LimitExceeded(&'static str),
    /// Malformed tag payload.
    MalformedTag {
        /// SWF tag code.
        code: u16,
        /// Reason description.
        reason: &'static str,
    },
}

/// Structured parse error carrying the byte offset and [`ParseErrorKind`] (TSD 2.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// Byte offset in the stream where the error occurred.
    pub offset: usize,
    /// Specific error kind.
    pub kind: ParseErrorKind,
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "parse error at byte offset {}: {:?}", self.offset, self.kind)
    }
}

impl std::error::Error for ParseError {}

/// SWF `RECT` bounding box in twips (`1/20` of a pixel) (`docs/notes/swf-format.md`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rect {
    /// Minimum X coordinate in twips.
    pub x_min: i32,
    /// Maximum X coordinate in twips.
    pub x_max: i32,
    /// Minimum Y coordinate in twips.
    pub y_min: i32,
    /// Maximum Y coordinate in twips.
    pub y_max: i32,
}

impl Rect {
    /// Returns the width in twips (`(x_max - x_min).max(0)`).
    #[must_use]
    pub fn width_twips(self) -> i32 {
        self.x_max.saturating_sub(self.x_min).max(0)
    }

    /// Returns the height in twips (`(y_max - y_min).max(0)`).
    #[must_use]
    pub fn height_twips(self) -> i32 {
        self.y_max.saturating_sub(self.y_min).max(0)
    }

    /// Returns the width in whole pixels (`width_twips / 20`).
    #[must_use]
    pub fn width_px(self) -> u32 {
        u32::try_from(self.width_twips().checked_div(20).unwrap_or(0)).unwrap_or(0)
    }

    /// Returns the height in whole pixels (`height_twips / 20`).
    #[must_use]
    pub fn height_px(self) -> u32 {
        u32::try_from(self.height_twips().checked_div(20).unwrap_or(0)).unwrap_or(0)
    }
}

/// SWF 2D affine transform `MATRIX` (`docs/notes/swf-format.md`).
///
/// Scale and rotate/skew components are stored as 16.16 fixed-point integers (`0x0001_0000 == 1.0`).
/// Translation components are in twips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Matrix {
    /// X scale in 16.16 fixed-point (default `0x0001_0000`).
    pub scale_x_fp16: i32,
    /// Y scale in 16.16 fixed-point (default `0x0001_0000`).
    pub scale_y_fp16: i32,
    /// First rotate/skew component in 16.16 fixed-point (default `0`).
    pub rotate_skew0_fp16: i32,
    /// Second rotate/skew component in 16.16 fixed-point (default `0`).
    pub rotate_skew1_fp16: i32,
    /// X translation in twips.
    pub translate_x_twips: i32,
    /// Y translation in twips.
    pub translate_y_twips: i32,
}

impl Default for Matrix {
    fn default() -> Self {
        Self::identity()
    }
}

impl Matrix {
    /// Returns the identity matrix (`scale = 1.0`, `rotate = 0.0`, `translate = 0`).
    #[must_use]
    pub const fn identity() -> Self {
        Self {
            scale_x_fp16: 0x0001_0000,
            scale_y_fp16: 0x0001_0000,
            rotate_skew0_fp16: 0,
            rotate_skew1_fp16: 0,
            translate_x_twips: 0,
            translate_y_twips: 0,
        }
    }

    /// Transforms a point `(x_twips, y_twips)` by this matrix into twips.
    #[must_use]
    #[allow(clippy::arithmetic_side_effects)]
    pub fn transform_twips(self, x_twips: i32, y_twips: i32) -> (i32, i32) {
        let x64 = i64::from(x_twips);
        let y64 = i64::from(y_twips);
        let out_x = ((x64 * i64::from(self.scale_x_fp16)
            + y64 * i64::from(self.rotate_skew1_fp16))
            >> 16)
            + i64::from(self.translate_x_twips);
        let out_y = ((x64 * i64::from(self.rotate_skew0_fp16)
            + y64 * i64::from(self.scale_y_fp16))
            >> 16)
            + i64::from(self.translate_y_twips);
        (
            i32::try_from(out_x).unwrap_or(0),
            i32::try_from(out_y).unwrap_or(0),
        )
    }
}

/// SWF color transform (`CXFORM` and `CXFORMWITHALPHA`, `docs/notes/swf-format.md`).
///
/// Multiplier terms use 8.8 fixed point (`256 == 1.0`). Addition terms are signed offsets in `-255..=255`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorTransform {
    /// Red multiplier (`256 == 1.0`).
    pub r_mult: i16,
    /// Green multiplier (`256 == 1.0`).
    pub g_mult: i16,
    /// Blue multiplier (`256 == 1.0`).
    pub b_mult: i16,
    /// Alpha multiplier (`256 == 1.0`).
    pub a_mult: i16,
    /// Red additive offset.
    pub r_add: i16,
    /// Green additive offset.
    pub g_add: i16,
    /// Blue additive offset.
    pub b_add: i16,
    /// Alpha additive offset.
    pub a_add: i16,
}

impl Default for ColorTransform {
    fn default() -> Self {
        Self::identity()
    }
}

impl ColorTransform {
    /// Returns the identity color transform (`mult = 256`, `add = 0`).
    #[must_use]
    pub const fn identity() -> Self {
        Self {
            r_mult: 256,
            g_mult: 256,
            b_mult: 256,
            a_mult: 256,
            r_add: 0,
            g_add: 0,
            b_add: 0,
            a_add: 0,
        }
    }

    /// Applies this color transform to an `(r, g, b, a)` color tuple.
    #[must_use]
    #[allow(clippy::arithmetic_side_effects)]
    pub fn apply(self, r: u8, g: u8, b: u8, a: u8) -> (u8, u8, b_u8::Byte, u8) {
        let tr = ((i32::from(r) * i32::from(self.r_mult)) >> 8) + i32::from(self.r_add);
        let tg = ((i32::from(g) * i32::from(self.g_mult)) >> 8) + i32::from(self.g_add);
        let tb = ((i32::from(b) * i32::from(self.b_mult)) >> 8) + i32::from(self.b_add);
        let ta = ((i32::from(a) * i32::from(self.a_mult)) >> 8) + i32::from(self.a_add);
        (
            u8::try_from(tr.clamp(0, 255)).unwrap_or(0),
            u8::try_from(tg.clamp(0, 255)).unwrap_or(0),
            u8::try_from(tb.clamp(0, 255)).unwrap_or(0),
            u8::try_from(ta.clamp(0, 255)).unwrap_or(0),
        )
    }
}

mod b_u8 {
    pub type Byte = u8;
}

/// Bounds-checked bit and byte reader over a borrowed byte slice (TSD 2.6).
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_buf: u8,
    bits_left: u8,
}

impl<'a> Reader<'a> {
    /// Creates a new [`Reader`] positioned at byte 0 of `data`.
    #[must_use]
    pub const fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_buf: 0,
            bits_left: 0,
        }
    }

    /// Returns the current byte offset in the underlying slice.
    #[must_use]
    pub const fn position(&self) -> usize {
        self.byte_pos
    }

    /// Returns the number of unread whole bytes remaining in the buffer.
    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.byte_pos)
    }

    /// Returns `true` if no whole bytes or buffered bits remain.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.byte_pos >= self.data.len() && self.bits_left == 0
    }

    /// Discards any partial byte in the bit buffer so subsequent reads are byte-aligned (TSD 2.6).
    pub fn align(&mut self) {
        self.bit_buf = 0;
        self.bits_left = 0;
    }

    /// Reads an unsigned 8-bit integer after aligning to the next byte boundary.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] with [`ParseErrorKind::UnexpectedEof`] if no bytes remain.
    pub fn u8(&mut self) -> Result<u8, ParseError> {
        self.align();
        let offset = self.byte_pos;
        let &b = self.data.get(self.byte_pos).ok_or(ParseError {
            offset,
            kind: ParseErrorKind::UnexpectedEof,
        })?;
        self.byte_pos = self.byte_pos.saturating_add(1);
        Ok(b)
    }

    /// Reads a little-endian unsigned 16-bit integer after byte alignment.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if fewer than 2 bytes remain.
    pub fn u16(&mut self) -> Result<u16, ParseError> {
        self.align();
        let offset = self.byte_pos;
        let slice = self.bytes(2).map_err(|_| ParseError {
            offset,
            kind: ParseErrorKind::UnexpectedEof,
        })?;
        let lo = slice.first().copied().unwrap_or(0);
        let hi = slice.get(1).copied().unwrap_or(0);
        Ok(u16::from_le_bytes([lo, hi]))
    }

    /// Reads a little-endian unsigned 32-bit integer after byte alignment.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if fewer than 4 bytes remain.
    pub fn u32(&mut self) -> Result<u32, ParseError> {
        self.align();
        let offset = self.byte_pos;
        let slice = self.bytes(4).map_err(|_| ParseError {
            offset,
            kind: ParseErrorKind::UnexpectedEof,
        })?;
        let b0 = slice.first().copied().unwrap_or(0);
        let b1 = slice.get(1).copied().unwrap_or(0);
        let b2 = slice.get(2).copied().unwrap_or(0);
        let b3 = slice.get(3).copied().unwrap_or(0);
        Ok(u32::from_le_bytes([b0, b1, b2, b3]))
    }

    /// Reads a little-endian signed 16-bit integer after byte alignment.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if fewer than 2 bytes remain.
    pub fn i16(&mut self) -> Result<i16, ParseError> {
        Ok(self.u16()?.cast_signed())
    }

    /// Reads a little-endian signed 32-bit integer after byte alignment.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if fewer than 4 bytes remain.
    pub fn i32(&mut self) -> Result<i32, ParseError> {
        Ok(self.u32()?.cast_signed())
    }

    /// Reads a variable-length encoded `EncodedU32` (1 to 5 bytes, MSB continuation bit).
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] on unexpected EOF.
    pub fn encoded_u32(&mut self) -> Result<u32, ParseError> {
        self.align();
        let mut result: u32 = 0;
        for shift in [0_u32, 7, 14, 21, 28] {
            let b = self.u8()?;
            result |= u32::from(b & 0x7F) << shift;
            if (b & 0x80) == 0 {
                return Ok(result);
            }
        }
        Ok(result)
    }

    /// Reads `n` unsigned bits (`UB[n]`, `0 <= n <= 32`) MSB-first (TSD 2.6).
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if `n > 32` or the stream ends before `n` bits are read.
    pub fn ubits(&mut self, n: u8) -> Result<u32, ParseError> {
        if n == 0 {
            return Ok(0);
        }
        if n > 32 {
            return Err(ParseError {
                offset: self.byte_pos,
                kind: ParseErrorKind::InvalidBitCount(n),
            });
        }
        let mut acc: u32 = 0;
        for _ in 0..n {
            if self.bits_left == 0 {
                let offset = self.byte_pos;
                let &next_byte = self.data.get(self.byte_pos).ok_or(ParseError {
                    offset,
                    kind: ParseErrorKind::UnexpectedEof,
                })?;
                self.byte_pos = self.byte_pos.saturating_add(1);
                self.bit_buf = next_byte;
                self.bits_left = 8;
            }
            let shift = self.bits_left.saturating_sub(1);
            let bit = u32::from((self.bit_buf >> shift) & 1);
            acc = (acc << 1) | bit;
            self.bits_left = shift;
        }
        Ok(acc)
    }

    /// Reads `n` signed bits (`SB[n]`, `0 <= n <= 32`) with two's-complement sign extension (TSD 2.6).
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if `n > 32` or on unexpected EOF.
    pub fn sbits(&mut self, n: u8) -> Result<i32, ParseError> {
        if n == 0 {
            return Ok(0);
        }
        let raw = self.ubits(n)?;
        if n == 32 {
            return Ok(raw.cast_signed());
        }
        let shift = 32_u8.saturating_sub(n);
        Ok((raw << shift).cast_signed() >> shift)
    }

    /// Reads `n` 16.16 signed fixed-point bits (`FB[n]`, `0 <= n <= 32`) as `f32` (TSD 2.6).
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if `n > 32` or on unexpected EOF.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn fbits(&mut self, n: u8) -> Result<f32, ParseError> {
        let raw = self.sbits(n)?;
        Ok((raw as f32) / 65536.0)
    }

    /// Reads a slice of `len` bytes after aligning to the next byte boundary (TSD 2.6).
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if fewer than `len` bytes remain.
    pub fn bytes(&mut self, len: usize) -> Result<&'a [u8], ParseError> {
        self.align();
        let start = self.byte_pos;
        let end = start.checked_add(len).ok_or(ParseError {
            offset: start,
            kind: ParseErrorKind::UnexpectedEof,
        })?;
        let slice = self.data.get(start..end).ok_or(ParseError {
            offset: start,
            kind: ParseErrorKind::UnexpectedEof,
        })?;
        self.byte_pos = end;
        Ok(slice)
    }

    /// Reads a NUL-terminated string (`STRING`) after byte alignment, decoding UTF-8 or Latin-1 lossily (TSD 2.6).
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if no NUL terminator is found before EOF.
    pub fn string(&mut self) -> Result<String, ParseError> {
        self.align();
        let start = self.byte_pos;
        let tail = self.data.get(start..).ok_or(ParseError {
            offset: start,
            kind: ParseErrorKind::UnexpectedEof,
        })?;
        let nul_rel = tail.iter().position(|&b| b == 0).ok_or(ParseError {
            offset: self.data.len(),
            kind: ParseErrorKind::UnexpectedEof,
        })?;
        let raw_bytes = tail.get(..nul_rel).unwrap_or(&[]);
        self.byte_pos = start.saturating_add(nul_rel).saturating_add(1);
        Ok(String::from_utf8_lossy(raw_bytes).into_owned())
    }

    /// Reads a bit-packed `RECT` structure and aligns to the next byte boundary (TSD 2.6).
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] on unexpected EOF.
    pub fn rect(&mut self) -> Result<Rect, ParseError> {
        self.align();
        let nbits = u8::try_from(self.ubits(5)?).unwrap_or(0);
        let x_min = self.sbits(nbits)?;
        let x_max = self.sbits(nbits)?;
        let y_min = self.sbits(nbits)?;
        let y_max = self.sbits(nbits)?;
        self.align();
        Ok(Rect {
            x_min,
            x_max,
            y_min,
            y_max,
        })
    }

    /// Reads a bit-packed `MATRIX` structure and aligns to the next byte boundary (TSD 2.6).
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] on unexpected EOF.
    pub fn matrix(&mut self) -> Result<Matrix, ParseError> {
        self.align();
        let mut mat = Matrix::identity();
        if self.ubits(1)? != 0 {
            let n_scale = u8::try_from(self.ubits(5)?).unwrap_or(0);
            mat.scale_x_fp16 = self.sbits(n_scale)?;
            mat.scale_y_fp16 = self.sbits(n_scale)?;
        }
        if self.ubits(1)? != 0 {
            let n_rot = u8::try_from(self.ubits(5)?).unwrap_or(0);
            mat.rotate_skew0_fp16 = self.sbits(n_rot)?;
            mat.rotate_skew1_fp16 = self.sbits(n_rot)?;
        }
        let n_trans = u8::try_from(self.ubits(5)?).unwrap_or(0);
        mat.translate_x_twips = self.sbits(n_trans)?;
        mat.translate_y_twips = self.sbits(n_trans)?;
        self.align();
        Ok(mat)
    }

    /// Reads a 3-channel `CXFORM` structure (without alpha) and aligns to the next byte boundary (TSD 2.6).
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] on unexpected EOF.
    pub fn cxform(&mut self) -> Result<ColorTransform, ParseError> {
        self.cxform_inner(false)
    }

    /// Reads a 4-channel `CXFORMWITHALPHA` structure and aligns to the next byte boundary.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] on unexpected EOF.
    pub fn cxform_with_alpha(&mut self) -> Result<ColorTransform, ParseError> {
        self.cxform_inner(true)
    }

    fn cxform_inner(&mut self, with_alpha: bool) -> Result<ColorTransform, ParseError> {
        self.align();
        let has_add = self.ubits(1)? != 0;
        let has_mult = self.ubits(1)? != 0;
        let nbits = u8::try_from(self.ubits(4)?).unwrap_or(0);
        let mut cx = ColorTransform::identity();
        if has_mult {
            cx.r_mult = i16::try_from(self.sbits(nbits)?).unwrap_or(256);
            cx.g_mult = i16::try_from(self.sbits(nbits)?).unwrap_or(256);
            cx.b_mult = i16::try_from(self.sbits(nbits)?).unwrap_or(256);
            if with_alpha {
                cx.a_mult = i16::try_from(self.sbits(nbits)?).unwrap_or(256);
            }
        }
        if has_add {
            cx.r_add = i16::try_from(self.sbits(nbits)?).unwrap_or(0);
            cx.g_add = i16::try_from(self.sbits(nbits)?).unwrap_or(0);
            cx.b_add = i16::try_from(self.sbits(nbits)?).unwrap_or(0);
            if with_alpha {
                cx.a_add = i16::try_from(self.sbits(nbits)?).unwrap_or(0);
            }
        }
        self.align();
        Ok(cx)
    }
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
    fn bit_alignment_and_signed_extension_edge_cases() {
        // 0b101_11100 = 0xBC, followed by 0x42
        let data = [0b1011_1100, 0x42, 0x34, 0x12];
        let mut r = Reader::new(&data);
        assert_eq!(r.ubits(3).unwrap(), 0b101);
        // 3-bit signed 0b111 = -1
        assert_eq!(r.sbits(3).unwrap(), -1);
        // Align discards the remaining 2 bits of byte 0 and moves to byte 1 (0x42).
        r.align();
        assert_eq!(r.u8().unwrap(), 0x42);
        assert_eq!(r.u16().unwrap(), 0x1234);
        assert!(r.is_empty());
    }

    #[test]
    fn fbits_and_eof_reporting() {
        // 16.16 fixed point: 1.5 = 0x0001_8000 (18 bits signed: 0b01_1000_0000_0000_0000)
        let bytes = 0x0001_8000_u32.to_be_bytes();
        let mut r = Reader::new(&bytes);
        let val = r.fbits(32).unwrap();
        assert!((val - 1.5).abs() < 1e-5);
        let err = r.u8().unwrap_err();
        assert_eq!(err.offset, 4);
        assert_eq!(err.kind, ParseErrorKind::UnexpectedEof);
    }

    #[test]
    fn rect_matrix_cxform_and_string_parsing() {
        // Empty rect (nbits=0 -> 1 byte 0x00), followed by "hi\0"
        let buf = [0x00, b'h', b'i', 0x00];
        let mut r = Reader::new(&buf);
        let rect = r.rect().unwrap();
        assert_eq!(rect, Rect::default());
        assert_eq!(r.string().unwrap(), "hi");
    }
}
