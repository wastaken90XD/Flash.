//! Display list, character dictionary, shape flattening, timeline engine, and button hit-testing
//! for `swfr` (TSD 1.6, 2.12 M2 & M3).
#![allow(
    clippy::arithmetic_side_effects,
    clippy::collapsible_if,
    clippy::collapsible_match
)]

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

/// `onClipEvent(load)` bit flag in `CLIPACTIONRECORD` (`0x0000_0001`).
pub const CLIP_EVENT_LOAD: u32 = 0x0000_0001;
/// `onClipEvent(enterFrame)` bit flag in `CLIPACTIONRECORD` (`0x0000_0002`).
pub const CLIP_EVENT_ENTER_FRAME: u32 = 0x0000_0002;
/// `onClipEvent(unload)` bit flag in `CLIPACTIONRECORD` (`0x0000_0004`).
pub const CLIP_EVENT_UNLOAD: u32 = 0x0000_0004;
/// `onClipEvent(mouseMove)` bit flag in `CLIPACTIONRECORD` (`0x0000_0008`).
pub const CLIP_EVENT_MOUSE_MOVE: u32 = 0x0000_0008;
/// `onClipEvent(mouseDown)` bit flag in `CLIPACTIONRECORD` (`0x0000_0010`).
pub const CLIP_EVENT_MOUSE_DOWN: u32 = 0x0000_0010;
/// `onClipEvent(mouseUp)` bit flag in `CLIPACTIONRECORD` (`0x0000_0020`).
pub const CLIP_EVENT_MOUSE_UP: u32 = 0x0000_0020;
/// `onClipEvent(keyDown)` bit flag in `CLIPACTIONRECORD` (`0x0000_0040`).
pub const CLIP_EVENT_KEY_DOWN: u32 = 0x0000_0040;
/// `onClipEvent(keyUp)` bit flag in `CLIPACTIONRECORD` (`0x0000_0080`).
pub const CLIP_EVENT_KEY_UP: u32 = 0x0000_0080;
/// `onClipEvent(data)` bit flag in `CLIPACTIONRECORD` (`0x0000_0100`).
pub const CLIP_EVENT_DATA: u32 = 0x0000_0100;
/// `onClipEvent(initialize)` bit flag in `CLIPACTIONRECORD` (`0x0000_0200`).
pub const CLIP_EVENT_INITIALIZE: u32 = 0x0000_0200;
/// `onClipEvent(press)` bit flag in `CLIPACTIONRECORD` (`0x0000_0400`).
pub const CLIP_EVENT_PRESS: u32 = 0x0000_0400;
/// `onClipEvent(release)` bit flag in `CLIPACTIONRECORD` (`0x0000_0800`).
pub const CLIP_EVENT_RELEASE: u32 = 0x0000_0800;
/// `onClipEvent(keyPress)` bit flag in `CLIPACTIONRECORD` (`0x0002_0000`).
pub const CLIP_EVENT_KEY_PRESS: u32 = 0x0002_0000;
/// `onClipEvent(construct)` bit flag in `CLIPACTIONRECORD` (`0x0004_0000`).
pub const CLIP_EVENT_CONSTRUCT: u32 = 0x0004_0000;

/// Decoded `CLIPACTIONRECORD` attached to a placed movie clip (`PlaceObject2` / `PlaceObject3`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipActionRecord {
    /// Bitmask of `CLIP_EVENT_*` flags.
    pub event_flags: u32,
    /// Optional key code when [`CLIP_EVENT_KEY_PRESS`] is set.
    pub key_code: Option<u8>,
    /// ActionScript bytecode payload.
    pub actions: Vec<u8>,
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
    /// Decoded `CLIPACTIONRECORD` handlers attached to this clip.
    pub clip_actions: Vec<ClipActionRecord>,
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
            (
                FillStyle::Bitmap {
                    bitmap_id,
                    matrix: sm,
                    repeating,
                    smoothed,
                },
                FillStyle::Bitmap { matrix: em, .. },
            ) => FillStyle::Bitmap {
                bitmap_id: *bitmap_id,
                matrix: lerp_matrix(*sm, *em),
                repeating: *repeating,
                smoothed: *smoothed,
            },
            _ => {
                if ratio >= 32_768 {
                    e.clone()
                } else {
                    s.clone()
                }
            }
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
                            end_curve =
                                (hdx, hdy, edx.saturating_sub(hdx), edy.saturating_sub(hdy));
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

    let mut fill_edges: BTreeMap<u16, Vec<DirectedEdge>> = BTreeMap::new();
    let mut line_edges: BTreeMap<u16, Vec<DirectedEdge>> = BTreeMap::new();

    let mut out_fills: Vec<FillPath> = Vec::new();
    let mut out_strokes: Vec<StrokePath> = Vec::new();

    let mut cur_x: i32 = 0;
    let mut cur_y: i32 = 0;
    let mut fill0: u16 = 0;
    let mut fill1: u16 = 0;
    let mut line: u16 = 0;

    let flush_group = |fill_edges: &mut BTreeMap<u16, Vec<DirectedEdge>>,
                       line_edges: &mut BTreeMap<u16, Vec<DirectedEdge>>,
                       cur_fills: &[FillStyle],
                       cur_lines: &[LineStyle],
                       out_fills: &mut Vec<FillPath>,
                       out_strokes: &mut Vec<StrokePath>| {
        for (&style_idx, edges) in fill_edges.iter() {
            if style_idx == 0 {
                continue;
            }
            if let Some(style) = cur_fills.get(usize::from(style_idx - 1)) {
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
            if let Some(style) = cur_lines.get(usize::from(style_idx - 1)) {
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
    let mut by_start: BTreeMap<(i32, i32), Vec<usize>> = BTreeMap::new();
    for (idx, edge) in edges.iter().enumerate().rev() {
        by_start.entry((edge.x0, edge.y0)).or_default().push(idx);
    }

    for start_idx in 0..edges.len() {
        if used.get(start_idx).copied().unwrap_or(true) {
            continue;
        }
        if let Some(used_flag) = used.get_mut(start_idx) {
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
            if let Some(bucket) = by_start.get_mut(&cur_pt) {
                while let Some(cand_idx) = bucket.pop() {
                    if !used.get(cand_idx).copied().unwrap_or(true) {
                        if let Some(&cand) = edges.get(cand_idx) {
                            next_match = Some((cand_idx, cand));
                            break;
                        }
                    }
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
            let palette = raw.get(..palette_bytes)?;
            let indices = raw.get(palette_bytes..)?;
            let row_stride = (width + 3) & !3;
            for y in 0..height {
                for x in 0..width {
                    let idx = usize::from(*indices.get(y * row_stride + x).unwrap_or(&0));
                    let p_off = idx * entry_bytes;
                    if p_off + entry_bytes <= palette.len() {
                        let r = palette.get(p_off).copied().unwrap_or(0);
                        let g = palette.get(p_off + 1).copied().unwrap_or(0);
                        let b = palette.get(p_off + 2).copied().unwrap_or(0);
                        let a = if has_alpha {
                            palette.get(p_off + 3).copied().unwrap_or(255)
                        } else {
                            255
                        };
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
fn unpremultiply(r: u8, g: u8, b: u8, a: u8) -> (u8, u8, u8) {
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
        let mut decoder = png::Decoder::new(Cursor::new(image_data));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().ok()?;
        let mut buf = vec![0_u8; reader.output_buffer_size()?];
        let info = reader.next_frame(&mut buf).ok()?;
        let bytes = buf.get(..info.buffer_size())?;
        let mut rgba = Vec::with_capacity((info.width * info.height * 4) as usize);
        match info.color_type {
            png::ColorType::Rgba => rgba.extend_from_slice(bytes),
            png::ColorType::Rgb => {
                for chunk in bytes.chunks_exact(3) {
                    let c0 = chunk.first().copied().unwrap_or(0);
                    let c1 = chunk.get(1).copied().unwrap_or(0);
                    let c2 = chunk.get(2).copied().unwrap_or(0);
                    rgba.extend_from_slice(&[c0, c1, c2, 255]);
                }
            }
            png::ColorType::GrayscaleAlpha => {
                for chunk in bytes.chunks_exact(2) {
                    let l = chunk.first().copied().unwrap_or(0);
                    let a = chunk.get(1).copied().unwrap_or(255);
                    rgba.extend_from_slice(&[l, l, l, a]);
                }
            }
            png::ColorType::Grayscale => {
                for &l in bytes {
                    rgba.extend_from_slice(&[l, l, l, 255]);
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

    let mut raw_stream = Vec::new();
    if let Some(tables) = jpeg_tables {
        if !tables.is_empty() {
            raw_stream.extend_from_slice(strip_erroneous_jpeg_markers(tables, true));
        }
    }
    raw_stream.extend_from_slice(strip_erroneous_jpeg_markers(
        image_data,
        !raw_stream.is_empty(),
    ));
    let combined = clean_swf_jpeg_stream(&raw_stream);

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
                pixels.get(i * channels).copied().unwrap_or(0),
                pixels.get(i * channels + 1).copied().unwrap_or(0),
                pixels.get(i * channels + 2).copied().unwrap_or(0),
            )
        } else if channels == 1 {
            let l = pixels.get(i).copied().unwrap_or(0);
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
    if data.starts_with(&[0xFF, 0xD9, 0xFF, 0xD8]) {
        data = data.get(4..).unwrap_or(data);
    }
    if strip_leading_soi && data.starts_with(&[0xFF, 0xD8]) {
        data = data.get(2..).unwrap_or(data);
    }
    if strip_leading_soi && data.ends_with(&[0xFF, 0xD9]) {
        data = data.get(..data.len().saturating_sub(2)).unwrap_or(data);
    }
    data
}

fn clean_swf_jpeg_stream(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0_usize;
    let len = raw.len();
    let mut seen_soi = false;
    while i < len {
        // Strip internal `FF D9 FF D8` or `FF D8 FF D9` glue sequences between tables and scan
        if i + 4 <= len
            && raw.get(i..i + 4) == Some(&[0xFF, 0xD9, 0xFF, 0xD8])
            && i + 4 < len
        {
            if !seen_soi {
                out.extend_from_slice(&[0xFF, 0xD8]);
                seen_soi = true;
            }
            i += 4;
            continue;
        }
        if i + 2 <= len && raw.get(i..i + 2) == Some(&[0xFF, 0xD8]) {
            if !seen_soi {
                out.extend_from_slice(&[0xFF, 0xD8]);
                seen_soi = true;
            }
            i += 2;
            continue;
        }
        if let Some(&b) = raw.get(i) {
            out.push(b);
        }
        i += 1;
    }
    out
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
        let effective_count = frame_count
            .max(u16::try_from(frame_tag_ranges.len()).unwrap_or(1))
            .max(1);

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
    /// SWF format version from header (`1..=43`).
    pub swf_version: u8,
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
            swf_version: movie.header.version,
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

    /// Registers dictionary character definitions from `tags` (recursing into `DefineSprite`).
    pub fn register_definitions(&mut self, tags: &[Tag]) {
        for tag in tags {
            match tag {
                Tag::DefineShape(s) => {
                    let mut shape = s.clone();
                    if self.swf_version < 8 {
                        clear_bitmap_smoothing(&mut shape.fill_styles);
                        for rec in &mut shape.records {
                            if let ShapeRecord::StyleChange {
                                new_fill_styles: Some(ref mut nfs),
                                ..
                            } = rec
                            {
                                clear_bitmap_smoothing(nfs);
                            }
                        }
                    }
                    self.dictionary.insert(shape.id, Character::Shape(shape));
                }
                Tag::DefineMorphShape(m) => {
                    let mut morph = m.clone();
                    if self.swf_version < 8 {
                        clear_bitmap_smoothing(&mut morph.start_fill_styles);
                        clear_bitmap_smoothing(&mut morph.end_fill_styles);
                    }
                    self.dictionary
                        .insert(morph.id, Character::MorphShape(morph));
                }
                Tag::JPEGTables { jpeg_data } => {
                    self.jpeg_tables = Some(jpeg_data.clone());
                }
                Tag::DefineBits { id, jpeg_data } => {
                    if let Some(bmp) =
                        decode_jpeg_or_png_bitmap(*id, self.jpeg_tables.as_deref(), jpeg_data, None)
                    {
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
                Tag::DefineFontInfo { font_id, codes, .. } => {
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
                    self.register_definitions(nested);
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
                    self.dictionary
                        .insert(snd.id, Character::Sound(snd.clone()));
                }
                _ => {}
            }
        }
    }

    /// Steps the root timeline and all active nested `DefineSprite` timelines by one frame,
    /// returning all triggered [`FrameEvents`].
    pub fn step_frame(&mut self) -> FrameEvents {
        let mut events = FrameEvents::default();
        let mut bg = self.background_color;
        let mut dirty = false;
        let swf_version = self.swf_version;

        step_timeline_one_frame(
            &mut self.root,
            "_root",
            swf_version,
            &self.dictionary,
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
    pub fn goto_frame(
        &mut self,
        target_path: &str,
        target_frame: u16,
        play_after: bool,
    ) -> FrameEvents {
        let mut events = FrameEvents::default();
        let mut bg = self.background_color;
        let mut dirty = false;
        let swf_version = self.swf_version;

        let parts = split_clip_path(target_path);
        if parts.is_empty() {
            seek_timeline(
                &mut self.root,
                "_root",
                swf_version,
                target_frame,
                play_after,
                &self.dictionary,
                &mut bg,
                &mut dirty,
                &mut events,
            );
        } else if let Some(st) = find_timeline_mut(&mut self.root, &parts) {
            let full_path = format!("_root.{}", parts.join("."));
            seek_timeline(
                st,
                &full_path,
                swf_version,
                target_frame,
                play_after,
                &self.dictionary,
                &mut bg,
                &mut dirty,
                &mut events,
            );
        }
        self.background_color = bg;
        self.full_redraw = true;
        events
    }

    /// Looks up a timeline by clip path (`"_root"`, `"_root.a.b"`, `"a.b"`, `"/a/b"`).
    #[must_use]
    pub fn timeline_by_path(&self, path: &str) -> Option<&Timeline> {
        let parts = split_clip_path(path);
        if parts.is_empty() {
            Some(&self.root)
        } else {
            find_timeline(&self.root, &parts)
        }
    }

    /// Looks up a mutable timeline by clip path (`"_root"`, `"_root.a.b"`, `"a.b"`, `"/a/b"`).
    pub fn timeline_by_path_mut(&mut self, path: &str) -> Option<&mut Timeline> {
        let parts = split_clip_path(path);
        if parts.is_empty() {
            Some(&mut self.root)
        } else {
            find_timeline_mut(&mut self.root, &parts)
        }
    }

    /// Looks up a placed [`DisplayObject`] by clip path (`"_root.a"`, `"a.b"`, etc.).
    #[must_use]
    pub fn display_object_by_path(&self, path: &str) -> Option<&DisplayObject> {
        let parts = split_clip_path(path);
        find_display_object(&self.root, &parts)
    }

    /// Looks up a mutable placed [`DisplayObject`] by clip path (`"_root.a"`, `"a.b"`, etc.).
    pub fn display_object_by_path_mut(&mut self, path: &str) -> Option<&mut DisplayObject> {
        let parts = split_clip_path(path);
        find_display_object_mut(&mut self.root, &parts)
    }

    /// Resolves a child clip name `child_name` inside `base_path` (case-insensitive for SWF <= 6).
    #[must_use]
    pub fn resolve_child_clip_path(&self, base_path: &str, child_name: &str) -> Option<String> {
        let tl = self.timeline_by_path(base_path)?;
        let case_sensitive = self.swf_version >= 7;
        for (&d, obj) in &tl.display_list {
            let candidate = child_clip_path(base_path, d, obj.name.as_deref());
            if let Some(ref n) = obj.name {
                let matched = if case_sensitive {
                    n == child_name
                } else {
                    n.eq_ignore_ascii_case(child_name)
                };
                if matched {
                    return Some(candidate);
                }
            } else if format!("_d{d}") == child_name {
                return Some(candidate);
            }
        }
        None
    }

    /// Marks the entire stage dirty for redraw (e.g. after script property mutations).
    pub fn mark_dirty(&mut self) {
        self.full_redraw = true;
        self.mark_stage_dirty();
    }

    /// Dispatches a keyboard press/release event immediately to any placed clips with
    /// `onClipEvent(keyDown)` / `onClipEvent(keyUp)` / `onClipEvent(keyPress)` handlers (TSD 4.6 item 10).
    #[must_use]
    pub fn dispatch_key_event(&self, keycode: u32, down: bool) -> FrameEvents {
        let mut events = FrameEvents::default();
        let flag = if down {
            CLIP_EVENT_KEY_DOWN
        } else {
            CLIP_EVENT_KEY_UP
        };
        collect_clip_input_events_recursive(&self.root, "_root", flag, Some(keycode), &mut events);
        events
    }

    /// Updates pointer coordinates (in stage twips) and button state, returning any triggered
    /// button and clip mouse action scripts (`DefineButton`, `DefineButton2`, `onClipEvent(mouse*)`).
    pub fn update_pointer(&mut self, x_twips: i32, y_twips: i32, pressed: bool) -> FrameEvents {
        let was_down = self.pointer_down;
        let had_pos = self.pointer_twips != (-10_000, -10_000);
        let moved = had_pos && self.pointer_twips != (x_twips, y_twips);
        self.pointer_twips = (x_twips, y_twips);
        self.pointer_down = pressed;

        let mut events = FrameEvents::default();
        let mut changed = false;

        if moved {
            collect_clip_input_events_recursive(
                &self.root,
                "_root",
                CLIP_EVENT_MOUSE_MOVE,
                None,
                &mut events,
            );
        }
        if !was_down && pressed {
            collect_clip_input_events_recursive(
                &self.root,
                "_root",
                CLIP_EVENT_MOUSE_DOWN,
                None,
                &mut events,
            );
        } else if was_down && !pressed {
            collect_clip_input_events_recursive(
                &self.root,
                "_root",
                CLIP_EVENT_MOUSE_UP,
                None,
                &mut events,
            );
        }

        update_buttons_in_timeline(
            &mut self.root,
            "_root",
            &self.dictionary,
            x_twips as f32,
            y_twips as f32,
            was_down,
            pressed,
            &mut changed,
            &mut events,
        );

        if changed {
            self.mark_stage_dirty();
        }
        events
    }

    /// Synchronizes any `DefineEditText` instances whose `variable_name` matches `var_name`.
    pub fn sync_variable(&mut self, var_name: &str, value: &str) {
        let clean_target = var_name
            .trim_start_matches("_root.")
            .trim_start_matches("_level0.")
            .trim_start_matches('/');
        let mut changed = false;
        sync_variable_in_timeline(
            &mut self.root,
            &self.dictionary,
            clean_target,
            value,
            self.swf_version >= 7,
            &mut changed,
        );
        if changed {
            self.mark_stage_dirty();
        }
    }

    fn mark_stage_dirty(&mut self) {
        self.dirty_rects.push(Rect {
            x_min: 0,
            x_max: i32::try_from(self.width_px).unwrap_or(0).saturating_mul(20),
            y_min: 0,
            y_max: i32::try_from(self.height_px)
                .unwrap_or(0)
                .saturating_mul(20),
        });
    }
}

fn clear_bitmap_smoothing(styles: &mut [FillStyle]) {
    for st in styles {
        if let FillStyle::Bitmap { smoothed, .. } = st {
            *smoothed = false;
        }
    }
}

fn child_clip_path(parent_path: &str, depth: u16, name: Option<&str>) -> String {
    match name {
        Some(n) => format!("{parent_path}.{n}"),
        None => format!("{parent_path}._d{depth}"),
    }
}

fn split_clip_path(path: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    for seg in path.split(['.', '/']) {
        if seg.is_empty() || seg == "_root" || seg == "_level0" {
            continue;
        }
        parts.push(seg);
    }
    parts
}

fn match_clip_segment(depth: u16, obj: &DisplayObject, seg: &str) -> bool {
    if let Some(ref name) = obj.name {
        if name.eq_ignore_ascii_case(seg) {
            return true;
        }
    }
    if let Some(d_str) = seg.strip_prefix("_d") {
        if d_str.parse::<u16>().ok() == Some(depth) {
            return true;
        }
    }
    false
}

fn find_timeline<'a>(tl: &'a Timeline, parts: &[&str]) -> Option<&'a Timeline> {
    let (&first, rest) = parts.split_first()?;
    for (&d, obj) in &tl.display_list {
        if match_clip_segment(d, obj, first) {
            let child_tl = obj.sprite_timeline.as_ref()?;
            if rest.is_empty() {
                return Some(child_tl);
            }
            return find_timeline(child_tl, rest);
        }
    }
    None
}

fn find_timeline_mut<'a>(tl: &'a mut Timeline, parts: &[&str]) -> Option<&'a mut Timeline> {
    let (&first, rest) = parts.split_first()?;
    for (&d, obj) in &mut tl.display_list {
        if match_clip_segment(d, obj, first) {
            let child_tl = obj.sprite_timeline.as_mut()?;
            if rest.is_empty() {
                return Some(child_tl);
            }
            return find_timeline_mut(child_tl, rest);
        }
    }
    None
}

fn find_display_object<'a>(tl: &'a Timeline, parts: &[&str]) -> Option<&'a DisplayObject> {
    let (&first, rest) = parts.split_first()?;
    for (&d, obj) in &tl.display_list {
        if match_clip_segment(d, obj, first) {
            if rest.is_empty() {
                return Some(obj);
            }
            let child_tl = obj.sprite_timeline.as_ref()?;
            return find_display_object(child_tl, rest);
        }
    }
    None
}

fn find_display_object_mut<'a>(
    tl: &'a mut Timeline,
    parts: &[&str],
) -> Option<&'a mut DisplayObject> {
    let (&first, rest) = parts.split_first()?;
    for (&d, obj) in &mut tl.display_list {
        if match_clip_segment(d, obj, first) {
            if rest.is_empty() {
                return Some(obj);
            }
            let child_tl = obj.sprite_timeline.as_mut()?;
            return find_display_object_mut(child_tl, rest);
        }
    }
    None
}

#[allow(clippy::too_many_arguments, clippy::arithmetic_side_effects)]
fn update_buttons_in_timeline(
    tl: &mut Timeline,
    path: &str,
    dict: &BTreeMap<u16, Character>,
    local_x_twips: f32,
    local_y_twips: f32,
    was_down: bool,
    pressed: bool,
    changed: &mut bool,
    events: &mut FrameEvents,
) {
    for (&d, obj) in &mut tl.display_list {
        let child_path = child_clip_path(path, d, obj.name.as_deref());
        if let Some(ref mut child_tl) = obj.sprite_timeline {
            if let Some((cx, cy)) = invert_matrix_point(&obj.matrix, local_x_twips, local_y_twips) {
                update_buttons_in_timeline(
                    child_tl,
                    &child_path,
                    dict,
                    cx,
                    cy,
                    was_down,
                    pressed,
                    changed,
                    events,
                );
            }
        }
        if let Some(Character::Button(btn)) = dict.get(&obj.character_id) {
            let inside = hit_test_button(
                btn,
                &obj.matrix,
                dict,
                local_x_twips.round() as i32,
                local_y_twips.round() as i32,
            );
            let prev_state = obj.button_state;
            let prev_pressed_in = obj.button_pressed_inside;

            let (new_state, cond_mask) = match (prev_state, inside, was_down, pressed) {
                (ButtonState::Up, true, false, false) => (ButtonState::Over, 0x0001), // IdleToOverUp
                (ButtonState::Up, true, true, true) if prev_pressed_in => {
                    (ButtonState::Down, 0x0020) // OutDownToOverDown
                }
                (ButtonState::Up, true, false, true) => {
                    obj.button_pressed_inside = true;
                    (ButtonState::Down, 0x0001 | 0x0004 | 0x0080) // IdleToOverUp | OverUpToOverDown | IdleToOverDown
                }
                (ButtonState::Up, true, true, true) if btn.track_as_menu => {
                    (ButtonState::Down, 0x0080) // IdleToOverDown
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
                (state, _, _, false) => {
                    obj.button_pressed_inside = false;
                    if inside {
                        (ButtonState::Over, 0)
                    } else {
                        (
                            ButtonState::Up,
                            if state == ButtonState::Over {
                                0x0002
                            } else {
                                0
                            },
                        )
                    }
                }
                (state, _, _, _) => (state, 0),
            };

            if new_state != prev_state {
                obj.button_state = new_state;
                *changed = true;
            }
            if cond_mask != 0 {
                for act in &btn.actions {
                    if (act.condition & cond_mask) != 0 && !act.actions.is_empty() {
                        events.actions.push((path.to_owned(), act.actions.clone()));
                    }
                }
            }
        }
    }
}

fn sync_variable_in_timeline(
    tl: &mut Timeline,
    dict: &BTreeMap<u16, Character>,
    clean_target: &str,
    value: &str,
    case_sensitive: bool,
    changed: &mut bool,
) {
    for obj in tl.display_list.values_mut() {
        if let Some(ref mut child_tl) = obj.sprite_timeline {
            sync_variable_in_timeline(
                child_tl,
                dict,
                clean_target,
                value,
                case_sensitive,
                changed,
            );
        }
        if let Some(Character::EditText(edit)) = dict.get(&obj.character_id) {
            let clean_var = edit
                .variable_name
                .trim_start_matches("_root.")
                .trim_start_matches("_level0.")
                .trim_start_matches('/');
            let matched = !clean_var.is_empty()
                && if case_sensitive {
                    clean_var == clean_target
                } else {
                    clean_var.eq_ignore_ascii_case(clean_target)
                };
            if matched && obj.dynamic_text.as_deref() != Some(value) {
                obj.dynamic_text = Some(value.to_owned());
                *changed = true;
            }
        }
    }
}

/// Decodes raw `CLIPACTIONS` bytes from a `PlaceObject2` / `PlaceObject3` tag into
/// structured [`ClipActionRecord`] entries.
#[must_use]
pub fn decode_clip_actions(raw: &[u8], swf_version: u8) -> Vec<ClipActionRecord> {
    let mut r = swfr_reader::Reader::new(raw);
    let Ok(_reserved) = r.u16() else {
        return Vec::new();
    };
    let all_res = if swf_version >= 6 {
        r.u32()
    } else {
        r.u16().map(u32::from)
    };
    if all_res.is_err() {
        return Vec::new();
    }

    let mut records = Vec::new();
    loop {
        let flags_res = if swf_version >= 6 {
            r.u32()
        } else {
            r.u16().map(u32::from)
        };
        let Ok(flags) = flags_res else {
            break;
        };
        if flags == 0 {
            break;
        }
        let Ok(size_u32) = r.u32() else {
            break;
        };
        let size = usize::try_from(size_u32).unwrap_or(0);
        if r.remaining() < size {
            break;
        }
        if (flags & CLIP_EVENT_KEY_PRESS) != 0 && size >= 1 {
            let Ok(kc) = r.u8() else {
                break;
            };
            let Ok(act) = r.bytes(size - 1) else {
                break;
            };
            records.push(ClipActionRecord {
                event_flags: flags,
                key_code: Some(kc),
                actions: act.to_vec(),
            });
        } else {
            let Ok(act) = r.bytes(size) else {
                break;
            };
            records.push(ClipActionRecord {
                event_flags: flags,
                key_code: None,
                actions: act.to_vec(),
            });
        }
    }
    records
}

fn collect_clip_input_events_recursive(
    tl: &Timeline,
    path: &str,
    flag: u32,
    keycode: Option<u32>,
    events: &mut FrameEvents,
) {
    for (&d, obj) in &tl.display_list {
        let child_path = child_clip_path(path, d, obj.name.as_deref());
        if let Some(ref child_tl) = obj.sprite_timeline {
            collect_clip_input_events_recursive(child_tl, &child_path, flag, keycode, events);
        }
        for rec in &obj.clip_actions {
            if (rec.event_flags & flag) != 0 && !rec.actions.is_empty() {
                events
                    .actions
                    .push((child_path.clone(), rec.actions.clone()));
            } else if flag == CLIP_EVENT_KEY_DOWN
                && (rec.event_flags & CLIP_EVENT_KEY_PRESS) != 0
                && !rec.actions.is_empty()
            {
                if let (Some(want), Some(got)) = (rec.key_code, keycode) {
                    if u32::from(want) == got {
                        events
                            .actions
                            .push((child_path.clone(), rec.actions.clone()));
                    }
                }
            }
        }
    }
}

fn collect_unload_actions_recursive(
    obj: &DisplayObject,
    obj_path: &str,
    out: &mut Vec<(String, Vec<u8>)>,
) {
    if let Some(ref child_tl) = obj.sprite_timeline {
        for (&cd, cobj) in &child_tl.display_list {
            let cpath = child_clip_path(obj_path, cd, cobj.name.as_deref());
            collect_unload_actions_recursive(cobj, &cpath, out);
        }
    }
    for rec in &obj.clip_actions {
        if (rec.event_flags & CLIP_EVENT_UNLOAD) != 0 && !rec.actions.is_empty() {
            out.push((obj_path.to_owned(), rec.actions.clone()));
        }
    }
}

fn collect_flag_actions_recursive(
    obj: &DisplayObject,
    obj_path: &str,
    flag: u32,
    children_first: bool,
    out: &mut Vec<(String, Vec<u8>)>,
) {
    if !children_first {
        for rec in &obj.clip_actions {
            if (rec.event_flags & flag) != 0 && !rec.actions.is_empty() {
                out.push((obj_path.to_owned(), rec.actions.clone()));
            }
        }
    }
    if let Some(ref child_tl) = obj.sprite_timeline {
        for (&cd, cobj) in &child_tl.display_list {
            let cpath = child_clip_path(obj_path, cd, cobj.name.as_deref());
            collect_flag_actions_recursive(cobj, &cpath, flag, children_first, out);
        }
    }
    if children_first {
        for rec in &obj.clip_actions {
            if (rec.event_flags & flag) != 0 && !rec.actions.is_empty() {
                out.push((obj_path.to_owned(), rec.actions.clone()));
            }
        }
    }
}

fn step_timeline_one_frame(
    tl: &mut Timeline,
    path: &str,
    swf_version: u8,
    dict: &BTreeMap<u16, Character>,
    bg: &mut Rgba,
    dirty: &mut bool,
    events: &mut FrameEvents,
) {
    let existing: Vec<(u16, u16)> = tl
        .display_list
        .iter()
        .map(|(&d, o)| (d, o.character_id))
        .collect();

    let mut frame_events = FrameEvents::default();
    if !tl.frame0_executed {
        execute_timeline_frame_tags(
            tl,
            0,
            path,
            swf_version,
            dict,
            bg,
            dirty,
            &mut frame_events,
            true,
        );
        tl.frame0_executed = true;
    } else if tl.playing && tl.frame_count > 1 {
        let next = (tl.current_frame.saturating_add(1)) % tl.frame_count;
        let saved_state: BTreeMap<u16, (u16, Option<String>, ButtonState, Option<Timeline>)> =
            if next == 0 {
                tl.display_list
                    .iter()
                    .map(|(&d, o)| {
                        (
                            d,
                            (
                                o.character_id,
                                o.dynamic_text.clone(),
                                o.button_state,
                                o.sprite_timeline.clone(),
                            ),
                        )
                    })
                    .collect()
            } else {
                BTreeMap::new()
            };
        if next == 0 {
            tl.display_list.clear();
            *dirty = true;
        }
        tl.current_frame = next;
        execute_timeline_frame_tags(
            tl,
            next,
            path,
            swf_version,
            dict,
            bg,
            dirty,
            &mut frame_events,
            true,
        );
        if next == 0 {
            for (d, obj) in &mut tl.display_list {
                if let Some((saved_cid, saved_text, saved_btn, saved_tl)) = saved_state.get(d) {
                    if *saved_cid == obj.character_id {
                        if saved_text.is_some() {
                            obj.dynamic_text.clone_from(saved_text);
                        }
                        obj.button_state = *saved_btn;
                        if saved_tl.is_some() {
                            obj.sprite_timeline.clone_from(saved_tl);
                        }
                    }
                }
            }
        }
    }

    // Advance surviving pre-existing nested DefineSprite timelines and collect enterFrame events
    let mut enter_frame_actions: Vec<(String, Vec<u8>)> = Vec::new();
    for (d, prev_cid) in existing {
        let Some(obj) = tl.display_list.get_mut(&d) else {
            continue;
        };
        if obj.character_id != prev_cid {
            continue;
        }
        let child_path = child_clip_path(path, d, obj.name.as_deref());
        if let Some(ref mut child_tl) = obj.sprite_timeline {
            let mut child_ev = FrameEvents::default();
            step_timeline_one_frame(
                child_tl,
                &child_path,
                swf_version,
                dict,
                bg,
                dirty,
                &mut child_ev,
            );
            events.start_sounds.extend(child_ev.start_sounds);
            events.stream_heads.extend(child_ev.stream_heads);
            events.stream_blocks.extend(child_ev.stream_blocks);
            enter_frame_actions.extend(child_ev.actions);
        }
        for rec in &obj.clip_actions {
            if (rec.event_flags & CLIP_EVENT_ENTER_FRAME) != 0 && !rec.actions.is_empty() {
                enter_frame_actions.push((child_path.clone(), rec.actions.clone()));
            }
        }
    }

    events.start_sounds.extend(frame_events.start_sounds);
    events.stream_heads.extend(frame_events.stream_heads);
    events.stream_blocks.extend(frame_events.stream_blocks);
    // If no unload actions were queued on this frame, enterFrame runs before parent frame DoAction;
    // otherwise frame_events.actions already ordered unload -> parent -> load.
    if enter_frame_actions.is_empty() {
        events.actions.extend(frame_events.actions);
    } else {
        events.actions.extend(enter_frame_actions);
        events.actions.extend(frame_events.actions);
    }
}

#[allow(clippy::too_many_arguments)]
fn seek_timeline(
    tl: &mut Timeline,
    path: &str,
    swf_version: u8,
    target_frame: u16,
    play_after: bool,
    dict: &BTreeMap<u16, Character>,
    bg: &mut Rgba,
    dirty: &mut bool,
    events: &mut FrameEvents,
) {
    let clamped = target_frame.min(tl.frame_count.saturating_sub(1));
    let saved_state: BTreeMap<u16, (u16, Option<String>, ButtonState, Option<Timeline>)> = tl
        .display_list
        .iter()
        .map(|(&d, o)| {
            (
                d,
                (
                    o.character_id,
                    o.dynamic_text.clone(),
                    o.button_state,
                    o.sprite_timeline.clone(),
                ),
            )
        })
        .collect();
    tl.display_list.clear();
    for f in 0..=clamped {
        let emit_actions = f == clamped;
        execute_timeline_frame_tags(
            tl,
            f,
            path,
            swf_version,
            dict,
            bg,
            dirty,
            events,
            emit_actions,
        );
    }
    for (d, obj) in &mut tl.display_list {
        if let Some((saved_cid, saved_text, saved_btn, saved_tl)) = saved_state.get(d) {
            if *saved_cid == obj.character_id {
                if saved_text.is_some() {
                    obj.dynamic_text.clone_from(saved_text);
                }
                obj.button_state = *saved_btn;
                if saved_tl.is_some() {
                    obj.sprite_timeline.clone_from(saved_tl);
                }
            }
        }
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
    swf_version: u8,
    dict: &BTreeMap<u16, Character>,
    bg: &mut Rgba,
    dirty: &mut bool,
    events: &mut FrameEvents,
    emit_actions: bool,
) {
    let Some(&(start, end)) = tl.frame_tag_ranges.get(usize::from(frame_idx)) else {
        return;
    };
    let Some(slice) = tl.tags.get(start..end).map(<[Tag]>::to_vec) else {
        return;
    };

    let mut pre_parent_actions: Vec<(String, Vec<u8>)> = Vec::new();
    let mut parent_actions: Vec<(String, Vec<u8>)> = Vec::new();
    let mut post_parent_actions: Vec<(String, Vec<u8>)> = Vec::new();

    for tag in &slice {
        match tag {
            Tag::SetBackgroundColor { r, g, b } => {
                *bg = Rgba::rgb(*r, *g, *b);
                *dirty = true;
            }
            Tag::PlaceObject(po) => {
                apply_place_object1(
                    tl,
                    po,
                    path,
                    swf_version,
                    dict,
                    bg,
                    dirty,
                    events,
                    &mut post_parent_actions,
                    emit_actions,
                );
                *dirty = true;
            }
            Tag::PlaceObject2(po2) | Tag::PlaceObject3(po2) => {
                apply_place_object2(
                    tl,
                    po2,
                    path,
                    swf_version,
                    dict,
                    bg,
                    dirty,
                    events,
                    &mut pre_parent_actions,
                    &mut post_parent_actions,
                    emit_actions,
                );
                *dirty = true;
            }
            Tag::RemoveObject { depth, .. } | Tag::RemoveObject2 { depth } => {
                if let Some(removed) = tl.display_list.remove(depth) {
                    if emit_actions {
                        let rpath = child_clip_path(path, *depth, removed.name.as_deref());
                        collect_unload_actions_recursive(&removed, &rpath, &mut pre_parent_actions);
                    }
                }
                *dirty = true;
            }
            Tag::DoInitAction { actions, .. } | Tag::DoAction { actions } => {
                if emit_actions && !actions.is_empty() {
                    parent_actions.push((path.to_owned(), actions.clone()));
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

    if emit_actions {
        events.actions.extend(pre_parent_actions);
        events.actions.extend(parent_actions);
        events.actions.extend(post_parent_actions);
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_place_object1(
    tl: &mut Timeline,
    po: &PlaceObjectTag,
    parent_path: &str,
    swf_version: u8,
    dict: &BTreeMap<u16, Character>,
    bg: &mut Rgba,
    dirty: &mut bool,
    events: &mut FrameEvents,
    post_parent_actions: &mut Vec<(String, Vec<u8>)>,
    emit_actions: bool,
) {
    let (dynamic_text, mut sprite_timeline) = init_instance_state(po.character_id, dict);
    if let Some(ref mut st) = sprite_timeline {
        let sp_path = format!("{parent_path}._d{}", po.depth);
        let mut child_ev = FrameEvents::default();
        execute_timeline_frame_tags(
            st,
            0,
            &sp_path,
            swf_version,
            dict,
            bg,
            dirty,
            &mut child_ev,
            emit_actions,
        );
        st.frame0_executed = true;
        events.start_sounds.extend(child_ev.start_sounds);
        events.stream_heads.extend(child_ev.stream_heads);
        events.stream_blocks.extend(child_ev.stream_blocks);
        post_parent_actions.extend(child_ev.actions);
    }
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
            clip_actions: Vec::new(),
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn apply_place_object2(
    tl: &mut Timeline,
    po2: &PlaceObject2Tag,
    parent_path: &str,
    swf_version: u8,
    dict: &BTreeMap<u16, Character>,
    bg: &mut Rgba,
    dirty: &mut bool,
    events: &mut FrameEvents,
    pre_parent_actions: &mut Vec<(String, Vec<u8>)>,
    post_parent_actions: &mut Vec<(String, Vec<u8>)>,
    emit_actions: bool,
) {
    if po2.is_move {
        let same_char = po2.character_id.is_none()
            || tl.display_list.get(&po2.depth).map(|o| o.character_id) == po2.character_id;
        if same_char {
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
                return;
            }
        }
    }

    let prev = tl.display_list.get(&po2.depth).cloned();
    if let Some(ref old_obj) = prev {
        if po2.character_id.is_some()
            && po2.character_id != Some(old_obj.character_id)
            && emit_actions
        {
            let old_path = child_clip_path(parent_path, po2.depth, old_obj.name.as_deref());
            collect_unload_actions_recursive(old_obj, &old_path, pre_parent_actions);
        }
    }

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
    let ratio = po2
        .ratio
        .or_else(|| prev.as_ref().map(|p| p.ratio))
        .unwrap_or(0);
    let name = po2
        .name
        .clone()
        .or_else(|| prev.as_ref().and_then(|p| p.name.clone()));
    let clip_depth = po2
        .clip_depth
        .or_else(|| prev.as_ref().and_then(|p| p.clip_depth));
    let clip_actions = match po2.clip_actions_raw.as_deref() {
        Some(raw) => decode_clip_actions(raw, swf_version),
        None => Vec::new(),
    };

    let sp_path = child_clip_path(parent_path, po2.depth, name.as_deref());

    let (dynamic_text, mut sprite_timeline) = init_instance_state(char_id, dict);
    let mut child_frame0_actions: Vec<(String, Vec<u8>)> = Vec::new();
    if let Some(ref mut st) = sprite_timeline {
        let mut child_ev = FrameEvents::default();
        execute_timeline_frame_tags(
            st,
            0,
            &sp_path,
            swf_version,
            dict,
            bg,
            dirty,
            &mut child_ev,
            emit_actions,
        );
        st.frame0_executed = true;
        events.start_sounds.extend(child_ev.start_sounds);
        events.stream_heads.extend(child_ev.stream_heads);
        events.stream_blocks.extend(child_ev.stream_blocks);
        child_frame0_actions = child_ev.actions;
    }

    let placed = DisplayObject {
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
        clip_actions,
    };

    if emit_actions {
        // 1. initialize (parent before child) and construct (parent before child) run before parent timeline DoAction
        collect_flag_actions_recursive(
            &placed,
            &sp_path,
            CLIP_EVENT_INITIALIZE,
            false,
            pre_parent_actions,
        );
        collect_flag_actions_recursive(
            &placed,
            &sp_path,
            CLIP_EVENT_CONSTRUCT,
            false,
            pre_parent_actions,
        );
        // 2. load handler runs after parent timeline's code and before the clip's own frame-0 code (TSD 4.6 item 10)
        for rec in &placed.clip_actions {
            if (rec.event_flags & CLIP_EVENT_LOAD) != 0 && !rec.actions.is_empty() {
                post_parent_actions.push((sp_path.clone(), rec.actions.clone()));
            }
        }
        // 3. Clip's own frame-0 code (and any nested child clip load + frame-0 code) runs after clip's load handler
        post_parent_actions.extend(child_frame0_actions);
    }

    tl.display_list.insert(po2.depth, placed);
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
    let Some((bx, by)) =
        invert_matrix_point(btn_matrix, stage_x_twips as f32, stage_y_twips as f32)
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
                if lx < (shape.bounds.x_min as f32)
                    || lx > (shape.bounds.x_max as f32)
                    || ly < (shape.bounds.y_min as f32)
                    || ly > (shape.bounds.y_max as f32)
                {
                    continue;
                }
                let flat = flatten_shape(shape);
                if point_in_flattened_shape(&flat, lx, ly) {
                    return true;
                }
            }
            Some(Character::MorphShape(morph)) => {
                if lx < (morph.start_bounds.x_min as f32)
                    || lx > (morph.start_bounds.x_max as f32)
                    || ly < (morph.start_bounds.y_min as f32)
                    || ly > (morph.start_bounds.y_max as f32)
                {
                    continue;
                }
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
