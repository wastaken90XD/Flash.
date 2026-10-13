//! CPU 2D software renderer (`tiny-skia`) for `swfr` with a 64-entry flattened-shape LRU cache,
//! dirty-rectangle tracking, clipping masks, gradients, bitmaps, embedded & device text,
//! and `XRGB8888` / `RGB565` output conversion (TSD 1.6, 2.7, 2.12 M2 & M3).

use std::collections::BTreeMap;
use swfr_reader::{ColorTransform, Matrix, Rect};
use swfr_scene::{
    flatten_morph_shape, flatten_shape, ButtonState, Character, DecodedBitmap, DisplayObject,
    FlattenedShape, PathCmd, Stage,
};
use swfr_swf::{
    ButtonDef, EditTextDef, FillStyle, FontDef, LineStyle, Rgba, ShapeDef, ShapeRecord, TextDef,
};
use tiny_skia::{
    Color, FillRule, FilterQuality, GradientStop as SkiaStop, LineCap, LineJoin, LinearGradient,
    Mask, Paint, Path, PathBuilder, Pattern, Pixmap, Point, RadialGradient, SpreadMode, Stroke,
    Transform,
};

/// Maximum entries in the flattened-shape LRU cache (TSD 2.12 M2 Task 3).
pub const SHAPE_LRU_CAPACITY: usize = 64;

#[derive(Debug, Clone)]
struct LruEntry {
    key: (u16, u16),
    last_used: u64,
    shape: FlattenedShape,
}

/// Bounded 64-entry LRU cache mapping `(character_id, ratio)` to [`FlattenedShape`].
#[derive(Debug, Clone, Default)]
pub struct ShapeCache {
    entries: Vec<LruEntry>,
    clock: u64,
}

impl ShapeCache {
    /// Creates an empty [`ShapeCache`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
            clock: 0,
        }
    }

    /// Returns a cached [`FlattenedShape`] or computes and inserts it, evicting the least-recently-used
    /// entry when at [`SHAPE_LRU_CAPACITY`].
    pub fn get_or_insert_with<F>(&mut self, key: (u16, u16), compute: F) -> &FlattenedShape
    where
        F: FnOnce() -> FlattenedShape,
    {
        self.clock = self.clock.wrapping_add(1);
        let now = self.clock;
        if let Some(pos) = self.entries.iter().position(|e| e.key == key) {
            if let Some(entry) = self.entries.get_mut(pos) {
                entry.last_used = now;
            }
            return &self.entries[pos].shape;
        }
        let shape = compute();
        if self.entries.len() >= SHAPE_LRU_CAPACITY {
            if let Some((oldest_idx, _)) = self
                .entries
                .iter()
                .enumerate()
                .min_by_key(|(_, e)| e.last_used)
            {
                self.entries.swap_remove(oldest_idx);
            }
        }
        self.entries.push(LruEntry {
            key,
            last_used: now,
            shape,
        });
        let last = self.entries.len().saturating_sub(1);
        &self.entries[last].shape
    }
}

/// Software 2D renderer backed by [`tiny_skia::Pixmap`].
#[derive(Debug, Clone)]
pub struct Renderer {
    pixmap: Pixmap,
    xrgb_buf: Vec<u32>,
    rgb565_buf: Vec<u16>,
    cache: ShapeCache,
    scale: u32,
}

impl Renderer {
    /// Creates a new [`Renderer`] for stage dimensions `(width_px, height_px)` and resolution scale (`1` or `2`).
    #[must_use]
    pub fn new(width_px: u32, height_px: u32, scale: u32) -> Self {
        let s = scale.clamp(1, 2);
        let w = width_px.saturating_mul(s).max(1);
        let h = height_px.saturating_mul(s).max(1);
        let pixmap = Pixmap::new(w, h).unwrap_or_else(|| Pixmap::new(1, 1).expect("1x1 pixmap"));
        let count = usize::try_from(w.saturating_mul(h)).unwrap_or(1);
        Self {
            pixmap,
            xrgb_buf: vec![0; count],
            rgb565_buf: vec![0; count],
            cache: ShapeCache::new(),
            scale: s,
        }
    }

    /// Current output width in pixels (`stage_width * scale`).
    #[must_use]
    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    /// Current output height in pixels (`stage_height * scale`).
    #[must_use]
    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }

    /// Current resolution scale (`1` or `2`).
    #[must_use]
    pub const fn scale(&self) -> u32 {
        self.scale
    }

    /// Resizes the renderer if `(stage_width_px * scale, stage_height_px * scale)` changed.
    pub fn ensure_size(&mut self, stage_width_px: u32, stage_height_px: u32, scale: u32) -> bool {
        let s = scale.clamp(1, 2);
        let target_w = stage_width_px.saturating_mul(s).max(1);
        let target_h = stage_height_px.saturating_mul(s).max(1);
        if self.pixmap.width() == target_w && self.pixmap.height() == target_h && self.scale == s {
            return false;
        }
        self.scale = s;
        if let Some(pm) = Pixmap::new(target_w, target_h) {
            self.pixmap = pm;
        }
        let count = usize::try_from(target_w.saturating_mul(target_h)).unwrap_or(1);
        self.xrgb_buf.resize(count, 0);
        self.rgb565_buf.resize(count, 0);
        true
    }

    /// Renders `stage` if `stage.full_redraw` or `!stage.dirty_rects.is_empty()`.
    /// Returns `true` if the framebuffer was updated, or `false` if nothing was dirty (allowing `can_dupe`).
    pub fn render_stage(&mut self, stage: &mut Stage, scale: u32) -> bool {
        let resized = self.ensure_size(stage.width_px, stage.height_px, scale);
        if !resized && !stage.full_redraw && stage.dirty_rects.is_empty() {
            return false;
        }
        stage.full_redraw = false;
        stage.dirty_rects.clear();

        let bg = stage.background_color;
        self.pixmap.fill(Color::from_rgba8(bg.r, bg.g, bg.b, 255));

        let root_xform = Transform::from_scale(self.scale as f32 / 20.0, self.scale as f32 / 20.0);
        render_display_list(
            &mut self.pixmap,
            &mut self.cache,
            &stage.root.display_list,
            &stage.dictionary,
            root_xform,
            ColorTransform::IDENTITY,
            None,
        );

        self.sync_output_buffers();
        true
    }

    /// Draws a visible software cursor reticle at `(x_px, y_px)` in output pixel coordinates.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn draw_cursor(&mut self, x_px: i32, y_px: i32, pressed: bool) {
        let w = self.pixmap.width() as i32;
        let h = self.pixmap.height() as i32;
        let outer = if pressed {
            0x00FF_4040_u32
        } else {
            0x0000_0000_u32
        };
        let inner = if pressed {
            0x00FF_FF00_u32
        } else {
            0x00FF_FFFF_u32
        };

        for dy in -4..=4 {
            for dx in -4..=4 {
                let px = x_px + dx;
                let py = y_px + dy;
                if px < 0 || py < 0 || px >= w || py >= h {
                    continue;
                }
                let dist = dx.abs() + dy.abs();
                let color = if dist <= 1 || (dx == 0 && dy.abs() <= 3) || (dy == 0 && dx.abs() <= 3)
                {
                    Some(inner)
                } else if dist <= 4 && (dx.abs() <= 1 || dy.abs() <= 1) {
                    Some(outer)
                } else {
                    None
                };
                if let Some(c) = color {
                    let idx = (py * w + px) as usize;
                    if let Some(slot) = self.xrgb_buf.get_mut(idx) {
                        *slot = c;
                    }
                    if let Some(slot565) = self.rgb565_buf.get_mut(idx) {
                        *slot565 = xrgb_to_rgb565(c);
                    }
                }
            }
        }
    }

    /// Returns the `XRGB8888` (`0x00RRGGBB`) framebuffer slice.
    #[must_use]
    pub fn xrgb8888(&self) -> &[u32] {
        &self.xrgb_buf
    }

    /// Returns the `RGB565` (`RRRRRGGGGGGBBBBB`) framebuffer slice.
    #[must_use]
    pub fn rgb565(&self) -> &[u16] {
        &self.rgb565_buf
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn sync_output_buffers(&mut self) {
        let data = self.pixmap.data();
        for (i, px) in data.chunks_exact(4).enumerate() {
            // tiny-skia stores premultiplied RGBA; since background is opaque (A=255), R/G/B are final
            let r = u32::from(px[0]);
            let g = u32::from(px[1]);
            let b = u32::from(px[2]);
            let xrgb = (r << 16) | (g << 8) | b;
            self.xrgb_buf[i] = xrgb;
            self.rgb565_buf[i] = xrgb_to_rgb565(xrgb);
        }
    }
}

#[allow(clippy::arithmetic_side_effects)]
const fn xrgb_to_rgb565(xrgb: u32) -> u16 {
    let r5 = ((xrgb >> 19) & 0x1F) as u16;
    let g6 = ((xrgb >> 10) & 0x3F) as u16;
    let b5 = ((xrgb >> 3) & 0x1F) as u16;
    (r5 << 11) | (g6 << 5) | b5
}

fn render_display_list(
    pixmap: &mut Pixmap,
    cache: &mut ShapeCache,
    display_list: &BTreeMap<u16, DisplayObject>,
    dict: &BTreeMap<u16, Character>,
    parent_xform: Transform,
    parent_cx: ColorTransform,
    inherited_mask: Option<&Mask>,
) {
    let mut active_clip: Option<(u16, Mask)> = None;

    for (&depth, obj) in display_list {
        if let Some((clip_end, _)) = active_clip {
            if depth > clip_end {
                active_clip = None;
            }
        }

        let obj_xform = parent_xform.pre_concat(matrix_to_skia(obj.matrix));
        let obj_cx = combine_cxform(parent_cx, obj.color_transform);

        if let Some(clip_end) = obj.clip_depth {
            if clip_end > depth {
                let mut mask_pm = Pixmap::new(pixmap.width(), pixmap.height())
                    .unwrap_or_else(|| Pixmap::new(1, 1).expect("1x1 mask pixmap"));
                render_object(
                    &mut mask_pm,
                    cache,
                    obj,
                    dict,
                    obj_xform,
                    ColorTransform::IDENTITY,
                    None,
                );
                let mask = Mask::from_pixmap(mask_pm.as_ref(), tiny_skia::MaskType::Alpha);
                active_clip = Some((clip_end, mask));
                continue;
            }
        }

        let mask_ref = active_clip.as_ref().map(|(_, m)| m).or(inherited_mask);

        render_object(pixmap, cache, obj, dict, obj_xform, obj_cx, mask_ref);
    }
}

#[allow(clippy::too_many_arguments)]
fn render_object(
    pixmap: &mut Pixmap,
    cache: &mut ShapeCache,
    obj: &DisplayObject,
    dict: &BTreeMap<u16, Character>,
    xform: Transform,
    cx: ColorTransform,
    mask: Option<&Mask>,
) {
    let Some(ch) = dict.get(&obj.character_id) else {
        return;
    };
    match ch {
        Character::Shape(shape_def) => {
            let flat = cache
                .get_or_insert_with((obj.character_id, 0), || flatten_shape(shape_def))
                .clone();
            render_flattened_shape(pixmap, &flat, dict, xform, cx, mask);
        }
        Character::MorphShape(morph_def) => {
            let flat = cache
                .get_or_insert_with((obj.character_id, obj.ratio), || {
                    flatten_morph_shape(morph_def, obj.ratio)
                })
                .clone();
            render_flattened_shape(pixmap, &flat, dict, xform, cx, mask);
        }
        Character::Bitmap(bmp) => {
            render_bitmap_quad(pixmap, bmp, xform, cx, mask);
        }
        Character::Text(txt) => {
            render_static_text(pixmap, txt, dict, xform, cx, mask);
        }
        Character::EditText(edit) => {
            let text_str = obj
                .dynamic_text
                .as_deref()
                .or(edit.initial_text.as_deref())
                .unwrap_or("");
            render_edit_text(pixmap, edit, text_str, dict, xform, cx, mask);
        }
        Character::Button(btn) => {
            render_button(pixmap, cache, btn, obj.button_state, dict, xform, cx, mask);
        }
        Character::Sprite { .. } => {
            if let Some(ref child_tl) = obj.sprite_timeline {
                render_display_list(pixmap, cache, &child_tl.display_list, dict, xform, cx, mask);
            }
        }
        Character::Font(_) | Character::Sound(_) => {}
    }
}

fn render_flattened_shape(
    pixmap: &mut Pixmap,
    flat: &FlattenedShape,
    dict: &BTreeMap<u16, Character>,
    xform: Transform,
    cx: ColorTransform,
    mask: Option<&Mask>,
) {
    for fill in &flat.fills {
        let Some(path) = build_skia_path(&fill.cmds) else {
            continue;
        };
        match &fill.style {
            FillStyle::Solid(rgba) => {
                let mut paint = Paint::default();
                paint.anti_alias = true;
                let c = apply_cxform(*rgba, cx);
                paint.set_color_rgba8(c.r, c.g, c.b, c.a);
                pixmap.fill_path(&path, &paint, FillRule::EvenOdd, xform, mask);
            }
            FillStyle::LinearGradient { matrix, stops } => {
                let grad_xform = matrix_to_skia(*matrix);
                let skia_stops: Vec<SkiaStop> = stops
                    .iter()
                    .map(|s| {
                        let c = apply_cxform(s.color, cx);
                        SkiaStop::new(
                            f32::from(s.ratio) / 255.0,
                            Color::from_rgba8(c.r, c.g, c.b, c.a),
                        )
                    })
                    .collect();
                if let Some(shader) = LinearGradient::new(
                    Point::from_xy(-16384.0, 0.0),
                    Point::from_xy(16384.0, 0.0),
                    skia_stops,
                    SpreadMode::Pad,
                    grad_xform,
                ) {
                    let paint = Paint {
                        shader,
                        anti_alias: true,
                        ..Paint::default()
                    };
                    pixmap.fill_path(&path, &paint, FillRule::EvenOdd, xform, mask);
                }
            }
            FillStyle::RadialGradient {
                matrix,
                stops,
                focal_point_q8,
            } => {
                let grad_xform = matrix_to_skia(*matrix);
                let skia_stops: Vec<SkiaStop> = stops
                    .iter()
                    .map(|s| {
                        let c = apply_cxform(s.color, cx);
                        SkiaStop::new(
                            f32::from(s.ratio) / 255.0,
                            Color::from_rgba8(c.r, c.g, c.b, c.a),
                        )
                    })
                    .collect();
                let focal_x = (f32::from(*focal_point_q8) / 256.0).clamp(-0.98, 0.98) * 16384.0;
                if let Some(shader) = RadialGradient::new(
                    Point::from_xy(focal_x, 0.0),
                    Point::from_xy(0.0, 0.0),
                    16384.0,
                    skia_stops,
                    SpreadMode::Pad,
                    grad_xform,
                ) {
                    let paint = Paint {
                        shader,
                        anti_alias: true,
                        ..Paint::default()
                    };
                    pixmap.fill_path(&path, &paint, FillRule::EvenOdd, xform, mask);
                }
            }
            FillStyle::Bitmap {
                bitmap_id,
                matrix,
                repeating,
                smoothed,
            } => {
                if let Some(Character::Bitmap(bmp)) = dict.get(bitmap_id) {
                    if let Some(bmp_pm) = decoded_bitmap_to_pixmap(bmp, cx) {
                        let spread = if *repeating {
                            SpreadMode::Repeat
                        } else {
                            SpreadMode::Pad
                        };
                        let quality = if *smoothed {
                            FilterQuality::Bilinear
                        } else {
                            FilterQuality::Nearest
                        };
                        let pat_xform = matrix_to_skia(*matrix);
                        let shader = Pattern::new(bmp_pm.as_ref(), spread, quality, 1.0, pat_xform);
                        let paint = Paint {
                            shader,
                            anti_alias: true,
                            ..Paint::default()
                        };
                        pixmap.fill_path(&path, &paint, FillRule::EvenOdd, xform, mask);
                    }
                }
            }
        }
    }

    for stroke in &flat.strokes {
        let Some(path) = build_skia_path(&stroke.cmds) else {
            continue;
        };
        let mut paint = Paint::default();
        paint.anti_alias = true;
        let c = apply_cxform(stroke.style.color, cx);
        paint.set_color_rgba8(c.r, c.g, c.b, c.a);
        let sk_stroke = Stroke {
            width: f32::from(stroke.style.width_twips).max(20.0),
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        pixmap.stroke_path(&path, &paint, &sk_stroke, xform, mask);
    }
}

fn render_bitmap_quad(
    pixmap: &mut Pixmap,
    bmp: &DecodedBitmap,
    xform: Transform,
    cx: ColorTransform,
    mask: Option<&Mask>,
) {
    let Some(bmp_pm) = decoded_bitmap_to_pixmap(bmp, cx) else {
        return;
    };
    let w_twips = (bmp.width as f32) * 20.0;
    let h_twips = (bmp.height as f32) * 20.0;
    let mut pb = PathBuilder::new();
    pb.move_to(0.0, 0.0);
    pb.line_to(w_twips, 0.0);
    pb.line_to(w_twips, h_twips);
    pb.line_to(0.0, h_twips);
    pb.close();
    let Some(path) = pb.finish() else {
        return;
    };
    let shader = Pattern::new(
        bmp_pm.as_ref(),
        SpreadMode::Pad,
        FilterQuality::Bilinear,
        1.0,
        Transform::from_scale(20.0, 20.0),
    );
    let paint = Paint {
        shader,
        anti_alias: true,
        ..Paint::default()
    };
    pixmap.fill_path(&path, &paint, FillRule::EvenOdd, xform, mask);
}

#[allow(clippy::arithmetic_side_effects)]
fn render_static_text(
    pixmap: &mut Pixmap,
    txt: &TextDef,
    dict: &BTreeMap<u16, Character>,
    parent_xform: Transform,
    cx: ColorTransform,
    mask: Option<&Mask>,
) {
    let text_xform = parent_xform.pre_concat(matrix_to_skia(txt.matrix));
    let mut cur_font: Option<&FontDef> = None;
    let mut cur_color = Rgba::rgb(0, 0, 0);
    let mut cur_height = 240.0_f32;
    let mut cur_x = 0.0_f32;
    let mut cur_y = 0.0_f32;

    for rec in &txt.records {
        if let Some(fid) = rec.font_id {
            if let Some(Character::Font(f)) = dict.get(&fid) {
                cur_font = Some(f);
            }
        }
        if let Some(c) = rec.color {
            cur_color = c;
        }
        if let Some(xoff) = rec.x_offset_twips {
            cur_x = f32::from(xoff);
        }
        if let Some(yoff) = rec.y_offset_twips {
            cur_y = f32::from(yoff);
        }
        if let Some(th) = rec.text_height_twips {
            cur_height = f32::from(th);
        }

        if let Some(font) = cur_font {
            let em_square = if font.version >= 3 {
                20480.0_f32
            } else {
                1024.0_f32
            };
            let glyph_scale = cur_height / em_square;
            for entry in &rec.glyphs {
                let g_idx = usize::try_from(entry.glyph_index).unwrap_or(usize::MAX);
                if let Some(glyph_records) = font.glyphs.get(g_idx) {
                    render_glyph_records(
                        pixmap,
                        glyph_records,
                        cur_color,
                        cx,
                        text_xform.pre_concat(Transform::from_row(
                            glyph_scale,
                            0.0,
                            0.0,
                            glyph_scale,
                            cur_x,
                            cur_y,
                        )),
                        mask,
                    );
                }
                cur_x += entry.advance_twips as f32;
            }
        }
    }
}

#[allow(clippy::arithmetic_side_effects, clippy::too_many_arguments)]
fn render_edit_text(
    pixmap: &mut Pixmap,
    edit: &EditTextDef,
    text_str: &str,
    dict: &BTreeMap<u16, Character>,
    xform: Transform,
    cx: ColorTransform,
    mask: Option<&Mask>,
) {
    if text_str.is_empty() {
        return;
    }
    let height_twips = f32::from(edit.font_height_twips.max(160));
    let mut cur_x = (edit.bounds.x_min + 40) as f32;
    let baseline_y = (edit.bounds.y_min as f32) + height_twips;

    // First try embedded font outlines if available
    if let Some(fid) = edit.font_id {
        if let Some(Character::Font(font)) = dict.get(&fid) {
            if !font.glyphs.is_empty() && !font.codes.is_empty() {
                let em_square = if font.version >= 3 {
                    20480.0_f32
                } else {
                    1024.0_f32
                };
                let scale = height_twips / em_square;
                for ch in text_str.chars() {
                    let code = ch as u16;
                    if let Some(g_idx) = font.codes.iter().position(|&c| c == code) {
                        if let Some(glyph_records) = font.glyphs.get(g_idx) {
                            render_glyph_records(
                                pixmap,
                                glyph_records,
                                edit.color,
                                cx,
                                xform.pre_concat(Transform::from_row(
                                    scale, 0.0, 0.0, scale, cur_x, baseline_y,
                                )),
                                mask,
                            );
                        }
                    }
                    cur_x += height_twips * 0.6;
                }
                return;
            }
        }
    }

    // Fallback built-in vector stroke glyphs for device fonts / dynamic text
    let c = apply_cxform(edit.color, cx);
    let mut paint = Paint::default();
    paint.anti_alias = true;
    paint.set_color_rgba8(c.r, c.g, c.b, c.a);
    let stroke = Stroke {
        width: (height_twips * 0.12).max(20.0),
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Stroke::default()
    };

    let char_w = height_twips * 0.5;
    let char_h = height_twips * 0.75;
    let top_y = baseline_y - char_h;

    for ch in text_str.chars() {
        if ch == '\n' {
            cur_x = (edit.bounds.x_min + 40) as f32;
            continue;
        }
        if let Some(path) = builtin_char_path(ch, cur_x, top_y, char_w, char_h) {
            pixmap.stroke_path(&path, &paint, &stroke, xform, mask);
        }
        cur_x += char_w * 1.35;
    }
}

fn render_glyph_records(
    pixmap: &mut Pixmap,
    records: &[ShapeRecord],
    color: Rgba,
    cx: ColorTransform,
    xform: Transform,
    mask: Option<&Mask>,
) {
    let shape = ShapeDef {
        id: 0,
        version: 3,
        bounds: Rect::default(),
        fill_styles: vec![FillStyle::Solid(color)],
        line_styles: Vec::new(),
        records: records.to_vec(),
    };
    let flat = flatten_shape(&shape);
    for fill in &flat.fills {
        if let Some(path) = build_skia_path(&fill.cmds) {
            let mut paint = Paint::default();
            paint.anti_alias = true;
            let c = apply_cxform(color, cx);
            paint.set_color_rgba8(c.r, c.g, c.b, c.a);
            pixmap.fill_path(&path, &paint, FillRule::EvenOdd, xform, mask);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_button(
    pixmap: &mut Pixmap,
    cache: &mut ShapeCache,
    btn: &ButtonDef,
    state: ButtonState,
    dict: &BTreeMap<u16, Character>,
    parent_xform: Transform,
    parent_cx: ColorTransform,
    mask: Option<&Mask>,
) {
    let mut matching: Vec<_> = btn
        .records
        .iter()
        .filter(|r| match state {
            ButtonState::Up => r.state_up,
            ButtonState::Over => r.state_over,
            ButtonState::Down => r.state_down,
        })
        .collect();
    matching.sort_by_key(|r| r.depth);

    for rec in matching {
        let dummy_obj = DisplayObject {
            character_id: rec.character_id,
            depth: rec.depth,
            matrix: rec.matrix,
            color_transform: rec.color_transform,
            ratio: 0,
            name: None,
            clip_depth: None,
            button_state: ButtonState::Up,
            button_pressed_inside: false,
            dynamic_text: None,
            sprite_timeline: None,
        };
        let child_xform = parent_xform.pre_concat(matrix_to_skia(rec.matrix));
        let child_cx = combine_cxform(parent_cx, rec.color_transform);
        render_object(pixmap, cache, &dummy_obj, dict, child_xform, child_cx, mask);
    }
}

fn build_skia_path(cmds: &[PathCmd]) -> Option<Path> {
    let mut pb = PathBuilder::new();
    for &cmd in cmds {
        match cmd {
            PathCmd::MoveTo(x, y) => pb.move_to(x as f32, y as f32),
            PathCmd::LineTo(x, y) => pb.line_to(x as f32, y as f32),
            PathCmd::QuadTo { cx, cy, ax, ay } => {
                pb.quad_to(cx as f32, cy as f32, ax as f32, ay as f32);
            }
            PathCmd::Close => pb.close(),
        }
    }
    pb.finish()
}

fn matrix_to_skia(m: Matrix) -> Transform {
    Transform::from_row(
        m.scale_x_fp16 as f32 / 65536.0,
        m.rotate_skew0_fp16 as f32 / 65536.0,
        m.rotate_skew1_fp16 as f32 / 65536.0,
        m.scale_y_fp16 as f32 / 65536.0,
        m.translate_x_twips as f32,
        m.translate_y_twips as f32,
    )
}

#[allow(clippy::arithmetic_side_effects)]
fn apply_cxform(c: Rgba, cx: ColorTransform) -> Rgba {
    let xform = |chan: u8, mult: i16, add: i16| -> u8 {
        let v = ((i32::from(chan) * i32::from(mult)) >> 8) + i32::from(add);
        v.clamp(0, 255) as u8
    };
    Rgba {
        r: xform(c.r, cx.r_mult, cx.r_add),
        g: xform(c.g, cx.g_mult, cx.g_add),
        b: xform(c.b, cx.b_mult, cx.b_add),
        a: xform(c.a, cx.a_mult, cx.a_add),
    }
}

#[allow(clippy::arithmetic_side_effects)]
const fn combine_cxform(parent: ColorTransform, child: ColorTransform) -> ColorTransform {
    let mult =
        |pm: i16, cm: i16| -> i16 { ((pm as i32 * cm as i32) >> 8).clamp(-32768, 32767) as i16 };
    let add = |pm: i16, pa: i16, ca: i16| -> i16 {
        (((ca as i32 * pm as i32) >> 8) + pa as i32).clamp(-32768, 32767) as i16
    };
    ColorTransform {
        r_mult: mult(parent.r_mult, child.r_mult),
        g_mult: mult(parent.g_mult, child.g_mult),
        b_mult: mult(parent.b_mult, child.b_mult),
        a_mult: mult(parent.a_mult, child.a_mult),
        r_add: add(parent.r_mult, parent.r_add, child.r_add),
        g_add: add(parent.g_mult, parent.g_add, child.g_add),
        b_add: add(parent.b_mult, parent.b_add, child.b_add),
        a_add: add(parent.a_mult, parent.a_add, child.a_add),
    }
}

#[allow(clippy::arithmetic_side_effects)]
fn decoded_bitmap_to_pixmap(bmp: &DecodedBitmap, cx: ColorTransform) -> Option<Pixmap> {
    let mut pm = Pixmap::new(bmp.width, bmp.height)?;
    let dst = pm.data_mut();
    for (src_px, dst_px) in bmp.rgba.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
        let c = apply_cxform(
            Rgba {
                r: src_px[0],
                g: src_px[1],
                b: src_px[2],
                a: src_px[3],
            },
            cx,
        );
        // Premultiply for tiny-skia
        let a16 = u16::from(c.a);
        dst_px[0] = ((u16::from(c.r) * a16 + 127) / 255) as u8;
        dst_px[1] = ((u16::from(c.g) * a16 + 127) / 255) as u8;
        dst_px[2] = ((u16::from(c.b) * a16 + 127) / 255) as u8;
        dst_px[3] = c.a;
    }
    Some(pm)
}

#[allow(clippy::arithmetic_side_effects, clippy::too_many_lines)]
fn builtin_char_path(ch: char, x: f32, y: f32, w: f32, h: f32) -> Option<Path> {
    let mut pb = PathBuilder::new();
    let mx = x + w * 0.5;
    let my = y + h * 0.5;
    let rx = x + w;
    let by = y + h;

    match ch.to_ascii_uppercase() {
        '0' | 'O' | 'D' => {
            pb.move_to(x, y);
            pb.line_to(rx, y);
            pb.line_to(rx, by);
            pb.line_to(x, by);
            pb.close();
        }
        '1' | 'I' => {
            pb.move_to(mx, y);
            pb.line_to(mx, by);
        }
        '2' | 'Z' => {
            pb.move_to(x, y);
            pb.line_to(rx, y);
            pb.line_to(rx, my);
            pb.line_to(x, my);
            pb.line_to(x, by);
            pb.line_to(rx, by);
        }
        '3' => {
            pb.move_to(x, y);
            pb.line_to(rx, y);
            pb.line_to(rx, by);
            pb.line_to(x, by);
            pb.move_to(x, my);
            pb.line_to(rx, my);
        }
        '4' => {
            pb.move_to(x, y);
            pb.line_to(x, my);
            pb.line_to(rx, my);
            pb.move_to(rx, y);
            pb.line_to(rx, by);
        }
        '5' | 'S' => {
            pb.move_to(rx, y);
            pb.line_to(x, y);
            pb.line_to(x, my);
            pb.line_to(rx, my);
            pb.line_to(rx, by);
            pb.line_to(x, by);
        }
        '6' | 'G' => {
            pb.move_to(rx, y);
            pb.line_to(x, y);
            pb.line_to(x, by);
            pb.line_to(rx, by);
            pb.line_to(rx, my);
            pb.line_to(x, my);
        }
        '7' => {
            pb.move_to(x, y);
            pb.line_to(rx, y);
            pb.line_to(mx, by);
        }
        '8' | 'B' => {
            pb.move_to(x, y);
            pb.line_to(rx, y);
            pb.line_to(rx, by);
            pb.line_to(x, by);
            pb.close();
            pb.move_to(x, my);
            pb.line_to(rx, my);
        }
        '9' => {
            pb.move_to(rx, my);
            pb.line_to(x, my);
            pb.line_to(x, y);
            pb.line_to(rx, y);
            pb.line_to(rx, by);
        }
        'A' => {
            pb.move_to(x, by);
            pb.line_to(mx, y);
            pb.line_to(rx, by);
            pb.move_to(x + w * 0.2, my);
            pb.line_to(x + w * 0.8, my);
        }
        'C' => {
            pb.move_to(rx, y);
            pb.line_to(x, y);
            pb.line_to(x, by);
            pb.line_to(rx, by);
        }
        'E' => {
            pb.move_to(rx, y);
            pb.line_to(x, y);
            pb.line_to(x, by);
            pb.line_to(rx, by);
            pb.move_to(x, my);
            pb.line_to(rx, my);
        }
        'F' => {
            pb.move_to(rx, y);
            pb.line_to(x, y);
            pb.line_to(x, by);
            pb.move_to(x, my);
            pb.line_to(rx, my);
        }
        'H' => {
            pb.move_to(x, y);
            pb.line_to(x, by);
            pb.move_to(rx, y);
            pb.line_to(rx, by);
            pb.move_to(x, my);
            pb.line_to(rx, my);
        }
        'L' => {
            pb.move_to(x, y);
            pb.line_to(x, by);
            pb.line_to(rx, by);
        }
        'M' | 'N' => {
            pb.move_to(x, by);
            pb.line_to(x, y);
            pb.line_to(mx, my);
            pb.line_to(rx, y);
            pb.line_to(rx, by);
        }
        'P' | 'R' => {
            pb.move_to(x, by);
            pb.line_to(x, y);
            pb.line_to(rx, y);
            pb.line_to(rx, my);
            pb.line_to(x, my);
            if ch.eq_ignore_ascii_case(&'R') {
                pb.line_to(rx, by);
            }
        }
        'T' => {
            pb.move_to(x, y);
            pb.line_to(rx, y);
            pb.move_to(mx, y);
            pb.line_to(mx, by);
        }
        'U' | 'V' | 'W' => {
            pb.move_to(x, y);
            pb.line_to(x, by);
            pb.line_to(rx, by);
            pb.line_to(rx, y);
        }
        'X' | 'K' | 'Y' => {
            pb.move_to(x, y);
            pb.line_to(rx, by);
            pb.move_to(rx, y);
            pb.line_to(x, by);
        }
        '-' => {
            pb.move_to(x, my);
            pb.line_to(rx, my);
        }
        '+' => {
            pb.move_to(x, my);
            pb.line_to(rx, my);
            pb.move_to(mx, y);
            pb.line_to(mx, by);
        }
        ' ' => return None,
        _ => {
            pb.move_to(x, y);
            pb.line_to(rx, y);
            pb.line_to(rx, by);
            pb.line_to(x, by);
            pb.close();
        }
    }
    pb.finish()
}
