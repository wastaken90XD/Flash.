# R3 — The `swf` Crate vs `swfr-swf` (`02-swf-crate.md`)

- **Pinned Release Tag:** `v0.7.1` (`swf` crate version `0.3.0`)
- **Pinned Commit SHA:** `89a7049965f3090e706265670df309a5128233ac`
- **Repository:** `https://github.com/ruffle-rs/ruffle`
- **Overall Confidence:** **High** (verified directly from `swf/src/**/*.rs`, `core/src/tag_utils.rs`, and `crates/swfr-swf/src/lib.rs` at `89a7049965f3090e706265670df309a5128233ac`).

---

## 1. Compression (`FWS`, `CWS`, `ZWS`, and Scaleform `GFX`/`CFX`) (`Confidence: High`)

- **Signatures (`swf/src/read.rs:2584-2594`):**
  - `read_compression_type` reads a 3-byte magic signature and maps:
    - `b"FWS" | b"GFX"` -> `Compression::None` (`swf/src/read.rs:2588`)
    - `b"CWS" | b"CFX"` -> `Compression::Zlib` (`swf/src/read.rs:2589`)
    - `b"ZWS"` -> `Compression::Lzma` (`swf/src/read.rs:2590`)
- **Decompression & Truncation Recovery (`swf/src/read.rs:72-186`):**
  - `decompress_swf` rejects `version == 0` (`swf/src/read.rs:88-90`) and computes `body_len = uncompressed_len.checked_sub(8)` (`swf/src/read.rs:94-96`).
  - Pre-allocates `Vec::with_capacity(body_len.min(MAX_DATA_CAPACITY) as usize)` where `MAX_DATA_CAPACITY = 128 * 1024 * 1024` (128 MiB; `swf/src/read.rs:16, 117`).
  - Reads at most `body_len` bytes via `decompress_stream.take(body_len as u64).read_to_end(&mut data)` (`swf/src/read.rs:118-123`). Crucially, if `read_to_end` returns `Err(e)` or `data.len() + 8 != uncompressed_len`, Ruffle logs an error/warning and **continues parsing whatever bytes were decompressed** (`swf/src/read.rs:121-133`) rather than failing the load.
  - For `ZWS` (`make_lzma_reader`, `swf/src/read.rs:202-244`), Ruffle skips the 4-byte compressed-length field (`swf/src/read.rs:226`) and passes `UnpackedSize::UseProvided(Some(unpacked_size as u64))` to `lzma_rs::lzma_decompress_with_options` to handle Flash's mangled 17-byte LZMA header (`swf/src/read.rs:210-239`).
  - Also supports Adobe `.swz` ASN.1 DER signed containers via `extract_swz` (`swf/src/read.rs:38-54`).

---

## 2. Bitstream Reading (`Confidence: High`)

- `BitReader<'a, 'b>` (`swf/src/read.rs:256-324`) wraps `bitstream_io::BitReader<&'b mut &'a [u8], bitstream_io::BigEndian>`.
- Provides `read_bit()`, compile-time constant `read_ubits_ct<const NUM_BITS: u32>()` (`swf/src/read.rs:276-282`), variable-width `read_ubits(num_bits)` (`swf/src/read.rs:286-292`), signed `read_sbits(num_bits)` (`swf/src/read.rs:295-301`, returning `0` when `num_bits == 0`), `read_sbits_fixed8`, `read_sbits_twips`, and `read_fbits` (`Fixed16`).
- Byte alignment happens automatically when a temporary `BitReader` is dropped or when `BitReader::reader(&mut self)` calls `self.bits.byte_align()` (`swf/src/read.rs:320-323`).

---

## 3. Twips, Matrices, and Color Transforms (`Confidence: High`)

- **`Twips(i32)` (`swf/src/types/twips.rs:19-105`):**
  - `TWIPS_PER_PIXEL = 20` (`swf/src/types/twips.rs:23`). Conversion to pixels is `f64::from(self.0) / 20.0`, and `from_pixels(px: f64)` truncates `(px * 20.0) as i32` (`swf/src/types/twips.rs:82-104`).
- **`Fixed8` (`i16`, 8.8) & `Fixed16` (`i32`, 16.16) (`swf/src/types/fixed.rs:1-653`):**
  - Exact fixed-point types with wrapping/saturating multiplication (`wrapping_mul`, `wrapping_mul_int`, `mul_int`) so matrix and color-transform math matches Flash Player's fixed-point overflow behavior.
- **`Matrix` (`swf/src/types/matrix.rs:19-175`, `swf/src/read.rs:704-724`):**
  - Stores `a, b, c, d: Fixed16` (`16.16`) and `tx, ty: Twips` (`i32`).
  - `Matrix * Matrix` (`swf/src/types/matrix.rs:112-135`) multiplies `a, b, c, d` in 16.16 fixed-point and computes `tx, ty` via `wrapping_mul_int` and `wrapping_add` in twips.
- **`ColorTransform` (`swf/src/types/color_transform.rs:5-134`, `swf/src/read.rs:679-702`):**
  - Stores `r_multiply, g_multiply, b_multiply, a_multiply: Fixed8` (`8.8`, where `Fixed8::ONE` is `0x0100 = 256`) and `r_add, g_add, b_add, a_add: i16`.
  - Notice `read_color_transform` (`swf/src/read.rs:686-691`) reads multiplier bits as **signed** 8.8 fixed-point (`bits.read_sbits_fixed8(num_bits)`), and `ColorTransform * ColorTransform` (`swf/src/types/color_transform.rs:75-98`) uses `wrapping_mul` and `wrapping_add(self.r_multiply.wrapping_mul_int(rhs.r_add))`.

---

## 4. Tag Coverage (`Confidence: High`)

- `TagCode` (`swf/src/tag_code.rs:4-90`) defines **64 distinct SWF tag codes** spanning SWF 1 through SWF 19+:
  - Timeline & control: `End (0)`, `ShowFrame (1)`, `SetBackgroundColor (9)`, `Protect (24)`, `FrameLabel (43)`, `ExportAssets (56)`, `ImportAssets (57)`, `EnableDebugger (58)`, `DoInitAction (59)`, `DebugId (63)`, `EnableDebugger2 (64)`, `ScriptLimits (65)`, `SetTabIndex (66)`, `FileAttributes (69)`, `ImportAssets2 (71)`, `SymbolClass (76)`, `Metadata (77)`, `DefineScalingGrid (78)`, `DefineSceneAndFrameLabelData (86)`, `DefineBinaryData (87)`, `EnableTelemetry (93)`.
  - Display list: `PlaceObject (4)`, `RemoveObject (5)`, `PlaceObject2 (26)`, `RemoveObject2 (28)`, `PlaceObject3 (70)`, `PlaceObject4 (94)`.
  - Shapes & morphs: `DefineShape (2)`, `DefineShape2 (22)`, `DefineShape3 (32)`, `DefineMorphShape (46)`, `DefineShape4 (83)`, `DefineMorphShape2 (84)`.
  - Bitmaps: `DefineBits (6)`, `JpegTables (8)`, `DefineBitsLossless (20)`, `DefineBitsJpeg2 (21)`, `DefineBitsJpeg3 (35)`, `DefineBitsLossless2 (36)`, `DefineBitsJpeg4 (90)`.
  - Text & fonts: `DefineFont (10)`, `DefineText (11)`, `DefineFontInfo (13)`, `DefineText2 (33)`, `DefineEditText (37)`, `DefineFont2 (48)`, `DefineFontInfo2 (62)`, `DefineFontAlignZones (73)`, `CsmTextSettings (74)`, `DefineFont3 (75)`, `DefineFontName (88)`, `DefineFont4 (91)`.
  - Sound & video: `DefineSound (14)`, `StartSound (15)`, `DefineButtonSound (17)`, `SoundStreamHead (18)`, `SoundStreamBlock (19)`, `SoundStreamHead2 (45)`, `DefineVideoStream (60)`, `VideoFrame (61)`, `StartSound2 (89)`.
  - Buttons & sprites: `DefineButton (7)`, `DefineButtonCxform (23)`, `DefineButton2 (34)`, `DefineSprite (39)`, `NameCharacter (40)`, `ProductInfo (41)`.
  - Bytecode: `DoAction (12)`, `DoAbc (72)`, `DoAbc2 (82)`.

---

## 5. AVM1 and AVM2 Action/Opcode Coverage (`Confidence: High`)

- **AVM1 (`swf/src/avm1/opcode.rs:1-131`, `swf/src/avm1/read.rs:1-537`):**
  - Defines all **94 standard SWF 3–8 AVM1 opcodes** (`0x00 End` through `0x9F GotoFrame2`).
  - Note how `Reader::read_f64_me` (`swf/src/avm1/read.rs:49-53`) handles AVM1 `Push(Double)`: Flash stores 64-bit floats in `ActionPush` with the two 32-bit little-endian words swapped (`self.read_u64()?.rotate_left(32)`).
  - Note also how `Reader::read_action` (`swf/src/avm1/read.rs:56-76`) checks whether the reader consumed exactly `length` bytes after `read_op`; if an action's payload length mismatches what was parsed, it logs a warning and **re-syncs `self.input = &start[length.min(start.len())..]`** (`swf/src/avm1/read.rs:68-73`) so subsequent opcodes in the block are not corrupted!
- **AVM2 (`swf/src/avm2/opcode.rs:1-170`, `swf/src/avm2/read.rs:1-991`):**
  - Full ABC (`ActionScript Byte Code`) file parser and writer covering constant pools, multinames, method signatures, metadata, instance/class/script traits, exception tables, and all 148 AVM2 opcodes (including Alchemy sign-extension and fast domain memory ops).

---

## 6. String Encoding (`SwfStr`) (`Confidence: High`)

- `SwfStr` (`swf/src/string.rs:17-273`) is a `#[repr(transparent)]` borrowed byte slice wrapper (`[u8]`) that defers character decoding until needed.
- `SwfStr::encoding_for_version(swf_version)` (`swf/src/string.rs:133-139`):
  - Returns `UTF_8` for `swf_version >= 6`.
  - Returns `WINDOWS_1252` by default for `swf_version <= 5` (with `SHIFT_JIS` available via `encoding_rs`).

---

## 7. Error Handling and Malformed Input Recovery (`Confidence: High`)

- **Critical Architectural Insight (`core/src/tag_utils.rs:63-99` vs `swf::parse_swf`):**
  - While `swf::parse_swf` (`swf/src/read.rs:28-35`) eagerly parses all tags into a `Vec<Tag<'_>>`, **`ruffle_core` never calls `swf::parse_swf`!**
  - Instead, `ruffle_core` keeps the decompressed bytes in an `Arc<SwfMovie>` and steps through tags lazily via `tag_utils::decode_tags` (`core/src/tag_utils.rs:63-99`):
    1. Each tag is bounded by `tag_slice = &reader.get_ref()[..tag_len]` and `end_slice = &reader.get_ref()[tag_len..]` (`core/src/tag_utils.rs:75-76`).
    2. If `tag_len > reader.get_ref().len()`, `decode_tags` logs `"Unexpected EOF when reading tag"`, advances to the end of the slice, and returns `Ok(false)` (`core/src/tag_utils.rs:69-73`) without aborting the movie!
    3. If a definition tag handler returns `Err(e)`, `decode_tags` logs `"Error running definition tag"` and advances `*reader.get_mut() = end_slice` (`core/src/tag_utils.rs:81-95`).
    4. Inside `swf::read::Reader::read_tag_with_code` (`swf/src/read.rs:636-643`), trailing unparsed bytes inside a tag only log `warn!("Data remaining in buffer when parsing {tag_code:?}")` rather than returning an error.

---

## 8. Diff Against `swfr-swf` (`Confidence: High`)

| Area | Ruffle `swf` (`0.3.0`) + `tag_utils` | `swfr-swf` (`crates/swfr-swf/src/lib.rs`) | Impact / Gap |
|---|---|---|---|
| **Decompression** | `FWS`/`GFX`, `CWS`/`CFX`, `ZWS`; keeps partial buffer on truncated zlib stream (`swf/src/read.rs:118-133`) | `FWS`, `CWS`, `ZWS` + 100x decompression-bomb guard (`crates/swfr-swf/src/lib.rs:297-382`); fails on truncated zlib | Some wild SWFs have slightly wrong `uncompressed_len` headers or trailing zlib errors |
| **Tag Stream Model** | Lazy streaming via `SwfSlice` + `decode_tags` (`core/src/tag_utils.rs:63-99`) | Eager `Vec<Tag>` with per-tag slice isolation and `Tag::Unknown` fallback (`crates/swfr-swf/src/lib.rs:395-451`) | `swfr-swf`'s per-tag slice fallback now isolates bad tags similarly to `decode_tags` |
| **Fixed-Point Math** | `Fixed16` (`16.16`) & `Fixed8` (`8.8`) + `Twips(i32)` (`swf/src/types/`) | `Matrix` uses `f32` (`a,b,c,d`) and `i32` (`tx,ty`); `ColorTransform` uses `i16` (`crates/swfr-swf/src/lib.rs:58-105`) | Minor rounding difference on deep matrix multiplication chains |
| **String Encoding** | `SwfStr` (`WINDOWS_1252` for SWF <= 5, `UTF_8` for SWF >= 6; `swf/src/string.rs:133-139`) | `String::from_utf8_lossy` (`crates/swfr-swf/src/lib.rs`) | SWF <= 5 strings with high Latin-1 bytes (`0x80..=0xFF`, e.g. `Å`) turn into `U+FFFD` in `swfr` |
| **AVM1 Action Length Resync** | Re-syncs to `start + length` on action payload mismatch (`swf/src/avm1/read.rs:67-73`) | Slices action payload by `len` before decoding (`crates/swfr-avm1/src/lib.rs`) | Equivalent resync protection |

---

## 9. Verification Against Regression Tests (`Confidence: High`)

1. **Claim 1 — `decode_tags` gracefully stops at a tag whose length exceeds EOF without panicking or failing the movie (`core/src/tag_utils.rs:69-73`):**
   - Verified against `tests/tests/swfs/avm1/bad_swf_tag_past_eof/test.swf` and `test.toml:1` (`num_frames = 1`, `output.txt` empty), which contains a truncated tag past EOF and runs 1 frame cleanly.
2. **Claim 2 — SWF <= 5 strings decode as `WINDOWS-1252` (`swf/src/string.rs:133-139`):**
   - Verified against `tests/tests/swfs/avm1/swf5_encoding/output.txt:1-3`, where a SWF 5 `ActionPush` containing byte `0xC5` (`test\xC5test`) traces `9` (`string.length`) and `testÅtest`.
3. **Claim 3 — `ColorTransform` fixed-point multiplication and offset clamping (`swf/src/types/color_transform.rs:75-133`):**
   - Verified against `tests/tests/swfs/avm1/color_transform/output.txt:1-12`, which verifies `ColorTransform` multiplier and offset composition (`combine` -> `(redMultiplier=0.25, ..., redOffset=35, greenOffset=50, blueOffset=65, alphaOffset=80)`).

---

## 10. What Could Not Be Verified (`Confidence: High`)

- `.swz` (`extract_swz`) signed Adobe Flex RSL extraction was inspected in source (`swf/src/read.rs:38-54`) but has no standalone regression test in `tests/tests/swfs/avm1/`.

---

## 11. Open Questions

1. Should `swfr-swf` decode SWF <= 5 strings using `WINDOWS-1252` (mapping bytes `0x80..=0xFF` instead of `from_utf8_lossy`) so legacy SWF 4/5 games preserve accented characters and string lengths?
2. Should `swfr-swf` tolerate truncated zlib streams where `read_to_end` yields partial data (while still enforcing the 100x decompression-bomb limit)?

---

## 12. Test Ideas for `swfr`

1. **`bad_swf_tag_past_eof` resilience test:** Feed a SWF whose final tag header declares a length past EOF into `swfr_swf::Movie::parse` and verify that earlier frames/tags still run rather than failing `retro_load_game`.
2. **SWF 5 Latin-1 (`WINDOWS-1252`) string encoding test:** Model a test on `tests/tests/swfs/avm1/swf5_encoding` pushing `"test\xC5test"` in a version-5 SWF and asserting `.length == 9` and output `"testÅtest"`.
3. **`GFX`/`CFX` header signature test:** Test whether accepting Scaleform `GFX`/`CFX` headers (`swf/src/read.rs:2588-2589`) is useful or should remain explicitly rejected.
