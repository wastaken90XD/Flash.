//! Display list, character dictionary, shape flattening, timeline engine, and button hit-testing
//! for `swfr` (TSD 1.6, 2.12 M2 & M3).

use std::collections::BTreeMap;
use std::io::Cursor;
use swfr_reader::{ColorTransform, Matrix, Rect};
use swfr_swf::{
    ButtonDef, EditTextDef, FillStyle, FontDef, GradientStop, LineStyle, LosslessBitmapDef,
    MorphShapeDef, Movie, PlaceObject2Tag, PlaceObjectTag, Rgba, ShapeDef, ShapeRecord, SoundDef,
    SoundInfo, SoundStreamHeadDef, Tag, TextDef,
};

/// Decoded RGBA8 bitmap stored in the character dictionary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedBitmap {
    /// Character ID.
    pub id: u16,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Unpremultiplied RGBA8 pixels (`width * height * 4` bytes).
    pub rgba: Vec<u8>,
}

/// Character stored in the movie's character dictionary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Character {
    /// Vector shape (`DefineShape`..`DefineShape4`).
    Shape(ShapeDef),
    /// Morphing vector shape (`DefineMorphShape`, `DefineMorphShape2`).
    MorphShape(MorphShapeDef),
    /// Decoded raster bitmap (`DefineBits*`, `DefineBitsLossless*`).
    Bitmap(DecodedBitmap),
    /// Embedded vector font (`DefineFont`..`DefineFont3`).
    Font(FontDef),
    /// Static glyph text block (`DefineText`, `DefineText2`).
    Text(TextDef),
    /// Dynamic / input text field (`DefineEditText`).
    EditText(EditTextDef),
    /// Interactive button (`DefineButton`, `DefineButton2`).
    Button(ButtonDef),
    /// Nested movie clip (`DefineSprite`).
    Sprite {
        /// Character ID.
        id: u16,
        /// Declared frame count.
        frame_count: u16,
        /// Nested timeline tags.
        tags: Vec<Tag>,
    },
    /// Defined event sound (`DefineSound`).
    Sound(SoundDef),
}

/// Interactive button visual state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonState {
    /// Mouse pointer is outside the button hit area.
    #[default]
    Up,
    /// Mouse pointer is inside the button hit area with button released.
    Over,
    /// Mouse pointer is inside the button hit area with button pressed.
    Down,
}

/// Placed display object at a specific depth in a timeline's display list.
#[derive(Debug, Clone)]
pub struct DisplayObject {
    /// Character ID in the dictionary.
    pub character_id: u16,
    /// Placement depth.
    pub depth: u16,
    /// Local-to-parent transform matrix.
    pub matrix: Matrix,
    /// Color transform (`CXFORMWITHALPHA`).
    pub color_transform: ColorTransform,
    /// Morph ratio (`0..=65535`).
    pub ratio: u16,
    /// Optional instance name (for AS1/AS2 movie clips and text fields).
    pub name: Option<String>,
    /// Optional clipping depth (`Some(max_depth)` masks depths `depth+1..=max_depth`).
    pub clip_depth: Option<u16>,
    /// Current interactive button state (when referencing `Character::Button`).
    pub button_state: ButtonState,
    /// Whether the pointer was pressed inside this button on the previous input poll.
    pub button_pressed_inside: bool,
    /// Current dynamic text string (when referencing `Character::EditText`).
    pub dynamic_text: Option<String>,
    /// Independent timeline when referencing `Character::Sprite`.
    pub sprite_timeline: Option<Timeline>,
}

/// 2D path command in twips (`1/20` pixel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathCmd {
    /// Move current point to `(x, y)` in twips.
    MoveTo(i32, i32),
    /// Straight line to `(x, y)` in twips.
    LineTo(i32, i32),
    /// Quadratic Bézier curve with control `(cx, cy)` and anchor `(ax, ay)` in twips.
    QuadTo {
        /// Control X in twips.
        cx: i32,
        /// Control Y in twips.
        cy: i32,
        /// Anchor X in twips.
        ax: i32,
        /// Anchor Y in twips.
        ay: i32,
    },
    /// Close current contour.
    Close,
}

/// Flattened fill path associated with a single [`FillStyle`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FillPath {
    /// Fill style for this closed contour set.
    pub style: FillStyle,
    /// Path commands in twips.
    pub cmds: Vec<PathCmd>,
}

/// Flattened stroke path associated with a single [`LineStyle`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrokePath {
    /// Line style for this stroke path.
    pub style: LineStyle,
    /// Path commands in twips.
    pub cmds: Vec<PathCmd>,
}

/// Flattened shape ready for rasterization or vector hit-testing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlattenedShape {
    /// Bounding box in twips.
    pub bounds: Rect,
    /// Closed fill paths in paint order.
    pub fills: Vec<FillPath>,
    /// Stroke paths in paint order.
    pub strokes: Vec<StrokePath>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DirectedEdge {
    x0: i32,
    y0: i32,
    cx: Option<(i32, i32)>,
    x1: i32,
    y1: i32,
}

impl DirectedEdge {
    const fn reversed(self) -> Self {
        Self {
            x0: self.x1,
            y0: self.y1,
            cx: self.cx,
            x1: self.x0,
            y1: self.y0,
        }
    }
}

/// Flattens a [`ShapeDef`] into closed fill paths (resolving SWF dual-fill `fill_style_0` /
/// `fill_style_1` edge lists per sub-shape group) and stroke paths.
#[must_use]
pub fn flatten_shape(shape: &ShapeDef) -> FlattenedShape {
    flatten_records(
        shape.bounds,
        &shape.fill_styles,
        &shape.line_styles,
        &shape.records,
    )
}

/// Interpolates a [`MorphShapeDef`] at `ratio` (`0..=65535`) and flattens the resulting shape.
#[must_use]
pub fn flatten_morph_shape(morph: &MorphShapeDef, ratio: u16) -> FlattenedShape {
    let t = i64::from(ratio);
    let lerp_i32 = |a: i32, b: i32| -> i32 {
        let diff = i64::from(b).saturating_sub(i64::from(a));
        let step = diff.saturating_mul(t) / 65535;
        i32::try_from(i64::from(a).saturating_add(step)).unwrap_or(b)
    };
    let lerp_u8 = |a: u8, b: u8| -> u8 {
        u8::try_from(lerp_i32(i32::from(a), i32::from(b)).clamp(0, 255)).unwrap_or(b)
    };
    let lerp_rgba = |a: Rgba, b: Rgba| -> Rgba {
        Rgba {
            r: lerp_u8(a.r, b.r),
            g: lerp_u8(a.g, b.g),
            b: lerp_u8(a.b, b.b),
            a: lerp_u8(a.a, b.a),
        }
    };
    let lerp_matrix = |a: Matrix, b: Matrix| -> Matrix {
        Matrix {
            scale_x_fp16: lerp_i32(a.scale_x_fp16, b.scale_x_fp16),
            scale_y_fp16: lerp_i32(a.scale_y_fp16, b.scale_y_fp16),
            rotate_skew0_fp16: lerp_i32(a.rotate_skew0_fp16, b.rotate_skew0_fp16),
            rotate_skew1_fp16: lerp_i32(a.rotate_skew1_fp16, b.rotate_skew1_fp16),
            translate_x_twips: lerp_i32(a.translate_x_twips, b.translate_x_twips),
            translate_y_twips: lerp_i32(a.translate_y_twips, b.translate_y_twips),
        }
    };

    let bounds = Rect {
        x_min: lerp_i32(morph.start_bounds.x_min, morph.end_bounds.x_min),
        x_max: lerp_i32(morph.start_bounds.x_max, morph.end_bounds.x_max),
        y_min: lerp_i32(morph.start_bounds.y_min, morph.end_bounds.y_min),
        y_max: lerp_i32(morph.start_bounds.y_max, morph.end_bounds.y_max),
    };

    let mut fill_styles = Vec::with_capacity(morph.start_fill_styles.len());
    for (s, e) in morph
        .start_fill_styles
        .iter()
        .zip(morph.end_fill_styles.iter())
    {
        let interp = match (s, e) {
            (FillStyle::Solid(sc), FillStyle::Solid(ec)) => FillStyle::Solid(lerp_rgba(*sc, *ec)),
            (
                FillStyle::LinearGradient {
                    matrix: sm,
                    stops: ss,
                },
                FillStyle::LinearGradient {
                    matrix: em,
                    stops: es,
                },
            ) => FillStyle::LinearGradient {
                matrix: lerp_matrix(*sm, *em),
                stops: ss
                    .iter()
                    .zip(es.iter())
                    .map(|(st, et)| GradientStop {
                        ratio: lerp_u8(st.ratio, et.ratio),
                        color: lerp_rgba(st.color, et.color),
                    })
                    .collect(),
            },
            (
                FillStyle::RadialGradient {
                    matrix: sm,
                    stops: ss,
                    focal_point_q8: sf,
                },
                FillStyle::RadialGradient {
                    matrix: em,
                    stops: es,
                    focal_point_q8: ef,
                },
            ) => FillStyle::RadialGradient {
                matrix: lerp_matrix(*sm, *em),
                stops: ss
                    .iter()
                    .zip(es.iter())
                    .map(|(st, et)| GradientStop {
                        ratio: lerp_u8(st.ratio, et.ratio),
                        color: lerp_rgba(st.color, et.color),
                    })
                    .collect(),
                focal_point_q8: i16::try_from(lerp_i32(i32::from(*sf), i32::from(*ef)))
                    .unwrap_or(*ef),
            },
            _ => s.clone(),
        };
        fill_styles.push(interp);
    }

    let mut line_styles = Vec::with_capacity(morph.start_line_styles.len());
    for (s, e) in morph
        .start_line_styles
        .iter()
        .zip(morph.end_line_styles.iter())
    {
        line_styles.push(LineStyle {
            width_twips: u16::try_from(
                lerp_i32(i32::from(s.width_twips), i32::from(e.width_twips)).max(0),
            )
            .unwrap_or(e.width_twips),
            color: lerp_rgba(s.color, e.color),
        });
    }

    let mut records = Vec::with_capacity(morph.start_records.len());
    let mut end_iter = morph.end_records.iter();
    for srec in &morph.start_records {
        match srec {
            ShapeRecord::StyleChange {
                move_to,
                fill_style_0,
                fill_style_1,
                line_style,
                new_fill_styles,
                new_line_styles,
            } => {
                let mut emove = *move_to;
                for erec in end_iter.by_ref() {
                    if let ShapeRecord::StyleChange {
                        move_to: Some(m), ..
                    } = erec
                    {
                        emove = Some(*m);
                        break;
                    }
                }
                let interp_move = match (*move_to, emove) {
                    (Some((sx, sy)), Some((ex, ey))) => Some((lerp_i32(sx, ex), lerp_i32(sy, ey))),
                    (m, _) => m,
                };
                records.push(ShapeRecord::StyleChange {
                    move_to: interp_move,
                    fill_style_0: *fill_style_0,
                    fill_style_1: *fill_style_1,
                    line_style: *line_style,
                    new_fill_styles: new_fill_styles.clone(),
                    new_line_styles: new_line_styles.clone(),
                });
            }
            ShapeRecord::StraightEdge { dx_twips, dy_twips } => {
                let mut end_edge = (*dx_twips, *dy_twips);
                for erec in end_iter.by_ref() {
                    match erec {
                        ShapeRecord::StraightEdge {
                            dx_twips: edx,
                            dy_twips: edy,
                        } => {
                            end_edge = (*edx, *edy);
                            break;
                        }
                        ShapeRecord::CurvedEdge {
                            control_dx_twips,
                            control_dy_twips,
                            anchor_dx_twips,
                            anchor_dy_twips,
                        } => {
                            end_edge = (
                                control_dx_twips.saturating_add(*anchor_dx_twips),
                                control_dy_twips.saturating_add(*anchor_dy_twips),
                            );
                            break;
                        }
                        ShapeRecord::StyleChange { .. } => {}
                    }
                }
                records.push(ShapeRecord::StraightEdge {
                    dx_twips: lerp_i32(*dx_twips, end_edge.0),
                    dy_twips: lerp_i32(*dy_twips, end_edge.1),
                });
            }
            ShapeRecord::CurvedEdge {
                control_dx_twips,
                control_dy_twips,
                anchor_dx_twips,
                anchor_dy_twips,
            } => {
                let mut end_curve = (
                    *control_dx_twips,
                    *control_dy_twips,
                    *anchor_dx_twips,
                    *anchor_dy_twips,
                );
                for erec in end_iter.by_ref() {
                    match erec {
                        ShapeRecord::CurvedEdge {
                            control_dx_twips: ecx,
                            control_dy_twips: ecy,
                            anchor_dx_twips: eax,
                            anchor_dy_twips: eay,
                        } => {
                            end_curve = (*ecx, *ecy, *eax, *eay);
                            break;
                        }
                        ShapeRecord::StraightEdge {
                            dx_twips: edx,
                            dy_twips: edy,
                        } => {
                            let hdx = *edx / 2;
                            let hdy = *edy / 2;
                            end_curve = (hdx, hdy, edx.saturating_sub(hdx), edy.saturating_sub(hdy));
                            break;
                        }
                        ShapeRecord::StyleChange { .. } => {}
                    }
                }
                records.push(ShapeRecord::CurvedEdge {
                    control_dx_twips: lerp_i32(*control_dx_twips, end_curve.0),
                    control_dy_twips: lerp_i32(*control_dy_twips, end_curve.1),
                    anchor_dx_twips: lerp_i32(*anchor_dx_twips, end_curve.2),
                    anchor_dy_twips: lerp_i32(*anchor_dy_twips, end_curve.3),
                });
            }
        }
    }

    flatten_records(bounds, &fill_styles, &line_styles, &records)
}

#[allow(clippy::too_many_lines)]
fn flatten_records(
    bounds: Rect,
    initial_fills: &[FillStyle],
    initial_lines: &[LineStyle],
    records: &[ShapeRecord],
) -> FlattenedShape {
    let mut cur_fills: Vec<FillStyle> = initial_fills.to_vec();
    let mut cur_lines: Vec<LineStyle> = initial_lines.to_vec();

    let mut fill_edges: BTreeMap<u32, Vec<DirectedEdge>> = BTreeMap::new();
    let mut line_edges: BTreeMap<u32, Vec<DirectedEdge>> = BTreeMap::new();

    let mut out_fills: Vec<FillPath> = Vec::new();
    let mut out_strokes: Vec<StrokePath> = Vec::new();

    let mut cur_x: i32 = 0;
    let mut cur_y: i32 = 0;
    let mut fill0: u32 = 0;
    let mut fill1: u32 = 0;
    let mut line: u32 = 0;

    let flush_group = |fill_edges: &mut BTreeMap<u32, Vec<DirectedEdge>>,
                       line_edges: &mut BTreeMap<u32, Vec<DirectedEdge>>,
                       cur_fills: &[FillStyle],
                       cur_lines: &[LineStyle],
                       out_fills: &mut Vec<FillPath>,
                       out_strokes: &mut Vec<StrokePath>| {
        for (&style_idx, edges) in fill_edges.iter() {
            if style_idx == 0 {
                continue;
            }
            if let Some(style) = cur_fills.get(usize::try_from(style_idx - 1).unwrap_or(usize::MAX))
            {
                let cmds = stitch_edges(edges, true);
                if !cmds.is_empty() {
                    out_fills.push(FillPath {
                        style: style.clone(),
                        cmds,
                    });
                }
            }
        }
        for (&style_idx, edges) in line_edges.iter() {
            if style_idx == 0 {
                continue;
            }
            if let Some(style) = cur_lines.get(usize::try_from(style_idx - 1).unwrap_or(usize::MAX))
            {
                let cmds = stitch_edges(edges, false);
                if !cmds.is_empty() {
                    out_strokes.push(StrokePath {
                        style: *style,
                        cmds,
                    });
                }
            }
        }
        fill_edges.clear();
        line_edges.clear();
    };

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
                if new_fill_styles.is_some() || new_line_styles.is_some() {
                    flush_group(
                        &mut fill_edges,
                        &mut line_edges,
                        &cur_fills,
                        &cur_lines,
                        &mut out_fills,
                        &mut out_strokes,
                    );
                    if let Some(nfs) = new_fill_styles {
                        cur_fills.clone_from(nfs);
                    }
                    if let Some(nls) = new_line_styles {
                        cur_lines.clone_from(nls);
                    }
                }
                if let Some((mx, my)) = *move_to {
                    cur_x = mx;
                    cur_y = my;
                }
                if let Some(f0) = *fill_style_0 {
                    fill0 = f0;
                }
                if let Some(f1) = *fill_style_1 {
                    fill1 = f1;
                }
                if let Some(ls) = *line_style {
                    line = ls;
                }
            }
            ShapeRecord::StraightEdge { dx_twips, dy_twips } => {
                let nx = cur_x.saturating_add(*dx_twips);
                let ny = cur_y.saturating_add(*dy_twips);
                let edge = DirectedEdge {
                    x0: cur_x,
                    y0: cur_y,
                    cx: None,
                    x1: nx,
                    y1: ny,
                };
                if fill1 != 0 {
                    fill_edges.entry(fill1).or_default().push(edge);
                }
                if fill0 != 0 {
                    fill_edges.entry(fill0).or_default().push(edge.reversed());
                }
                if line != 0 {
                    line_edges.entry(line).or_default().push(edge);
                }
                cur_x = nx;
                cur_y = ny;
            }
            ShapeRecord::CurvedEdge {
                control_dx_twips,
                control_dy_twips,
                anchor_dx_twips,
                anchor_dy_twips,
            } => {
                let cx = cur_x.saturating_add(*control_dx_twips);
                let cy = cur_y.saturating_add(*control_dy_twips);
                let nx = cx.saturating_add(*anchor_dx_twips);
                let ny = cy.saturating_add(*anchor_dy_twips);
                let edge = DirectedEdge {
                    x0: cur_x,
                    y0: cur_y,
                    cx: Some((cx, cy)),
                    x1: nx,
                    y1: ny,
                };
                if fill1 != 0 {
                    fill_edges.entry(fill1).or_default().push(edge);
                }
                if fill0 != 0 {
                    fill_edges.entry(fill0).or_default().push(edge.reversed());
                }
                if line != 0 {
                    line_edges.entry(line).or_default().push(edge);
                }
                cur_x = nx;
                cur_y = ny;
            }
        }
    }

    flush_group(
        &mut fill_edges,
        &mut line_edges,
        &cur_fills,
        &cur_lines,
        &mut out_fills,
        &mut out_strokes,
    );

    FlattenedShape {
        bounds,
        fills: out_fills,
        strokes: out_strokes,
    }
}

fn stitch_edges(edges: &[DirectedEdge], close_contours: bool) -> Vec<PathCmd> {
    let mut used = vec![false; edges.len()];
    let mut cmds = Vec::new();

    for start_idx in 0..edges.len() {
        if used.get(start_idx).copied().unwrap_or(true) {
            continue;
        }
        if let Some( used_flag) = used.get_mut(start_idx) {
            *used_flag = true;
        }
        let Some(&first) = edges.get(start_idx) else {
            continue;
        };
        let start_pt = (first.x0, first.y0);
        let mut cur_pt = (first.x1, first.y1);
        cmds.push(PathCmd::MoveTo(first.x0, first.y0));
        push_edge_cmd(&mut cmds, first);

        loop {
            if close_contours && cur_pt == start_pt {
                break;
            }
            let mut next_match = None;
            for (cand_idx, cand) in edges.iter().enumerate() {
                if !used.get(cand_idx).copied().unwrap_or(true) && (cand.x0, cand.y0) == cur_pt {
                    next_match = Some((cand_idx, *cand));
                    break;
                }
            }
            if let Some((cand_idx, cand)) = next_match {
                if let Some(u) = used.get_mut(cand_idx) {
                    *u = true;
                }
                push_edge_cmd(&mut cmds, cand);
                cur_pt = (cand.x1, cand.y1);
            } else {
                break;
            }
        }
        if close_contours {
            if cur_pt != start_pt {
                cmds.push(PathCmd::LineTo(start_pt.0, start_pt.1));
            }
            cmds.push(PathCmd::Close);
        }
    }

    cmds
}

fn push_edge_cmd(cmds: &mut Vec<PathCmd>, edge: DirectedEdge) {
    if let Some((cx, cy)) = edge.cx {
        cmds.push(PathCmd::QuadTo {
            cx,
            cy,
            ax: edge.x1,
            ay: edge.y1,
        });
    } else {
        cmds.push(PathCmd::LineTo(edge.x1, edge.y1));
    }
}

/// Tests whether a point `(px_twips, py_twips)` in shape-local twip coordinates lies inside
/// any closed fill contour of `shape` using the even-odd ray-casting rule (subdividing
/// quadratic Béziers into 8 linear segments for exact curved hit-testing).
#[must_use]
#[allow(clippy::arithmetic_side_effects)]
pub fn point_in_flattened_shape(shape: &FlattenedShape, px_twips: f32, py_twips: f32) -> bool {
    let mut inside = false;
    for fill in &shape.fills {
        let mut cur = (0.0_f32, 0.0_f32);
        let mut start = (0.0_f32, 0.0_f32);
        for &cmd in &fill.cmds {
            match cmd {
                PathCmd::MoveTo(x, y) => {
                    cur = (x as f32, y as f32);
                    start = cur;
                }
                PathCmd::LineTo(x, y) => {
                    let next = (x as f32, y as f32);
                    if ray_crosses_segment(px_twips, py_twips, cur, next) {
                        inside = !inside;
                    }
                    cur = next;
                }
                PathCmd::QuadTo { cx, cy, ax, ay } => {
                    let p0 = cur;
                    let p1 = (cx as f32, cy as f32);
                    let p2 = (ax as f32, ay as f32);
                    let mut prev = p0;
                    for step in 1..=8 {
                        let t = (step as f32) / 8.0;
                        let omt = 1.0 - t;
                        let qx = omt * omt * p0.0 + 2.0 * omt * t * p1.0 + t * t * p2.0;
                        let qy = omt * omt * p0.1 + 2.0 * omt * t * p1.1 + t * t * p2.1;
                        let next = (qx, qy);
                        if ray_crosses_segment(px_twips, py_twips, prev, next) {
                            inside = !inside;
                        }
                        prev = next;
                    }
                    cur = p2;
                }
                PathCmd::Close => {
                    if cur != start && ray_crosses_segment(px_twips, py_twips, cur, start) {
                        inside = !inside;
                    }
                    cur = start;
                }
            }
        }
    }
    inside
}

#[allow(clippy::arithmetic_side_effects)]
fn ray_crosses_segment(px: f32, py: f32, a: (f32, f32), b: (f32, f32)) -> bool {
    if (a.1 > py) == (b.1 > py) {
        return false;
    }
    let x_intersect = a.0 + (py - a.1) * (b.0 - a.0) / (b.1 - a.1);
    px < x_intersect
}

/// Decodes a `DefineBitsLossless` / `DefineBitsLossless2` tag into unpremultiplied RGBA8 pixels.
#[must_use]
#[allow(clippy::arithmetic_side_effects)]
pub fn decode_lossless_bitmap(bmp: &LosslessBitmapDef) -> Option<DecodedBitmap> {
    let width = usize::from(bmp.width);
    let height = usize::from(bmp.height);
    if width == 0 || height == 0 || width.saturating_mul(height) > 16_777_216 {
        return None;
    }
    let raw = miniz_oxide::inflate::decompress_to_vec_zlib(&bmp.zlib_data).ok()?;
    let has_alpha = bmp.version >= 2;
    let mut rgba = Vec::with_capacity(width * height * 4);

    match bmp.format {
        3 => {
            // 8-bit colormapped: palette of (color_table_size + 1) entries, then 4-byte padded rows
            let table_len = usize::from(bmp.color_table_size.unwrap_or(0)) + 1;
            let entry_bytes = if has_alpha { 4 } else { 3 };
            let palette_bytes = table_len * entry_bytes;
            if raw.len() < palette_bytes {
                return None;
            }
            let palette = &raw[..palette_bytes];
            let indices = &raw[palette_bytes..];
            let row_stride = (width + 3) & !3;
            for y in 0..height {
                for x in 0..width {
                    let idx = usize::from(*indices.get(y * row_stride + x).unwrap_or(&0));
                    let p_off = idx * entry_bytes;
                    if p_off + entry_bytes <= palette.len() {
                        let r = palette[p_off];
                        let g = palette[p_off + 1];
                        let b = palette[p_off + 2];
                        let a = if has_alpha { palette[p_off + 3] } else { 255 };
                        // DefineBitsLossless2 stores premultiplied RGB; unpremultiply for consistent RGBA8
                        let (ur, ug, ub) = unpremultiply(r, g, b, a);
                        rgba.extend_from_slice(&[ur, ug, ub, a]);
                    } else {
                        rgba.extend_from_slice(&[0, 0, 0, 0]);
                    }
                }
            }
        }
        4 => {
            // 15-bit RGB (PIX15): 2 bytes per pixel, 4-byte padded rows
            let row_stride = ((width * 2) + 3) & !3;
            for y in 0..height {
                for x in 0..width {
                    let off = y * row_stride + x * 2;
                    let hi = u16::from(*raw.get(off).unwrap_or(&0));
                    let lo = u16::from(*raw.get(off + 1).unwrap_or(&0));
                    let pix = (hi << 8) | lo;
                    let r = (((pix >> 10) & 0x1F) * 255 / 31) as u8;
                    let g = (((pix >> 5) & 0x1F) * 255 / 31) as u8;
                    let b = ((pix & 0x1F) * 255 / 31) as u8;
                    rgba.extend_from_slice(&[r, g, b, 255]);
                }
            }
        }
        5 => {
            // 24-bit RGB (PIX24) or 32-bit ARGB: 4 bytes per pixel [A/0, R, G, B]
            for i in 0..(width * height) {
                let off = i * 4;
                let a = if has_alpha {
                    *raw.get(off).unwrap_or(&255)
                } else {
                    255
                };
                let r = *raw.get(off + 1).unwrap_or(&0);
                let g = *raw.get(off + 2).unwrap_or(&0);
                let b = *raw.get(off + 3).unwrap_or(&0);
                let (ur, ug, ub) = if has_alpha {
                    unpremultiply(r, g, b, a)
                } else {
                    (r, g, b)
                };
                rgba.extend_from_slice(&[ur, ug, ub, a]);
            }
        }
        _ => return None,
    }

    Some(DecodedBitmap {
        id: bmp.id,
        width: u32::from(bmp.width),
        height: u32::from(bmp.height),
        rgba,
    })
}

#[allow(clippy::arithmetic_side_effects)]
const fn unpremultiply(r: u8, g: u8, b: u8, a: u8) -> (u8, u8, u8) {
    if a == 0 || a == 255 {
        return (r, g, b);
    }
    let a16 = a as u16;
    let ur = ((r as u16 * 255) / a16).min(255) as u8;
    let ug = ((g as u16 * 255) / a16).min(255) as u8;
    let ub = ((b as u16 * 255) / a16).min(255) as u8;
    (ur, ug, ub)
}

/// Decodes a JPEG/PNG/GIF payload from `DefineBits*` (cleaning SWF erroneous `FF D9 FF D8` markers)
/// and applies optional zlib-compressed alpha data (`DefineBitsJPEG3` / `DefineBitsJPEG4`).
#[must_use]
#[allow(clippy::arithmetic_side_effects)]
pub fn decode_jpeg_or_png_bitmap(
    id: u16,
    jpeg_tables: Option<&[u8]>,
    image_data: &[u8],
    alpha_data: Option<&[u8]>,
) -> Option<DecodedBitmap> {
    // Check if image_data is actually a PNG (`89 50 4E 47`)
    if image_data.starts_with(b"\x89PNG\r\n\x1a\n") {
        let decoder = png::Decoder::new(Cursor::new(image_data));
        let mut reader = decoder.read_info().ok()?;
        let mut buf = vec![0_u8; reader.output_buffer_size()?];
        let info = reader.next_frame(&mut buf).ok()?;
        let bytes = &buf[..info.buffer_size()];
        let mut rgba = Vec::with_capacity((info.width * info.height * 4) as usize);
        match info.color_type {
            png::ColorType::Rgba => rgba.extend_from_slice(bytes),
            png::ColorType::Rgb => {
                for chunk in bytes.chunks_exact(3) {
                    rgba.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
                }
            }
            _ => return None,
        }
        return Some(DecodedBitmap {
            id,
            width: info.width,
            height: info.height,
            rgba,
        });
    }

    let mut combined = Vec::new();
    if let Some(tables) = jpeg_tables {
        if !tables.is_empty() {
            combined.extend_from_slice(strip_erroneous_jpeg_markers(tables, true));
        }
    }
    combined.extend_from_slice(strip_erroneous_jpeg_markers(
        image_data,
        !combined.is_empty(),
    ));

    let mut decoder = zune_jpeg::JpegDecoder::new(Cursor::new(&combined));
    let pixels = decoder.decode().ok()?;
    let info = decoder.info()?;
    let width = u32::from(info.width);
    let height = u32::from(info.height);
    let count = (width * height) as usize;

    let alpha_plane = alpha_data
        .filter(|a| !a.is_empty())
        .and_then(|a| miniz_oxide::inflate::decompress_to_vec_zlib(a).ok());

    let mut rgba = Vec::with_capacity(count * 4);
    let channels = pixels.len() / count.max(1);
    for i in 0..count {
        let (r, g, b) = if channels >= 3 {
            (
                pixels[i * channels],
                pixels[i * channels + 1],
                pixels[i * channels + 2],
            )
        } else if channels == 1 {
            let l = pixels[i];
            (l, l, l)
        } else {
            (0, 0, 0)
        };
        let a = alpha_plane
            .as_ref()
            .and_then(|ap| ap.get(i).copied())
            .unwrap_or(255);
        rgba.extend_from_slice(&[r, g, b, a]);
    }

    Some(DecodedBitmap {
        id,
        width,
        height,
        rgba,
    })
}

fn strip_erroneous_jpeg_markers(mut data: &[u8], strip_leading_soi: bool) -> &[u8] {
    // Old Flash authoring tools sometimes emit a leading `FF D9 FF D8` before the real stream.
    if data.len() >= 4 && data[0..4] == [0xFF, 0xD9, 0xFF, 0xD8] {
        data = &data[4..];
    }
    if strip_leading_soi && data.len() >= 2 && data[0..2] == [0xFF, 0xD8] {
        data = &data[2..];
    }
    if strip_leading_soi && data.len() >= 2 && data[data.len() - 2..] == [0xFF, 0xD9] {
        data = &data[..data.len() - 2];
    }
    data
}

/// Timeline playback and display-list controller (used for both root and nested `DefineSprite` clips).
#[derive(Debug, Clone)]
pub struct Timeline {
    /// Total frames declared in this timeline.
    pub frame_count: u16,
    /// Current 0-based frame index (`0..frame_count`).
    pub current_frame: u16,
    /// Whether automatic playback is active (`play()` vs `stop()`).
    pub playing: bool,
    /// Tag stream for this timeline.
    pub tags: Vec<Tag>,
    /// Start tag index for each 0-based frame (`frame_tag_ranges[f] = (start_idx, end_idx)`).
    pub frame_tag_ranges: Vec<(usize, usize)>,
    /// Lowercase frame label -> 0-based frame index map.
    pub frame_labels: BTreeMap<String, u16>,
    /// Active display list keyed by `u16` depth.
    pub display_list: BTreeMap<u16, DisplayObject>,
    /// Whether frame 0's tags have been executed since instantiation or loop wrap.
    pub frame0_executed: bool,
}

impl Timeline {
    /// Constructs a new [`Timeline`] from a tag stream and declared frame count.
    #[must_use]
    pub fn new(frame_count: u16, tags: Vec<Tag>) -> Self {
        let mut frame_tag_ranges = Vec::new();
        let mut frame_labels = BTreeMap::new();
        let mut start_idx = 0_usize;
        let mut cur_frame: u16 = 0;

        for (idx, tag) in tags.iter().enumerate() {
            match tag {
                Tag::FrameLabel { name, .. } => {
                    frame_labels.insert(name.to_ascii_lowercase(), cur_frame);
                }
                Tag::DefineSceneAndFrameLabelData { labels, .. } => {
                    for &(fnum, ref label) in labels {
                        if let Ok(f16) = u16::try_from(fnum) {
                            frame_labels.insert(label.to_ascii_lowercase(), f16);
                        }
                    }
                }
                Tag::ShowFrame => {
                    frame_tag_ranges.push((start_idx, idx.saturating_add(1)));
                    start_idx = idx.saturating_add(1);
                    cur_frame = cur_frame.saturating_add(1);
                }
                _ => {}
            }
        }
        if frame_tag_ranges.is_empty() && !tags.is_empty() {
            frame_tag_ranges.push((0, tags.len()));
        }
        let effective_count = frame_count.max(u16::try_from(frame_tag_ranges.len()).unwrap_or(1)).max(1);

        Self {
            frame_count: effective_count,
            current_frame: 0,
            playing: true,
            tags,
            frame_tag_ranges,
            frame_labels,
            display_list: BTreeMap::new(),
            frame0_executed: false,
        }
    }

    /// Resolves a case-insensitive frame label to its 0-based frame index.
    #[must_use]
    pub fn resolve_label(&self, label: &str) -> Option<u16> {
        self.frame_labels.get(&label.to_ascii_lowercase()).copied()
    }
}

/// Side effects emitted during a timeline frame step or interactive button transition.
#[derive(Debug, Clone, Default)]
pub struct FrameEvents {
    /// Queued AVM1 bytecode blocks (`(target_path, bytecode)`).
    pub actions: Vec<(String, Vec<u8>)>,
    /// Queued event sound triggers (`(sound_id, SoundInfo)`).
    pub start_sounds: Vec<(u16, SoundInfo)>,
    /// Queued streaming audio header updates.
    pub stream_heads: Vec<SoundStreamHeadDef>,
    /// Queued streaming audio frame blocks.
    pub stream_blocks: Vec<Vec<u8>>,
}

/// Complete scene stage owning the character dictionary, root timeline, background color,
/// and dirty-rectangle list.
#[derive(Debug, Clone)]
pub struct Stage {
    /// Stage width in pixels.
    pub width_px: u32,
    /// Stage height in pixels.
    pub height_px: u32,
    /// Frame rate in frames per second.
    pub fps: f32,
    /// Stage background color (`SetBackgroundColor`, default white `0xFF_FF_FF`).
    pub background_color: Rgba,
    /// Character dictionary keyed by `u16` character ID.
    pub dictionary: BTreeMap<u16, Character>,
    /// Shared `JPEGTables` data for `DefineBits` tags.
    pub jpeg_tables: Option<Vec<u8>>,
    /// Font code table mapping (`font_id -> codes`) from `DefineFontInfo` / `DefineFontInfo2`.
    pub font_codes: BTreeMap<u16, Vec<u16>>,
    /// Root movie timeline.
    pub root: Timeline,
    /// Dirty rectangles in twips accumulated since the last render.
    pub dirty_rects: Vec<Rect>,
    /// Whether the entire stage needs redrawing (e.g. background color changed or first frame).
    pub full_redraw: bool,
    /// Current pointer position in stage twips (`(x_twips, y_twips)`).
    pub pointer_twips: (i32, i32),
    /// Whether the primary pointer button is currently pressed.
    pub pointer_down: bool,
}

impl Stage {
    /// Creates a new [`Stage`] from a parsed [`Movie`], pre-populating dictionary definitions.
    #[must_use]
    pub fn from_movie(movie: &Movie) -> Self {
        let width_px = movie.header.width_px();
        let height_px = movie.header.height_px();
        let fps = movie.header.fps();
        let mut stage = Self {
            width_px,
            height_px,
            fps,
            background_color: Rgba::rgb(255, 255, 255),
            dictionary: BTreeMap::new(),
            jpeg_tables: None,
            font_codes: BTreeMap::new(),
            root: Timeline::new(movie.header.frame_count, movie.tags.clone()),
            dirty_rects: Vec::new(),
            full_redraw: true,
            pointer_twips: (-10_000, -10_000),
            pointer_down: false,
        };
        // Pre-register all dictionary definitions so forward/goto jumps always find characters
        stage.register_definitions(&movie.tags);
        stage
    }

    fn register_definitions(&mut self, tags: &[Tag]) {
        for tag in tags {
            match tag {
                Tag::DefineShape(s) => {
                    self.dictionary.insert(s.id, Character::Shape(s.clone()));
                }
                Tag::DefineMorphShape(m) => {
                    self.dictionary
                        .insert(m.id, Character::MorphShape(m.clone()));
                }
                Tag::JPEGTables { jpeg_data } => {
                    self.jpeg_tables = Some(jpeg_data.clone());
                }
                Tag::DefineBits { id, jpeg_data } => {
                    if let Some(bmp) = decode_jpeg_or_png_bitmap(
                        *id,
                        self.jpeg_tables.as_deref(),
                        jpeg_data,
                        None,
                    ) {
                        self.dictionary.insert(*id, Character::Bitmap(bmp));
                    }
                }
                Tag::DefineBitsJPEG2 { id, image_data } => {
                    if let Some(bmp) = decode_jpeg_or_png_bitmap(*id, None, image_data, None) {
                        self.dictionary.insert(*id, Character::Bitmap(bmp));
                    }
                }
                Tag::DefineBitsJPEG3 {
                    id,
                    image_data,
                    alpha_data,
                }
                | Tag::DefineBitsJPEG4 {
                    id,
                    image_data,
                    alpha_data,
                    ..
                } => {
                    if let Some(bmp) =
                        decode_jpeg_or_png_bitmap(*id, None, image_data, Some(alpha_data))
                    {
                        self.dictionary.insert(*id, Character::Bitmap(bmp));
                    }
                }
                Tag::DefineBitsLossless(lossless) => {
                    if let Some(bmp) = decode_lossless_bitmap(lossless) {
                        self.dictionary.insert(lossless.id, Character::Bitmap(bmp));
                    }
                }
                Tag::DefineFont(f) => {
                    self.dictionary.insert(f.id, Character::Font(f.clone()));
                }
                Tag::DefineFontInfo {
                    font_id, codes, ..
                } => {
                    self.font_codes.insert(*font_id, codes.clone());
                    if let Some(Character::Font(f)) = self.dictionary.get_mut(font_id) {
                        f.codes.clone_from(codes);
                    }
                }
                Tag::DefineText(t) => {
                    self.dictionary.insert(t.id, Character::Text(t.clone()));
                }
                Tag::DefineEditText(e) => {
                    self.dictionary.insert(e.id, Character::EditText(e.clone()));
                }
                Tag::DefineButton(b) => {
                    self.dictionary.insert(b.id, Character::Button(b.clone()));
                }
                Tag::DefineSprite {
                    id,
                    frame_count,
                    tags: nested,
                } => {
                    self.dictionary.insert(
                        *id,
                        Character::Sprite {
                            id: *id,
                            frame_count: *frame_count,
                            tags: nested.clone(),
                        },
                    );
                }
                Tag::DefineSound(snd) => {
                    self.dictionary.insert(snd.id, Character::Sound(snd.clone()));
                }
                _ => {}
            }
        }
    }

    /// Steps the root timeline and all active nested `DefineSprite` timelines by one frame,
    /// returning all triggered [`FrameEvents`].
    pub fn step_frame(&mut self) -> FrameEvents {
        let mut events = FrameEvents::default();
        let dict = self.dictionary.clone();
        let mut bg = self.background_color;
        let mut dirty = false;

        step_timeline_one_frame(
            &mut self.root,
            "_root",
            &dict,
            &mut bg,
            &mut dirty,
            &mut events,
        );

        if bg != self.background_color {
            self.background_color = bg;
            self.full_redraw = true;
        }
        if dirty {
            self.mark_stage_dirty();
        }
        events
    }

    /// Jumps the root timeline (or a named child sprite timeline) to `target_frame` (0-based)
    /// and reconstructs the display list from frame `0..=target_frame`.
    pub fn goto_frame(&mut self, target_path: &str, target_frame: u16, play_after: bool) -> FrameEvents {
        let mut events = FrameEvents::default();
        let dict = self.dictionary.clone();
        let mut bg = self.background_color;
        let mut dirty = false;

        if target_path.is_empty() || target_path == "_root" || target_path == "/" {
            seek_timeline(
                &mut self.root,
                "_root",
                target_frame,
                play_after,
                &dict,
                &mut bg,
                &mut dirty,
                &mut events,
            );
        } else {
            let clean = target_path.trim_start_matches("_root.").trim_start_matches('/');
            for obj in self.root.display_list.values_mut() {
                if obj.name.as_deref() == Some(clean) {
                    if let Some(ref mut st) = obj.sprite_timeline {
                        seek_timeline(
                            st,
                            clean,
                            target_frame,
                            play_after,
                            &dict,
                            &mut bg,
                            &mut dirty,
                            &mut events,
                        );
                    }
                }
            }
        }
        self.background_color = bg;
        self.full_redraw = true;
        events
    }

    /// Updates pointer coordinates (in stage twips) and button state, returning any triggered
    /// button action scripts (`DefineButton` / `DefineButton2`).
    pub fn update_pointer(
        &mut self,
        x_twips: i32,
        y_twips: i32,
        pressed: bool,
    ) -> FrameEvents {
        let was_down = self.pointer_down;
        self.pointer_twips = (x_twips, y_twips);
        self.pointer_down = pressed;

        let mut events = FrameEvents::default();
        let dict = self.dictionary.clone();
        let mut changed = false;

        for obj in self.root.display_list.values_mut() {
            if let Some(Character::Button(btn)) = dict.get(&obj.character_id) {
                let inside = hit_test_button(btn, &obj.matrix, &dict, x_twips, y_twips);
                let prev_state = obj.button_state;
                let prev_pressed_in = obj.button_pressed_inside;

                let (new_state, cond_mask) = match (prev_state, inside, was_down, pressed) {
                    (ButtonState::Up, true, false, false) => (ButtonState::Over, 0x0001), // IdleToOverUp
                    (ButtonState::Up, true, _, true) => {
                        obj.button_pressed_inside = true;
                        (ButtonState::Down, 0x0001 | 0x0004 | 0x0080) // IdleToOverUp | OverUpToOverDown | IdleToOverDown
                    }
                    (ButtonState::Over, false, _, false) => (ButtonState::Up, 0x0002), // OverUpToIdle
                    (ButtonState::Over, true, false, true) => {
                        obj.button_pressed_inside = true;
                        (ButtonState::Down, 0x0004) // OverUpToOverDown
                    }
                    (ButtonState::Down, true, true, false) => {
                        obj.button_pressed_inside = false;
                        (ButtonState::Over, 0x0008) // OverDownToOverUp (release inside!)
                    }
                    (ButtonState::Down, false, true, true) => (ButtonState::Up, 0x0010), // OverDownToOutDown
                    (ButtonState::Down, false, true, false) => {
                        obj.button_pressed_inside = false;
                        (ButtonState::Up, 0x0040 | 0x0100) // OutDownToIdle | OverDownToIdle
                    }
                    (ButtonState::Up, true, true, true) if prev_pressed_in => {
                        (ButtonState::Down, 0x0020) // OutDownToOverDown
                    }
                    (state, _, _, false) => {
                        obj.button_pressed_inside = false;
                        if inside {
                            (ButtonState::Over, 0)
                        } else {
                            (ButtonState::Up, if state == ButtonState::Over { 0x0002 } else { 0 })
                        }
                    }
                    (state, _, _, _) => (state, 0),
                };

                if new_state != prev_state {
                    obj.button_state = new_state;
                    changed = true;
                }
                if cond_mask != 0 {
                    for act in &btn.actions {
                        if (act.condition & cond_mask) != 0 && !act.actions.is_empty() {
                            events.actions.push(("_root".to_owned(), act.actions.clone()));
                        }
                    }
                }
            }
        }

        if changed {
            self.mark_stage_dirty();
        }
        events
    }

    /// Synchronizes any `DefineEditText` instances whose `variable_name` matches `var_name`.
    pub fn sync_variable(&mut self, var_name: &str, value: &str) {
        let dict = &self.dictionary;
        let mut changed = false;
        for obj in self.root.display_list.values_mut() {
            if let Some(Character::EditText(edit)) = dict.get(&obj.character_id) {
                let clean_var = edit.variable_name.trim_start_matches("_root.");
                if !clean_var.is_empty() && clean_var == var_name {
                    if obj.dynamic_text.as_deref() != Some(value) {
                        obj.dynamic_text = Some(value.to_owned());
                        changed = true;
                    }
                }
            }
        }
        if changed {
            self.mark_stage_dirty();
        }
    }

    fn mark_stage_dirty(&mut self) {
        self.dirty_rects.push(Rect {
            x_min: 0,
            x_max: i32::try_from(self.width_px).unwrap_or(0).saturating_mul(20),
            y_min: 0,
            y_max: i32::try_from(self.height_px).unwrap_or(0).saturating_mul(20),
        });
    }
}

fn step_timeline_one_frame(
    tl: &mut Timeline,
    path: &str,
    dict: &BTreeMap<u16, Character>,
    bg: &mut Rgba,
    dirty: &mut bool,
    events: &mut FrameEvents,
) {
    if !tl.frame0_executed {
        execute_timeline_frame_tags(tl, 0, path, dict, bg, dirty, events, true);
        tl.frame0_executed = true;
    } else if tl.playing && tl.frame_count > 1 {
        let next = (tl.current_frame.saturating_add(1)) % tl.frame_count;
        if next == 0 {
            tl.display_list.clear();
            *dirty = true;
        }
        tl.current_frame = next;
        execute_timeline_frame_tags(tl, next, path, dict, bg, dirty, events, true);
    }

    // Advance nested DefineSprite timelines independently
    let depths: Vec<u16> = tl.display_list.keys().copied().collect();
    for d in depths {
        let Some(obj) = tl.display_list.get_mut(&d) else {
            continue;
        };
        let child_path = obj
            .name
            .as_deref()
            .map_or_else(|| format!("{path}._d{d}"), |n| format!("{path}.{n}"));
        if let Some(ref mut child_tl) = obj.sprite_timeline {
            step_timeline_one_frame(child_tl, &child_path, dict, bg, dirty, events);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn seek_timeline(
    tl: &mut Timeline,
    path: &str,
    target_frame: u16,
    play_after: bool,
    dict: &BTreeMap<u16, Character>,
    bg: &mut Rgba,
    dirty: &mut bool,
    events: &mut FrameEvents,
) {
    let clamped = target_frame.min(tl.frame_count.saturating_sub(1));
    tl.display_list.clear();
    for f in 0..=clamped {
        let emit_actions = f == clamped;
        execute_timeline_frame_tags(tl, f, path, dict, bg, dirty, events, emit_actions);
    }
    tl.current_frame = clamped;
    tl.playing = play_after;
    tl.frame0_executed = true;
    *dirty = true;
}

#[allow(clippy::too_many_arguments)]
fn execute_timeline_frame_tags(
    tl: &mut Timeline,
    frame_idx: u16,
    path: &str,
    dict: &BTreeMap<u16, Character>,
    bg: &mut Rgba,
    dirty: &mut bool,
    events: &mut FrameEvents,
    emit_actions: bool,
) {
    let Some(&(start, end)) = tl.frame_tag_ranges.get(usize::from(frame_idx)) else {
        return;
    };
    let Some(slice) = tl.tags.get(start..end) else {
        return;
    };

    for tag in slice {
        match tag {
            Tag::SetBackgroundColor { r, g, b } => {
                *bg = Rgba::rgb(*r, *g, *b);
                *dirty = true;
            }
            Tag::PlaceObject(po) => {
                apply_place_object1(tl, po, dict);
                *dirty = true;
            }
            Tag::PlaceObject2(po2) | Tag::PlaceObject3(po2) => {
                apply_place_object2(tl, po2, dict, bg, dirty, events);
                *dirty = true;
            }
            Tag::RemoveObject { depth, .. } | Tag::RemoveObject2 { depth } => {
                tl.display_list.remove(depth);
                *dirty = true;
            }
            Tag::DoInitAction { actions, .. } | Tag::DoAction { actions } => {
                if emit_actions && !actions.is_empty() {
                    events.actions.push((path.to_owned(), actions.clone()));
                }
            }
            Tag::StartSound {
                sound_id,
                sound_info,
            } => {
                if emit_actions {
                    events.start_sounds.push((*sound_id, sound_info.clone()));
                }
            }
            Tag::SoundStreamHead(head) => {
                if emit_actions {
                    events.stream_heads.push(head.clone());
                }
            }
            Tag::SoundStreamBlock { data } => {
                if emit_actions {
                    events.stream_blocks.push(data.clone());
                }
            }
            _ => {}
        }
    }
}

fn apply_place_object1(
    tl: &mut Timeline,
    po: &PlaceObjectTag,
    dict: &BTreeMap<u16, Character>,
) {
    let (dynamic_text, sprite_timeline) = init_instance_state(po.character_id, dict);
    tl.display_list.insert(
        po.depth,
        DisplayObject {
            character_id: po.character_id,
            depth: po.depth,
            matrix: po.matrix,
            color_transform: po.color_transform.unwrap_or(ColorTransform::IDENTITY),
            ratio: 0,
            name: None,
            clip_depth: None,
            button_state: ButtonState::Up,
            button_pressed_inside: false,
            dynamic_text,
            sprite_timeline,
        },
    );
}

fn apply_place_object2(
    tl: &mut Timeline,
    po2: &PlaceObject2Tag,
    dict: &BTreeMap<u16, Character>,
    bg: &mut Rgba,
    dirty: &mut bool,
    events: &mut FrameEvents,
) {
    if po2.is_move && po2.character_id.is_none() {
        if let Some(existing) = tl.display_list.get_mut(&po2.depth) {
            if let Some(m) = po2.matrix {
                existing.matrix = m;
            }
            if let Some(cx) = po2.color_transform {
                existing.color_transform = cx;
            }
            if let Some(r) = po2.ratio {
                existing.ratio = r;
            }
            if let Some(ref name) = po2.name {
                existing.name = Some(name.clone());
            }
            if let Some(cd) = po2.clip_depth {
                existing.clip_depth = Some(cd);
            }
        }
        return;
    }

    let prev = tl.display_list.get(&po2.depth).cloned();
    let char_id = po2
        .character_id
        .or_else(|| prev.as_ref().map(|p| p.character_id))
        .unwrap_or(0);
    let matrix = po2
        .matrix
        .or_else(|| prev.as_ref().map(|p| p.matrix))
        .unwrap_or(Matrix::IDENTITY);
    let color_transform = po2
        .color_transform
        .or_else(|| prev.as_ref().map(|p| p.color_transform))
        .unwrap_or(ColorTransform::IDENTITY);
    let ratio = po2.ratio.or_else(|| prev.as_ref().map(|p| p.ratio)).unwrap_or(0);
    let name = po2
        .name
        .clone()
        .or_else(|| prev.as_ref().and_then(|p| p.name.clone()));
    let clip_depth = po2
        .clip_depth
        .or_else(|| prev.as_ref().and_then(|p| p.clip_depth));

    let (dynamic_text, mut sprite_timeline) = init_instance_state(char_id, dict);
    if let Some(ref mut st) = sprite_timeline {
        let sp_path = name.as_deref().unwrap_or("_sprite");
        execute_timeline_frame_tags(st, 0, sp_path, dict, bg, dirty, events, true);
        st.frame0_executed = true;
    }

    tl.display_list.insert(
        po2.depth,
        DisplayObject {
            character_id: char_id,
            depth: po2.depth,
            matrix,
            color_transform,
            ratio,
            name,
            clip_depth,
            button_state: ButtonState::Up,
            button_pressed_inside: false,
            dynamic_text,
            sprite_timeline,
        },
    );
}

fn init_instance_state(
    char_id: u16,
    dict: &BTreeMap<u16, Character>,
) -> (Option<String>, Option<Timeline>) {
    match dict.get(&char_id) {
        Some(Character::EditText(edit)) => (edit.initial_text.clone(), None),
        Some(Character::Sprite {
            frame_count, tags, ..
        }) => (None, Some(Timeline::new(*frame_count, tags.clone()))),
        _ => (None, None),
    }
}

/// Tests whether `(stage_x_twips, stage_y_twips)` hits any `state_hit_test` record of `btn`
/// placed with `btn_matrix`.
#[must_use]
#[allow(clippy::arithmetic_side_effects)]
pub fn hit_test_button(
    btn: &ButtonDef,
    btn_matrix: &Matrix,
    dict: &BTreeMap<u16, Character>,
    stage_x_twips: i32,
    stage_y_twips: i32,
) -> bool {
    let Some((bx, by)) = invert_matrix_point(btn_matrix, stage_x_twips as f32, stage_y_twips as f32)
    else {
        return false;
    };

    for rec in &btn.records {
        if !rec.state_hit_test {
            continue;
        }
        let Some((lx, ly)) = invert_matrix_point(&rec.matrix, bx, by) else {
            continue;
        };
        match dict.get(&rec.character_id) {
            Some(Character::Shape(shape)) => {
                let flat = flatten_shape(shape);
                if point_in_flattened_shape(&flat, lx, ly) {
                    return true;
                }
            }
            Some(Character::MorphShape(morph)) => {
                let flat = flatten_morph_shape(morph, 0);
                if point_in_flattened_shape(&flat, lx, ly) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

#[allow(clippy::arithmetic_side_effects)]
fn invert_matrix_point(m: &Matrix, x: f32, y: f32) -> Option<(f32, f32)> {
    let a = m.scale_x_fp16 as f32 / 65536.0;
    let b = m.rotate_skew0_fp16 as f32 / 65536.0;
    let c = m.rotate_skew1_fp16 as f32 / 65536.0;
    let d = m.scale_y_fp16 as f32 / 65536.0;
    let tx = m.translate_x_twips as f32;
    let ty = m.translate_y_twips as f32;

    let det = a * d - b * c;
    if det.abs() < 1e-6 {
        return None;
    }
    let dx = x - tx;
    let dy = y - ty;
    let lx = (d * dx - c * dy) / det;
    let ly = (-b * dx + a * dy) / det;
    Some((lx, ly))
}
