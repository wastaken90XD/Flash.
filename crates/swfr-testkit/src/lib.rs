//! Programmatic SWF movie builder, AS1/AS2 bytecode assembler, deterministic mutator,
//! and golden PNG comparison helpers for `swfr` (TSD 1.6, 2.6, 2.13, 3.7).

use std::fs;
use std::io::Cursor;
use std::path::Path;
use swfr_reader::{ColorTransform, Matrix, Rect};
use swfr_swf::{
    ButtonDef, Compression, EditTextDef, FillStyle, FontDef, LineStyle, LosslessBitmapDef,
    MorphShapeDef, PlaceObject2Tag, PlaceObjectTag, Rgba, ShapeDef, ShapeRecord, SoundDef,
    SoundInfo, SoundStreamHeadDef, Tag, TextDef,
};

/// Bit-level and byte-level buffer writer mirroring [`swfr_reader::Reader`].
#[derive(Debug, Clone, Default)]
pub struct BitWriter {
    buf: Vec<u8>,
    bit_acc: u8,
    bits_used: u8,
}

impl BitWriter {
    /// Creates an empty [`BitWriter`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buf: Vec::new(),
            bit_acc: 0,
            bits_used: 0,
        }
    }

    /// Flushes any partial byte (padding low bits with 0) so the stream is byte-aligned.
    pub fn align(&mut self) {
        if self.bits_used > 0 {
            self.buf.push(self.bit_acc);
            self.bit_acc = 0;
            self.bits_used = 0;
        }
    }

    /// Consumes the writer and returns the byte-aligned buffer.
    #[must_use]
    pub fn into_bytes(mut self) -> Vec<u8> {
        self.align();
        self.buf
    }

    /// Writes `n` unsigned bits (`UB[n]`) MSB-first.
    pub fn write_ubits(&mut self, val: u32, n: u8) {
        for i in (0..n).rev() {
            let bit = u8::try_from((val >> i) & 1).unwrap_or(0);
            let shift = 7_u8.saturating_sub(self.bits_used);
            self.bit_acc |= bit << shift;
            self.bits_used = self.bits_used.saturating_add(1);
            if self.bits_used == 8 {
                self.buf.push(self.bit_acc);
                self.bit_acc = 0;
                self.bits_used = 0;
            }
        }
    }

    /// Writes `n` signed bits (`SB[n]`) MSB-first.
    pub fn write_sbits(&mut self, val: i32, n: u8) {
        self.write_ubits(val.cast_unsigned(), n);
    }

    /// Writes an unsigned 8-bit integer after aligning to the next byte boundary.
    pub fn write_u8(&mut self, val: u8) {
        self.align();
        self.buf.push(val);
    }

    /// Writes a little-endian unsigned 16-bit integer after byte alignment.
    pub fn write_u16(&mut self, val: u16) {
        self.align();
        self.buf.extend_from_slice(&val.to_le_bytes());
    }

    /// Writes a little-endian signed 16-bit integer after byte alignment.
    pub fn write_i16(&mut self, val: i16) {
        self.write_u16(val.cast_unsigned());
    }

    /// Writes a little-endian unsigned 32-bit integer after byte alignment.
    pub fn write_u32(&mut self, val: u32) {
        self.align();
        self.buf.extend_from_slice(&val.to_le_bytes());
    }

    /// Writes a little-endian signed 32-bit integer after byte alignment.
    pub fn write_i32(&mut self, val: i32) {
        self.write_u32(val.cast_unsigned());
    }

    /// Writes a variable-length `EncodedU32`.
    pub fn write_encoded_u32(&mut self, mut val: u32) {
        self.align();
        loop {
            let mut byte = u8::try_from(val & 0x7F).unwrap_or(0);
            val >>= 7;
            if val != 0 {
                byte |= 0x80;
            }
            self.buf.push(byte);
            if val == 0 {
                break;
            }
        }
    }

    /// Writes a raw byte slice after byte alignment.
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.align();
        self.buf.extend_from_slice(bytes);
    }

    /// Writes a NUL-terminated string after byte alignment.
    pub fn write_string(&mut self, s: &str) {
        self.align();
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(0);
    }

    /// Writes a bit-packed `RECT` structure and aligns to the next byte boundary.
    pub fn write_rect(&mut self, rect: Rect) {
        self.align();
        let nbits = min_sbits_for(&[rect.x_min, rect.x_max, rect.y_min, rect.y_max]);
        self.write_ubits(u32::from(nbits), 5);
        self.write_sbits(rect.x_min, nbits);
        self.write_sbits(rect.x_max, nbits);
        self.write_sbits(rect.y_min, nbits);
        self.write_sbits(rect.y_max, nbits);
        self.align();
    }

    /// Writes a bit-packed `MATRIX` structure and aligns to the next byte boundary.
    pub fn write_matrix(&mut self, mat: Matrix) {
        self.align();
        let has_scale = mat.scale_x_fp16 != 0x0001_0000 || mat.scale_y_fp16 != 0x0001_0000;
        self.write_ubits(u32::from(has_scale), 1);
        if has_scale {
            let n = min_sbits_for(&[mat.scale_x_fp16, mat.scale_y_fp16]);
            self.write_ubits(u32::from(n), 5);
            self.write_sbits(mat.scale_x_fp16, n);
            self.write_sbits(mat.scale_y_fp16, n);
        }
        let has_rot = mat.rotate_skew0_fp16 != 0 || mat.rotate_skew1_fp16 != 0;
        self.write_ubits(u32::from(has_rot), 1);
        if has_rot {
            let n = min_sbits_for(&[mat.rotate_skew0_fp16, mat.rotate_skew1_fp16]);
            self.write_ubits(u32::from(n), 5);
            self.write_sbits(mat.rotate_skew0_fp16, n);
            self.write_sbits(mat.rotate_skew1_fp16, n);
        }
        let n_trans = if mat.translate_x_twips == 0 && mat.translate_y_twips == 0 {
            0
        } else {
            min_sbits_for(&[mat.translate_x_twips, mat.translate_y_twips])
        };
        self.write_ubits(u32::from(n_trans), 5);
        self.write_sbits(mat.translate_x_twips, n_trans);
        self.write_sbits(mat.translate_y_twips, n_trans);
        self.align();
    }

    /// Writes a `CXFORM` or `CXFORMWITHALPHA` structure.
    pub fn write_cxform(&mut self, cx: ColorTransform, with_alpha: bool) {
        self.align();
        let has_add = cx.r_add != 0
            || cx.g_add != 0
            || cx.b_add != 0
            || (with_alpha && cx.a_add != 0);
        let has_mult = cx.r_mult != 256
            || cx.g_mult != 256
            || cx.b_mult != 256
            || (with_alpha && cx.a_mult != 256);
        let mut vals = Vec::new();
        if has_mult {
            vals.extend_from_slice(&[
                i32::from(cx.r_mult),
                i32::from(cx.g_mult),
                i32::from(cx.b_mult),
            ]);
            if with_alpha {
                vals.push(i32::from(cx.a_mult));
            }
        }
        if has_add {
            vals.extend_from_slice(&[
                i32::from(cx.r_add),
                i32::from(cx.g_add),
                i32::from(cx.b_add),
            ]);
            if with_alpha {
                vals.push(i32::from(cx.a_add));
            }
        }
        let nbits = if vals.is_empty() {
            1
        } else {
            min_sbits_for(&vals).clamp(1, 15)
        };
        self.write_ubits(u32::from(has_add), 1);
        self.write_ubits(u32::from(has_mult), 1);
        self.write_ubits(u32::from(nbits), 4);
        if has_mult {
            self.write_sbits(i32::from(cx.r_mult), nbits);
            self.write_sbits(i32::from(cx.g_mult), nbits);
            self.write_sbits(i32::from(cx.b_mult), nbits);
            if with_alpha {
                self.write_sbits(i32::from(cx.a_mult), nbits);
            }
        }
        if has_add {
            self.write_sbits(i32::from(cx.r_add), nbits);
            self.write_sbits(i32::from(cx.g_add), nbits);
            self.write_sbits(i32::from(cx.b_add), nbits);
            if with_alpha {
                self.write_sbits(i32::from(cx.a_add), nbits);
            }
        }
        self.align();
    }
}

fn min_sbits_for(vals: &[i32]) -> u8 {
    let mut max_bits: u8 = 0;
    for &v in vals {
        if v == 0 {
            continue;
        }
        let mag = if v < 0 {
            (!v).cast_unsigned()
        } else {
            v.cast_unsigned()
        };
        let bits = u8::try_from(32_u32.saturating_sub(mag.leading_zeros())).unwrap_or(31);
        let needed = bits.saturating_add(1).min(31);
        if needed > max_bits {
            max_bits = needed;
        }
    }
    max_bits
}

/// Programmatic SWF movie builder emitting `FWS`, `CWS`, or `ZWS` byte streams (TSD 1.6, M1).
#[derive(Debug, Clone)]
pub struct SwfBuilder {
    version: u8,
    compression: Compression,
    frame_size: Rect,
    frame_rate_q8: u16,
    frame_count: u16,
    raw_tags: Vec<(u16, Vec<u8>, bool)>,
}

impl Default for SwfBuilder {
    fn default() -> Self {
        Self::new(8, 320, 240, 60.0, 1)
    }
}

impl SwfBuilder {
    /// Creates a new [`SwfBuilder`] with stage dimensions in pixels and frame rate in fps.
    #[must_use]
    #[allow(clippy::arithmetic_side_effects)]
    pub fn new(version: u8, width_px: i32, height_px: i32, fps: f32, frame_count: u16) -> Self {
        let fps_q8 = ((fps.clamp(1.0, 120.0) * 256.0).round() as i32).clamp(256, 30720);
        Self {
            version,
            compression: Compression::Uncompressed,
            frame_size: Rect {
                x_min: 0,
                x_max: width_px.saturating_mul(20),
                y_min: 0,
                y_max: height_px.saturating_mul(20),
            },
            frame_rate_q8: u16::try_from(fps_q8).unwrap_or(60 * 256),
            frame_count,
            raw_tags: Vec::new(),
        }
    }

    /// Sets the container compression mode (`Uncompressed`, `Zlib`, or `Lzma`).
    #[must_use]
    pub const fn with_compression(mut self, compression: Compression) -> Self {
        self.compression = compression;
        self
    }

    /// Appends a raw `(tag_code, payload)` using automatic short or long header encoding.
    pub fn push_raw_tag(&mut self, code: u16, payload: Vec<u8>) {
        let force_long = payload.len() >= 0x3F;
        self.raw_tags.push((code, payload, force_long));
    }

    /// Appends a raw `(tag_code, payload)` explicitly using the 6-byte long `RECORDHEADER` form.
    pub fn push_long_raw_tag(&mut self, code: u16, payload: Vec<u8>) {
        self.raw_tags.push((code, payload, true));
    }

    /// Encodes and appends a structured [`swfr_swf::Tag`].
    pub fn push_tag(&mut self, tag: &Tag) {
        let (code, payload) = encode_tag(tag);
        self.push_raw_tag(code, payload);
    }

    /// Finalizes the movie (appending `Tag::End` if not already last) and returns the serialized SWF bytes.
    #[must_use]
    pub fn build(&self) -> Vec<u8> {
        let mut body = BitWriter::new();
        body.write_rect(self.frame_size);
        body.write_u16(self.frame_rate_q8);
        body.write_u16(self.frame_count);

        for &(code, ref payload, force_long) in &self.raw_tags {
            write_tag_record(&mut body, code, payload, force_long);
        }
        if self.raw_tags.last().map(|t| t.0) != Some(0) {
            write_tag_record(&mut body, 0, &[], false);
        }

        let body_bytes = body.into_bytes();
        let uncomp_file_len = u32::try_from(8_usize.saturating_add(body_bytes.len())).unwrap_or(8);

        match self.compression {
            Compression::Uncompressed => {
                let mut out = Vec::with_capacity(8_usize.saturating_add(body_bytes.len()));
                out.extend_from_slice(b"FWS");
                out.push(self.version);
                out.extend_from_slice(&uncomp_file_len.to_le_bytes());
                out.extend_from_slice(&body_bytes);
                out
            }
            Compression::Zlib => {
                let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&body_bytes, 6);
                let mut out = Vec::with_capacity(8_usize.saturating_add(compressed.len()));
                out.extend_from_slice(b"CWS");
                out.push(self.version);
                out.extend_from_slice(&uncomp_file_len.to_le_bytes());
                out.extend_from_slice(&compressed);
                out
            }
            Compression::Lzma => {
                let mut cursor = Cursor::new(&body_bytes);
                let mut lzma_out = Vec::new();
                let _ = lzma_rs::lzma_compress(&mut cursor, &mut lzma_out);
                // Standard lzma_rs output is: [0..5 props][5..13 uncomp_len_u64][13.. compressed_data]
                let props = lzma_out.get(0..5).unwrap_or(&[0x5D, 0, 0, 0x10, 0]);
                let comp_payload = lzma_out.get(13..).unwrap_or(&[]);
                let comp_len = u32::try_from(comp_payload.len()).unwrap_or(0);

                let mut out = Vec::with_capacity(17_usize.saturating_add(comp_payload.len()));
                out.extend_from_slice(b"ZWS");
                out.push(self.version.max(13));
                out.extend_from_slice(&uncomp_file_len.to_le_bytes());
                out.extend_from_slice(&comp_len.to_le_bytes());
                out.extend_from_slice(props);
                out.extend_from_slice(comp_payload);
                out
            }
        }
    }
}

fn write_tag_record(w: &mut BitWriter, code: u16, payload: &[u8], force_long: bool) {
    let len = payload.len();
    if !force_long && len < 0x3F {
        let header = (code << 6) | u16::try_from(len).unwrap_or(0);
        w.write_u16(header);
    } else {
        let header = (code << 6) | 0x3F;
        w.write_u16(header);
        w.write_u32(u32::try_from(len).unwrap_or(0));
    }
    w.write_bytes(payload);
}

/// Encodes a structured [`swfr_swf::Tag`] into its `(tag_code, payload_bytes)`.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn encode_tag(tag: &Tag) -> (u16, Vec<u8>) {
    let mut w = BitWriter::new();
    let code: u16 = match tag {
        Tag::End => 0,
        Tag::ShowFrame => 1,
        Tag::DefineShape(shape) => {
            let tag_code = match shape.version {
                1 => 2,
                2 => 22,
                3 => 32,
                _ => 83,
            };
            encode_shape(&mut w, shape);
            tag_code
        }
        Tag::PlaceObject(po) => {
            w.write_u16(po.character_id);
            w.write_u16(po.depth);
            w.write_matrix(po.matrix);
            if let Some(cx) = po.color_transform {
                w.write_cxform(cx, false);
            }
            4
        }
        Tag::RemoveObject {
            character_id,
            depth,
        } => {
            w.write_u16(*character_id);
            w.write_u16(*depth);
            5
        }
        Tag::DefineBits { id, jpeg_data } => {
            w.write_u16(*id);
            w.write_bytes(jpeg_data);
            6
        }
        Tag::DefineButton(btn) => {
            let tag_code = if btn.version == 1 { 7 } else { 34 };
            encode_button(&mut w, btn);
            tag_code
        }
        Tag::JPEGTables { jpeg_data } => {
            w.write_bytes(jpeg_data);
            8
        }
        Tag::SetBackgroundColor { r, g, b } => {
            w.write_u8(*r);
            w.write_u8(*g);
            w.write_u8(*b);
            9
        }
        Tag::DefineFont(font) => {
            let tag_code = match font.version {
                1 => 10,
                2 => 48,
                _ => 75,
            };
            encode_font(&mut w, font);
            tag_code
        }
        Tag::DefineText(txt) => {
            let tag_code = if txt.version == 1 { 11 } else { 33 };
            encode_text(&mut w, txt);
            tag_code
        }
        Tag::DoAction { actions } => {
            w.write_bytes(actions);
            12
        }
        Tag::DefineFontInfo {
            font_id,
            name,
            codes,
        } => {
            w.write_u16(*font_id);
            w.write_u8(u8::try_from(name.len()).unwrap_or(0));
            w.write_bytes(name.as_bytes());
            w.write_u8(0x01); // wide codes flag
            for &c in codes {
                w.write_u16(c);
            }
            13
        }
        Tag::DefineSound(snd) => {
            w.write_u16(snd.id);
            w.write_ubits(u32::from(snd.format.code()), 4);
            w.write_ubits(u32::from(snd.sample_rate_code), 2);
            w.write_ubits(u32::from(snd.is_16bit), 1);
            w.write_ubits(u32::from(snd.is_stereo), 1);
            w.write_u32(snd.sample_count);
            w.write_bytes(&snd.data);
            14
        }
        Tag::StartSound {
            sound_id,
            sound_info,
        } => {
            w.write_u16(*sound_id);
            encode_sound_info(&mut w, sound_info);
            15
        }
        Tag::DefineButtonSound { button_id, raw } => {
            w.write_u16(*button_id);
            w.write_bytes(raw);
            17
        }
        Tag::SoundStreamHead(head) => {
            let tag_code = if head.version == 1 { 18 } else { 45 };
            w.write_ubits(0, 4);
            w.write_ubits(u32::from(head.playback_rate_code), 2);
            w.write_ubits(u32::from(head.playback_is_16bit), 1);
            w.write_ubits(u32::from(head.playback_is_stereo), 1);
            w.write_ubits(u32::from(head.stream_format.code()), 4);
            w.write_ubits(u32::from(head.stream_rate_code), 2);
            w.write_ubits(u32::from(head.stream_is_16bit), 1);
            w.write_ubits(u32::from(head.stream_is_stereo), 1);
            w.write_u16(head.stream_sample_count);
            if let Some(seek) = head.latency_seek {
                w.write_i16(seek);
            }
            tag_code
        }
        Tag::SoundStreamBlock { data } => {
            w.write_bytes(data);
            19
        }
        Tag::DefineBitsLossless(bmp) => {
            let tag_code = if bmp.version == 1 { 20 } else { 36 };
            w.write_u16(bmp.id);
            w.write_u8(bmp.format);
            w.write_u16(bmp.width);
            w.write_u16(bmp.height);
            if bmp.format == 3 {
                w.write_u8(bmp.color_table_size.unwrap_or(0));
            }
            w.write_bytes(&bmp.zlib_data);
            tag_code
        }
        Tag::DefineBitsJPEG2 { id, image_data } => {
            w.write_u16(*id);
            w.write_bytes(image_data);
            21
        }
        Tag::DefineButtonCxform { button_id, cxform } => {
            w.write_u16(*button_id);
            w.write_cxform(*cxform, false);
            23
        }
        Tag::Protect { password_md5 } => {
            if let Some(pw) = password_md5 {
                w.write_u16(0);
                w.write_string(pw);
            }
            24
        }
        Tag::PlaceObject2(po2) => {
            encode_place_object2(&mut w, po2, false);
            26
        }
        Tag::RemoveObject2 { depth } => {
            w.write_u16(*depth);
            28
        }
        Tag::DefineBitsJPEG3 {
            id,
            image_data,
            alpha_data,
        } => {
            w.write_u16(*id);
            w.write_u32(u32::try_from(image_data.len()).unwrap_or(0));
            w.write_bytes(image_data);
            w.write_bytes(alpha_data);
            35
        }
        Tag::DefineEditText(edit) => {
            encode_edit_text(&mut w, edit);
            37
        }
        Tag::DefineSprite {
            id,
            frame_count,
            tags,
        } => {
            w.write_u16(*id);
            w.write_u16(*frame_count);
            for nested in tags {
                let (ncode, npayload) = encode_tag(nested);
                write_tag_record(&mut w, ncode, &npayload, false);
            }
            if tags.last() != Some(&Tag::End) {
                write_tag_record(&mut w, 0, &[], false);
            }
            39
        }
        Tag::FrameLabel { name, is_anchor } => {
            w.write_string(name);
            if *is_anchor {
                w.write_u8(1);
            }
            43
        }
        Tag::DefineMorphShape(morph) => {
            let tag_code = if morph.version == 1 { 46 } else { 84 };
            encode_morph_shape(&mut w, morph);
            tag_code
        }
        Tag::ExportAssets { exports } => {
            w.write_u16(u16::try_from(exports.len()).unwrap_or(0));
            for &(id, ref name) in exports {
                w.write_u16(id);
                w.write_string(name);
            }
            56
        }
        Tag::ImportAssets { url, imports } => {
            w.write_string(url);
            w.write_u16(u16::try_from(imports.len()).unwrap_or(0));
            for &(id, ref name) in imports {
                w.write_u16(id);
                w.write_string(name);
            }
            57
        }
        Tag::EnableDebugger { password_md5 } => {
            if let Some(pw) = password_md5 {
                w.write_string(pw);
            }
            58
        }
        Tag::DoInitAction { sprite_id, actions } => {
            w.write_u16(*sprite_id);
            w.write_bytes(actions);
            59
        }
        Tag::ScriptLimits {
            max_recursion_depth,
            script_timeout_seconds,
        } => {
            w.write_u16(*max_recursion_depth);
            w.write_u16(*script_timeout_seconds);
            65
        }
        Tag::SetTabIndex { depth, tab_index } => {
            w.write_u16(*depth);
            w.write_u16(*tab_index);
            66
        }
        Tag::FileAttributes {
            use_direct_blit,
            use_gpu,
            has_metadata,
            actionscript3,
            use_network,
        } => {
            w.write_ubits(0, 1);
            w.write_ubits(u32::from(*use_direct_blit), 1);
            w.write_ubits(u32::from(*use_gpu), 1);
            w.write_ubits(u32::from(*has_metadata), 1);
            w.write_ubits(u32::from(*actionscript3), 1);
            w.write_ubits(0, 2);
            w.write_ubits(u32::from(*use_network), 1);
            w.write_ubits(0, 24);
            69
        }
        Tag::PlaceObject3(po3) => {
            encode_place_object2(&mut w, po3, true);
            70
        }
        Tag::DefineFontAlignZones {
            font_id,
            csm_table_hint,
            raw_zones,
        } => {
            w.write_u16(*font_id);
            w.write_ubits(u32::from(*csm_table_hint), 2);
            w.write_ubits(0, 6);
            w.write_bytes(raw_zones);
            73
        }
        Tag::CSMTextSettings {
            text_id,
            use_flash_type,
            grid_fit,
            thickness_bits,
            sharpness_bits,
        } => {
            w.write_u16(*text_id);
            w.write_ubits(u32::from(*use_flash_type), 2);
            w.write_ubits(u32::from(*grid_fit), 3);
            w.write_ubits(0, 3);
            w.write_u32(*thickness_bits);
            w.write_u32(*sharpness_bits);
            w.write_u8(0);
            74
        }
        Tag::SymbolClass { symbols } => {
            w.write_u16(u16::try_from(symbols.len()).unwrap_or(0));
            for &(id, ref name) in symbols {
                w.write_u16(id);
                w.write_string(name);
            }
            76
        }
        Tag::Metadata { xml } => {
            w.write_string(xml);
            77
        }
        Tag::DefineScalingGrid {
            character_id,
            splitter,
        } => {
            w.write_u16(*character_id);
            w.write_rect(*splitter);
            78
        }
        Tag::DefineSceneAndFrameLabelData { scenes, labels } => {
            w.write_encoded_u32(u32::try_from(scenes.len()).unwrap_or(0));
            for &(off, ref name) in scenes {
                w.write_encoded_u32(off);
                w.write_string(name);
            }
            w.write_encoded_u32(u32::try_from(labels.len()).unwrap_or(0));
            for &(fnum, ref label) in labels {
                w.write_encoded_u32(fnum);
                w.write_string(label);
            }
            86
        }
        Tag::DefineFontName {
            font_id,
            font_name,
            font_copyright,
        } => {
            w.write_u16(*font_id);
            w.write_string(font_name);
            w.write_string(font_copyright);
            88
        }
        Tag::StartSound2 {
            class_name,
            sound_info,
        } => {
            w.write_string(class_name);
            encode_sound_info(&mut w, sound_info);
            89
        }
        Tag::DefineBitsJPEG4 {
            id,
            deblock_param,
            image_data,
            alpha_data,
        } => {
            w.write_u16(*id);
            w.write_u32(u32::try_from(image_data.len()).unwrap_or(0));
            w.write_u16(*deblock_param);
            w.write_bytes(image_data);
            w.write_bytes(alpha_data);
            90
        }
        Tag::Unknown { code, raw } => {
            w.write_bytes(raw);
            *code
        }
    };
    (code, w.into_bytes())
}

fn write_rgba(w: &mut BitWriter, c: Rgba, has_alpha: bool) {
    w.write_u8(c.r);
    w.write_u8(c.g);
    w.write_u8(c.b);
    if has_alpha {
        w.write_u8(c.a);
    }
}

fn encode_fill_styles(w: &mut BitWriter, styles: &[FillStyle], shape_ver: u8) {
    let has_alpha = shape_ver >= 3;
    w.write_u8(u8::try_from(styles.len()).unwrap_or(0));
    for style in styles {
        match style {
            FillStyle::Solid(c) => {
                w.write_u8(0x00);
                write_rgba(w, *c, has_alpha);
            }
            FillStyle::LinearGradient { matrix, stops } => {
                w.write_u8(0x10);
                w.write_matrix(*matrix);
                w.write_ubits(0, 4);
                w.write_ubits(u32::try_from(stops.len()).unwrap_or(0), 4);
                for stop in stops {
                    w.write_u8(stop.ratio);
                    write_rgba(w, stop.color, has_alpha);
                }
            }
            FillStyle::RadialGradient {
                matrix,
                stops,
                focal_point_q8,
            } => {
                let stype = if *focal_point_q8 != 0 { 0x13 } else { 0x12 };
                w.write_u8(stype);
                w.write_matrix(*matrix);
                w.write_ubits(0, 4);
                w.write_ubits(u32::try_from(stops.len()).unwrap_or(0), 4);
                for stop in stops {
                    w.write_u8(stop.ratio);
                    write_rgba(w, stop.color, has_alpha);
                }
                if stype == 0x13 {
                    w.write_i16(*focal_point_q8);
                }
            }
            FillStyle::Bitmap {
                bitmap_id,
                matrix,
                repeating,
                smoothed,
            } => {
                let stype = match (*repeating, *smoothed) {
                    (true, true) => 0x40,
                    (false, true) => 0x41,
                    (true, false) => 0x42,
                    (false, false) => 0x43,
                };
                w.write_u8(stype);
                w.write_u16(*bitmap_id);
                w.write_matrix(*matrix);
            }
        }
    }
}

fn encode_line_styles(w: &mut BitWriter, styles: &[LineStyle], shape_ver: u8) {
    let has_alpha = shape_ver >= 3;
    w.write_u8(u8::try_from(styles.len()).unwrap_or(0));
    for ls in styles {
        w.write_u16(ls.width_twips);
        if shape_ver >= 4 {
            w.write_u16(0); // flags = 0 (round caps/joins, no fill)
            write_rgba(w, ls.color, true);
        } else {
            write_rgba(w, ls.color, has_alpha);
        }
    }
}

fn encode_shape_records(w: &mut BitWriter, records: &[ShapeRecord], shape_ver: u8) {
    w.align();
    let num_fill_bits: u8 = 4;
    let num_line_bits: u8 = 4;
    w.write_ubits(u32::from(num_fill_bits), 4);
    w.write_ubits(u32::from(num_line_bits), 4);

    for rec in records {
        match rec {
            ShapeRecord::StyleChange {
                move_to,
                fill_style_0,
                fill_style_1,
                line_style,
                new_fill_styles,
                new_line_styles,
            } => {
                let has_new = new_fill_styles.is_some() && new_line_styles.is_some();
                w.write_ubits(0, 1);
                w.write_ubits(u32::from(has_new), 1);
                w.write_ubits(u32::from(line_style.is_some()), 1);
                w.write_ubits(u32::from(fill_style_1.is_some()), 1);
                w.write_ubits(u32::from(fill_style_0.is_some()), 1);
                w.write_ubits(u32::from(move_to.is_some()), 1);
                if let Some((mx, my)) = *move_to {
                    let mbits = min_sbits_for(&[mx, my]).max(1);
                    w.write_ubits(u32::from(mbits), 5);
                    w.write_sbits(mx, mbits);
                    w.write_sbits(my, mbits);
                }
                if let Some(f0) = *fill_style_0 {
                    w.write_ubits(u32::from(f0), num_fill_bits);
                }
                if let Some(f1) = *fill_style_1 {
                    w.write_ubits(u32::from(f1), num_fill_bits);
                }
                if let Some(ls) = *line_style {
                    w.write_ubits(u32::from(ls), num_line_bits);
                }
                if let (Some(nfs), Some(nls)) = (new_fill_styles, new_line_styles) {
                    w.align();
                    encode_fill_styles(w, nfs, shape_ver);
                    encode_line_styles(w, nls, shape_ver);
                    w.write_ubits(u32::from(num_fill_bits), 4);
                    w.write_ubits(u32::from(num_line_bits), 4);
                }
            }
            ShapeRecord::StraightEdge { dx_twips, dy_twips } => {
                w.write_ubits(1, 1); // edge record
                w.write_ubits(1, 1); // straight edge
                let nbits = min_sbits_for(&[*dx_twips, *dy_twips]).max(2);
                w.write_ubits(u32::from(nbits.saturating_sub(2)), 4);
                w.write_ubits(1, 1); // general line
                w.write_sbits(*dx_twips, nbits);
                w.write_sbits(*dy_twips, nbits);
            }
            ShapeRecord::CurvedEdge {
                control_dx_twips,
                control_dy_twips,
                anchor_dx_twips,
                anchor_dy_twips,
            } => {
                w.write_ubits(1, 1); // edge record
                w.write_ubits(0, 1); // curved edge
                let nbits = min_sbits_for(&[
                    *control_dx_twips,
                    *control_dy_twips,
                    *anchor_dx_twips,
                    *anchor_dy_twips,
                ])
                .max(2);
                w.write_ubits(u32::from(nbits.saturating_sub(2)), 4);
                w.write_sbits(*control_dx_twips, nbits);
                w.write_sbits(*control_dy_twips, nbits);
                w.write_sbits(*anchor_dx_twips, nbits);
                w.write_sbits(*anchor_dy_twips, nbits);
            }
        }
    }
    // EndShapeRecord (6 zero bits)
    w.write_ubits(0, 6);
    w.align();
}

fn encode_shape(w: &mut BitWriter, shape: &ShapeDef) {
    w.write_u16(shape.id);
    w.write_rect(shape.bounds);
    if shape.version >= 4 {
        w.write_rect(shape.bounds);
        w.write_u8(0);
    }
    encode_fill_styles(w, &shape.fill_styles, shape.version);
    encode_line_styles(w, &shape.line_styles, shape.version);
    encode_shape_records(w, &shape.records, shape.version);
}

fn encode_morph_shape(w: &mut BitWriter, morph: &MorphShapeDef) {
    w.write_u16(morph.id);
    w.write_rect(morph.start_bounds);
    w.write_rect(morph.end_bounds);
    if morph.version >= 2 {
        w.write_rect(morph.start_bounds);
        w.write_rect(morph.end_bounds);
        w.write_u8(0);
    }

    let mut body = BitWriter::new();
    let fill_count = morph.start_fill_styles.len().min(morph.end_fill_styles.len());
    body.write_u8(u8::try_from(fill_count).unwrap_or(0));
    for i in 0..fill_count {
        if let (Some(FillStyle::Solid(sc)), Some(FillStyle::Solid(ec))) =
            (morph.start_fill_styles.get(i), morph.end_fill_styles.get(i))
        {
            body.write_u8(0x00);
            write_rgba(&mut body, *sc, true);
            write_rgba(&mut body, *ec, true);
        }
    }
    let line_count = morph.start_line_styles.len().min(morph.end_line_styles.len());
    body.write_u8(u8::try_from(line_count).unwrap_or(0));
    for i in 0..line_count {
        if let (Some(sls), Some(els)) =
            (morph.start_line_styles.get(i), morph.end_line_styles.get(i))
        {
            body.write_u16(sls.width_twips);
            body.write_u16(els.width_twips);
            if morph.version >= 2 {
                body.write_u16(0);
            }
            write_rgba(&mut body, sls.color, true);
            write_rgba(&mut body, els.color, true);
        }
    }
    encode_shape_records(&mut body, &morph.start_records, 3);
    let first_part = body.into_bytes();

    let mut end_part = BitWriter::new();
    encode_shape_records(&mut end_part, &morph.end_records, 3);
    let end_bytes = end_part.into_bytes();

    w.write_u32(u32::try_from(first_part.len()).unwrap_or(0));
    w.write_bytes(&first_part);
    w.write_bytes(&end_bytes);
}

fn encode_place_object2(w: &mut BitWriter, po2: &PlaceObject2Tag, is_v3: bool) {
    let has_clip_actions = po2.clip_actions_raw.is_some();
    let has_clip_depth = po2.clip_depth.is_some();
    let has_name = po2.name.is_some();
    let has_ratio = po2.ratio.is_some();
    let has_cxform = po2.color_transform.is_some();
    let has_matrix = po2.matrix.is_some();
    let has_char = po2.character_id.is_some();

    w.write_ubits(u32::from(has_clip_actions), 1);
    w.write_ubits(u32::from(has_clip_depth), 1);
    w.write_ubits(u32::from(has_name), 1);
    w.write_ubits(u32::from(has_ratio), 1);
    w.write_ubits(u32::from(has_cxform), 1);
    w.write_ubits(u32::from(has_matrix), 1);
    w.write_ubits(u32::from(has_char), 1);
    w.write_ubits(u32::from(po2.is_move), 1);
    if is_v3 {
        w.write_ubits(0, 8);
    }
    w.write_u16(po2.depth);
    if let Some(cid) = po2.character_id {
        w.write_u16(cid);
    }
    if let Some(mat) = po2.matrix {
        w.write_matrix(mat);
    }
    if let Some(cx) = po2.color_transform {
        w.write_cxform(cx, true);
    }
    if let Some(r) = po2.ratio {
        w.write_u16(r);
    }
    if let Some(ref name) = po2.name {
        w.write_string(name);
    }
    if let Some(cd) = po2.clip_depth {
        w.write_u16(cd);
    }
    if let Some(ref raw) = po2.clip_actions_raw {
        w.write_bytes(raw);
    }
}

fn encode_font(w: &mut BitWriter, font: &FontDef) {
    w.write_u16(font.id);
    if font.version == 1 {
        let num_glyphs = font.glyphs.len();
        if num_glyphs == 0 {
            return;
        }
        let mut encoded_glyphs = Vec::with_capacity(num_glyphs);
        for g in &font.glyphs {
            let mut gw = BitWriter::new();
            encode_shape_records(&mut gw, g, 1);
            encoded_glyphs.push(gw.into_bytes());
        }
        let table_bytes = num_glyphs.saturating_mul(2);
        let mut running = table_bytes;
        for gbytes in &encoded_glyphs {
            w.write_u16(u16::try_from(running).unwrap_or(0));
            running = running.saturating_add(gbytes.len());
        }
        for gbytes in &encoded_glyphs {
            w.write_bytes(gbytes);
        }
        return;
    }

    let mut flags: u8 = 0x04; // wide codes
    if font.is_italic {
        flags |= 0x02;
    }
    if font.is_bold {
        flags |= 0x01;
    }
    w.write_u8(flags);
    w.write_u8(1); // language code
    w.write_u8(u8::try_from(font.name.len()).unwrap_or(0));
    w.write_bytes(font.name.as_bytes());
    let num_glyphs = font.glyphs.len();
    w.write_u16(u16::try_from(num_glyphs).unwrap_or(0));
    if num_glyphs == 0 {
        return;
    }
    let mut encoded_glyphs = Vec::with_capacity(num_glyphs);
    for g in &font.glyphs {
        let mut gw = BitWriter::new();
        encode_shape_records(&mut gw, g, 1);
        encoded_glyphs.push(gw.into_bytes());
    }
    let table_bytes = num_glyphs.saturating_add(1).saturating_mul(2);
    let mut running = table_bytes;
    for gbytes in &encoded_glyphs {
        w.write_u16(u16::try_from(running).unwrap_or(0));
        running = running.saturating_add(gbytes.len());
    }
    w.write_u16(u16::try_from(running).unwrap_or(0));
    for gbytes in &encoded_glyphs {
        w.write_bytes(gbytes);
    }
    for i in 0..num_glyphs {
        let code = font.codes.get(i).copied().unwrap_or(0);
        w.write_u16(code);
    }
}

fn encode_text(w: &mut BitWriter, txt: &TextDef) {
    let has_alpha = txt.version >= 2;
    w.write_u16(txt.id);
    w.write_rect(txt.bounds);
    w.write_matrix(txt.matrix);
    let glyph_bits: u8 = 8;
    let advance_bits: u8 = 12;
    w.write_u8(glyph_bits);
    w.write_u8(advance_bits);

    for rec in &txt.records {
        w.write_ubits(1, 1);
        w.write_ubits(0, 3);
        w.write_ubits(u32::from(rec.font_id.is_some()), 1);
        w.write_ubits(u32::from(rec.color.is_some()), 1);
        w.write_ubits(u32::from(rec.y_offset_twips.is_some()), 1);
        w.write_ubits(u32::from(rec.x_offset_twips.is_some()), 1);
        if let Some(fid) = rec.font_id {
            w.write_u16(fid);
        }
        if let Some(c) = rec.color {
            write_rgba(w, c, has_alpha);
        }
        if let Some(xoff) = rec.x_offset_twips {
            w.write_i16(xoff);
        }
        if let Some(yoff) = rec.y_offset_twips {
            w.write_i16(yoff);
        }
        if rec.font_id.is_some() {
            w.write_u16(rec.text_height_twips.unwrap_or(240));
        }
        w.write_u8(u8::try_from(rec.glyphs.len()).unwrap_or(0));
        for g in &rec.glyphs {
            w.write_ubits(g.glyph_index, glyph_bits);
            w.write_sbits(g.advance_twips, advance_bits);
        }
        w.align();
    }
    w.write_u8(0); // end of text records
}

fn encode_edit_text(w: &mut BitWriter, edit: &EditTextDef) {
    w.write_u16(edit.id);
    w.write_rect(edit.bounds);
    let has_text = edit.initial_text.is_some();
    let has_font = edit.font_id.is_some();
    w.write_ubits(u32::from(has_text), 1);
    w.write_ubits(u32::from(edit.word_wrap), 1);
    w.write_ubits(u32::from(edit.multiline), 1);
    w.write_ubits(0, 1); // password
    w.write_ubits(u32::from(edit.read_only), 1);
    w.write_ubits(1, 1); // has_text_color
    w.write_ubits(0, 1); // has_max_length
    w.write_ubits(u32::from(has_font), 1);
    w.write_ubits(0, 1); // has_font_class
    w.write_ubits(0, 1); // auto_size
    w.write_ubits(0, 1); // has_layout
    w.write_ubits(0, 1); // no_select
    w.write_ubits(0, 1); // border
    w.write_ubits(0, 1); // was_static
    w.write_ubits(u32::from(edit.html), 1);
    w.write_ubits(u32::from(edit.use_outlines), 1);

    if let Some(fid) = edit.font_id {
        w.write_u16(fid);
        w.write_u16(edit.font_height_twips);
    }
    write_rgba(w, edit.color, true);
    w.write_string(&edit.variable_name);
    if let Some(ref txt) = edit.initial_text {
        w.write_string(txt);
    }
}

fn encode_button(w: &mut BitWriter, btn: &ButtonDef) {
    w.write_u16(btn.id);
    let mut rec_writer = BitWriter::new();
    for rec in &btn.records {
        rec_writer.write_ubits(0, 4);
        rec_writer.write_ubits(u32::from(rec.state_hit_test), 1);
        rec_writer.write_ubits(u32::from(rec.state_down), 1);
        rec_writer.write_ubits(u32::from(rec.state_over), 1);
        rec_writer.write_ubits(u32::from(rec.state_up), 1);
        rec_writer.write_u16(rec.character_id);
        rec_writer.write_u16(rec.depth);
        rec_writer.write_matrix(rec.matrix);
        if btn.version >= 2 {
            rec_writer.write_cxform(rec.color_transform, true);
        }
    }
    rec_writer.write_u8(0); // end of button records
    let rec_bytes = rec_writer.into_bytes();

    if btn.version == 1 {
        w.write_bytes(&rec_bytes);
        if let Some(first) = btn.actions.first() {
            w.write_bytes(&first.actions);
        }
    } else {
        w.write_u8(u8::from(btn.track_as_menu));
        let action_offset = if btn.actions.is_empty() {
            0
        } else {
            u16::try_from(2_usize.saturating_add(rec_bytes.len())).unwrap_or(0)
        };
        w.write_u16(action_offset);
        w.write_bytes(&rec_bytes);
        for (idx, act) in btn.actions.iter().enumerate() {
            let is_last = idx.saturating_add(1) == btn.actions.len();
            let cond_size = if is_last {
                0
            } else {
                u16::try_from(4_usize.saturating_add(act.actions.len())).unwrap_or(0)
            };
            w.write_u16(cond_size);
            w.write_u16(act.condition);
            w.write_bytes(&act.actions);
        }
    }
}

fn encode_sound_info(w: &mut BitWriter, info: &SoundInfo) {
    w.write_ubits(0, 2);
    w.write_ubits(u32::from(info.sync_stop), 1);
    w.write_ubits(u32::from(info.sync_no_multiple), 1);
    w.write_ubits(u32::from(!info.envelope.is_empty()), 1);
    w.write_ubits(u32::from(info.loop_count.is_some()), 1);
    w.write_ubits(u32::from(info.out_point.is_some()), 1);
    w.write_ubits(u32::from(info.in_point.is_some()), 1);
    if let Some(inp) = info.in_point {
        w.write_u32(inp);
    }
    if let Some(outp) = info.out_point {
        w.write_u32(outp);
    }
    if let Some(lc) = info.loop_count {
        w.write_u16(lc);
    }
    if !info.envelope.is_empty() {
        w.write_u8(u8::try_from(info.envelope.len()).unwrap_or(0));
        for pt in &info.envelope {
            w.write_u32(pt.pos44);
            w.write_u16(pt.left_level);
            w.write_u16(pt.right_level);
        }
    }
}

/// Programmatic ActionScript 1 & 2 (AVM1) bytecode assembler (`docs/notes/swf-format.md`).
#[derive(Debug, Clone, Default)]
pub struct ActionBuilder {
    bytes: Vec<u8>,
}

impl ActionBuilder {
    /// Creates an empty [`ActionBuilder`].
    #[must_use]
    pub const fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    /// Emits `ActionEnd` (`0x00`) and returns the assembled bytecode block.
    #[must_use]
    pub fn build(mut self) -> Vec<u8> {
        self.bytes.push(0x00);
        self.bytes
    }

    /// Emits a zero-operand action opcode (`< 0x80`).
    pub fn op(&mut self, opcode: u8) -> &mut Self {
        self.bytes.push(opcode);
        self
    }

    /// Emits `ActionPush` (`0x96`) with a single UTF-8 string literal (`type = 0`).
    pub fn push_str(&mut self, s: &str) -> &mut Self {
        let len = u16::try_from(s.len().saturating_add(2)).unwrap_or(0);
        self.bytes.push(0x96);
        self.bytes.extend_from_slice(&len.to_le_bytes());
        self.bytes.push(0x00); // type 0 = string
        self.bytes.extend_from_slice(s.as_bytes());
        self.bytes.push(0x00);
        self
    }

    /// Emits `ActionPush` (`0x96`) with a 64-bit IEEE-754 float (`type = 6`, SWF middle-endian 4-byte swap).
    pub fn push_f64(&mut self, val: f64) -> &mut Self {
        let le = val.to_le_bytes();
        self.bytes.push(0x96);
        self.bytes.extend_from_slice(&9_u16.to_le_bytes());
        self.bytes.push(0x06); // type 6 = double (high 4 bytes LE followed by low 4 bytes LE)
        self.bytes.extend_from_slice(le.get(4..8).unwrap_or(&[0; 4]));
        self.bytes.extend_from_slice(le.get(0..4).unwrap_or(&[0; 4]));
        self
    }

    /// Emits `ActionPush` (`0x96`) with a 32-bit signed integer (`type = 7`).
    pub fn push_i32(&mut self, val: i32) -> &mut Self {
        self.bytes.push(0x96);
        self.bytes.extend_from_slice(&5_u16.to_le_bytes());
        self.bytes.push(0x07); // type 7 = i32
        self.bytes.extend_from_slice(&val.to_le_bytes());
        self
    }

    /// Emits `ActionPush` (`0x96`) with a boolean (`type = 5`).
    pub fn push_bool(&mut self, val: bool) -> &mut Self {
        self.bytes.push(0x96);
        self.bytes.extend_from_slice(&2_u16.to_le_bytes());
        self.bytes.push(0x05);
        self.bytes.push(u8::from(val));
        self
    }

    /// Emits `ActionPush` (`0x96`) with `null` (`type = 2`).
    pub fn push_null(&mut self) -> &mut Self {
        self.bytes.push(0x96);
        self.bytes.extend_from_slice(&1_u16.to_le_bytes());
        self.bytes.push(0x02);
        self
    }

    /// Emits `ActionPush` (`0x96`) with `undefined` (`type = 3`).
    pub fn push_undefined(&mut self) -> &mut Self {
        self.bytes.push(0x96);
        self.bytes.extend_from_slice(&1_u16.to_le_bytes());
        self.bytes.push(0x03);
        self
    }

    /// Emits `ActionTrace` (`0x26`).
    pub fn trace(&mut self) -> &mut Self {
        self.op(0x26)
    }

    /// Emits `ActionGetVariable` (`0x1C`).
    pub fn get_var(&mut self) -> &mut Self {
        self.op(0x1C)
    }

    /// Emits `ActionSetVariable` (`0x1D`).
    pub fn set_var(&mut self) -> &mut Self {
        self.op(0x1D)
    }

    /// Emits `ActionAdd2` (`0x47`, ECMA-262 `+`).
    pub fn add2(&mut self) -> &mut Self {
        self.op(0x47)
    }

    /// Emits `ActionSubtract` (`0x0B`).
    pub fn sub(&mut self) -> &mut Self {
        self.op(0x0B)
    }

    /// Emits `ActionMultiply` (`0x0C`).
    pub fn mul(&mut self) -> &mut Self {
        self.op(0x0C)
    }

    /// Emits `ActionDivide` (`0x0D`).
    pub fn div(&mut self) -> &mut Self {
        self.op(0x0D)
    }

    /// Emits `ActionModulo` (`0x3F`).
    pub fn modulo(&mut self) -> &mut Self {
        self.op(0x3F)
    }

    /// Emits `ActionEquals2` (`0x49`, ECMA-262 `==`).
    pub fn equals2(&mut self) -> &mut Self {
        self.op(0x49)
    }

    /// Emits `ActionStrictEquals` (`0x66`, ECMA-262 `===`).
    pub fn strict_equals(&mut self) -> &mut Self {
        self.op(0x66)
    }

    /// Emits `ActionLess2` (`0x48`, ECMA-262 `<`).
    pub fn less2(&mut self) -> &mut Self {
        self.op(0x48)
    }

    /// Emits `ActionGreater` (`0x67`, ECMA-262 `>`).
    pub fn greater(&mut self) -> &mut Self {
        self.op(0x67)
    }

    /// Emits `ActionNot` (`0x12`, `!`).
    pub fn not(&mut self) -> &mut Self {
        self.op(0x12)
    }

    /// Emits `ActionAnd` (`0x10`).
    pub fn and(&mut self) -> &mut Self {
        self.op(0x10)
    }

    /// Emits `ActionOr` (`0x11`).
    pub fn or(&mut self) -> &mut Self {
        self.op(0x11)
    }

    /// Emits `ActionBitAnd` (`0x60`).
    pub fn bit_and(&mut self) -> &mut Self {
        self.op(0x60)
    }

    /// Emits `ActionBitOr` (`0x61`).
    pub fn bit_or(&mut self) -> &mut Self {
        self.op(0x61)
    }

    /// Emits `ActionBitXor` (`0x62`).
    pub fn bit_xor(&mut self) -> &mut Self {
        self.op(0x62)
    }

    /// Emits `ActionBitLShift` (`0x63`).
    pub fn shl(&mut self) -> &mut Self {
        self.op(0x63)
    }

    /// Emits `ActionBitRShift` (`0x64`).
    pub fn shr(&mut self) -> &mut Self {
        self.op(0x64)
    }

    /// Emits `ActionBitURShift` (`0x65`).
    pub fn ushr(&mut self) -> &mut Self {
        self.op(0x65)
    }

    /// Emits `ActionTypeOf` (`0x44`).
    pub fn typeof_op(&mut self) -> &mut Self {
        self.op(0x44)
    }

    /// Emits `ActionToNumber` (`0x4A`).
    pub fn to_number(&mut self) -> &mut Self {
        self.op(0x4A)
    }

    /// Emits `ActionToString` (`0x4B`).
    pub fn to_string_op(&mut self) -> &mut Self {
        self.op(0x4B)
    }

    /// Emits `ActionToInteger` (`0x18`).
    pub fn to_int(&mut self) -> &mut Self {
        self.op(0x18)
    }

    /// Emits `ActionIncrement` (`0x50`).
    pub fn increment(&mut self) -> &mut Self {
        self.op(0x50)
    }

    /// Emits `ActionDecrement` (`0x51`).
    pub fn decrement(&mut self) -> &mut Self {
        self.op(0x51)
    }

    /// Emits `ActionStringAdd` (`0x21`).
    pub fn string_add(&mut self) -> &mut Self {
        self.op(0x21)
    }

    /// Emits `ActionStringLength` (`0x14`).
    pub fn string_len(&mut self) -> &mut Self {
        self.op(0x14)
    }

    /// Emits `ActionGetMember` (`0x4E`).
    pub fn get_member(&mut self) -> &mut Self {
        self.op(0x4E)
    }

    /// Emits `ActionSetMember` (`0x4F`).
    pub fn set_member(&mut self) -> &mut Self {
        self.op(0x4F)
    }

    /// Emits `ActionInitObject` (`0x43`).
    pub fn init_object(&mut self) -> &mut Self {
        self.op(0x43)
    }

    /// Emits `ActionInitArray` (`0x42`).
    pub fn init_array(&mut self) -> &mut Self {
        self.op(0x42)
    }

    /// Emits `ActionCallFunction` (`0x3D`).
    pub fn call_function(&mut self) -> &mut Self {
        self.op(0x3D)
    }

    /// Emits `ActionCallMethod` (`0x52`).
    pub fn call_method(&mut self) -> &mut Self {
        self.op(0x52)
    }

    /// Emits `ActionDefineFunction` (`0x9B`).
    pub fn define_function(&mut self, name: &str, params: &[&str], body: &[u8]) -> &mut Self {
        let mut header = Vec::new();
        header.extend_from_slice(name.as_bytes());
        header.push(0);
        let num_params = u16::try_from(params.len()).unwrap_or(0);
        header.extend_from_slice(&num_params.to_le_bytes());
        for &p in params {
            header.extend_from_slice(p.as_bytes());
            header.push(0);
        }
        let code_size = u16::try_from(body.len()).unwrap_or(0);
        header.extend_from_slice(&code_size.to_le_bytes());

        self.bytes.push(0x9B);
        let tag_len = u16::try_from(header.len()).unwrap_or(0);
        self.bytes.extend_from_slice(&tag_len.to_le_bytes());
        self.bytes.extend_from_slice(&header);
        self.bytes.extend_from_slice(body);
        self
    }

    /// Emits `ActionReturn` (`0x3E`).
    pub fn ret(&mut self) -> &mut Self {
        self.op(0x3E)
    }

    /// Emits `ActionPlay` (`0x06`).
    pub fn play(&mut self) -> &mut Self {
        self.op(0x06)
    }

    /// Emits `ActionStop` (`0x07`).
    pub fn stop(&mut self) -> &mut Self {
        self.op(0x07)
    }

    /// Emits `ActionNextFrame` (`0x04`).
    pub fn next_frame(&mut self) -> &mut Self {
        self.op(0x04)
    }

    /// Emits `ActionPrevFrame` (`0x05`).
    pub fn prev_frame(&mut self) -> &mut Self {
        self.op(0x05)
    }

    /// Emits `ActionGotoFrame` (`0x81`, 0-based frame index).
    pub fn goto_frame(&mut self, frame: u16) -> &mut Self {
        self.bytes.push(0x81);
        self.bytes.extend_from_slice(&2_u16.to_le_bytes());
        self.bytes.extend_from_slice(&frame.to_le_bytes());
        self
    }

    /// Emits `ActionGotoLabel` (`0x8C`).
    pub fn goto_label(&mut self, label: &str) -> &mut Self {
        let len = u16::try_from(label.len().saturating_add(1)).unwrap_or(0);
        self.bytes.push(0x8C);
        self.bytes.extend_from_slice(&len.to_le_bytes());
        self.bytes.extend_from_slice(label.as_bytes());
        self.bytes.push(0);
        self
    }

    /// Emits `ActionIf` (`0x9D`, signed 16-bit byte offset).
    pub fn branch_if(&mut self, offset: i16) -> &mut Self {
        self.bytes.push(0x9D);
        self.bytes.extend_from_slice(&2_u16.to_le_bytes());
        self.bytes.extend_from_slice(&offset.to_le_bytes());
        self
    }

    /// Emits `ActionJump` (`0x99`, signed 16-bit byte offset).
    pub fn jump(&mut self, offset: i16) -> &mut Self {
        self.bytes.push(0x99);
        self.bytes.extend_from_slice(&2_u16.to_le_bytes());
        self.bytes.extend_from_slice(&offset.to_le_bytes());
        self
    }
}

/// Deterministic seeded `xorshift64` SWF mutator for fuzzing and hostile-input testing (TSD 2.13).
#[derive(Debug, Clone)]
pub struct Mutator {
    state: u64,
}

impl Mutator {
    /// Creates a new [`Mutator`] with a non-zero seed.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        }
    }

    /// Advances the `xorshift64` state and returns the next pseudo-random `u64`.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Mutates `seed_swf` using one of the 6 strategies from TSD 2.13:
    /// 1. Bit flip
    /// 2. Byte replace
    /// 3. Truncate
    /// 4. Duplicate a chunk
    /// 5. Corrupt a tag length
    /// 6. Set a count field to its maximum (`0xFF` / `0xFFFF`).
    #[must_use]
    pub fn mutate(&mut self, seed_swf: &[u8]) -> Vec<u8> {
        let mut buf = seed_swf.to_vec();
        if buf.is_empty() {
            return buf;
        }
        let kind = self.next_u64() % 6;
        let len = buf.len();
        let idx = usize::try_from(self.next_u64() % (len as u64)).unwrap_or(0);

        match kind {
            0 => {
                // Bit flip
                let bit = u8::try_from(self.next_u64() % 8).unwrap_or(0);
                if let Some(b) = buf.get_mut(idx) {
                    *b ^= 1_u8 << bit;
                }
            }
            1 => {
                // Byte replace
                let rep = u8::try_from(self.next_u64() & 0xFF).unwrap_or(0);
                if let Some(b) = buf.get_mut(idx) {
                    *b = rep;
                }
            }
            2 => {
                // Truncate
                buf.truncate(idx);
            }
            3 => {
                // Duplicate a chunk
                let chunk_len = usize::try_from((self.next_u64() % 16).saturating_add(1))
                    .unwrap_or(1)
                    .min(len.saturating_sub(idx));
                let end = idx.saturating_add(chunk_len);
                if let Some(slice) = buf.get(idx..end) {
                    let copy = slice.to_vec();
                    buf.extend_from_slice(&copy);
                }
            }
            4 => {
                // Corrupt a tag length field (set low 6 bits to 0x3F long tag or large value)
                if let Some(b) = buf.get_mut(idx) {
                    *b |= 0x3F;
                }
            }
            _ => {
                // Set a count field to maximum (0xFF / 0xFFFF)
                if let Some(b) = buf.get_mut(idx) {
                    *b = 0xFF;
                }
                if let Some(b2) = buf.get_mut(idx.saturating_add(1)) {
                    *b2 = 0xFF;
                }
            }
        }
        buf
    }
}

/// Encodes an `XRGB8888` frame (`width * height` `u32` pixels) as an RGB PNG byte vector.
///
/// # Errors
///
/// Returns an error string if PNG encoding fails.
pub fn encode_frame_png(width: u32, height: u32, xrgb: &[u32]) -> Result<Vec<u8>, String> {
    let mut rgb = Vec::with_capacity(
        usize::try_from(width.saturating_mul(height).saturating_mul(3)).unwrap_or(0),
    );
    for &px in xrgb {
        let r = u8::try_from((px >> 16) & 0xFF).unwrap_or(0);
        let g = u8::try_from((px >> 8) & 0xFF).unwrap_or(0);
        let b = u8::try_from(px & 0xFF).unwrap_or(0);
        rgb.push(r);
        rgb.push(g);
        rgb.push(b);
    }
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&rgb).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

/// Decodes an RGB or RGBA PNG byte slice into `(width, height, Vec<u32> XRGB8888)`.
///
/// # Errors
///
/// Returns an error string if PNG decoding fails.
pub fn decode_frame_png(png_bytes: &[u8]) -> Result<(u32, u32, Vec<u32>), String> {
    let decoder = png::Decoder::new(Cursor::new(png_bytes));
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let buf_size = reader.output_buffer_size().ok_or("missing PNG buffer size")?;
    let mut buf = vec![0_u8; buf_size];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let bytes = buf.get(..info.buffer_size()).unwrap_or(&[]);
    let step = match info.color_type {
        png::ColorType::Rgb => 3_usize,
        png::ColorType::Rgba => 4_usize,
        _ => return Err("unsupported PNG color type".to_owned()),
    };
    let mut xrgb = Vec::with_capacity(
        usize::try_from(info.width.saturating_mul(info.height)).unwrap_or(0),
    );
    let mut idx = 0_usize;
    while idx.saturating_add(step) <= bytes.len() {
        let r = u32::from(bytes.get(idx).copied().unwrap_or(0));
        let g = u32::from(bytes.get(idx.saturating_add(1)).copied().unwrap_or(0));
        let b = u32::from(bytes.get(idx.saturating_add(2)).copied().unwrap_or(0));
        xrgb.push((r << 16) | (g << 8) | b);
        idx = idx.saturating_add(step);
    }
    Ok((info.width, info.height, xrgb))
}

/// Compares a rendered `XRGB8888` frame against `golden_dir/<name>.png` (approved) or
/// `provisional_dir/<name>.png` (provisional, TSD 3.7), with per-channel tolerance `2` and
/// at most `1%` of pixels outside tolerance (TSD 2.13).
///
/// If neither file exists and `create_provisional` is `true`, sanity-checks the frame
/// (non-empty, non-uniform unless `allow_uniform` is set) and writes `provisional_dir/<name>.png`.
///
/// # Errors
///
/// Returns an error string on dimension mismatch, sanity check failure, or pixel diff exceeding tolerance.
#[allow(clippy::arithmetic_side_effects)]
pub fn verify_or_create_golden(
    approved_dir: &Path,
    provisional_dir: &Path,
    name: &str,
    expectation_note: &str,
    width: u32,
    height: u32,
    xrgb: &[u32],
    allow_uniform: bool,
    create_provisional: bool,
) -> Result<(), String> {
    if expectation_note.trim().is_empty() {
        return Err("provisional golden requires a non-empty one-line expectation note".to_owned());
    }
    let expected_len = usize::try_from(width.saturating_mul(height)).unwrap_or(0);
    if xrgb.len() != expected_len || expected_len == 0 {
        return Err(format!(
            "invalid frame buffer length {} for {}x{}",
            xrgb.len(),
            width,
            height
        ));
    }
    if !allow_uniform {
        let first = xrgb.first().copied().unwrap_or(0);
        if xrgb.iter().all(|&px| px == first) {
            return Err(format!(
                "sanity check failed for {name}: frame is uniformly {first:#010x}"
            ));
        }
    }

    let approved_path = approved_dir.join(format!("{name}.png"));
    let provisional_path = provisional_dir.join(format!("{name}.png"));
    let target_path = if approved_path.exists() {
        Some(approved_path)
    } else if provisional_path.exists() {
        Some(provisional_path.clone())
    } else {
        None
    };

    if let Some(ref path) = target_path {
        let png_bytes = fs::read(path).map_err(|e| e.to_string())?;
        let (gw, gh, gxrgb) = decode_frame_png(&png_bytes)?;
        if gw != width || gh != height {
            return Err(format!(
                "golden dimension mismatch for {name}: expected {gw}x{gh}, got {width}x{height}"
            ));
        }
        let mut outliers = 0_usize;
        for (&actual, &gold) in xrgb.iter().zip(gxrgb.iter()) {
            let ar = ((actual >> 16) & 0xFF) as i32;
            let ag = ((actual >> 8) & 0xFF) as i32;
            let ab = (actual & 0xFF) as i32;
            let gr = ((gold >> 16) & 0xFF) as i32;
            let gg = ((gold >> 8) & 0xFF) as i32;
            let gb = (gold & 0xFF) as i32;
            if (ar - gr).abs() > 2 || (ag - gg).abs() > 2 || (ab - gb).abs() > 2 {
                outliers += 1;
            }
        }
        let max_outliers = expected_len / 100; // 1%
        if outliers > max_outliers {
            return Err(format!(
                "golden comparison failed for {name}: {outliers}/{expected_len} pixels exceeded tolerance 2"
            ));
        }
        return Ok(());
    }

    if create_provisional {
        fs::create_dir_all(provisional_dir).map_err(|e| e.to_string())?;
        let png_bytes = encode_frame_png(width, height, xrgb)?;
        fs::write(&provisional_path, png_bytes).map_err(|e| e.to_string())?;
        return Ok(());
    }

    Err(format!("missing golden image for {name}"))
}
