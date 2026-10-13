//! SWF container decompression, header parsing, and tag decoding for `swfr` (TSD 2.6, 2.11, M1).
//!
//! Specification notes and binary record layouts are documented in `docs/notes/swf-format.md`
//! and `docs/notes/tag-coverage.md`.

use std::io::Cursor;
use swfr_reader::{ColorTransform, Matrix, ParseError, ParseErrorKind, Reader, Rect};

/// Resource limits enforced during SWF container decompression and tag decoding (TSD 2.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Maximum input file size in bytes (default: 64 MiB).
    pub max_input_file_size: usize,
    /// Maximum decompressed size in bytes (default: 64 MiB).
    pub max_decompressed_size: usize,
    /// Maximum number of tags in a movie (default: 2,000,000).
    pub max_tag_count: u32,
    /// Maximum number of edges in a single shape (default: 1,000,000).
    pub max_shape_edges: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_file_size: 64 * 1024 * 1024,
            max_decompressed_size: 64 * 1024 * 1024,
            max_tag_count: 2_000_000,
            max_shape_edges: 1_000_000,
        }
    }
}

/// SWF container compression format (`FWS`, `CWS`, `ZWS`) (`docs/notes/swf-format.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    /// Uncompressed SWF (`"FWS"`).
    Uncompressed,
    /// Zlib-compressed SWF (`"CWS"`, SWF 6+).
    Zlib,
    /// LZMA-compressed SWF (`"ZWS"`, SWF 13+).
    Lzma,
}

/// Decoded SWF movie header (TSD 2.6).
#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    /// SWF format version (`1..=40+`).
    pub version: u8,
    /// Container compression method.
    pub compression: Compression,
    /// Stage bounding box in twips.
    pub frame_size: Rect,
    /// Frame rate in frames per second (clamped to `1.0..=120.0`).
    pub frame_rate: f32,
    /// Raw 8.8 fixed-point frame rate clamped to `256..=30720`.
    pub frame_rate_q8: u16,
    /// Declared total frame count on the root timeline.
    pub frame_count: u16,
}

impl Header {
    /// Stage width in whole pixels (`>= 1`).
    #[must_use]
    pub fn width_px(&self) -> u32 {
        self.frame_size.width_px().max(1)
    }

    /// Stage height in whole pixels (`>= 1`).
    #[must_use]
    pub fn height_px(&self) -> u32 {
        self.frame_size.height_px().max(1)
    }

    /// Frame rate in frames per second (`1.0..=120.0`).
    #[must_use]
    pub const fn fps(&self) -> f32 {
        self.frame_rate
    }
}

/// RGBA color value.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rgba {
    /// Red channel (`0..=255`).
    pub r: u8,
    /// Green channel (`0..=255`).
    pub g: u8,
    /// Blue channel (`0..=255`).
    pub b: u8,
    /// Alpha channel (`0..=255`).
    pub a: u8,
}

impl Rgba {
    /// Constructs an opaque RGBA color (`a = 255`).
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
}

/// Gradient control stop (`GRADRECORD`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GradientStop {
    /// Stop position (`0..=255`).
    pub ratio: u8,
    /// Stop color.
    pub color: Rgba,
}

/// Fill style definition (`FILLSTYLE`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FillStyle {
    /// Solid RGBA fill (`0x00`).
    Solid(Rgba),
    /// Linear gradient fill (`0x10`).
    LinearGradient {
        /// Gradient transform matrix.
        matrix: Matrix,
        /// Gradient color stops.
        stops: Vec<GradientStop>,
    },
    /// Radial or focal gradient fill (`0x12`, `0x13`).
    RadialGradient {
        /// Gradient transform matrix.
        matrix: Matrix,
        /// Gradient color stops.
        stops: Vec<GradientStop>,
        /// Focal point in 8.8 fixed point (`0` for standard radial).
        focal_point_q8: i16,
    },
    /// Bitmap fill (`0x40..=0x43`).
    Bitmap {
        /// Bitmap character ID.
        bitmap_id: u16,
        /// Bitmap transform matrix.
        matrix: Matrix,
        /// `true` if repeating, `false` if clipped.
        repeating: bool,
        /// `true` if bilinear smoothed.
        smoothed: bool,
    },
}

/// Stroke line style definition (`LINESTYLE` / `LINESTYLE2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineStyle {
    /// Stroke width in twips (`20 twips == 1 px`).
    pub width_twips: u16,
    /// Stroke RGBA color.
    pub color: Rgba,
}

/// Vector shape record (`SHAPERECORD`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShapeRecord {
    /// Style change and/or pen move record (`STYLECHANGERECORD`).
    StyleChange {
        /// New absolute pen position `(x_twips, y_twips)` if `state_move_to` is set.
        move_to: Option<(i32, i32)>,
        /// New `fill_style_0` index (`1`-based; `0` means none).
        fill_style_0: Option<u16>,
        /// New `fill_style_1` index (`1`-based; `0` means none).
        fill_style_1: Option<u16>,
        /// New `line_style` index (`1`-based; `0` means none).
        line_style: Option<u16>,
        /// Replacement fill styles when `state_new_styles` is set.
        new_fill_styles: Option<Vec<FillStyle>>,
        /// Replacement line styles when `state_new_styles` is set.
        new_line_styles: Option<Vec<LineStyle>>,
    },
    /// Straight edge record (`STRAIGHTEDGERECORD`) with relative delta in twips.
    StraightEdge {
        /// Relative X delta in twips.
        dx_twips: i32,
        /// Relative Y delta in twips.
        dy_twips: i32,
    },
    /// Quadratic Bézier curved edge record (`CURVEDEDGERECORD`) with relative deltas in twips.
    CurvedEdge {
        /// Control point relative X delta in twips.
        control_dx_twips: i32,
        /// Control point relative Y delta in twips.
        control_dy_twips: i32,
        /// Anchor point relative X delta from control point in twips.
        anchor_dx_twips: i32,
        /// Anchor point relative Y delta from control point in twips.
        anchor_dy_twips: i32,
    },
}

/// Decoded vector shape character (`DefineShape`, `DefineShape2`, `DefineShape3`, `DefineShape4`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeDef {
    /// Shape tag version (`1..=4`).
    pub version: u8,
    /// Character ID.
    pub id: u16,
    /// Shape bounding box in twips.
    pub bounds: Rect,
    /// Initial fill styles (`1`-indexed in shape records).
    pub fill_styles: Vec<FillStyle>,
    /// Initial line styles (`1`-indexed in shape records).
    pub line_styles: Vec<LineStyle>,
    /// Decoded shape records.
    pub records: Vec<ShapeRecord>,
}

/// Decoded morph shape character (`DefineMorphShape`, `DefineMorphShape2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MorphShapeDef {
    /// Morph tag version (`1` or `2`).
    pub version: u8,
    /// Character ID.
    pub id: u16,
    /// Start bounding box in twips (`ratio == 0`).
    pub start_bounds: Rect,
    /// End bounding box in twips (`ratio == 65535`).
    pub end_bounds: Rect,
    /// Start fill styles.
    pub start_fill_styles: Vec<FillStyle>,
    /// End fill styles.
    pub end_fill_styles: Vec<FillStyle>,
    /// Start line styles.
    pub start_line_styles: Vec<LineStyle>,
    /// End line styles.
    pub end_line_styles: Vec<LineStyle>,
    /// Start shape records.
    pub start_records: Vec<ShapeRecord>,
    /// End shape records.
    pub end_records: Vec<ShapeRecord>,
}

/// Lossless Zlib-compressed bitmap definition (`DefineBitsLossless`, `DefineBitsLossless2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LosslessBitmapDef {
    /// Tag version (`1` = RGB/colormapped without alpha, `2` = RGBA with alpha).
    pub version: u8,
    /// Character ID.
    pub id: u16,
    /// Bitmap format (`3` = 8-bit colormapped, `4` = 15-bit RGB, `5` = 24/32-bit RGB/ARGB).
    pub format: u8,
    /// Width in pixels.
    pub width: u16,
    /// Height in pixels.
    pub height: u16,
    /// Color table size minus 1 (present when `format == 3`).
    pub color_table_size: Option<u8>,
    /// Zlib-compressed bitmap data.
    pub zlib_data: Vec<u8>,
}

/// Legacy `PlaceObject` (tag 4) payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceObjectTag {
    /// Character ID to place.
    pub character_id: u16,
    /// Display list depth.
    pub depth: u16,
    /// Placement transform matrix.
    pub matrix: Matrix,
    /// Optional color transform.
    pub color_transform: Option<ColorTransform>,
}

/// `PlaceObject2` (tag 26) and `PlaceObject3` (tag 70) payload.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlaceObject2Tag {
    /// Whether an existing instance at `depth` is modified (`move_flag`).
    pub is_move: bool,
    /// Display list depth.
    pub depth: u16,
    /// Character ID when placing a new character (`has_character`).
    pub character_id: Option<u16>,
    /// Transform matrix (`has_matrix`).
    pub matrix: Option<Matrix>,
    /// Color transform (`has_color_transform`).
    pub color_transform: Option<ColorTransform>,
    /// Morph ratio (`has_ratio`).
    pub ratio: Option<u16>,
    /// Instance name (`has_name`).
    pub name: Option<String>,
    /// Clipping depth (`has_clip_depth`).
    pub clip_depth: Option<u16>,
    /// Raw clip actions payload (`has_clip_actions`).
    pub clip_actions_raw: Option<Vec<u8>>,
}

/// Embedded font character definition (`DefineFont`, `DefineFont2`, `DefineFont3`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FontDef {
    /// Font tag version (`1`, `2`, or `3`).
    pub version: u8,
    /// Font character ID.
    pub id: u16,
    /// Font family name.
    pub name: String,
    /// Bold flag.
    pub is_bold: bool,
    /// Italic flag.
    pub is_italic: bool,
    /// Glyph outline shape records per glyph index.
    pub glyphs: Vec<Vec<ShapeRecord>>,
    /// Character code table mapping glyph index -> UTF-16 code unit.
    pub codes: Vec<u16>,
}

/// Single glyph entry inside a [`TextRecord`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphEntry {
    /// Index into the active font's glyph table.
    pub glyph_index: u32,
    /// Horizontal advance in twips.
    pub advance_twips: i32,
}

/// Type alias for [`GlyphEntry`].
pub type TextGlyphEntry = GlyphEntry;

/// Text run record (`TEXTRECORD`) inside `DefineText` / `DefineText2`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRecord {
    /// Optional font character ID change.
    pub font_id: Option<u16>,
    /// Optional text color change.
    pub color: Option<Rgba>,
    /// Optional X offset in twips.
    pub x_offset_twips: Option<i16>,
    /// Optional Y offset in twips.
    pub y_offset_twips: Option<i16>,
    /// Optional text height in twips when `font_id` is set.
    pub text_height_twips: Option<u16>,
    /// Glyph entries in this run.
    pub glyphs: Vec<GlyphEntry>,
}

/// Static text character definition (`DefineText`, `DefineText2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextDef {
    /// Tag version (`1` or `2`).
    pub version: u8,
    /// Character ID.
    pub id: u16,
    /// Bounding box in twips.
    pub bounds: Rect,
    /// Text placement matrix.
    pub matrix: Matrix,
    /// Text run records.
    pub records: Vec<TextRecord>,
}

/// Dynamic/input text field definition (`DefineEditText`, tag 37).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditTextDef {
    /// Character ID.
    pub id: u16,
    /// Text field bounding box in twips.
    pub bounds: Rect,
    /// Referenced font ID if `has_font`.
    pub font_id: Option<u16>,
    /// Font height in twips.
    pub font_height_twips: u16,
    /// Text RGBA color.
    pub color: Rgba,
    /// AVM1 variable name bound to this text field.
    pub variable_name: String,
    /// Initial text string (`has_text`).
    pub initial_text: Option<String>,
    /// Word wrap flag.
    pub word_wrap: bool,
    /// Multiline flag.
    pub multiline: bool,
    /// Read-only flag.
    pub read_only: bool,
    /// HTML text flag.
    pub html: bool,
    /// Use embedded font outlines (`true`) or device font (`false`).
    pub use_outlines: bool,
}

/// Button state record (`BUTTONRECORD`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonRecord {
    /// Active in Up state (`state_up`).
    pub state_up: bool,
    /// Active in Over state (`state_over`).
    pub state_over: bool,
    /// Active in Down state (`state_down`).
    pub state_down: bool,
    /// Used for hit-testing (`state_hit_test`).
    pub state_hit_test: bool,
    /// Referenced character ID.
    pub character_id: u16,
    /// Placement depth within button.
    pub depth: u16,
    /// Transform matrix.
    pub matrix: Matrix,
    /// Color transform (present in `DefineButton2`).
    pub color_transform: ColorTransform,
}

/// Button condition and action bytecode block (`BUTTONCONDACTION`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonAction {
    /// Condition bitmask (`cond_idle_to_over_up`, `cond_over_down_to_over_up`, etc.; `0` for `DefineButton` click).
    pub condition: u16,
    /// Raw AVM1 action bytecode.
    pub actions: Vec<u8>,
}

/// Button character definition (`DefineButton`, `DefineButton2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonDef {
    /// Tag version (`1` or `2`).
    pub version: u8,
    /// Character ID.
    pub id: u16,
    /// Track as menu flag (`DefineButton2`).
    pub track_as_menu: bool,
    /// Button state records.
    pub records: Vec<ButtonRecord>,
    /// Button action blocks.
    pub actions: Vec<ButtonAction>,
}

/// Audio codec format (`SoundFormat` 4-bit field in `DefineSound` and `SoundStreamHead*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundFormat {
    /// Uncompressed native-endian PCM (`0`).
    UncompressedNativeEndian,
    /// SWF IMA ADPCM (`1`).
    Adpcm,
    /// MP3 (`2`).
    Mp3,
    /// Uncompressed little-endian PCM (`3`).
    UncompressedLittleEndian,
    /// Nellymoser 16 kHz (`4`, unsupported in v1).
    Nellymoser16k,
    /// Nellymoser 8 kHz (`5`, unsupported in v1).
    Nellymoser8k,
    /// Nellymoser (`6`, unsupported in v1).
    Nellymoser,
    /// Speex (`11`, unsupported in v1).
    Speex,
    /// Other/unknown codec code.
    Unknown(u8),
}

/// Type alias for [`SoundFormat`].
pub type AudioFormat = SoundFormat;

#[allow(non_upper_case_globals)]
impl SoundFormat {
    /// Alias for [`SoundFormat::UncompressedLittleEndian`].
    pub const PcmLittleEndian: Self = Self::UncompressedLittleEndian;

    /// Converts a 4-bit SWF sound format code into [`SoundFormat`].
    #[must_use]
    pub const fn from_code(code: u8) -> Self {
        match code {
            0 => Self::UncompressedNativeEndian,
            1 => Self::Adpcm,
            2 => Self::Mp3,
            3 => Self::UncompressedLittleEndian,
            4 => Self::Nellymoser16k,
            5 => Self::Nellymoser8k,
            6 => Self::Nellymoser,
            11 => Self::Speex,
            other => Self::Unknown(other),
        }
    }

    /// Returns the 4-bit SWF sound format code.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::UncompressedNativeEndian => 0,
            Self::Adpcm => 1,
            Self::Mp3 => 2,
            Self::UncompressedLittleEndian => 3,
            Self::Nellymoser16k => 4,
            Self::Nellymoser8k => 5,
            Self::Nellymoser => 6,
            Self::Speex => 11,
            Self::Unknown(c) => c,
        }
    }
}

/// Decoded event sound definition (`DefineSound`, tag 14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundDef {
    /// Sound character ID.
    pub id: u16,
    /// Audio codec format.
    pub format: SoundFormat,
    /// Sample rate code (`0` = 5512 Hz, `1` = 11025 Hz, `2` = 22050 Hz, `3` = 44100 Hz).
    pub sample_rate_code: u8,
    /// `true` if 16-bit samples, `false` if 8-bit samples.
    pub is_16bit: bool,
    /// `true` if stereo, `false` if mono.
    pub is_stereo: bool,
    /// Number of sound samples (stereo pairs count as 1 sample).
    pub sample_count: u32,
    /// Raw encoded sound payload.
    pub data: Vec<u8>,
}

impl SoundDef {
    /// Returns the nominal sample rate in Hz (`5512`, `11025`, `22050`, or `44100`).
    #[must_use]
    pub const fn sample_rate_hz(&self) -> u32 {
        match self.sample_rate_code {
            0 => 5_512,
            1 => 11_025,
            2 => 22_050,
            _ => 44_100,
        }
    }
}

/// Sound envelope control point (`SOUNDENVELOPE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoundEnvelopePoint {
    /// Position in 44.1 kHz samples.
    pub pos44: u32,
    /// Left channel volume (`0..=32768`, where `32768 == 1.0`).
    pub left_level: u16,
    /// Right channel volume (`0..=32768`, where `32768 == 1.0`).
    pub right_level: u16,
}

/// Playback control flags and parameters (`SOUNDINFO` in `StartSound` / `StartSound2`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SoundInfo {
    /// Stop the sound instance immediately (`sync_stop`).
    pub sync_stop: bool,
    /// Do not start a new instance if one is already playing (`sync_no_multiple`).
    pub sync_no_multiple: bool,
    /// Optional start sample offset (`in_point`).
    pub in_point: Option<u32>,
    /// Optional end sample offset (`out_point`).
    pub out_point: Option<u32>,
    /// Optional loop count (`loop_count`).
    pub loop_count: Option<u16>,
    /// Volume envelope points.
    pub envelope: Vec<SoundEnvelopePoint>,
}

/// Timeline streaming sound configuration (`SoundStreamHead`, `SoundStreamHead2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundStreamHeadDef {
    /// Tag version (`1` or `2`).
    pub version: u8,
    /// Recommended playback rate code (`0..=3`).
    pub playback_rate_code: u8,
    /// Playback 16-bit flag.
    pub playback_is_16bit: bool,
    /// Playback stereo flag.
    pub playback_is_stereo: bool,
    /// Stream compression format.
    pub stream_format: SoundFormat,
    /// Stream sample rate code (`0..=3`).
    pub stream_rate_code: u8,
    /// Stream 16-bit flag.
    pub stream_is_16bit: bool,
    /// Stream stereo flag.
    pub stream_is_stereo: bool,
    /// Average number of samples per frame.
    pub stream_sample_count: u16,
    /// Optional latency seek for MP3 streams.
    pub latency_seek: Option<i16>,
}

impl SoundStreamHeadDef {
    /// Returns the stream sample rate in Hz (`5512`, `11025`, `22050`, or `44100`).
    #[must_use]
    pub const fn stream_rate_hz(&self) -> u32 {
        match self.stream_rate_code {
            0 => 5_512,
            1 => 11_025,
            2 => 22_050,
            _ => 44_100,
        }
    }
}

/// Decoded SWF tag (TSD 2.6, M1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tag {
    /// `End` (tag 0).
    End,
    /// `ShowFrame` (tag 1).
    ShowFrame,
    /// `DefineShape` / `DefineShape2` / `DefineShape3` / `DefineShape4` (tags 2, 22, 32, 83).
    DefineShape(ShapeDef),
    /// `PlaceObject` (tag 4).
    PlaceObject(PlaceObjectTag),
    /// `RemoveObject` (tag 5).
    RemoveObject {
        /// Character ID.
        character_id: u16,
        /// Display depth.
        depth: u16,
    },
    /// `DefineBits` (tag 6).
    DefineBits {
        /// Character ID.
        id: u16,
        /// Raw JPEG image data.
        jpeg_data: Vec<u8>,
    },
    /// `DefineButton` / `DefineButton2` (tags 7, 34).
    DefineButton(ButtonDef),
    /// `JPEGTables` (tag 8).
    JPEGTables {
        /// Shared JPEG encoding tables.
        jpeg_data: Vec<u8>,
    },
    /// `SetBackgroundColor` (tag 9).
    SetBackgroundColor {
        /// Red (`0..=255`).
        r: u8,
        /// Green (`0..=255`).
        g: u8,
        /// Blue (`0..=255`).
        b: u8,
    },
    /// `DefineFont` / `DefineFont2` / `DefineFont3` (tags 10, 48, 75).
    DefineFont(FontDef),
    /// `DefineText` / `DefineText2` (tags 11, 33).
    DefineText(TextDef),
    /// `DoAction` (tag 12).
    DoAction {
        /// Raw AVM1 action bytes.
        actions: Vec<u8>,
    },
    /// `DefineFontInfo` / `DefineFontInfo2` (tags 13, 62).
    DefineFontInfo {
        /// Referenced font ID.
        font_id: u16,
        /// Font name.
        name: String,
        /// Character code table.
        codes: Vec<u16>,
    },
    /// `DefineSound` (tag 14).
    DefineSound(SoundDef),
    /// `StartSound` (tag 15).
    StartSound {
        /// Sound character ID.
        sound_id: u16,
        /// Sound playback control info.
        sound_info: SoundInfo,
    },
    /// `DefineButtonSound` (tag 17).
    DefineButtonSound {
        /// Button character ID.
        button_id: u16,
        /// Raw button sound payload.
        raw: Vec<u8>,
    },
    /// `SoundStreamHead` / `SoundStreamHead2` (tags 18, 45).
    SoundStreamHead(SoundStreamHeadDef),
    /// `SoundStreamBlock` (tag 19).
    SoundStreamBlock {
        /// Raw stream block audio data.
        data: Vec<u8>,
    },
    /// `DefineBitsLossless` / `DefineBitsLossless2` (tags 20, 36).
    DefineBitsLossless(LosslessBitmapDef),
    /// `DefineBitsJPEG2` (tag 21).
    DefineBitsJPEG2 {
        /// Character ID.
        id: u16,
        /// Image data.
        image_data: Vec<u8>,
    },
    /// `DefineButtonCxform` (tag 23).
    DefineButtonCxform {
        /// Button ID.
        button_id: u16,
        /// Color transform.
        cxform: ColorTransform,
    },
    /// `Protect` (tag 24).
    Protect {
        /// Optional password hash string.
        password_md5: Option<String>,
    },
    /// `PlaceObject2` (tag 26).
    PlaceObject2(PlaceObject2Tag),
    /// `RemoveObject2` (tag 28).
    RemoveObject2 {
        /// Display depth to remove.
        depth: u16,
    },
    /// `DefineBitsJPEG3` (tag 35).
    DefineBitsJPEG3 {
        /// Character ID.
        id: u16,
        /// JPEG image bytes.
        image_data: Vec<u8>,
        /// Zlib-compressed alpha map bytes.
        alpha_data: Vec<u8>,
    },
    /// `DefineEditText` (tag 37).
    DefineEditText(EditTextDef),
    /// `DefineSprite` (tag 39).
    DefineSprite {
        /// Sprite character ID.
        id: u16,
        /// Declared sprite frame count.
        frame_count: u16,
        /// Nested timeline control tags.
        tags: Vec<Tag>,
    },
    /// `FrameLabel` (tag 43).
    FrameLabel {
        /// Label string.
        name: String,
        /// Named anchor flag.
        is_anchor: bool,
    },
    /// `DefineMorphShape` / `DefineMorphShape2` (tags 46, 84).
    DefineMorphShape(MorphShapeDef),
    /// `ExportAssets` (tag 56).
    ExportAssets {
        /// Exported `(character_id, symbol_name)` pairs.
        exports: Vec<(u16, String)>,
    },
    /// `ImportAssets` / `ImportAssets2` (tags 57, 71).
    ImportAssets {
        /// Source SWF URL.
        url: String,
        /// Imported `(character_id, symbol_name)` pairs.
        imports: Vec<(u16, String)>,
    },
    /// `EnableDebugger` / `EnableDebugger2` (tags 58, 64).
    EnableDebugger {
        /// Optional password hash.
        password_md5: Option<String>,
    },
    /// `DoInitAction` (tag 59).
    DoInitAction {
        /// Target sprite character ID.
        sprite_id: u16,
        /// Raw AVM1 action bytes.
        actions: Vec<u8>,
    },
    /// `ScriptLimits` (tag 65).
    ScriptLimits {
        /// Maximum recursion depth.
        max_recursion_depth: u16,
        /// Script timeout in seconds.
        script_timeout_seconds: u16,
    },
    /// `SetTabIndex` (tag 66).
    SetTabIndex {
        /// Display depth.
        depth: u16,
        /// Tab order index.
        tab_index: u16,
    },
    /// `FileAttributes` (tag 69).
    FileAttributes {
        /// Direct blit flag.
        use_direct_blit: bool,
        /// GPU compositing flag.
        use_gpu: bool,
        /// Metadata present flag.
        has_metadata: bool,
        /// ActionScript 3 (`DoABC`) flag.
        actionscript3: bool,
        /// Network sandbox flag.
        use_network: bool,
    },
    /// `PlaceObject3` (tag 70).
    PlaceObject3(PlaceObject2Tag),
    /// `DefineFontAlignZones` (tag 73).
    DefineFontAlignZones {
        /// Font ID.
        font_id: u16,
        /// Table hint (`0..=2`).
        csm_table_hint: u8,
        /// Raw zone data.
        raw_zones: Vec<u8>,
    },
    /// `CSMTextSettings` (tag 74).
    CSMTextSettings {
        /// Text character ID.
        text_id: u16,
        /// FlashType rendering flag.
        use_flash_type: u8,
        /// Grid fit mode.
        grid_fit: u8,
        /// Raw 32-bit float thickness bits.
        thickness_bits: u32,
        /// Raw 32-bit float sharpness bits.
        sharpness_bits: u32,
    },
    /// `SymbolClass` (tag 76).
    SymbolClass {
        /// `(character_id, class_name)` bindings.
        symbols: Vec<(u16, String)>,
    },
    /// `Metadata` (tag 77).
    Metadata {
        /// RDF/XML metadata string.
        xml: String,
    },
    /// `DefineScalingGrid` (tag 78).
    DefineScalingGrid {
        /// Target character ID.
        character_id: u16,
        /// 9-slice center rectangle in twips.
        splitter: Rect,
    },
    /// `DefineSceneAndFrameLabelData` (tag 86).
    DefineSceneAndFrameLabelData {
        /// `(frame_offset, scene_name)` pairs.
        scenes: Vec<(u32, String)>,
        /// `(frame_number, frame_label)` pairs.
        labels: Vec<(u32, String)>,
    },
    /// `DefineFontName` (tag 88).
    DefineFontName {
        /// Font ID.
        font_id: u16,
        /// Full font name.
        font_name: String,
        /// Copyright string.
        font_copyright: String,
    },
    /// `StartSound2` (tag 89).
    StartSound2 {
        /// Sound class name.
        class_name: String,
        /// Playback control info.
        sound_info: SoundInfo,
    },
    /// `DefineBitsJPEG4` (tag 90).
    DefineBitsJPEG4 {
        /// Character ID.
        id: u16,
        /// Deblocking filter parameter.
        deblock_param: u16,
        /// JPEG image bytes.
        image_data: Vec<u8>,
        /// Zlib alpha map bytes.
        alpha_data: Vec<u8>,
    },
    /// Any tag code not decoded in v1 or preserved raw (TSD 2.6).
    Unknown {
        /// 10-bit SWF tag code.
        code: u16,
        /// Uninterpreted tag payload bytes.
        raw: Vec<u8>,
    },
}

/// Parsed SWF movie containing its [`Header`] and decoded [`Tag`] list (TSD 2.6).
#[derive(Debug, Clone, PartialEq)]
pub struct Movie {
    /// Movie header.
    pub header: Header,
    /// Top-level tags in stream order.
    pub tags: Vec<Tag>,
}

impl Movie {
    /// Decompresses and parses `raw_bytes` using [`Limits::default()`].
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if the container header is invalid, decompression fails,
    /// any resource limit in [`Limits`] is exceeded, or a tag record is truncated.
    pub fn parse(raw_bytes: &[u8]) -> Result<Self, ParseError> {
        Self::parse_with_limits(raw_bytes, &Limits::default())
    }

    /// Decompresses and parses `raw_bytes` enforcing `limits` (TSD 2.11, M1).
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] on malformed data or limit violations.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn parse_with_limits(raw_bytes: &[u8], limits: &Limits) -> Result<Self, ParseError> {
        if raw_bytes.len() > limits.max_input_file_size {
            return Err(ParseError {
                offset: 0,
                kind: ParseErrorKind::LimitExceeded("input file size exceeds max_input_file_size"),
            });
        }
        if raw_bytes.len() < 8 {
            return Err(ParseError {
                offset: 0,
                kind: ParseErrorKind::InvalidHeader("SWF file shorter than 8-byte preamble"),
            });
        }

        let sig = raw_bytes.get(0..3).unwrap_or(&[]);
        let compression = match sig {
            b"FWS" => Compression::Uncompressed,
            b"CWS" => Compression::Zlib,
            b"ZWS" => Compression::Lzma,
            _ => {
                return Err(ParseError {
                    offset: 0,
                    kind: ParseErrorKind::InvalidHeader(
                        "unknown SWF signature (expected FWS/CWS/ZWS)",
                    ),
                });
            }
        };
        let version = raw_bytes.get(3).copied().unwrap_or(0);
        let file_len = u32::from_le_bytes([
            raw_bytes.get(4).copied().unwrap_or(0),
            raw_bytes.get(5).copied().unwrap_or(0),
            raw_bytes.get(6).copied().unwrap_or(0),
            raw_bytes.get(7).copied().unwrap_or(0),
        ]) as usize;

        if file_len < 8 {
            return Err(ParseError {
                offset: 4,
                kind: ParseErrorKind::InvalidHeader("SWF declared file_length < 8"),
            });
        }
        if file_len > limits.max_decompressed_size {
            return Err(ParseError {
                offset: 4,
                kind: ParseErrorKind::LimitExceeded(
                    "declared decompressed file_length exceeds max_decompressed_size",
                ),
            });
        }

        let body_bytes: Vec<u8> = match compression {
            Compression::Uncompressed => raw_bytes.get(8..).unwrap_or(&[]).to_vec(),
            Compression::Zlib => {
                let compressed = raw_bytes.get(8..).unwrap_or(&[]);
                miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(
                    compressed,
                    limits.max_decompressed_size,
                )
                .map_err(|_| ParseError {
                    offset: 8,
                    kind: ParseErrorKind::DecompressionError(
                        "zlib decompression failed or exceeded limit",
                    ),
                })?
            }
            Compression::Lzma => {
                // ZWS header: [0..4 sig+ver][4..8 uncomp_len][8..12 comp_len][12..17 lzma_props][17.. payload]
                if raw_bytes.len() < 17 {
                    return Err(ParseError {
                        offset: 8,
                        kind: ParseErrorKind::InvalidHeader("ZWS header shorter than 17 bytes"),
                    });
                }
                let props = raw_bytes.get(12..17).unwrap_or(&[]);
                let payload = raw_bytes.get(17..).unwrap_or(&[]);
                let expected_uncomp = (file_len.saturating_sub(8)) as u64;
                let mut lzma_stream = Vec::with_capacity(13_usize.saturating_add(payload.len()));
                lzma_stream.extend_from_slice(props);
                lzma_stream.extend_from_slice(&expected_uncomp.to_le_bytes());
                lzma_stream.extend_from_slice(payload);

                let mut cursor = Cursor::new(lzma_stream);
                let mut out = Vec::new();
                lzma_rs::lzma_decompress(&mut cursor, &mut out).map_err(|_| ParseError {
                    offset: 12,
                    kind: ParseErrorKind::DecompressionError("LZMA decompression failed"),
                })?;
                if out.len() > limits.max_decompressed_size {
                    return Err(ParseError {
                        offset: 12,
                        kind: ParseErrorKind::LimitExceeded(
                            "LZMA output exceeded max_decompressed_size",
                        ),
                    });
                }
                out
            }
        };

        let mut reader = Reader::new(&body_bytes);
        let frame_size = reader.rect()?;
        let raw_fps_q8 = reader.u16()?;
        let frame_rate_q8 = raw_fps_q8.clamp(256, 120 * 256);
        let frame_rate = (f32::from(frame_rate_q8)) / 256.0;
        let frame_count = reader.u16()?;

        let header = Header {
            version,
            compression,
            frame_size,
            frame_rate,
            frame_rate_q8,
            frame_count,
        };

        let tags = parse_tag_stream(&mut reader, version, limits)?;
        Ok(Self { header, tags })
    }
}

fn parse_tag_stream(
    reader: &mut Reader<'_>,
    swf_version: u8,
    limits: &Limits,
) -> Result<Vec<Tag>, ParseError> {
    let mut tags = Vec::new();
    while !reader.is_empty() {
        if tags.len() >= (limits.max_tag_count as usize) {
            return Err(ParseError {
                offset: reader.position(),
                kind: ParseErrorKind::LimitExceeded("tag count exceeds max_tag_count"),
            });
        }
        let tag_offset = reader.position();
        let code_and_len = reader.u16()?;
        let code = code_and_len >> 6;
        let short_len = usize::from(code_and_len & 0x3F);
        let length = if short_len == 0x3F {
            usize::try_from(reader.u32()?).unwrap_or(0)
        } else {
            short_len
        };
        let payload = reader.bytes(length).map_err(|_| ParseError {
            offset: tag_offset,
            kind: ParseErrorKind::UnexpectedEof,
        })?;
        let tag = decode_tag(code, payload, swf_version, limits, tag_offset)?;
        let is_end = tag == Tag::End;
        tags.push(tag);
        if is_end {
            break;
        }
    }
    Ok(tags)
}

#[allow(clippy::too_many_lines)]
fn decode_tag(
    code: u16,
    payload: &[u8],
    swf_version: u8,
    limits: &Limits,
    tag_offset: usize,
) -> Result<Tag, ParseError> {
    let mut r = Reader::new(payload);
    match code {
        0 => Ok(Tag::End),
        1 => Ok(Tag::ShowFrame),
        2 | 22 | 32 | 83 => {
            let shape_ver = match code {
                2 => 1,
                22 => 2,
                32 => 3,
                _ => 4,
            };
            let shape = decode_shape(&mut r, shape_ver, limits, tag_offset)?;
            Ok(Tag::DefineShape(shape))
        }
        4 => {
            let character_id = r.u16()?;
            let depth = r.u16()?;
            let matrix = r.matrix()?;
            let color_transform = if !r.is_empty() {
                Some(r.cxform()?)
            } else {
                None
            };
            Ok(Tag::PlaceObject(PlaceObjectTag {
                character_id,
                depth,
                matrix,
                color_transform,
            }))
        }
        5 => {
            let character_id = r.u16()?;
            let depth = r.u16()?;
            Ok(Tag::RemoveObject {
                character_id,
                depth,
            })
        }
        6 => {
            let id = r.u16()?;
            let jpeg_data = r.bytes(r.remaining())?.to_vec();
            Ok(Tag::DefineBits { id, jpeg_data })
        }
        7 | 34 => {
            let ver = if code == 7 { 1 } else { 2 };
            let btn = decode_button(&mut r, ver)?;
            Ok(Tag::DefineButton(btn))
        }
        8 => Ok(Tag::JPEGTables {
            jpeg_data: payload.to_vec(),
        }),
        9 => {
            let red = r.u8()?;
            let green = r.u8()?;
            let blue = r.u8()?;
            Ok(Tag::SetBackgroundColor {
                r: red,
                g: green,
                b: blue,
            })
        }
        10 | 48 | 75 => {
            let ver = match code {
                10 => 1,
                48 => 2,
                _ => 3,
            };
            let font = decode_font(&mut r, ver, limits, tag_offset)?;
            Ok(Tag::DefineFont(font))
        }
        11 | 33 => {
            let ver = if code == 11 { 1 } else { 2 };
            let txt = decode_text(&mut r, ver)?;
            Ok(Tag::DefineText(txt))
        }
        12 => Ok(Tag::DoAction {
            actions: payload.to_vec(),
        }),
        13 | 62 => {
            let font_id = r.u16()?;
            let name_len = usize::from(r.u8()?);
            let name_bytes = r.bytes(name_len)?;
            let name = String::from_utf8_lossy(name_bytes).into_owned();
            let flags = r.u8()?;
            if code == 62 {
                let _lang = r.u8()?;
            }
            let wide = (flags & 0x01) != 0 || code == 62;
            let mut codes = Vec::new();
            while !r.is_empty() {
                if wide {
                    codes.push(r.u16()?);
                } else {
                    codes.push(u16::from(r.u8()?));
                }
            }
            Ok(Tag::DefineFontInfo {
                font_id,
                name,
                codes,
            })
        }
        14 => {
            let id = r.u16()?;
            let format_code = u8::try_from(r.ubits(4)?).unwrap_or(0);
            let sample_rate_code = u8::try_from(r.ubits(2)?).unwrap_or(3);
            let is_16bit = r.ubits(1)? != 0;
            let is_stereo = r.ubits(1)? != 0;
            let sample_count = r.u32()?;
            let data = r.bytes(r.remaining())?.to_vec();
            Ok(Tag::DefineSound(SoundDef {
                id,
                format: SoundFormat::from_code(format_code),
                sample_rate_code,
                is_16bit,
                is_stereo,
                sample_count,
                data,
            }))
        }
        15 => {
            let sound_id = r.u16()?;
            let sound_info = decode_sound_info(&mut r)?;
            Ok(Tag::StartSound {
                sound_id,
                sound_info,
            })
        }
        17 => {
            let button_id = r.u16()?;
            let raw = r.bytes(r.remaining())?.to_vec();
            Ok(Tag::DefineButtonSound { button_id, raw })
        }
        18 | 45 => {
            let version = if code == 18 { 1 } else { 2 };
            let _reserved = r.ubits(4)?;
            let playback_rate_code = u8::try_from(r.ubits(2)?).unwrap_or(3);
            let playback_is_16bit = r.ubits(1)? != 0;
            let playback_is_stereo = r.ubits(1)? != 0;
            let stream_format_code = u8::try_from(r.ubits(4)?).unwrap_or(0);
            let stream_rate_code = u8::try_from(r.ubits(2)?).unwrap_or(3);
            let stream_is_16bit = r.ubits(1)? != 0;
            let stream_is_stereo = r.ubits(1)? != 0;
            let stream_sample_count = r.u16()?;
            let stream_format = SoundFormat::from_code(stream_format_code);
            let latency_seek = if stream_format == SoundFormat::Mp3 && r.remaining() >= 2 {
                Some(r.i16()?)
            } else {
                None
            };
            Ok(Tag::SoundStreamHead(SoundStreamHeadDef {
                version,
                playback_rate_code,
                playback_is_16bit,
                playback_is_stereo,
                stream_format,
                stream_rate_code,
                stream_is_16bit,
                stream_is_stereo,
                stream_sample_count,
                latency_seek,
            }))
        }
        19 => Ok(Tag::SoundStreamBlock {
            data: payload.to_vec(),
        }),
        20 | 36 => {
            let version = if code == 20 { 1 } else { 2 };
            let id = r.u16()?;
            let format = r.u8()?;
            let width = r.u16()?;
            let height = r.u16()?;
            let color_table_size = if format == 3 { Some(r.u8()?) } else { None };
            let zlib_data = r.bytes(r.remaining())?.to_vec();
            Ok(Tag::DefineBitsLossless(LosslessBitmapDef {
                version,
                id,
                format,
                width,
                height,
                color_table_size,
                zlib_data,
            }))
        }
        21 => {
            let id = r.u16()?;
            let image_data = r.bytes(r.remaining())?.to_vec();
            Ok(Tag::DefineBitsJPEG2 { id, image_data })
        }
        23 => {
            let button_id = r.u16()?;
            let cxform = r.cxform()?;
            Ok(Tag::DefineButtonCxform { button_id, cxform })
        }
        24 => {
            let password_md5 = if !r.is_empty() {
                let _reserved = r.u16();
                r.string().ok()
            } else {
                None
            };
            Ok(Tag::Protect { password_md5 })
        }
        26 => {
            let po2 = decode_place_object2(&mut r, false)?;
            Ok(Tag::PlaceObject2(po2))
        }
        28 => {
            let depth = r.u16()?;
            Ok(Tag::RemoveObject2 { depth })
        }
        35 => {
            let id = r.u16()?;
            let alpha_offset = usize::try_from(r.u32()?).unwrap_or(0);
            let image_data = r.bytes(alpha_offset)?.to_vec();
            let alpha_data = r.bytes(r.remaining())?.to_vec();
            Ok(Tag::DefineBitsJPEG3 {
                id,
                image_data,
                alpha_data,
            })
        }
        37 => {
            let edit = decode_edit_text(&mut r)?;
            Ok(Tag::DefineEditText(edit))
        }
        39 => {
            let id = r.u16()?;
            let frame_count = r.u16()?;
            let nested_tags = parse_tag_stream(&mut r, swf_version, limits)?;
            Ok(Tag::DefineSprite {
                id,
                frame_count,
                tags: nested_tags,
            })
        }
        43 => {
            let name = r.string()?;
            let is_anchor = if !r.is_empty() { r.u8()? == 1 } else { false };
            Ok(Tag::FrameLabel { name, is_anchor })
        }
        46 | 84 => {
            let ver = if code == 46 { 1 } else { 2 };
            let morph = decode_morph_shape(&mut r, ver, limits, tag_offset)?;
            Ok(Tag::DefineMorphShape(morph))
        }
        56 => {
            let count = usize::from(r.u16()?);
            let mut exports = Vec::with_capacity(count.min(1024));
            for _ in 0..count {
                let id = r.u16()?;
                let name = r.string()?;
                exports.push((id, name));
            }
            Ok(Tag::ExportAssets { exports })
        }
        57 | 71 => {
            let url = r.string()?;
            if code == 71 {
                let _download_now = r.u8()?;
                let _sha1 = r.u8()?;
            } else {
                // Tag 57 has no extra bytes before count.
            }
            let count = usize::from(r.u16()?);
            let mut imports = Vec::with_capacity(count.min(1024));
            for _ in 0..count {
                let id = r.u16()?;
                let name = r.string()?;
                imports.push((id, name));
            }
            Ok(Tag::ImportAssets { url, imports })
        }
        58 | 64 => {
            if code == 64 && r.remaining() >= 2 {
                let _reserved = r.u16()?;
            }
            let password_md5 = if !r.is_empty() {
                Some(r.string()?)
            } else {
                None
            };
            Ok(Tag::EnableDebugger { password_md5 })
        }
        59 => {
            let sprite_id = r.u16()?;
            let actions = r.bytes(r.remaining())?.to_vec();
            Ok(Tag::DoInitAction { sprite_id, actions })
        }
        65 => {
            let max_recursion_depth = r.u16()?;
            let script_timeout_seconds = r.u16()?;
            Ok(Tag::ScriptLimits {
                max_recursion_depth,
                script_timeout_seconds,
            })
        }
        66 => {
            let depth = r.u16()?;
            let tab_index = r.u16()?;
            Ok(Tag::SetTabIndex { depth, tab_index })
        }
        69 => {
            let _reserved1 = r.ubits(1)?;
            let use_direct_blit = r.ubits(1)? != 0;
            let use_gpu = r.ubits(1)? != 0;
            let has_metadata = r.ubits(1)? != 0;
            let actionscript3 = r.ubits(1)? != 0;
            let _reserved2 = r.ubits(2)?;
            let use_network = r.ubits(1)? != 0;
            let _reserved3 = r.ubits(24)?;
            Ok(Tag::FileAttributes {
                use_direct_blit,
                use_gpu,
                has_metadata,
                actionscript3,
                use_network,
            })
        }
        70 => {
            let po3 = decode_place_object2(&mut r, true)?;
            Ok(Tag::PlaceObject3(po3))
        }
        73 => {
            let font_id = r.u16()?;
            let csm_table_hint = u8::try_from(r.ubits(2)?).unwrap_or(0);
            let _reserved = r.ubits(6)?;
            let raw_zones = r.bytes(r.remaining())?.to_vec();
            Ok(Tag::DefineFontAlignZones {
                font_id,
                csm_table_hint,
                raw_zones,
            })
        }
        74 => {
            let text_id = r.u16()?;
            let use_flash_type = u8::try_from(r.ubits(2)?).unwrap_or(0);
            let grid_fit = u8::try_from(r.ubits(3)?).unwrap_or(0);
            let _reserved = r.ubits(3)?;
            let thickness_bits = r.u32()?;
            let sharpness_bits = r.u32()?;
            let _reserved_byte = r.u8()?;
            Ok(Tag::CSMTextSettings {
                text_id,
                use_flash_type,
                grid_fit,
                thickness_bits,
                sharpness_bits,
            })
        }
        76 => {
            let count = usize::from(r.u16()?);
            let mut symbols = Vec::with_capacity(count.min(1024));
            for _ in 0..count {
                let id = r.u16()?;
                let name = r.string()?;
                symbols.push((id, name));
            }
            Ok(Tag::SymbolClass { symbols })
        }
        77 => {
            let xml = r.string()?;
            Ok(Tag::Metadata { xml })
        }
        78 => {
            let character_id = r.u16()?;
            let splitter = r.rect()?;
            Ok(Tag::DefineScalingGrid {
                character_id,
                splitter,
            })
        }
        86 => {
            let scene_count = usize::try_from(r.encoded_u32()?).unwrap_or(0);
            let mut scenes = Vec::with_capacity(scene_count.min(256));
            for _ in 0..scene_count {
                let offset = r.encoded_u32()?;
                let name = r.string()?;
                scenes.push((offset, name));
            }
            let label_count = usize::try_from(r.encoded_u32()?).unwrap_or(0);
            let mut labels = Vec::with_capacity(label_count.min(256));
            for _ in 0..label_count {
                let frame_num = r.encoded_u32()?;
                let label = r.string()?;
                labels.push((frame_num, label));
            }
            Ok(Tag::DefineSceneAndFrameLabelData { scenes, labels })
        }
        88 => {
            let font_id = r.u16()?;
            let font_name = r.string()?;
            let font_copyright = r.string()?;
            Ok(Tag::DefineFontName {
                font_id,
                font_name,
                font_copyright,
            })
        }
        89 => {
            let class_name = r.string()?;
            let sound_info = decode_sound_info(&mut r)?;
            Ok(Tag::StartSound2 {
                class_name,
                sound_info,
            })
        }
        90 => {
            let id = r.u16()?;
            let alpha_offset = usize::try_from(r.u32()?).unwrap_or(0);
            let deblock_param = r.u16()?;
            let image_data = r.bytes(alpha_offset)?.to_vec();
            let alpha_data = r.bytes(r.remaining())?.to_vec();
            Ok(Tag::DefineBitsJPEG4 {
                id,
                deblock_param,
                image_data,
                alpha_data,
            })
        }
        other => Ok(Tag::Unknown {
            code: other,
            raw: payload.to_vec(),
        }),
    }
}

fn read_rgba(r: &mut Reader<'_>, has_alpha: bool) -> Result<Rgba, ParseError> {
    let red = r.u8()?;
    let green = r.u8()?;
    let blue = r.u8()?;
    let alpha = if has_alpha { r.u8()? } else { 255 };
    Ok(Rgba {
        r: red,
        g: green,
        b: blue,
        a: alpha,
    })
}

fn read_count(r: &mut Reader<'_>, allow_extended: bool) -> Result<usize, ParseError> {
    let first = r.u8()?;
    if allow_extended && first == 0xFF {
        Ok(usize::from(r.u16()?))
    } else {
        Ok(usize::from(first))
    }
}

fn decode_single_fill_style(r: &mut Reader<'_>, has_alpha: bool) -> Result<FillStyle, ParseError> {
    let style_type = r.u8()?;
    let style = match style_type {
        0x00 => FillStyle::Solid(read_rgba(r, has_alpha)?),
        0x10 | 0x12 | 0x13 => {
            let matrix = r.matrix()?;
            let _spread_and_interp = r.ubits(4)?;
            let num_grads = usize::try_from(r.ubits(4)?).unwrap_or(0);
            let mut stops = Vec::with_capacity(num_grads);
            for _ in 0..num_grads {
                let ratio = r.u8()?;
                let color = read_rgba(r, has_alpha)?;
                stops.push(GradientStop { ratio, color });
            }
            let focal_point_q8 = if style_type == 0x13 { r.i16()? } else { 0 };
            if style_type == 0x10 {
                FillStyle::LinearGradient { matrix, stops }
            } else {
                FillStyle::RadialGradient {
                    matrix,
                    stops,
                    focal_point_q8,
                }
            }
        }
        0x40..=0x43 => {
            let bitmap_id = r.u16()?;
            let matrix = r.matrix()?;
            let repeating = style_type == 0x40 || style_type == 0x42;
            let smoothed = style_type == 0x40 || style_type == 0x41;
            FillStyle::Bitmap {
                bitmap_id,
                matrix,
                repeating,
                smoothed,
            }
        }
        _ => FillStyle::Solid(Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }),
    };
    Ok(style)
}

fn decode_fill_style_array(
    r: &mut Reader<'_>,
    shape_ver: u8,
) -> Result<Vec<FillStyle>, ParseError> {
    let count = read_count(r, shape_ver >= 2)?;
    let has_alpha = shape_ver >= 3;
    let mut styles = Vec::with_capacity(count.min(256));
    for _ in 0..count {
        styles.push(decode_single_fill_style(r, has_alpha)?);
    }
    Ok(styles)
}

fn decode_line_style_array(
    r: &mut Reader<'_>,
    shape_ver: u8,
) -> Result<Vec<LineStyle>, ParseError> {
    let count = read_count(r, shape_ver >= 2)?;
    let has_alpha = shape_ver >= 3;
    let mut styles = Vec::with_capacity(count.min(256));
    for _ in 0..count {
        let width_twips = r.u16()?;
        if shape_ver >= 4 {
            let _start_cap_style = r.ubits(2)?;
            let join_style = r.ubits(2)?;
            let has_fill = r.ubits(1)? != 0;
            let _no_h_scale = r.ubits(1)? != 0;
            let _no_v_scale = r.ubits(1)? != 0;
            let _pixel_hinting = r.ubits(1)? != 0;
            let _reserved = r.ubits(5)?;
            let _no_close = r.ubits(1)? != 0;
            let _end_cap_style = r.ubits(2)?;
            if join_style == 2 {
                let _miter_limit = r.u16()?;
            }
            if has_fill {
                let fill = decode_single_fill_style(r, true)?;
                let color = match fill {
                    FillStyle::Solid(c) => c,
                    FillStyle::LinearGradient { ref stops, .. }
                    | FillStyle::RadialGradient { ref stops, .. } => stops
                        .first()
                        .map_or(Rgba {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 255,
                        }, |s| s.color),
                    FillStyle::Bitmap { .. } => Rgba {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 255,
                    },
                };
                styles.push(LineStyle { width_twips, color });
            } else {
                let color = read_rgba(r, true)?;
                styles.push(LineStyle { width_twips, color });
            }
        } else {
            let color = read_rgba(r, has_alpha)?;
            styles.push(LineStyle { width_twips, color });
        }
    }
    Ok(styles)
}

fn decode_shape_records(
    r: &mut Reader<'_>,
    shape_ver: u8,
    limits: &Limits,
    tag_offset: usize,
) -> Result<Vec<ShapeRecord>, ParseError> {
    r.align();
    let mut num_fill_bits = u8::try_from(r.ubits(4)?).unwrap_or(0);
    let mut num_line_bits = u8::try_from(r.ubits(4)?).unwrap_or(0);
    let mut records = Vec::new();
    let mut edge_count: u32 = 0;

    loop {
        let type_flag = r.ubits(1)?;
        if type_flag == 0 {
            let state_new_styles = r.ubits(1)? != 0;
            let state_line_style = r.ubits(1)? != 0;
            let state_fill_style_1 = r.ubits(1)? != 0;
            let state_fill_style_0 = r.ubits(1)? != 0;
            let state_move_to = r.ubits(1)? != 0;
            if !state_new_styles
                && !state_line_style
                && !state_fill_style_1
                && !state_fill_style_0
                && !state_move_to
            {
                break;
            }
            let move_to = if state_move_to {
                let move_bits = u8::try_from(r.ubits(5)?).unwrap_or(0);
                let mx = r.sbits(move_bits)?;
                let my = r.sbits(move_bits)?;
                Some((mx, my))
            } else {
                None
            };
            let fill_style_0 = if state_fill_style_0 {
                Some(u16::try_from(r.ubits(num_fill_bits)?).unwrap_or(0))
            } else {
                None
            };
            let fill_style_1 = if state_fill_style_1 {
                Some(u16::try_from(r.ubits(num_fill_bits)?).unwrap_or(0))
            } else {
                None
            };
            let line_style = if state_line_style {
                Some(u16::try_from(r.ubits(num_line_bits)?).unwrap_or(0))
            } else {
                None
            };
            let (new_fill_styles, new_line_styles) = if state_new_styles {
                r.align();
                let nfs = decode_fill_style_array(r, shape_ver)?;
                let nls = decode_line_style_array(r, shape_ver)?;
                num_fill_bits = u8::try_from(r.ubits(4)?).unwrap_or(0);
                num_line_bits = u8::try_from(r.ubits(4)?).unwrap_or(0);
                (Some(nfs), Some(nls))
            } else {
                (None, None)
            };
            records.push(ShapeRecord::StyleChange {
                move_to,
                fill_style_0,
                fill_style_1,
                line_style,
                new_fill_styles,
                new_line_styles,
            });
        } else {
            edge_count = edge_count.saturating_add(1);
            if edge_count > limits.max_shape_edges {
                return Err(ParseError {
                    offset: tag_offset.saturating_add(r.position()),
                    kind: ParseErrorKind::LimitExceeded("shape edge count exceeds max_shape_edges"),
                });
            }
            let straight_flag = r.ubits(1)? != 0;
            let num_bits = u8::try_from(r.ubits(4)?).unwrap_or(0).saturating_add(2);
            if straight_flag {
                let general_line = r.ubits(1)? != 0;
                let (dx_twips, dy_twips) = if general_line {
                    (r.sbits(num_bits)?, r.sbits(num_bits)?)
                } else {
                    let vert_line = r.ubits(1)? != 0;
                    if vert_line {
                        (0, r.sbits(num_bits)?)
                    } else {
                        (r.sbits(num_bits)?, 0)
                    }
                };
                records.push(ShapeRecord::StraightEdge { dx_twips, dy_twips });
            } else {
                let control_dx_twips = r.sbits(num_bits)?;
                let control_dy_twips = r.sbits(num_bits)?;
                let anchor_dx_twips = r.sbits(num_bits)?;
                let anchor_dy_twips = r.sbits(num_bits)?;
                records.push(ShapeRecord::CurvedEdge {
                    control_dx_twips,
                    control_dy_twips,
                    anchor_dx_twips,
                    anchor_dy_twips,
                });
            }
        }
    }
    r.align();
    Ok(records)
}

fn decode_shape(
    r: &mut Reader<'_>,
    shape_ver: u8,
    limits: &Limits,
    tag_offset: usize,
) -> Result<ShapeDef, ParseError> {
    let id = r.u16()?;
    let bounds = r.rect()?;
    if shape_ver >= 4 {
        let _edge_bounds = r.rect()?;
        let _flags = r.u8()?;
    }
    let fill_styles = decode_fill_style_array(r, shape_ver)?;
    let line_styles = decode_line_style_array(r, shape_ver)?;
    let records = decode_shape_records(r, shape_ver, limits, tag_offset)?;
    Ok(ShapeDef {
        version: shape_ver,
        id,
        bounds,
        fill_styles,
        line_styles,
        records,
    })
}

fn decode_morph_shape(
    r: &mut Reader<'_>,
    version: u8,
    limits: &Limits,
    tag_offset: usize,
) -> Result<MorphShapeDef, ParseError> {
    let id = r.u16()?;
    let start_bounds = r.rect()?;
    let end_bounds = r.rect()?;
    if version >= 2 {
        let _start_edge_bounds = r.rect()?;
        let _end_edge_bounds = r.rect()?;
        let _flags = r.u8()?;
    }
    let _offset_to_end_edges = r.u32()?;

    let fill_count = read_count(r, true)?;
    let mut start_fill_styles = Vec::with_capacity(fill_count.min(128));
    let mut end_fill_styles = Vec::with_capacity(fill_count.min(128));
    for _ in 0..fill_count {
        let style_type = r.u8()?;
        match style_type {
            0x00 => {
                start_fill_styles.push(FillStyle::Solid(read_rgba(r, true)?));
                end_fill_styles.push(FillStyle::Solid(read_rgba(r, true)?));
            }
            0x10 | 0x12 | 0x13 => {
                let start_mat = r.matrix()?;
                let end_mat = r.matrix()?;
                let num_grads = usize::from(r.u8()? & 0x0F);
                let mut start_stops = Vec::with_capacity(num_grads);
                let mut end_stops = Vec::with_capacity(num_grads);
                for _ in 0..num_grads {
                    let s_ratio = r.u8()?;
                    let s_color = read_rgba(r, true)?;
                    let e_ratio = r.u8()?;
                    let e_color = read_rgba(r, true)?;
                    start_stops.push(GradientStop {
                        ratio: s_ratio,
                        color: s_color,
                    });
                    end_stops.push(GradientStop {
                        ratio: e_ratio,
                        color: e_color,
                    });
                }
                let (s_focal, e_focal) = if style_type == 0x13 {
                    (r.i16()?, r.i16()?)
                } else {
                    (0, 0)
                };
                if style_type == 0x10 {
                    start_fill_styles.push(FillStyle::LinearGradient {
                        matrix: start_mat,
                        stops: start_stops,
                    });
                    end_fill_styles.push(FillStyle::LinearGradient {
                        matrix: end_mat,
                        stops: end_stops,
                    });
                } else {
                    start_fill_styles.push(FillStyle::RadialGradient {
                        matrix: start_mat,
                        stops: start_stops,
                        focal_point_q8: s_focal,
                    });
                    end_fill_styles.push(FillStyle::RadialGradient {
                        matrix: end_mat,
                        stops: end_stops,
                        focal_point_q8: e_focal,
                    });
                }
            }
            0x40..=0x43 => {
                let bitmap_id = r.u16()?;
                let start_mat = r.matrix()?;
                let end_mat = r.matrix()?;
                let repeating = style_type == 0x40 || style_type == 0x42;
                let smoothed = style_type == 0x40 || style_type == 0x41;
                start_fill_styles.push(FillStyle::Bitmap {
                    bitmap_id,
                    matrix: start_mat,
                    repeating,
                    smoothed,
                });
                end_fill_styles.push(FillStyle::Bitmap {
                    bitmap_id,
                    matrix: end_mat,
                    repeating,
                    smoothed,
                });
            }
            _ => {
                start_fill_styles.push(FillStyle::Solid(Rgba {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                }));
                end_fill_styles.push(FillStyle::Solid(Rgba {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                }));
            }
        }
    }

    let line_count = read_count(r, true)?;
    let mut start_line_styles = Vec::with_capacity(line_count.min(128));
    let mut end_line_styles = Vec::with_capacity(line_count.min(128));
    for _ in 0..line_count {
        let start_w = r.u16()?;
        let end_w = r.u16()?;
        if version >= 2 {
            let _start_cap = r.ubits(2)?;
            let join_style = r.ubits(2)?;
            let has_fill = r.ubits(1)? != 0;
            let _rest = r.ubits(11)?;
            if join_style == 2 {
                let _miter = r.u16()?;
            }
            if has_fill {
                let _fill = decode_single_fill_style(r, true)?;
                start_line_styles.push(LineStyle {
                    width_twips: start_w,
                    color: Rgba {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 255,
                    },
                });
                end_line_styles.push(LineStyle {
                    width_twips: end_w,
                    color: Rgba {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 255,
                    },
                });
                continue;
            }
        }
        let start_c = read_rgba(r, true)?;
        let end_c = read_rgba(r, true)?;
        start_line_styles.push(LineStyle {
            width_twips: start_w,
            color: start_c,
        });
        end_line_styles.push(LineStyle {
            width_twips: end_w,
            color: end_c,
        });
    }

    let start_records = decode_shape_records(r, 3, limits, tag_offset)?;
    let end_records = if !r.is_empty() {
        decode_shape_records(r, 3, limits, tag_offset)?
    } else {
        Vec::new()
    };

    Ok(MorphShapeDef {
        version,
        id,
        start_bounds,
        end_bounds,
        start_fill_styles,
        end_fill_styles,
        start_line_styles,
        end_line_styles,
        start_records,
        end_records,
    })
}

fn decode_place_object2(r: &mut Reader<'_>, is_v3: bool) -> Result<PlaceObject2Tag, ParseError> {
    let has_clip_actions = r.ubits(1)? != 0;
    let has_clip_depth = r.ubits(1)? != 0;
    let has_name = r.ubits(1)? != 0;
    let has_ratio = r.ubits(1)? != 0;
    let has_color_transform = r.ubits(1)? != 0;
    let has_matrix = r.ubits(1)? != 0;
    let has_character = r.ubits(1)? != 0;
    let is_move = r.ubits(1)? != 0;

    let mut has_class_name = false;
    let mut has_image = false;
    if is_v3 {
        let _reserved = r.ubits(1)?;
        let _has_opaque_bg = r.ubits(1)? != 0;
        let _has_visible = r.ubits(1)? != 0;
        has_image = r.ubits(1)? != 0;
        has_class_name = r.ubits(1)? != 0;
        let _has_cache_as_bitmap = r.ubits(1)? != 0;
        let _has_blend_mode = r.ubits(1)? != 0;
        let _has_filter_list = r.ubits(1)? != 0;
    }

    let depth = r.u16()?;
    if has_class_name || (has_image && has_character) {
        let _class_name = r.string()?;
    }
    let character_id = if has_character { Some(r.u16()?) } else { None };
    let matrix = if has_matrix { Some(r.matrix()?) } else { None };
    let color_transform = if has_color_transform {
        Some(r.cxform_with_alpha()?)
    } else {
        None
    };
    let ratio = if has_ratio { Some(r.u16()?) } else { None };
    let name = if has_name { Some(r.string()?) } else { None };
    let clip_depth = if has_clip_depth { Some(r.u16()?) } else { None };
    let clip_actions_raw = if has_clip_actions || !r.is_empty() {
        let rem = r.remaining();
        if rem > 0 {
            Some(r.bytes(rem)?.to_vec())
        } else {
            None
        }
    } else {
        None
    };

    Ok(PlaceObject2Tag {
        is_move,
        depth,
        character_id,
        matrix,
        color_transform,
        ratio,
        name,
        clip_depth,
        clip_actions_raw,
    })
}

fn decode_font(
    r: &mut Reader<'_>,
    version: u8,
    limits: &Limits,
    tag_offset: usize,
) -> Result<FontDef, ParseError> {
    let id = r.u16()?;
    if version == 1 {
        if r.is_empty() {
            return Ok(FontDef {
                version,
                id,
                ..FontDef::default()
            });
        }
        let first_offset = usize::from(r.u16()?);
        let num_glyphs = first_offset.checked_div(2).unwrap_or(0);
        for _ in 1..num_glyphs {
            let _off = r.u16()?;
        }
        let mut glyphs = Vec::with_capacity(num_glyphs.min(256));
        for _ in 0..num_glyphs {
            glyphs.push(decode_shape_records(r, 1, limits, tag_offset)?);
        }
        return Ok(FontDef {
            version,
            id,
            glyphs,
            ..FontDef::default()
        });
    }

    let flags = r.u8()?;
    let is_italic = (flags & 0x02) != 0;
    let is_bold = (flags & 0x01) != 0;
    let wide_offsets = (flags & 0x08) != 0;
    let wide_codes = (flags & 0x04) != 0 || version >= 3;
    let _lang_code = r.u8()?;
    let name_len = usize::from(r.u8()?);
    let name_bytes = r.bytes(name_len)?;
    let name = String::from_utf8_lossy(name_bytes).into_owned();
    let num_glyphs = usize::from(r.u16()?);

    if num_glyphs == 0 {
        return Ok(FontDef {
            version,
            id,
            name,
            is_bold,
            is_italic,
            glyphs: Vec::new(),
            codes: Vec::new(),
        });
    }

    for _ in 0..=num_glyphs {
        if wide_offsets {
            let _off = r.u32()?;
        } else {
            let _off = r.u16()?;
        }
    }
    let mut glyphs = Vec::with_capacity(num_glyphs.min(256));
    for _ in 0..num_glyphs {
        glyphs.push(decode_shape_records(r, 1, limits, tag_offset)?);
    }
    let mut codes = Vec::with_capacity(num_glyphs.min(256));
    for _ in 0..num_glyphs {
        if wide_codes {
            codes.push(r.u16()?);
        } else {
            codes.push(u16::from(r.u8()?));
        }
    }
    Ok(FontDef {
        version,
        id,
        name,
        is_bold,
        is_italic,
        glyphs,
        codes,
    })
}

fn decode_text(r: &mut Reader<'_>, version: u8) -> Result<TextDef, ParseError> {
    let id = r.u16()?;
    let bounds = r.rect()?;
    let matrix = r.matrix()?;
    let glyph_bits = r.u8()?;
    let advance_bits = r.u8()?;
    let has_alpha = version >= 2;
    let mut records = Vec::new();

    while !r.is_empty() {
        let type_flag = r.ubits(1)?;
        if type_flag == 0 {
            let glyph_count = usize::try_from(r.ubits(7)?).unwrap_or(0);
            if glyph_count == 0 {
                break;
            }
            let mut glyphs = Vec::with_capacity(glyph_count);
            for _ in 0..glyph_count {
                let glyph_index = r.ubits(glyph_bits)?;
                let advance_twips = r.sbits(advance_bits)?;
                glyphs.push(GlyphEntry {
                    glyph_index,
                    advance_twips,
                });
            }
            r.align();
            records.push(TextRecord {
                font_id: None,
                color: None,
                x_offset_twips: None,
                y_offset_twips: None,
                text_height_twips: None,
                glyphs,
            });
        } else {
            let _reserved = r.ubits(3)?;
            let has_font = r.ubits(1)? != 0;
            let has_color = r.ubits(1)? != 0;
            let has_y_offset = r.ubits(1)? != 0;
            let has_x_offset = r.ubits(1)? != 0;
            let font_id = if has_font { Some(r.u16()?) } else { None };
            let color = if has_color {
                Some(read_rgba(r, has_alpha)?)
            } else {
                None
            };
            let x_offset_twips = if has_x_offset { Some(r.i16()?) } else { None };
            let y_offset_twips = if has_y_offset { Some(r.i16()?) } else { None };
            let text_height_twips = if has_font { Some(r.u16()?) } else { None };
            let glyph_count = usize::from(r.u8()?);
            let mut glyphs = Vec::with_capacity(glyph_count);
            for _ in 0..glyph_count {
                let glyph_index = r.ubits(glyph_bits)?;
                let advance_twips = r.sbits(advance_bits)?;
                glyphs.push(GlyphEntry {
                    glyph_index,
                    advance_twips,
                });
            }
            r.align();
            records.push(TextRecord {
                font_id,
                color,
                x_offset_twips,
                y_offset_twips,
                text_height_twips,
                glyphs,
            });
        }
    }
    Ok(TextDef {
        version,
        id,
        bounds,
        matrix,
        records,
    })
}

fn decode_edit_text(r: &mut Reader<'_>) -> Result<EditTextDef, ParseError> {
    let id = r.u16()?;
    let bounds = r.rect()?;
    let has_text = r.ubits(1)? != 0;
    let word_wrap = r.ubits(1)? != 0;
    let multiline = r.ubits(1)? != 0;
    let _password = r.ubits(1)? != 0;
    let read_only = r.ubits(1)? != 0;
    let has_text_color = r.ubits(1)? != 0;
    let has_max_length = r.ubits(1)? != 0;
    let has_font = r.ubits(1)? != 0;
    let has_font_class = r.ubits(1)? != 0;
    let _auto_size = r.ubits(1)? != 0;
    let has_layout = r.ubits(1)? != 0;
    let _no_select = r.ubits(1)? != 0;
    let _border = r.ubits(1)? != 0;
    let _was_static = r.ubits(1)? != 0;
    let html = r.ubits(1)? != 0;
    let use_outlines = r.ubits(1)? != 0;

    let font_id = if has_font { Some(r.u16()?) } else { None };
    if has_font_class {
        let _font_class = r.string()?;
    }
    let font_height_twips = if has_font { r.u16()? } else { 240 };
    let color = if has_text_color {
        read_rgba(r, true)?
    } else {
        Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }
    };
    if has_max_length {
        let _max_len = r.u16()?;
    }
    if has_layout {
        let _align = r.u8()?;
        let _left_margin = r.u16()?;
        let _right_margin = r.u16()?;
        let _indent = r.u16()?;
        let _leading = r.i16()?;
    }
    let variable_name = r.string()?;
    let initial_text = if has_text { Some(r.string()?) } else { None };

    Ok(EditTextDef {
        id,
        bounds,
        font_id,
        font_height_twips,
        color,
        variable_name,
        initial_text,
        word_wrap,
        multiline,
        read_only,
        html,
        use_outlines,
    })
}

fn decode_button(r: &mut Reader<'_>, version: u8) -> Result<ButtonDef, ParseError> {
    let id = r.u16()?;
    let track_as_menu = if version >= 2 {
        (r.u8()? & 0x01) != 0
    } else {
        false
    };
    let action_offset = if version >= 2 {
        usize::from(r.u16()?)
    } else {
        0
    };

    let mut records = Vec::new();
    while !r.is_empty() {
        let _reserved = r.ubits(2)?;
        let has_blend_mode = r.ubits(1)? != 0;
        let has_filter_list = r.ubits(1)? != 0;
        let state_hit_test = r.ubits(1)? != 0;
        let state_down = r.ubits(1)? != 0;
        let state_over = r.ubits(1)? != 0;
        let state_up = r.ubits(1)? != 0;
        if !has_blend_mode
            && !has_filter_list
            && !state_hit_test
            && !state_down
            && !state_over
            && !state_up
        {
            break;
        }
        let character_id = r.u16()?;
        let depth = r.u16()?;
        let matrix = r.matrix()?;
        let color_transform = if version >= 2 {
            r.cxform_with_alpha()?
        } else {
            ColorTransform::identity()
        };
        if version >= 2 && has_filter_list {
            let filter_count = r.u8()?;
            if filter_count > 0 {
                // Filters are out of scope in v1.
                break;
            }
        }
        if version >= 2 && has_blend_mode {
            let _blend = r.u8()?;
        }
        records.push(ButtonRecord {
            state_up,
            state_over,
            state_down,
            state_hit_test,
            character_id,
            depth,
            matrix,
            color_transform,
        });
    }

    let mut actions = Vec::new();
    if version == 1 {
        let rem = r.remaining();
        if rem > 0 {
            actions.push(ButtonAction {
                condition: 0,
                actions: r.bytes(rem)?.to_vec(),
            });
        }
    } else if action_offset > 0 && !r.is_empty() {
        while r.remaining() >= 4 {
            let cond_action_size = usize::from(r.u16()?);
            let condition = r.u16()?;
            let bytecode_len = if cond_action_size == 0 {
                r.remaining()
            } else {
                cond_action_size.saturating_sub(4).min(r.remaining())
            };
            let code_bytes = r.bytes(bytecode_len)?.to_vec();
            actions.push(ButtonAction {
                condition,
                actions: code_bytes,
            });
            if cond_action_size == 0 {
                break;
            }
        }
    }

    Ok(ButtonDef {
        version,
        id,
        track_as_menu,
        records,
        actions,
    })
}

fn decode_sound_info(r: &mut Reader<'_>) -> Result<SoundInfo, ParseError> {
    let _reserved = r.ubits(2)?;
    let sync_stop = r.ubits(1)? != 0;
    let sync_no_multiple = r.ubits(1)? != 0;
    let has_envelope = r.ubits(1)? != 0;
    let has_loops = r.ubits(1)? != 0;
    let has_out_point = r.ubits(1)? != 0;
    let has_in_point = r.ubits(1)? != 0;

    let in_point = if has_in_point { Some(r.u32()?) } else { None };
    let out_point = if has_out_point { Some(r.u32()?) } else { None };
    let loop_count = if has_loops { Some(r.u16()?) } else { None };
    let mut envelope = Vec::new();
    if has_envelope {
        let env_points = usize::from(r.u8()?);
        for _ in 0..env_points {
            let pos44 = r.u32()?;
            let left_level = r.u16()?;
            let right_level = r.u16()?;
            envelope.push(SoundEnvelopePoint {
                pos44,
                left_level,
                right_level,
            });
        }
    }
    Ok(SoundInfo {
        sync_stop,
        sync_no_multiple,
        in_point,
        out_point,
        loop_count,
        envelope,
    })
}
