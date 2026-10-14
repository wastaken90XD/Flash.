//! Display list, character dictionary, shape flattening, timeline engine, and button hit-testing
//! for `swfr` (TSD 1.6, 2.12 M2 & M3).
#![allow(
    clippy::arithmetic_side_effects,
    clippy::collapsible_if,
    clippy::collapsible_match
)]

use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;
use std::sync::Arc;
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
    /// Pre-computed premultiplied RGBA8 pixels (`width * height * 4` bytes) for zero-copy rendering
    /// when `ColorTransform::IDENTITY` is active.
    pub premultiplied_rgba: Vec<u8>,
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
        /// Shared nested timeline tags.
        tags: Arc<[Tag]>,
        /// Pre-computed `(start_idx, end_idx)` tag slice ranges per frame.
        frame_tag_ranges: Arc<[(usize, usize)]>,
        /// Pre-computed lowercase frame label -> 0-based frame index map.
        frame_labels: Arc<BTreeMap<String, u16>>,
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
    /// Visibility flag (`_visible`, default `true`).
    pub visible: bool,
}

impl DisplayObject {
    /// Returns the horizontal translation in pixels (`_x`).
    #[must_use]
    pub fn x_px(&self) -> f64 {
        f64::from(self.matrix.translate_x_twips) / 20.0
    }

    /// Sets the horizontal translation from pixels (`_x`).
    pub fn set_x_px(&mut self, x: f64) {
        if x.is_finite() {
            self.matrix.translate_x_twips = (x * 20.0).round() as i32;
        }
    }

    /// Returns the vertical translation in pixels (`_y`).
    #[must_use]
    pub fn y_px(&self) -> f64 {
        f64::from(self.matrix.translate_y_twips) / 20.0
    }

    /// Sets the vertical translation from pixels (`_y`).
    pub fn set_y_px(&mut self, y: f64) {
        if y.is_finite() {
            self.matrix.translate_y_twips = (y * 20.0).round() as i32;
        }
    }

    /// Returns the horizontal scale percentage (`_xscale`, where `100.0` is `1.0x`).
    #[must_use]
    pub fn xscale_pct(&self) -> f64 {
        let a = f64::from(self.matrix.scale_x_fp16) / 65536.0;
        let b = f64::from(self.matrix.rotate_skew0_fp16) / 65536.0;
        let mag = a.hypot(b);
        let sign = if a < 0.0 && b == 0.0 { -1.0 } else { 1.0 };
        sign * mag * 100.0
    }

    /// Sets the horizontal scale percentage (`_xscale`).
    pub fn set_xscale_pct(&mut self, pct: f64) {
        if !pct.is_finite() {
            return;
        }
        let rot_rad = self.rotation_deg().to_radians();
        let s = pct / 100.0;
        self.matrix.scale_x_fp16 = (s * rot_rad.cos() * 65536.0).round() as i32;
        self.matrix.rotate_skew0_fp16 = (s * rot_rad.sin() * 65536.0).round() as i32;
    }

    /// Returns the vertical scale percentage (`_yscale`, where `100.0` is `1.0x`).
    #[must_use]
    pub fn yscale_pct(&self) -> f64 {
        let c = f64::from(self.matrix.rotate_skew1_fp16) / 65536.0;
        let d = f64::from(self.matrix.scale_y_fp16) / 65536.0;
        let mag = c.hypot(d);
        let sign = if d < 0.0 && c == 0.0 { -1.0 } else { 1.0 };
        sign * mag * 100.0
    }

    /// Sets the vertical scale percentage (`_yscale`).
    pub fn set_yscale_pct(&mut self, pct: f64) {
        if !pct.is_finite() {
            return;
        }
        let rot_rad = self.rotation_deg().to_radians();
        let s = pct / 100.0;
        self.matrix.rotate_skew1_fp16 = (-s * rot_rad.sin() * 65536.0).round() as i32;
        self.matrix.scale_y_fp16 = (s * rot_rad.cos() * 65536.0).round() as i32;
    }

    /// Returns the rotation angle in degrees (`_rotation`).
    #[must_use]
    pub fn rotation_deg(&self) -> f64 {
        let a = f64::from(self.matrix.scale_x_fp16) / 65536.0;
        let b = f64::from(self.matrix.rotate_skew0_fp16) / 65536.0;
        b.atan2(a).to_degrees()
    }

    /// Sets the rotation angle in degrees (`_rotation`), preserving `_xscale` and `_yscale`.
    pub fn set_rotation_deg(&mut self, deg: f64) {
        if !deg.is_finite() {
            return;
        }
        let sx = self.xscale_pct() / 100.0;
        let sy = self.yscale_pct() / 100.0;
        let rad = deg.to_radians();
        let (sin_r, cos_r) = rad.sin_cos();
        self.matrix.scale_x_fp16 = (sx * cos_r * 65536.0).round() as i32;
        self.matrix.rotate_skew0_fp16 = (sx * sin_r * 65536.0).round() as i32;
        self.matrix.rotate_skew1_fp16 = (-sy * sin_r * 65536.0).round() as i32;
        self.matrix.scale_y_fp16 = (sy * cos_r * 65536.0).round() as i32;
    }

    /// Returns the alpha transparency percentage (`_alpha`, `0.0..=100.0`).
    #[must_use]
    pub fn alpha_pct(&self) -> f64 {
        (f64::from(self.color_transform.a_mult) * 100.0) / 256.0
    }

    /// Sets the alpha transparency percentage (`_alpha`).
    pub fn set_alpha_pct(&mut self, pct: f64) {
        if pct.is_finite() {
            self.color_transform.a_mult =
                ((pct * 256.0) / 100.0).round().clamp(-32768.0, 32767.0) as i16;
        }
    }
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

/// Flattens a font glyph's [`ShapeRecord`] slice into a single [`Vec<PathCmd>`] contour list.
#[must_use]
pub fn flatten_glyph_records(records: &[ShapeRecord]) -> Vec<PathCmd> {
    let flat = flatten_records(
        Rect::default(),
        &[FillStyle::Solid(Rgba::rgb(255, 255, 255))],
        &[],
        records,
    );
    let mut out = Vec::new();
    for fill in flat.fills {
        out.extend(fill.cmds);
    }
    out
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
                let mut end_move = *move_to;
                if move_to.is_some() {
                    for erec in end_iter.by_ref() {
                        if let ShapeRecord::StyleChange {
                            move_to: Some(em), ..
                        } = erec
                        {
                            end_move = Some(*em);
                            break;
                        }
                    }
                }
                let interp_move = match (*move_to, end_move) {
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
                let mut end_edge = (*dx_twips, *dy_twips, None);
                for erec in end_iter.by_ref() {
                    match erec {
                        ShapeRecord::StraightEdge {
                            dx_twips: edx,
                            dy_twips: edy,
                        } => {
                            end_edge = (*edx, *edy, None);
                            break;
                        }
                        ShapeRecord::CurvedEdge {
                            control_dx_twips,
                            control_dy_twips,
                            anchor_dx_twips,
                            anchor_dy_twips,
                        } => {
                            end_edge = (
                                *control_dx_twips,
                                *control_dy_twips,
                                Some((*anchor_dx_twips, *anchor_dy_twips)),
                            );
                            break;
                        }
                        ShapeRecord::StyleChange { .. } => {}
                    }
                }
                if let (ecx, ecy, Some((eax, eay))) = end_edge {
                    let hdx = *dx_twips / 2;
                    let hdy = *dy_twips / 2;
                    records.push(ShapeRecord::CurvedEdge {
                        control_dx_twips: lerp_i32(hdx, ecx),
                        control_dy_twips: lerp_i32(hdy, ecy),
                        anchor_dx_twips: lerp_i32(dx_twips.saturating_sub(hdx), eax),
                        anchor_dy_twips: lerp_i32(dy_twips.saturating_sub(hdy), eay),
                    });
                } else {
                    records.push(ShapeRecord::StraightEdge {
                        dx_twips: lerp_i32(*dx_twips, end_edge.0),
                        dy_twips: lerp_i32(*dy_twips, end_edge.1),
                    });
                }
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
    if edges.is_empty() {
        return Vec::new();
    }
    let mut start_map: BTreeMap<(i32, i32), (usize, Vec<usize>)> = BTreeMap::new();
    for (idx, e) in edges.iter().enumerate() {
        start_map.entry((e.x0, e.y0)).or_default().1.push(idx);
    }

    let mut used = vec![false; edges.len()];
    let mut cmds = Vec::with_capacity(edges.len().saturating_add(4));

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
            if let Some((cursor, bucket)) = start_map.get_mut(&cur_pt) {
                while let Some(&cand_idx) = bucket.get(*cursor) {
                    *cursor += 1;
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
    let b = shape.bounds;
    if b.x_min < b.x_max && b.y_min < b.y_max {
        if px_twips < (b.x_min as f32)
            || px_twips > (b.x_max as f32)
            || py_twips < (b.y_min as f32)
            || py_twips > (b.y_max as f32)
        {
            return false;
        }
    }
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
                    let pc = (cx as f32, cy as f32);
                    let p2 = (ax as f32, ay as f32);
                    let mut prev = p0;
                    for step in 1..=8 {
                        let t = (step as f32) / 8.0;
                        let omt = 1.0 - t;
                        let qx = omt * omt * p0.0 + 2.0 * omt * t * pc.0 + t * t * p2.0;
                        let qy = omt * omt * p0.1 + 2.0 * omt * t * pc.1 + t * t * p2.1;
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

#[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
fn build_premultiplied_rgba(rgba: &[u8]) -> Vec<u8> {
    let mut out = vec![0_u8; rgba.len()];
    for (src, dst) in rgba.chunks_exact(4).zip(out.chunks_exact_mut(4)) {
        let a = u16::from(src[3]);
        if a == 255 {
            dst.copy_from_slice(src);
        } else if a > 0 {
            dst[0] = ((u16::from(src[0]) * a + 127) / 255) as u8;
            dst[1] = ((u16::from(src[1]) * a + 127) / 255) as u8;
            dst[2] = ((u16::from(src[2]) * a + 127) / 255) as u8;
            dst[3] = src[3];
        }
    }
    out
}

/// Decodes a `DefineBitsLossless` / `DefineBitsLossless2` tag into unpremultiplied & premultiplied RGBA8 pixels.
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
                        let (ur, ug, ub) = unpremultiply(r, g, b, a);
                        rgba.extend_from_slice(&[ur, ug, ub, a]);
                    } else {
                        rgba.extend_from_slice(&[0, 0, 0, 0]);
                    }
                }
            }
        }
        4 => {
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

    let premultiplied_rgba = build_premultiplied_rgba(&rgba);
    Some(DecodedBitmap {
        id: bmp.id,
        width: u32::from(bmp.width),
        height: u32::from(bmp.height),
        rgba,
        premultiplied_rgba,
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
    if image_data.starts_with(b"\x89PNG\r\n\x1a\n") {
        let decoder = png::Decoder::new(Cursor::new(image_data));
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
            _ => return None,
        }
        let premultiplied_rgba = build_premultiplied_rgba(&rgba);
        return Some(DecodedBitmap {
            id,
            width: info.width,
            height: info.height,
            rgba,
            premultiplied_rgba,
        });
    }

    let combined = clean_swf_jpeg_stream(jpeg_tables, image_data);
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

    let premultiplied_rgba = build_premultiplied_rgba(&rgba);
    Some(DecodedBitmap {
        id,
        width,
        height,
        rgba,
        premultiplied_rgba,
    })
}

#[allow(clippy::indexing_slicing)]
fn clean_swf_jpeg_stream(jpeg_tables: Option<&[u8]>, image_data: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(
        jpeg_tables
            .map_or(0, <[u8]>::len)
            .saturating_add(image_data.len()),
    );
    if let Some(tables) = jpeg_tables {
        if !tables.is_empty() {
            raw.extend_from_slice(tables);
        }
    }
    raw.extend_from_slice(image_data);

    // Strip any leading erroneous `FF D9 FF D8` or `FF D8 FF D9` before the real stream SOI (`FF D8`).
    let mut slice = raw.as_slice();
    while slice.len() >= 4
        && (slice.starts_with(&[0xFF, 0xD9, 0xFF, 0xD8])
            || slice.starts_with(&[0xFF, 0xD8, 0xFF, 0xD9]))
    {
        slice = &slice[2..];
    }

    // Remove interior `FF D9 FF D8` / `FF D8 FF D9` sequences emitted between the DQT/DHT tables
    // and the image SOF0/SOS segments by Flash authoring tools.
    let mut cleaned = Vec::with_capacity(slice.len());
    let mut i = 0_usize;
    while i < slice.len() {
        if i > 0
            && i + 4 <= slice.len()
            && ((slice[i] == 0xFF
                && slice[i + 1] == 0xD9
                && slice[i + 2] == 0xFF
                && slice[i + 3] == 0xD8)
                || (slice[i] == 0xFF
                    && slice[i + 1] == 0xD8
                    && slice[i + 2] == 0xFF
                    && slice[i + 3] == 0xD9))
        {
            i += 4;
            continue;
        }
        cleaned.push(slice[i]);
        i += 1;
    }
    cleaned
}

fn scan_timeline_tags(tags: &[Tag]) -> (Vec<(usize, usize)>, BTreeMap<String, u16>) {
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
    (frame_tag_ranges, frame_labels)
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
    /// Shared tag stream for this timeline.
    pub tags: Arc<[Tag]>,
    /// Start/end tag index for each 0-based frame (`frame_tag_ranges[f] = (start_idx, end_idx)`).
    pub frame_tag_ranges: Arc<[(usize, usize)]>,
    /// Lowercase frame label -> 0-based frame index map.
    pub frame_labels: Arc<BTreeMap<String, u16>>,
    /// Active display list keyed by `u16` depth.
    pub display_list: BTreeMap<u16, DisplayObject>,
    /// Whether frame 0's tags have been executed since instantiation or loop wrap.
    pub frame0_executed: bool,
}

impl Timeline {
    /// Constructs a new [`Timeline`] from a tag stream and declared frame count.
    #[must_use]
    pub fn new(frame_count: u16, tags: Vec<Tag>) -> Self {
        let (ranges, labels) = scan_timeline_tags(&tags);
        Self::from_shared(
            frame_count,
            Arc::from(tags),
            Arc::from(ranges),
            Arc::new(labels),
        )
    }

    /// Constructs a new [`Timeline`] from pre-computed shared `Arc` slices (`O(1)` clone for sprites).
    #[must_use]
    pub fn from_shared(
        frame_count: u16,
        tags: Arc<[Tag]>,
        frame_tag_ranges: Arc<[(usize, usize)]>,
        frame_labels: Arc<BTreeMap<String, u16>>,
    ) -> Self {
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
    /// Queued button/clip state transitions (`(clip_path, cond_mask)`) for AS1/AS2 `onRelease`/`onPress` handlers.
    pub button_transitions: Vec<(String, u16)>,
    /// Queued event sound triggers (`(sound_id, SoundInfo)`).
    pub start_sounds: Vec<(u16, SoundInfo)>,
    /// Queued streaming audio header updates.
    pub stream_heads: Vec<SoundStreamHeadDef>,
    /// Queued streaming audio frame blocks.
    pub stream_blocks: Vec<Vec<u8>>,
}

/// Complete scene stage owning the character dictionary, root timeline, background color,
/// pre-flattened vector caches, and dirty-rectangle list.
#[derive(Debug, Clone)]
pub struct Stage {
    /// SWF format version from header (`1..=43`).
    pub swf_version: u8,
    /// Uncompressed SWF file length in bytes (for `getBytesLoaded()` / `getBytesTotal()`).
    pub swf_byte_len: u32,
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
    /// Pre-flattened static shapes keyed by `u16` character ID.
    pub flattened_shapes: BTreeMap<u16, Arc<FlattenedShape>>,
    /// Pre-flattened font glyph contours keyed by `(font_id, glyph_index)`.
    pub flattened_glyphs: BTreeMap<(u16, usize), Arc<[PathCmd]>>,
    /// Exported symbol linkage table (`ExportAssets`, lowercase name -> `character_id`).
    pub exports: BTreeMap<String, u16>,
    /// Set of `DoInitAction` sprite IDs that have already executed once.
    pub executed_init_actions: BTreeSet<u16>,
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
    /// Next synthetic character ID for `createEmptyMovieClip`.
    next_synthetic_id: u16,
}

impl Stage {
    /// Creates a new [`Stage`] from a parsed [`Movie`], pre-populating dictionary definitions
    /// and pre-flattening all static shapes and font glyphs once at load time.
    #[must_use]
    pub fn from_movie(movie: &Movie) -> Self {
        let width_px = movie.header.width_px();
        let height_px = movie.header.height_px();
        let fps = movie.header.fps();
        let swf_byte_len = movie.decompressed_len.max(1024);
        let mut stage = Self {
            swf_version: movie.header.version,
            swf_byte_len,
            width_px,
            height_px,
            fps,
            background_color: Rgba::rgb(255, 255, 255),
            dictionary: BTreeMap::new(),
            flattened_shapes: BTreeMap::new(),
            flattened_glyphs: BTreeMap::new(),
            exports: BTreeMap::new(),
            executed_init_actions: BTreeSet::new(),
            jpeg_tables: None,
            font_codes: BTreeMap::new(),
            root: Timeline::new(movie.header.frame_count, movie.tags.clone()),
            dirty_rects: Vec::new(),
            full_redraw: true,
            pointer_twips: (-10_000, -10_000),
            pointer_down: false,
            next_synthetic_id: 60_000,
        };
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
                    let flat = Arc::new(flatten_shape(&shape));
                    self.flattened_shapes.insert(shape.id, flat);
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
                    let mut font = f.clone();
                    if font.codes.is_empty() {
                        if let Some(codes) = self.font_codes.get(&font.id) {
                            font.codes.clone_from(codes);
                        }
                    }
                    for (g_idx, recs) in font.glyphs.iter().enumerate() {
                        let cmds: Arc<[PathCmd]> = Arc::from(flatten_glyph_records(recs));
                        self.flattened_glyphs.insert((font.id, g_idx), cmds);
                    }
                    self.dictionary.insert(font.id, Character::Font(font));
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
                    let (ranges, labels) = scan_timeline_tags(nested);
                    self.dictionary.insert(
                        *id,
                        Character::Sprite {
                            id: *id,
                            frame_count: *frame_count,
                            tags: Arc::from(nested.as_slice()),
                            frame_tag_ranges: Arc::from(ranges),
                            frame_labels: Arc::new(labels),
                        },
                    );
                }
                Tag::DefineSound(snd) => {
                    self.dictionary
                        .insert(snd.id, Character::Sound(snd.clone()));
                }
                Tag::ExportAssets { exports } => {
                    for (cid, name) in exports {
                        self.exports.insert(name.clone(), *cid);
                        self.exports.insert(name.to_ascii_lowercase(), *cid);
                    }
                }
                _ => {}
            }
        }
    }

    /// Resolves a relative or absolute Flash target path (`_root`, `/a/b`, `../c`, `child`)
    /// against `base_path` into a canonical `_root` dot-separated path.
    #[must_use]
    pub fn resolve_target_path(base_path: &str, target: &str) -> String {
        let trimmed = target.trim();
        if trimmed.is_empty() || trimmed == "this" || trimmed == "." {
            return if base_path.is_empty() {
                "_root".to_owned()
            } else {
                base_path.to_owned()
            };
        }
        if trimmed == "/" || trimmed == "_root" || trimmed == "_level0" {
            return "_root".to_owned();
        }

        let is_abs = trimmed.starts_with('/')
            || trimmed == "_root"
            || trimmed.starts_with("_root.")
            || trimmed.starts_with("_root/")
            || trimmed == "_level0"
            || trimmed.starts_with("_level0.")
            || trimmed.starts_with("_level0/");

        let mut parts: Vec<&str> = if is_abs || base_path.is_empty() || base_path == "/" {
            vec!["_root"]
        } else {
            base_path.split('.').collect()
        };

        let normalized = trimmed
            .trim_start_matches("_root.")
            .trim_start_matches("_root/")
            .trim_start_matches("_level0.")
            .trim_start_matches("_level0/")
            .trim_start_matches('/');

        for token in normalized.split(['.', '/']) {
            match token {
                "" | "." | "this" => {}
                "_root" | "_level0" => {
                    parts.clear();
                    parts.push("_root");
                }
                ".." | "_parent" => {
                    if parts.len() > 1 {
                        parts.pop();
                    }
                }
                seg => parts.push(seg),
            }
        }
        parts.join(".")
    }

    /// Finds the [`Timeline`] at `target_path` (`_root` or any nested `DefineSprite` path).
    #[must_use]
    pub fn find_timeline(&self, target_path: &str) -> Option<&Timeline> {
        let canon = Self::resolve_target_path("_root", target_path);
        if canon == "_root" {
            return Some(&self.root);
        }
        let mut cur = &self.root;
        for seg in canon.strip_prefix("_root.")?.split('.') {
            let obj = find_child_in_timeline(cur, seg)?;
            cur = obj.sprite_timeline.as_ref()?;
        }
        Some(cur)
    }

    /// Finds the mutable [`Timeline`] at `target_path` (`_root` or any nested `DefineSprite` path).
    pub fn find_timeline_mut(&mut self, target_path: &str) -> Option<&mut Timeline> {
        let canon = Self::resolve_target_path("_root", target_path);
        if canon == "_root" {
            return Some(&mut self.root);
        }
        let mut cur = &mut self.root;
        for seg in canon.strip_prefix("_root.")?.split('.') {
            let obj = find_child_in_timeline_mut(cur, seg)?;
            cur = obj.sprite_timeline.as_mut()?;
        }
        Some(cur)
    }

    /// Finds the [`DisplayObject`] at `target_path` (e.g. `_root.hero` or `_root.panel.btn`).
    #[must_use]
    pub fn find_display_object(&self, target_path: &str) -> Option<&DisplayObject> {
        let canon = Self::resolve_target_path("_root", target_path);
        let (parent_path, child_seg) = canon.rsplit_once('.')?;
        let parent_tl = self.find_timeline(parent_path)?;
        find_child_in_timeline(parent_tl, child_seg)
    }

    /// Finds the mutable [`DisplayObject`] at `target_path`.
    pub fn find_display_object_mut(&mut self, target_path: &str) -> Option<&mut DisplayObject> {
        let canon = Self::resolve_target_path("_root", target_path);
        let (parent_path, child_seg) = canon.rsplit_once('.')?;
        let parent_tl = self.find_timeline_mut(parent_path)?;
        find_child_in_timeline_mut(parent_tl, child_seg)
    }

    /// Checks if `parent_path` has a placed child display object matching `child_name`,
    /// returning its canonical path (e.g. `"_root.hero"`).
    #[must_use]
    pub fn resolve_child_clip(&self, parent_path: &str, child_name: &str) -> Option<String> {
        let canon_parent = Self::resolve_target_path("_root", parent_path);
        let tl = self.find_timeline(&canon_parent)?;
        let obj = find_child_in_timeline(tl, child_name)?;
        Some(child_clip_path(
            &canon_parent,
            obj.depth,
            obj.name.as_deref(),
        ))
    }

    /// Returns the next available unused depth (`>= 0`) in the timeline at `target_path`.
    #[must_use]
    pub fn get_next_highest_depth(&self, target_path: &str) -> u16 {
        self.find_timeline(target_path)
            .and_then(|tl| {
                tl.display_list
                    .keys()
                    .next_back()
                    .map(|d| d.saturating_add(1))
            })
            .unwrap_or(1)
    }

    /// Creates an empty `MovieClip` at `(parent_path, name, depth)` and returns its canonical path.
    pub fn create_empty_movie_clip(&mut self, parent_path: &str, name: &str, depth: u16) -> String {
        let canon_parent = Self::resolve_target_path("_root", parent_path);
        let cid = self.next_synthetic_id;
        self.next_synthetic_id = self.next_synthetic_id.wrapping_add(1).max(60_000);
        self.dictionary.insert(
            cid,
            Character::Sprite {
                id: cid,
                frame_count: 1,
                tags: Arc::from([]),
                frame_tag_ranges: Arc::from([]),
                frame_labels: Arc::new(BTreeMap::new()),
            },
        );
        let child_path = format!("{canon_parent}.{name}");
        if let Some(tl) = self.find_timeline_mut(&canon_parent) {
            tl.display_list.insert(
                depth,
                DisplayObject {
                    character_id: cid,
                    depth,
                    matrix: Matrix::IDENTITY,
                    color_transform: ColorTransform::IDENTITY,
                    ratio: 0,
                    name: Some(name.to_owned()),
                    clip_depth: None,
                    button_state: ButtonState::Up,
                    button_pressed_inside: false,
                    dynamic_text: None,
                    sprite_timeline: Some(Timeline::new(1, Vec::new())),
                    clip_actions: Vec::new(),
                    visible: true,
                },
            );
        }
        self.mark_stage_dirty();
        child_path
    }

    /// Attaches an exported library symbol (`ExportAssets`) into `parent_path` at `depth`.
    pub fn attach_movie(
        &mut self,
        parent_path: &str,
        id_name: &str,
        new_name: &str,
        depth: u16,
    ) -> Option<(String, FrameEvents)> {
        let cid = self
            .exports
            .get(id_name)
            .or_else(|| self.exports.get(&id_name.to_ascii_lowercase()))
            .copied()?;
        let canon_parent = Self::resolve_target_path("_root", parent_path);
        let child_path = format!("{canon_parent}.{new_name}");
        let (dynamic_text, mut sprite_timeline) = init_instance_state(cid, &self.dictionary);
        let mut events = FrameEvents::default();
        let mut bg = self.background_color;
        let mut dirty = false;
        if let Some(ref mut st) = sprite_timeline {
            execute_timeline_frame_tags(
                st,
                0,
                &child_path,
                self.swf_version,
                &self.dictionary,
                &mut self.executed_init_actions,
                &mut bg,
                &mut dirty,
                &mut events,
                true,
            );
            st.frame0_executed = true;
        }
        if let Some(tl) = self.find_timeline_mut(&canon_parent) {
            tl.display_list.insert(
                depth,
                DisplayObject {
                    character_id: cid,
                    depth,
                    matrix: Matrix::IDENTITY,
                    color_transform: ColorTransform::IDENTITY,
                    ratio: 0,
                    name: Some(new_name.to_owned()),
                    clip_depth: None,
                    button_state: ButtonState::Up,
                    button_pressed_inside: false,
                    dynamic_text,
                    sprite_timeline,
                    clip_actions: Vec::new(),
                    visible: true,
                },
            );
        }
        self.background_color = bg;
        self.mark_stage_dirty();
        Some((child_path, events))
    }

    /// Duplicates the display object at `source_path` into the same parent timeline at `depth` with `new_name`.
    pub fn duplicate_movie_clip(
        &mut self,
        source_path: &str,
        new_name: &str,
        depth: u16,
    ) -> Option<String> {
        let canon_src = Self::resolve_target_path("_root", source_path);
        let (parent_path, child_seg) = canon_src.rsplit_once('.')?;
        let tl = self.find_timeline_mut(parent_path)?;
        let mut cloned = find_child_in_timeline(tl, child_seg)?.clone();
        cloned.depth = depth;
        cloned.name = Some(new_name.to_owned());
        if let Some(ref mut st) = cloned.sprite_timeline {
            st.current_frame = 0;
        }
        tl.display_list.insert(depth, cloned);
        self.mark_stage_dirty();
        Some(format!("{parent_path}.{new_name}"))
    }

    /// Removes the display object at `target_path` from its parent timeline.
    pub fn remove_movie_clip(&mut self, target_path: &str) -> bool {
        let canon = Self::resolve_target_path("_root", target_path);
        let Some((parent_path, child_seg)) = canon.rsplit_once('.') else {
            return false;
        };
        let Some(tl) = self.find_timeline_mut(parent_path) else {
            return false;
        };
        let Some(depth) = find_child_in_timeline(tl, child_seg).map(|o| o.depth) else {
            return false;
        };
        let removed = tl.display_list.remove(&depth).is_some();
        if removed {
            self.mark_stage_dirty();
        }
        removed
    }

    /// Swaps the depth of the clip at `target_path` with `new_depth` (or with whatever clip is at `new_depth`).
    pub fn swap_depths(&mut self, target_path: &str, new_depth: u16) -> bool {
        let canon = Self::resolve_target_path("_root", target_path);
        let Some((parent_path, child_seg)) = canon.rsplit_once('.') else {
            return false;
        };
        let Some(tl) = self.find_timeline_mut(parent_path) else {
            return false;
        };
        let Some(old_depth) = find_child_in_timeline(tl, child_seg).map(|o| o.depth) else {
            return false;
        };
        if old_depth == new_depth {
            return true;
        }
        let Some(mut obj_a) = tl.display_list.remove(&old_depth) else {
            return false;
        };
        obj_a.depth = new_depth;
        if let Some(mut obj_b) = tl.display_list.remove(&new_depth) {
            obj_b.depth = old_depth;
            tl.display_list.insert(old_depth, obj_b);
        }
        tl.display_list.insert(new_depth, obj_a);
        self.mark_stage_dirty();
        true
    }

    /// Computes the approximate stage-space bounding box (in twips) of the clip at `target_path`.
    #[must_use]
    pub fn clip_bounds_stage_twips(&self, target_path: &str) -> Option<Rect> {
        let canon = Self::resolve_target_path("_root", target_path);
        if canon == "_root" {
            return Some(Rect {
                x_min: 0,
                x_max: (self.width_px as i32) * 20,
                y_min: 0,
                y_max: (self.height_px as i32) * 20,
            });
        }
        let mut world_matrix = Matrix::IDENTITY;
        let mut cur_tl = &self.root;
        let mut target_obj: Option<&DisplayObject> = None;
        for seg in canon.strip_prefix("_root.")?.split('.') {
            let obj = find_child_in_timeline(cur_tl, seg)?;
            world_matrix = concat_matrices(&world_matrix, &obj.matrix);
            target_obj = Some(obj);
            if let Some(ref st) = obj.sprite_timeline {
                cur_tl = st;
            }
        }
        let obj = target_obj?;
        let local_bounds = self.object_local_bounds(obj)?;
        Some(transform_rect(&world_matrix, local_bounds))
    }

    fn object_local_bounds(&self, obj: &DisplayObject) -> Option<Rect> {
        match self.dictionary.get(&obj.character_id)? {
            Character::Shape(s) => Some(s.bounds),
            Character::MorphShape(m) => Some(m.start_bounds),
            Character::Text(t) => Some(t.bounds),
            Character::EditText(e) => Some(e.bounds),
            Character::Bitmap(b) => Some(Rect {
                x_min: 0,
                x_max: (b.width as i32) * 20,
                y_min: 0,
                y_max: (b.height as i32) * 20,
            }),
            Character::Button(btn) => {
                let mut acc: Option<Rect> = None;
                for r in &btn.records {
                    if let Some(Character::Shape(s)) = self.dictionary.get(&r.character_id) {
                        let tr = transform_rect(&r.matrix, s.bounds);
                        acc = Some(match acc {
                            None => tr,
                            Some(prev) => union_rect(prev, tr),
                        });
                    }
                }
                acc
            }
            Character::Sprite { .. } => {
                let st = obj.sprite_timeline.as_ref()?;
                let mut acc: Option<Rect> = None;
                for child in st.display_list.values() {
                    if !child.visible {
                        continue;
                    }
                    if let Some(cb) = self.object_local_bounds(child) {
                        let tr = transform_rect(&child.matrix, cb);
                        acc = Some(match acc {
                            None => tr,
                            Some(prev) => union_rect(prev, tr),
                        });
                    }
                }
                acc.or(Some(Rect {
                    x_min: 0,
                    x_max: 0,
                    y_min: 0,
                    y_max: 0,
                }))
            }
            Character::Font(_) | Character::Sound(_) => None,
        }
    }

    /// Tests whether `(stage_x_twips, stage_y_twips)` hits the display object at `target_path`.
    #[must_use]
    pub fn hit_test_point(
        &self,
        target_path: &str,
        stage_x_twips: i32,
        stage_y_twips: i32,
        shape_flag: bool,
    ) -> bool {
        let Some(bounds) = self.clip_bounds_stage_twips(target_path) else {
            return false;
        };
        let in_bbox = stage_x_twips >= bounds.x_min
            && stage_x_twips <= bounds.x_max
            && stage_y_twips >= bounds.y_min
            && stage_y_twips <= bounds.y_max;
        if !shape_flag || !in_bbox {
            return in_bbox;
        }
        let canon = Self::resolve_target_path("_root", target_path);
        if canon == "_root" {
            return in_bbox;
        }
        let mut world_matrix = Matrix::IDENTITY;
        let mut cur_tl = &self.root;
        let mut target_obj: Option<&DisplayObject> = None;
        let Some(rel) = canon.strip_prefix("_root.") else {
            return in_bbox;
        };
        for seg in rel.split('.') {
            let Some(obj) = find_child_in_timeline(cur_tl, seg) else {
                return false;
            };
            world_matrix = concat_matrices(&world_matrix, &obj.matrix);
            target_obj = Some(obj);
            if let Some(ref st) = obj.sprite_timeline {
                cur_tl = st;
            }
        }
        let Some(obj) = target_obj else {
            return false;
        };
        self.hit_test_display_object_shape(
            obj,
            &world_matrix,
            stage_x_twips as f32,
            stage_y_twips as f32,
        )
    }

    fn hit_test_display_object_shape(
        &self,
        obj: &DisplayObject,
        world_matrix: &Matrix,
        sx: f32,
        sy: f32,
    ) -> bool {
        if !obj.visible {
            return false;
        }
        match self.dictionary.get(&obj.character_id) {
            Some(Character::Shape(s)) => {
                let Some((lx, ly)) = invert_matrix_point(world_matrix, sx, sy) else {
                    return false;
                };
                if let Some(flat) = self.flattened_shapes.get(&obj.character_id) {
                    point_in_flattened_shape(flat, lx, ly)
                } else {
                    let flat = flatten_shape(s);
                    point_in_flattened_shape(&flat, lx, ly)
                }
            }
            Some(Character::Sprite { .. }) => {
                let Some(ref st) = obj.sprite_timeline else {
                    return false;
                };
                for child in st.display_list.values() {
                    let cm = concat_matrices(world_matrix, &child.matrix);
                    if self.hit_test_display_object_shape(child, &cm, sx, sy) {
                        return true;
                    }
                }
                false
            }
            _ => {
                let Some((lx, ly)) = invert_matrix_point(world_matrix, sx, sy) else {
                    return false;
                };
                if let Some(b) = self.object_local_bounds(obj) {
                    lx >= (b.x_min as f32)
                        && lx <= (b.x_max as f32)
                        && ly >= (b.y_min as f32)
                        && ly <= (b.y_max as f32)
                } else {
                    false
                }
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
            &mut self.executed_init_actions,
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

    /// Jumps the root timeline (or any nested sprite timeline at `target_path`) to `target_frame`
    /// (0-based) and reconstructs the display list from frame `0..=target_frame`.
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
        let canon = Self::resolve_target_path("_root", target_path);

        if canon == "_root" {
            seek_timeline(
                &mut self.root,
                "_root",
                swf_version,
                target_frame,
                play_after,
                &self.dictionary,
                &mut self.executed_init_actions,
                &mut bg,
                &mut dirty,
                &mut events,
            );
        } else if let Some(tl) = find_timeline_mut_in(&mut self.root, &canon) {
            seek_timeline(
                tl,
                &canon,
                swf_version,
                target_frame,
                play_after,
                &self.dictionary,
                &mut self.executed_init_actions,
                &mut bg,
                &mut dirty,
                &mut events,
            );
        }
        self.background_color = bg;
        self.full_redraw = true;
        events
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

    /// Updates pointer coordinates (in stage twips) and button state across the root and all
    /// nested `DefineSprite` timelines, returning any triggered button and clip mouse actions.
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

        update_buttons_recursive(
            &mut self.root,
            "_root",
            &Matrix::IDENTITY,
            &self.dictionary,
            &self.flattened_shapes,
            x_twips,
            y_twips,
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

    /// Synchronizes any `DefineEditText` instances (at root or inside nested sprites) whose
    /// `variable_name` matches `var_name`.
    pub fn sync_variable(&mut self, var_name: &str, value: &str) {
        let clean_target = var_name
            .trim_start_matches("_root.")
            .trim_start_matches("_level0.")
            .trim_start_matches('/');
        let mut changed = false;
        sync_variable_recursive(
            &mut self.root,
            "_root",
            &self.dictionary,
            clean_target,
            value,
            &mut changed,
        );
        if changed {
            self.mark_stage_dirty();
        }
    }

    /// Marks the entire stage as needing a redraw on the next frame.
    pub fn mark_stage_dirty(&mut self) {
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

fn find_child_in_timeline<'a>(tl: &'a Timeline, seg: &str) -> Option<&'a DisplayObject> {
    if let Some(depth_str) = seg.strip_prefix("_d") {
        if let Ok(d) = depth_str.parse::<u16>() {
            if let Some(obj) = tl.display_list.get(&d) {
                return Some(obj);
            }
        }
    }
    tl.display_list.values().find(|o| {
        o.name
            .as_deref()
            .is_some_and(|n| n == seg || n.eq_ignore_ascii_case(seg))
    })
}

fn find_child_in_timeline_mut<'a>(
    tl: &'a mut Timeline,
    seg: &str,
) -> Option<&'a mut DisplayObject> {
    if let Some(depth_str) = seg.strip_prefix("_d") {
        if let Ok(d) = depth_str.parse::<u16>() {
            if tl.display_list.contains_key(&d) {
                return tl.display_list.get_mut(&d);
            }
        }
    }
    let matching_depth = tl
        .display_list
        .iter()
        .find(|(_, o)| {
            o.name
                .as_deref()
                .is_some_and(|n| n == seg || n.eq_ignore_ascii_case(seg))
        })
        .map(|(&d, _)| d)?;
    tl.display_list.get_mut(&matching_depth)
}

fn find_timeline_mut_in<'a>(root: &'a mut Timeline, canon: &str) -> Option<&'a mut Timeline> {
    if canon == "_root" {
        return Some(root);
    }
    let mut cur = root;
    for seg in canon.strip_prefix("_root.")?.split('.') {
        let obj = find_child_in_timeline_mut(cur, seg)?;
        cur = obj.sprite_timeline.as_mut()?;
    }
    Some(cur)
}

#[allow(clippy::too_many_arguments)]
fn update_buttons_recursive(
    tl: &mut Timeline,
    parent_path: &str,
    parent_matrix: &Matrix,
    dict: &BTreeMap<u16, Character>,
    flattened_shapes: &BTreeMap<u16, Arc<FlattenedShape>>,
    x_twips: i32,
    y_twips: i32,
    was_down: bool,
    pressed: bool,
    changed: &mut bool,
    events: &mut FrameEvents,
) {
    for (&depth, obj) in &mut tl.display_list {
        if !obj.visible {
            continue;
        }
        let world_matrix = concat_matrices(parent_matrix, &obj.matrix);
        let obj_path = child_clip_path(parent_path, depth, obj.name.as_deref());

        if let Some(Character::Button(btn)) = dict.get(&obj.character_id) {
            let inside = hit_test_button_with_cache(
                btn,
                &world_matrix,
                dict,
                flattened_shapes,
                x_twips,
                y_twips,
            );
            let prev_state = obj.button_state;
            let prev_pressed_in = obj.button_pressed_inside;

            let (new_state, cond_mask) = match (prev_state, inside, was_down, pressed) {
                (ButtonState::Up, true, false, false) => (ButtonState::Over, 0x0001),
                (ButtonState::Up, true, true, true) if prev_pressed_in => {
                    (ButtonState::Down, 0x0020)
                }
                (ButtonState::Up, true, false, true) => {
                    obj.button_pressed_inside = true;
                    (ButtonState::Down, 0x0001 | 0x0004 | 0x0080)
                }
                (ButtonState::Up, true, true, true) if btn.track_as_menu => {
                    (ButtonState::Down, 0x0080)
                }
                (ButtonState::Over, false, _, false) => (ButtonState::Up, 0x0002),
                (ButtonState::Over, true, false, true) => {
                    obj.button_pressed_inside = true;
                    (ButtonState::Down, 0x0004)
                }
                (ButtonState::Down, true, true, false) => {
                    obj.button_pressed_inside = false;
                    (ButtonState::Over, 0x0008)
                }
                (ButtonState::Down, false, true, true) => (ButtonState::Up, 0x0010),
                (ButtonState::Down, false, true, false) => {
                    obj.button_pressed_inside = false;
                    (ButtonState::Up, 0x0040 | 0x0100)
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
                events
                    .button_transitions
                    .push((obj_path.clone(), cond_mask));
                for act in &btn.actions {
                    let matches = if btn.version == 1 {
                        (cond_mask & 0x0008) != 0
                    } else {
                        (act.condition & cond_mask) != 0
                    };
                    if matches && !act.actions.is_empty() {
                        events
                            .actions
                            .push((parent_path.to_owned(), act.actions.clone()));
                    }
                }
            }
        }

        if let Some(ref mut child_tl) = obj.sprite_timeline {
            update_buttons_recursive(
                child_tl,
                &obj_path,
                &world_matrix,
                dict,
                flattened_shapes,
                x_twips,
                y_twips,
                was_down,
                pressed,
                changed,
                events,
            );
        }
    }
}

fn sync_variable_recursive(
    tl: &mut Timeline,
    parent_path: &str,
    dict: &BTreeMap<u16, Character>,
    clean_target: &str,
    value: &str,
    changed: &mut bool,
) {
    let rel_parent = parent_path.strip_prefix("_root.").unwrap_or("");
    for (&depth, obj) in &mut tl.display_list {
        if let Some(Character::EditText(edit)) = dict.get(&obj.character_id) {
            let clean_var = edit
                .variable_name
                .trim_start_matches("_root.")
                .trim_start_matches("_level0.")
                .trim_start_matches('/');
            let qualified = if rel_parent.is_empty() {
                clean_var.to_owned()
            } else {
                format!("{rel_parent}.{clean_var}")
            };
            if !clean_var.is_empty()
                && (clean_var == clean_target
                    || clean_var.eq_ignore_ascii_case(clean_target)
                    || qualified.eq_ignore_ascii_case(clean_target))
            {
                if obj.dynamic_text.as_deref() != Some(value) {
                    obj.dynamic_text = Some(value.to_owned());
                    *changed = true;
                }
            }
        }
        // Also support `txtInstance.text = value` when `clean_target` is `<instance>.text`
        if let Some(inst_prefix) = clean_target.strip_suffix(".text") {
            let obj_path = child_clip_path(parent_path, depth, obj.name.as_deref());
            let rel_obj = obj_path.strip_prefix("_root.").unwrap_or(&obj_path);
            if rel_obj.eq_ignore_ascii_case(inst_prefix)
                || obj
                    .name
                    .as_deref()
                    .is_some_and(|n| n.eq_ignore_ascii_case(inst_prefix))
            {
                if obj.dynamic_text.as_deref() != Some(value) {
                    obj.dynamic_text = Some(value.to_owned());
                    *changed = true;
                }
            }
        }
        if let Some(ref mut child_tl) = obj.sprite_timeline {
            let child_path = child_clip_path(parent_path, depth, obj.name.as_deref());
            sync_variable_recursive(child_tl, &child_path, dict, clean_target, value, changed);
        }
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
        Some(n) if !n.is_empty() => format!("{parent_path}.{n}"),
        _ => format!("{parent_path}._d{depth}"),
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
        let cpath = child_clip_path(path, d, obj.name.as_deref());
        for rec in &obj.clip_actions {
            if (rec.event_flags & flag) != 0 && !rec.actions.is_empty() {
                events.actions.push((cpath.clone(), rec.actions.clone()));
            }
            if flag == CLIP_EVENT_KEY_DOWN && (rec.event_flags & CLIP_EVENT_KEY_PRESS) != 0 {
                if let (Some(expected), Some(actual)) = (rec.key_code, keycode) {
                    if u32::from(expected) == actual && !rec.actions.is_empty() {
                        events.actions.push((cpath.clone(), rec.actions.clone()));
                    }
                }
            }
        }
        if let Some(ref child_tl) = obj.sprite_timeline {
            collect_clip_input_events_recursive(child_tl, &cpath, flag, keycode, events);
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

type SavedInstanceMap = BTreeMap<
    u16,
    (
        u16,
        Option<String>,
        ButtonState,
        bool,
        Option<Timeline>,
        bool,
    ),
>;

fn save_display_list_state(tl: &Timeline) -> SavedInstanceMap {
    tl.display_list
        .iter()
        .map(|(&d, o)| {
            (
                d,
                (
                    o.character_id,
                    o.dynamic_text.clone(),
                    o.button_state,
                    o.button_pressed_inside,
                    o.sprite_timeline.clone(),
                    o.visible,
                ),
            )
        })
        .collect()
}

fn restore_display_list_state(tl: &mut Timeline, saved: &SavedInstanceMap) {
    for (d, obj) in &mut tl.display_list {
        if let Some((saved_cid, saved_text, saved_btn, saved_pressed, saved_tl, saved_vis)) =
            saved.get(d)
        {
            if *saved_cid == obj.character_id {
                if saved_text.is_some() {
                    obj.dynamic_text.clone_from(saved_text);
                }
                obj.button_state = *saved_btn;
                obj.button_pressed_inside = *saved_pressed;
                if saved_tl.is_some() {
                    obj.sprite_timeline.clone_from(saved_tl);
                }
                obj.visible = *saved_vis;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn step_timeline_one_frame(
    tl: &mut Timeline,
    path: &str,
    swf_version: u8,
    dict: &BTreeMap<u16, Character>,
    executed_init_actions: &mut BTreeSet<u16>,
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
            executed_init_actions,
            bg,
            dirty,
            &mut frame_events,
            true,
        );
        tl.frame0_executed = true;
    } else if tl.playing && tl.frame_count > 1 {
        let next = (tl.current_frame.saturating_add(1)) % tl.frame_count;
        let saved_state = if next == 0 {
            save_display_list_state(tl)
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
            executed_init_actions,
            bg,
            dirty,
            &mut frame_events,
            true,
        );
        if next == 0 {
            restore_display_list_state(tl, &saved_state);
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
                executed_init_actions,
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
    executed_init_actions: &mut BTreeSet<u16>,
    bg: &mut Rgba,
    dirty: &mut bool,
    events: &mut FrameEvents,
) {
    let clamped = target_frame.min(tl.frame_count.saturating_sub(1));
    let saved_state = save_display_list_state(tl);
    tl.display_list.clear();
    for f in 0..=clamped {
        let emit_actions = f == clamped;
        execute_timeline_frame_tags(
            tl,
            f,
            path,
            swf_version,
            dict,
            executed_init_actions,
            bg,
            dirty,
            events,
            emit_actions,
        );
    }
    restore_display_list_state(tl, &saved_state);
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
    executed_init_actions: &mut BTreeSet<u16>,
    bg: &mut Rgba,
    dirty: &mut bool,
    events: &mut FrameEvents,
    emit_actions: bool,
) {
    let Some(&(start, end)) = tl.frame_tag_ranges.get(usize::from(frame_idx)) else {
        return;
    };
    let tags = Arc::clone(&tl.tags);
    let Some(slice) = tags.get(start..end) else {
        return;
    };

    let mut init_actions: Vec<(String, Vec<u8>)> = Vec::new();
    let mut pre_parent_actions: Vec<(String, Vec<u8>)> = Vec::new();
    let mut parent_actions: Vec<(String, Vec<u8>)> = Vec::new();
    let mut post_parent_actions: Vec<(String, Vec<u8>)> = Vec::new();

    for tag in slice {
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
                    executed_init_actions,
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
                    executed_init_actions,
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
            Tag::DoInitAction { sprite_id, actions } => {
                if executed_init_actions.insert(*sprite_id) && !actions.is_empty() {
                    init_actions.push(("_root".to_owned(), actions.clone()));
                }
            }
            Tag::DoAction { actions } => {
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

    if !init_actions.is_empty() {
        events.actions.extend(init_actions);
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
    executed_init_actions: &mut BTreeSet<u16>,
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
            executed_init_actions,
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
            visible: true,
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
    executed_init_actions: &mut BTreeSet<u16>,
    bg: &mut Rgba,
    dirty: &mut bool,
    events: &mut FrameEvents,
    pre_parent_actions: &mut Vec<(String, Vec<u8>)>,
    post_parent_actions: &mut Vec<(String, Vec<u8>)>,
    emit_actions: bool,
) {
    // In-place move/modification when either `is_move && character_id.is_none()` OR
    // an instance with the same `character_id` already exists at `po2.depth` (tween keyframe).
    if let Some(existing) = tl.display_list.get_mut(&po2.depth) {
        let same_char =
            po2.character_id.is_none() || po2.character_id == Some(existing.character_id);
        if po2.is_move && same_char {
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
        None => prev
            .as_ref()
            .map_or_else(Vec::new, |p| p.clip_actions.clone()),
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
            executed_init_actions,
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
        visible: true,
    };

    if emit_actions {
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
        for rec in &placed.clip_actions {
            if (rec.event_flags & CLIP_EVENT_LOAD) != 0 && !rec.actions.is_empty() {
                post_parent_actions.push((sp_path.clone(), rec.actions.clone()));
            }
        }
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
            frame_count,
            tags,
            frame_tag_ranges,
            frame_labels,
            ..
        }) => (
            None,
            Some(Timeline::from_shared(
                *frame_count,
                Arc::clone(tags),
                Arc::clone(frame_tag_ranges),
                Arc::clone(frame_labels),
            )),
        ),
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
    hit_test_button_with_cache(
        btn,
        btn_matrix,
        dict,
        &BTreeMap::new(),
        stage_x_twips,
        stage_y_twips,
    )
}

#[allow(clippy::arithmetic_side_effects)]
fn hit_test_button_with_cache(
    btn: &ButtonDef,
    btn_matrix: &Matrix,
    dict: &BTreeMap<u16, Character>,
    flattened_shapes: &BTreeMap<u16, Arc<FlattenedShape>>,
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
                let sb = shape.bounds;
                if sb.x_min < sb.x_max
                    && sb.y_min < sb.y_max
                    && (lx < (sb.x_min as f32)
                        || lx > (sb.x_max as f32)
                        || ly < (sb.y_min as f32)
                        || ly > (sb.y_max as f32))
                {
                    continue;
                }
                if let Some(flat) = flattened_shapes.get(&rec.character_id) {
                    if point_in_flattened_shape(flat, lx, ly) {
                        return true;
                    }
                } else {
                    let flat = flatten_shape(shape);
                    if point_in_flattened_shape(&flat, lx, ly) {
                        return true;
                    }
                }
            }
            Some(Character::MorphShape(morph)) => {
                let flat = flatten_morph_shape(morph, 0);
                if point_in_flattened_shape(&flat, lx, ly) {
                    return true;
                }
            }
            Some(Character::Sprite { .. }) => {
                if let Some(Character::Sprite { tags, .. }) = dict.get(&rec.character_id) {
                    for tag in tags.iter() {
                        if let Tag::PlaceObject2(po2) = tag {
                            if let Some(cid) = po2.character_id {
                                if let Some(Character::Shape(s)) = dict.get(&cid) {
                                    if let Some(flat) = flattened_shapes.get(&cid) {
                                        if point_in_flattened_shape(flat, lx, ly) {
                                            return true;
                                        }
                                    } else {
                                        let flat = flatten_shape(s);
                                        if point_in_flattened_shape(&flat, lx, ly) {
                                            return true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    false
}

/// Concatenates two SWF [`Matrix`] transforms (`parent * child`).
#[must_use]
#[allow(clippy::arithmetic_side_effects)]
pub fn concat_matrices(p: &Matrix, c: &Matrix) -> Matrix {
    let pa = f64::from(p.scale_x_fp16) / 65536.0;
    let pb = f64::from(p.rotate_skew0_fp16) / 65536.0;
    let pc = f64::from(p.rotate_skew1_fp16) / 65536.0;
    let pd = f64::from(p.scale_y_fp16) / 65536.0;
    let ptx = f64::from(p.translate_x_twips);
    let pty = f64::from(p.translate_y_twips);

    let ca = f64::from(c.scale_x_fp16) / 65536.0;
    let cb = f64::from(c.rotate_skew0_fp16) / 65536.0;
    let cc = f64::from(c.rotate_skew1_fp16) / 65536.0;
    let cd = f64::from(c.scale_y_fp16) / 65536.0;
    let ctx = f64::from(c.translate_x_twips);
    let cty = f64::from(c.translate_y_twips);

    let ra = pa * ca + pc * cb;
    let rb = pb * ca + pd * cb;
    let rc = pa * cc + pc * cd;
    let rd = pb * cc + pd * cd;
    let rtx = pa * ctx + pc * cty + ptx;
    let rty = pb * ctx + pd * cty + pty;

    Matrix {
        scale_x_fp16: (ra * 65536.0).round() as i32,
        rotate_skew0_fp16: (rb * 65536.0).round() as i32,
        rotate_skew1_fp16: (rc * 65536.0).round() as i32,
        scale_y_fp16: (rd * 65536.0).round() as i32,
        translate_x_twips: rtx.round() as i32,
        translate_y_twips: rty.round() as i32,
    }
}

fn transform_rect(m: &Matrix, r: Rect) -> Rect {
    let corners = [
        (r.x_min as f64, r.y_min as f64),
        (r.x_max as f64, r.y_min as f64),
        (r.x_min as f64, r.y_max as f64),
        (r.x_max as f64, r.y_max as f64),
    ];
    let a = f64::from(m.scale_x_fp16) / 65536.0;
    let b = f64::from(m.rotate_skew0_fp16) / 65536.0;
    let c = f64::from(m.rotate_skew1_fp16) / 65536.0;
    let d = f64::from(m.scale_y_fp16) / 65536.0;
    let tx = f64::from(m.translate_x_twips);
    let ty = f64::from(m.translate_y_twips);

    let mut x_min = i32::MAX;
    let mut x_max = i32::MIN;
    let mut y_min = i32::MAX;
    let mut y_max = i32::MIN;
    for (x, y) in corners {
        let nx = (a * x + c * y + tx).round() as i32;
        let ny = (b * x + d * y + ty).round() as i32;
        x_min = x_min.min(nx);
        x_max = x_max.max(nx);
        y_min = y_min.min(ny);
        y_max = y_max.max(ny);
    }
    Rect {
        x_min,
        x_max,
        y_min,
        y_max,
    }
}

fn union_rect(a: Rect, b: Rect) -> Rect {
    Rect {
        x_min: a.x_min.min(b.x_min),
        x_max: a.x_max.max(b.x_max),
        y_min: a.y_min.min(b.y_min),
        y_max: a.y_max.max(b.y_max),
    }
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
