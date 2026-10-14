//! CPU 2D software renderer (`tiny-skia`) for `swfr` with a 64-entry flattened-shape LRU cache,
//! dirty-rectangle tracking, clipping masks, gradients, bitmaps, embedded & device text,
//! and `XRGB8888` / `RGB565` output conversion (TSD 1.6, 2.7, 2.12 M2 & M3).
#![allow(
    clippy::arithmetic_side_effects,
    clippy::collapsible_if,
    clippy::collapsible_match
)]

use std::collections::BTreeMap;
use std::rc::Rc;
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
pub const SHAPE_LRU_CAPACITY: usize = 4096;

/// Pre-built `tiny_skia::Path` contours for a flattened shape.
#[derive(Debug, Clone)]
pub struct CompiledShape {
    fills: Vec<(FillStyle, Path)>,
    strokes: Vec<(LineStyle, Path)>,
}

impl CompiledShape {
    fn from_flattened(flat: &FlattenedShape) -> Self {
        let mut fills = Vec::with_capacity(flat.fills.len());
        for f in &flat.fills {
            if let Some(p) = build_skia_path(&f.cmds) {
                fills.push((f.style.clone(), p));
            }
        }
        let mut strokes = Vec::with_capacity(flat.strokes.len());
        for s in &flat.strokes {
            if let Some(p) = build_skia_path(&s.cmds) {
                strokes.push((s.style, p));
            }
        }
        Self { fills, strokes }
    }
}

#[derive(Debug, Clone)]
struct LruEntry {
    key: (u16, u16),
    last_used: u64,
    shape: FlattenedShape,
    compiled: Rc<CompiledShape>,
}

/// Bounded LRU cache mapping `(character_id, ratio)` to [`FlattenedShape`] and pre-built `tiny_skia` paths,
/// plus cached font glyph paths and identity-transform bitmap pixmaps.
#[derive(Debug, Clone, Default)]
pub struct ShapeCache {
    entries: Vec<LruEntry>,
    glyph_paths: BTreeMap<(u16, u32), Rc<Vec<Path>>>,
    bitmap_pixmaps: BTreeMap<u16, Pixmap>,
    clock: u64,
}

impl ShapeCache {
    /// Creates an empty [`ShapeCache`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
            glyph_paths: BTreeMap::new(),
            bitmap_pixmaps: BTreeMap::new(),
            clock: 0,
        }
    }

    /// Returns a cached [`FlattenedShape`] or computes and inserts it, evicting the least-recently-used
    /// entry when at [`SHAPE_LRU_CAPACITY`].
    #[allow(clippy::indexing_slicing)]
    pub fn get_or_insert_with<F>(&mut self, key: (u16, u16), compute: F) -> &FlattenedShape
    where
        F: FnOnce() -> FlattenedShape,
    {
        let idx = self.ensure_entry(key, compute);
        &self.entries[idx].shape
    }

    #[allow(clippy::indexing_slicing)]
    fn get_or_insert_compiled<F>(&mut self, key: (u16, u16), compute: F) -> Rc<CompiledShape>
    where
        F: FnOnce() -> FlattenedShape,
    {
        let idx = self.ensure_entry(key, compute);
        Rc::clone(&self.entries[idx].compiled)
    }

    #[allow(clippy::indexing_slicing)]
    fn ensure_entry<F>(&mut self, key: (u16, u16), compute: F) -> usize
    where
        F: FnOnce() -> FlattenedShape,
    {
        self.clock = self.clock.wrapping_add(1);
        let now = self.clock;
        if let Some(pos) = self.entries.iter().position(|e| e.key == key) {
            if let Some(entry) = self.entries.get_mut(pos) {
                entry.last_used = now;
            }
            return pos;
        }
        let shape = compute();
        let compiled = Rc::new(CompiledShape::from_flattened(&shape));
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
            compiled,
        });
        self.entries.len().saturating_sub(1)
    }

    fn get_or_insert_glyph(
        &mut self,
        font_id: u16,
        glyph_idx: u32,
        records: &[ShapeRecord],
    ) -> Rc<Vec<Path>> {
        if let Some(existing) = self.glyph_paths.get(&(font_id, glyph_idx)) {
            return Rc::clone(existing);
        }
        let shape = ShapeDef {
            id: 0,
            version: 3,
            bounds: Rect::default(),
            fill_styles: vec![FillStyle::Solid(Rgba::rgb(255, 255, 255))],
            line_styles: Vec::new(),
            records: records.to_vec(),
        };
        let flat = flatten_shape(&shape);
        let mut paths = Vec::with_capacity(flat.fills.len());
        for fill in &flat.fills {
            if let Some(p) = build_skia_path(&fill.cmds) {
                paths.push(p);
            }
        }
        let rc = Rc::new(paths);
        if self.glyph_paths.len() < SHAPE_LRU_CAPACITY {
            self.glyph_paths
                .insert((font_id, glyph_idx), Rc::clone(&rc));
        }
        rc
    }

    fn get_or_insert_bitmap(&mut self, bmp: &DecodedBitmap) -> Option<&Pixmap> {
        if !self.bitmap_pixmaps.contains_key(&bmp.id) {
            let pm = decoded_bitmap_to_pixmap(bmp, ColorTransform::IDENTITY)?;
            if self.bitmap_pixmaps.len() < 1024 {
                self.bitmap_pixmaps.insert(bmp.id, pm);
            } else {
                return None;
            }
        }
        self.bitmap_pixmaps.get(&bmp.id)
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
    /// Creates a new [`Renderer`] for stage dimensions `(width_px, height_px)` and resolution scale (`1..=4`).
    #[must_use]
    #[allow(clippy::expect_used)]
    pub fn new(width_px: u32, height_px: u32, scale: u32) -> Self {
        let s = scale.clamp(1, 4);
        let w = width_px.saturating_mul(s).clamp(1, 8192);
        let h = height_px.saturating_mul(s).clamp(1, 8192);
        let pixmap = Pixmap::new(w, h).unwrap_or_else(|| Pixmap::new(1, 1).expect("1x1 pixmap"));
        let count = usize::try_from(pixmap.width().saturating_mul(pixmap.height())).unwrap_or(1);
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

    /// Current resolution scale (`1..=4`).
    #[must_use]
    pub const fn scale(&self) -> u32 {
        self.scale
    }

    /// Resizes the renderer if `(stage_width_px * scale, stage_height_px * scale)` changed.
    pub fn ensure_size(&mut self, stage_width_px: u32, stage_height_px: u32, scale: u32) -> bool {
        let s = scale.clamp(1, 4);
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

    #[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
    fn sync_output_buffers(&mut self) {
        let data = self.pixmap.data();
        for (i, px) in data.chunks_exact(4).enumerate() {
            // tiny-skia stores premultiplied RGBA; since background is opaque (A=255), R/G/B are final
            let r = u32::from(px[0]);
            let g = u32::from(px[1]);
            let b = u32::from(px[2]);
            let xrgb = (r << 16) | (g << 8) | b;
            if let Some(slot) = self.xrgb_buf.get_mut(i) {
                *slot = xrgb;
            }
            if let Some(slot565) = self.rgb565_buf.get_mut(i) {
                *slot565 = xrgb_to_rgb565(xrgb);
            }
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
                if let Some(mut mask_pm) = Pixmap::new(pixmap.width(), pixmap.height()) {
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
                }
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
            let compiled =
                cache.get_or_insert_compiled((obj.character_id, 0), || flatten_shape(shape_def));
            render_compiled_shape(pixmap, cache, &compiled, dict, xform, cx, mask);
        }
        Character::MorphShape(morph_def) => {
            let compiled = cache.get_or_insert_compiled((obj.character_id, obj.ratio), || {
                flatten_morph_shape(morph_def, obj.ratio)
            });
            render_compiled_shape(pixmap, cache, &compiled, dict, xform, cx, mask);
        }
        Character::Bitmap(bmp) => {
            render_bitmap_quad(pixmap, cache, bmp, xform, cx, mask);
        }
        Character::Text(txt) => {
            render_static_text(pixmap, cache, txt, dict, xform, cx, mask);
        }
        Character::EditText(edit) => {
            let text_str = obj
                .dynamic_text
                .as_deref()
                .or(edit.initial_text.as_deref())
                .unwrap_or("");
            render_edit_text(pixmap, cache, edit, text_str, dict, xform, cx, mask);
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

fn render_compiled_shape(
    pixmap: &mut Pixmap,
    cache: &mut ShapeCache,
    compiled: &CompiledShape,
    dict: &BTreeMap<u16, Character>,
    xform: Transform,
    cx: ColorTransform,
    mask: Option<&Mask>,
) {
    for (style, path) in &compiled.fills {
        match style {
            FillStyle::Solid(rgba) => {
                let mut paint = Paint {
                    anti_alias: true,
                    ..Paint::default()
                };
                let c = apply_cxform(*rgba, cx);
                paint.set_color_rgba8(c.r, c.g, c.b, c.a);
                pixmap.fill_path(path, &paint, FillRule::EvenOdd, xform, mask);
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
                    pixmap.fill_path(path, &paint, FillRule::EvenOdd, xform, mask);
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
                    0.0,
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
                    pixmap.fill_path(path, &paint, FillRule::EvenOdd, xform, mask);
                }
            }
            FillStyle::Bitmap {
                bitmap_id,
                matrix,
                repeating,
                smoothed,
            } => {
                if let Some(Character::Bitmap(bmp)) = dict.get(bitmap_id) {
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
                    if cx == ColorTransform::IDENTITY {
                        if let Some(bmp_pm) = cache.get_or_insert_bitmap(bmp) {
                            let shader =
                                Pattern::new(bmp_pm.as_ref(), spread, quality, 1.0, pat_xform);
                            let paint = Paint {
                                shader,
                                anti_alias: true,
                                ..Paint::default()
                            };
                            pixmap.fill_path(path, &paint, FillRule::EvenOdd, xform, mask);
                        }
                    } else if let Some(bmp_pm) = decoded_bitmap_to_pixmap(bmp, cx) {
                        let shader = Pattern::new(bmp_pm.as_ref(), spread, quality, 1.0, pat_xform);
                        let paint = Paint {
                            shader,
                            anti_alias: true,
                            ..Paint::default()
                        };
                        pixmap.fill_path(path, &paint, FillRule::EvenOdd, xform, mask);
                    }
                }
            }
        }
    }

    for (style, path) in &compiled.strokes {
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        let c = apply_cxform(style.color, cx);
        paint.set_color_rgba8(c.r, c.g, c.b, c.a);
        let sk_stroke = Stroke {
            width: f32::from(style.width_twips).max(20.0),
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        pixmap.stroke_path(path, &paint, &sk_stroke, xform, mask);
    }
}

fn render_bitmap_quad(
    pixmap: &mut Pixmap,
    cache: &mut ShapeCache,
    bmp: &DecodedBitmap,
    xform: Transform,
    cx: ColorTransform,
    mask: Option<&Mask>,
) {
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
    if cx == ColorTransform::IDENTITY {
        if let Some(bmp_pm) = cache.get_or_insert_bitmap(bmp) {
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
    } else if let Some(bmp_pm) = decoded_bitmap_to_pixmap(bmp, cx) {
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
}

#[allow(clippy::arithmetic_side_effects, clippy::too_many_arguments)]
fn render_static_text(
    pixmap: &mut Pixmap,
    cache: &mut ShapeCache,
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
                        cache,
                        font.id,
                        entry.glyph_index,
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
    cache: &mut ShapeCache,
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
                                cache,
                                font.id,
                                u32::try_from(g_idx).unwrap_or(0),
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
    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
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

#[allow(clippy::too_many_arguments)]
fn render_glyph_records(
    pixmap: &mut Pixmap,
    cache: &mut ShapeCache,
    font_id: u16,
    glyph_idx: u32,
    records: &[ShapeRecord],
    color: Rgba,
    cx: ColorTransform,
    xform: Transform,
    mask: Option<&Mask>,
) {
    let paths = cache.get_or_insert_glyph(font_id, glyph_idx, records);
    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    let c = apply_cxform(color, cx);
    paint.set_color_rgba8(c.r, c.g, c.b, c.a);
    for path in paths.iter() {
        pixmap.fill_path(path, &paint, FillRule::EvenOdd, xform, mask);
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
            clip_actions: Vec::new(),
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
fn combine_cxform(parent: ColorTransform, child: ColorTransform) -> ColorTransform {
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

#[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
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
        '0' | 'O' => {
            pb.move_to(x, y);
            pb.line_to(rx, y);
            pb.line_to(rx, by);
            pb.line_to(x, by);
            pb.close();
        }
        'D' => {
            pb.move_to(x, y);
            pb.line_to(x + w * 0.7, y);
            pb.line_to(rx, y + h * 0.25);
            pb.line_to(rx, y + h * 0.75);
            pb.line_to(x + w * 0.7, by);
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
        'K' => {
            pb.move_to(x, y);
            pb.line_to(x, by);
            pb.move_to(rx, y);
            pb.line_to(x, my);
            pb.line_to(rx, by);
        }
        'L' => {
            pb.move_to(x, y);
            pb.line_to(x, by);
            pb.line_to(rx, by);
        }
        'M' => {
            pb.move_to(x, by);
            pb.line_to(x, y);
            pb.line_to(mx, my);
            pb.line_to(rx, y);
            pb.line_to(rx, by);
        }
        'N' => {
            pb.move_to(x, by);
            pb.line_to(x, y);
            pb.line_to(rx, by);
            pb.line_to(rx, y);
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
        'U' => {
            pb.move_to(x, y);
            pb.line_to(x, by);
            pb.line_to(rx, by);
            pb.line_to(rx, y);
        }
        'V' => {
            pb.move_to(x, y);
            pb.line_to(mx, by);
            pb.line_to(rx, y);
        }
        'W' => {
            pb.move_to(x, y);
            pb.line_to(x + w * 0.25, by);
            pb.line_to(mx, my);
            pb.line_to(x + w * 0.75, by);
            pb.line_to(rx, y);
        }
        'X' => {
            pb.move_to(x, y);
            pb.line_to(rx, by);
            pb.move_to(rx, y);
            pb.line_to(x, by);
        }
        'Y' => {
            pb.move_to(x, y);
            pb.line_to(mx, my);
            pb.line_to(rx, y);
            pb.move_to(mx, my);
            pb.line_to(mx, by);
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
