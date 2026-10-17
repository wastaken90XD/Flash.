# SWF Binary Primitives, Container & Tag Format Notes (`docs/notes/swf-format.md`)

- **Sources:**
  - `docs/reference/swf.ksy` (Kaitai Struct SWF specification, CC0-1.0)
  - `docs/reference/lzma.txt` (7-Zip / LZMA SDK specification, Public Domain)
  - Adobe SWF File Format Specification Version 19 (Chapters 1–11)

## 1. Bit and Byte Ordering (`swfr-reader`)

- **Integer byte order:** Multi-byte integer fields (`UI16`, `UI32`, `SI16`, `SI32`) are little-endian.
- **Bitfields (`UB[n]`, `SB[n]`, `FB[n]`):** Packed MSB-first within each byte (bit 7 down to bit 0).
  - Reading any byte-aligned field (`u8`, `u16`, `u32`, `bytes`, `string`, or starting a new `RECT`, `MATRIX`, `CXFORM`) calls `align()` first to discard any unread low bits of the current partial byte.
  - `SB[n]` sign-extends bit `n - 1` into a 32-bit signed integer (`i32`).
  - `FB[n]` is a 16.16 signed fixed-point value stored in `SB[n]`, equal to `(raw_sbits as f32) / 65536.0`.

## 2. Geometric & Color Primitives

- **`RECT`:**
  - Align to byte boundary.
  - Read `nbits = UB[5]`.
  - Read `x_min = SB[nbits]`, `x_max = SB[nbits]`, `y_min = SB[nbits]`, `y_max = SB[nbits]`, then `align()`.
  - Coordinates are in twips (`1/20` of a logical pixel).
- **`MATRIX`:**
  - Align to byte boundary.
  - `has_scale = UB[1]`: if `1`, `n_scale_bits = UB[5]`, `scale_x = FB[n_scale_bits]`, `scale_y = FB[n_scale_bits]` (default `1.0`, `1.0`).
  - `has_rotate = UB[1]`: if `1`, `n_rotate_bits = UB[5]`, `rotate_skew0 = FB[n_rotate_bits]`, `rotate_skew1 = FB[n_rotate_bits]` (default `0.0`, `0.0`).
  - `n_translate_bits = UB[5]`, `translate_x = SB[n_translate_bits]`, `translate_y = SB[n_translate_bits]`, then `align()`.
- **`CXFORM` / `CXFORMWITHALPHA`:**
  - Align to byte boundary.
  - `has_add_terms = UB[1]`, `has_mult_terms = UB[1]`, `nbits = UB[4]`.
  - If `has_mult_terms`: read `r_mult`, `g_mult`, `b_mult` (and `a_mult` for `CXFORMWITHALPHA`) as `SB[nbits]` (8.8 fixed point where `256 == 1.0`).
  - If `has_add_terms`: read `r_add`, `g_add`, `b_add` (and `a_add` for `CXFORMWITHALPHA`) as `SB[nbits]`, then `align()`.

## 3. Container Headers (`FWS`, `CWS`, `ZWS`)

- Bytes `0..3`: Signature `"FWS"` (uncompressed), `"CWS"` (zlib from byte 8 onward), or `"ZWS"` (LZMA from byte 12 onward).
- Byte `3`: `version` (`u8`).
- Bytes `4..8`: `file_length` (`u32` LE) — total uncompressed length including the 8-byte preamble.
- **ZWS LZMA layout (`docs/reference/lzma.txt`):**
  - Bytes `8..12`: `compressed_length` (`u32` LE).
  - Bytes `12..17`: 5-byte LZMA properties (`lc`, `lp`, `pb` byte + 32-bit dictionary size).
  - Bytes `17..`: Raw LZMA compressed payload. Standard LZMA stream decoders expect `[5-byte props][8-byte uncompressed_length LE][compressed payload]`, where `uncompressed_length = file_length - 8`.
- **Decompressed body:** Starts with `frame_size: RECT`, `frame_rate: u16` (8.8 fixed-point LE), `frame_count: u16` (LE), followed by a sequence of tags until `End` (tag code `0`).

## 4. Tag Record Header (`RECORDHEADER`)

- `tag_code_and_length = u16` (LE):
  - `code = tag_code_and_length >> 6` (10-bit tag type).
  - `short_len = tag_code_and_length & 0x3F`.
  - If `short_len == 0x3F` (`63`), read 32-bit `long_len = u32` (LE); otherwise `length = short_len`.
