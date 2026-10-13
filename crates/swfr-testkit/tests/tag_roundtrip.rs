//! M1 verification: SWF v19 tag round-trip tests, FWS/CWS/ZWS container tests,
//! decompression limit tests, and 100,000-iteration deterministic mutator test.
#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use swfr_reader::{ColorTransform, Matrix, Rect};
use swfr_swf::{
    AudioFormat, ButtonAction, ButtonDef, ButtonRecord, Compression, EditTextDef, FillStyle,
    FontDef, GradientStop, Limits, LineStyle, LosslessBitmapDef, MorphShapeDef, Movie,
    PlaceObject2Tag, PlaceObjectTag, Rgba, ShapeDef, ShapeRecord, SoundDef, SoundEnvelopePoint,
    SoundInfo, SoundStreamHeadDef, Tag, TextDef, TextGlyphEntry, TextRecord,
};
use swfr_testkit::{Mutator, SwfBuilder};

#[allow(clippy::too_many_lines)]
fn all_supported_tags() -> Vec<Tag> {
    let rect = Rect {
        x_min: 0,
        x_max: 2000,
        y_min: 0,
        y_max: 1600,
    };
    let sample_records = vec![
        ShapeRecord::StyleChange {
            move_to: Some((100, 100)),
            fill_style_0: Some(1),
            fill_style_1: None,
            line_style: Some(1),
            new_fill_styles: None,
            new_line_styles: None,
        },
        ShapeRecord::StraightEdge {
            dx_twips: 400,
            dy_twips: 0,
        },
        ShapeRecord::CurvedEdge {
            control_dx_twips: 200,
            control_dy_twips: 200,
            anchor_dx_twips: -600,
            anchor_dy_twips: -200,
        },
    ];

    vec![
        Tag::FileAttributes {
            use_direct_blit: false,
            use_gpu: false,
            has_metadata: true,
            actionscript3: false,
            use_network: false,
        },
        Tag::SetBackgroundColor {
            r: 0x12,
            g: 0x34,
            b: 0x56,
        },
        Tag::Metadata {
            xml: "<rdf:RDF/>".to_owned(),
        },
        Tag::Protect {
            password_md5: Some("d41d8cd98f00b204e9800998ecf8427e".to_owned()),
        },
        Tag::EnableDebugger {
            password_md5: Some("debug123".to_owned()),
        },
        Tag::ScriptLimits {
            max_recursion_depth: 256,
            script_timeout_seconds: 15,
        },
        Tag::SetTabIndex {
            depth: 1,
            tab_index: 2,
        },
        Tag::FrameLabel {
            name: "intro".to_owned(),
            is_anchor: false,
        },
        Tag::DefineSceneAndFrameLabelData {
            scenes: vec![(0, "Scene 1".to_owned())],
            labels: vec![(0, "intro".to_owned())],
        },
        Tag::DefineShape(ShapeDef {
            id: 1,
            version: 1,
            bounds: rect,
            fill_styles: vec![FillStyle::Solid(Rgba::rgb(255, 0, 0))],
            line_styles: vec![LineStyle {
                width_twips: 20,
                color: Rgba::rgb(0, 0, 0),
            }],
            records: sample_records.clone(),
        }),
        Tag::DefineShape(ShapeDef {
            id: 2,
            version: 2,
            bounds: rect,
            fill_styles: vec![FillStyle::LinearGradient {
                matrix: Matrix::IDENTITY,
                stops: vec![
                    GradientStop {
                        ratio: 0,
                        color: Rgba::rgb(0, 255, 0),
                    },
                    GradientStop {
                        ratio: 255,
                        color: Rgba::rgb(0, 0, 255),
                    },
                ],
            }],
            line_styles: Vec::new(),
            records: sample_records.clone(),
        }),
        Tag::DefineShape(ShapeDef {
            id: 3,
            version: 3,
            bounds: rect,
            fill_styles: vec![
                FillStyle::RadialGradient {
                    matrix: Matrix::IDENTITY,
                    stops: vec![
                        GradientStop {
                            ratio: 0,
                            color: Rgba {
                                r: 255,
                                g: 255,
                                b: 0,
                                a: 200,
                            },
                        },
                        GradientStop {
                            ratio: 255,
                            color: Rgba {
                                r: 255,
                                g: 0,
                                b: 255,
                                a: 128,
                            },
                        },
                    ],
                    focal_point_q8: 64,
                },
                FillStyle::Bitmap {
                    bitmap_id: 10,
                    matrix: Matrix::IDENTITY,
                    repeating: true,
                    smoothed: false,
                },
            ],
            line_styles: vec![LineStyle {
                width_twips: 40,
                color: Rgba {
                    r: 255,
                    g: 255,
                    b: 255,
                    a: 255,
                },
            }],
            records: sample_records.clone(),
        }),
        Tag::DefineShape(ShapeDef {
            id: 4,
            version: 4,
            bounds: rect,
            fill_styles: vec![FillStyle::Solid(Rgba {
                r: 10,
                g: 20,
                b: 30,
                a: 255,
            })],
            line_styles: vec![LineStyle {
                width_twips: 20,
                color: Rgba {
                    r: 200,
                    g: 100,
                    b: 50,
                    a: 255,
                },
            }],
            records: sample_records.clone(),
        }),
        Tag::DefineScalingGrid {
            character_id: 4,
            splitter: rect,
        },
        Tag::DefineMorphShape(MorphShapeDef {
            id: 5,
            version: 1,
            start_bounds: rect,
            end_bounds: rect,
            start_fill_styles: vec![FillStyle::Solid(Rgba {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            })],
            end_fill_styles: vec![FillStyle::Solid(Rgba {
                r: 0,
                g: 0,
                b: 255,
                a: 255,
            })],
            start_line_styles: vec![LineStyle {
                width_twips: 20,
                color: Rgba::rgb(0, 0, 0),
            }],
            end_line_styles: vec![LineStyle {
                width_twips: 40,
                color: Rgba::rgb(255, 255, 255),
            }],
            start_records: sample_records.clone(),
            end_records: sample_records.clone(),
        }),
        Tag::DefineMorphShape(MorphShapeDef {
            id: 6,
            version: 2,
            start_bounds: rect,
            end_bounds: rect,
            start_fill_styles: vec![FillStyle::Solid(Rgba::rgb(10, 20, 30))],
            end_fill_styles: vec![FillStyle::Solid(Rgba::rgb(40, 50, 60))],
            start_line_styles: Vec::new(),
            end_line_styles: Vec::new(),
            start_records: sample_records.clone(),
            end_records: sample_records.clone(),
        }),
        Tag::JPEGTables {
            jpeg_data: vec![0xFF, 0xD8, 0xFF, 0xD9],
        },
        Tag::DefineBits {
            id: 10,
            jpeg_data: vec![0xFF, 0xD8, 0x00, 0xFF, 0xD9],
        },
        Tag::DefineBitsJPEG2 {
            id: 11,
            image_data: vec![0xFF, 0xD8, 0xFF, 0xD9],
        },
        Tag::DefineBitsJPEG3 {
            id: 12,
            image_data: vec![0xFF, 0xD8, 0xFF, 0xD9],
            alpha_data: vec![0x78, 0x9C, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01],
        },
        Tag::DefineBitsJPEG4 {
            id: 13,
            deblock_param: 256,
            image_data: vec![0xFF, 0xD8, 0xFF, 0xD9],
            alpha_data: vec![0x78, 0x9C],
        },
        Tag::DefineBitsLossless(LosslessBitmapDef {
            id: 14,
            version: 1,
            format: 5,
            width: 2,
            height: 2,
            color_table_size: None,
            zlib_data: vec![1, 2, 3, 4],
        }),
        Tag::DefineBitsLossless(LosslessBitmapDef {
            id: 15,
            version: 2,
            format: 3,
            width: 2,
            height: 2,
            color_table_size: Some(1),
            zlib_data: vec![5, 6, 7, 8],
        }),
        Tag::DefineFont(FontDef {
            id: 20,
            version: 1,
            name: String::new(),
            is_bold: false,
            is_italic: false,
            glyphs: vec![sample_records.clone()],
            codes: Vec::new(),
        }),
        Tag::DefineFontInfo {
            font_id: 20,
            name: "TestFont".to_owned(),
            codes: vec![65],
        },
        Tag::DefineFont(FontDef {
            id: 21,
            version: 2,
            name: "Sans".to_owned(),
            is_bold: true,
            is_italic: false,
            glyphs: vec![sample_records.clone()],
            codes: vec![65],
        }),
        Tag::DefineFont(FontDef {
            id: 22,
            version: 3,
            name: "Serif".to_owned(),
            is_bold: false,
            is_italic: true,
            glyphs: vec![sample_records],
            codes: vec![66],
        }),
        Tag::DefineFontAlignZones {
            font_id: 22,
            csm_table_hint: 1,
            raw_zones: vec![0, 1, 2],
        },
        Tag::DefineFontName {
            font_id: 22,
            font_name: "Serif Regular".to_owned(),
            font_copyright: "CC0".to_owned(),
        },
        Tag::DefineText(TextDef {
            id: 30,
            version: 1,
            bounds: rect,
            matrix: Matrix::IDENTITY,
            records: vec![TextRecord {
                font_id: Some(20),
                color: Some(Rgba::rgb(255, 255, 255)),
                x_offset_twips: Some(20),
                y_offset_twips: Some(200),
                text_height_twips: Some(240),
                glyphs: vec![TextGlyphEntry {
                    glyph_index: 0,
                    advance_twips: 120,
                }],
            }],
        }),
        Tag::DefineText(TextDef {
            id: 31,
            version: 2,
            bounds: rect,
            matrix: Matrix::IDENTITY,
            records: vec![TextRecord {
                font_id: Some(21),
                color: Some(Rgba {
                    r: 10,
                    g: 20,
                    b: 30,
                    a: 200,
                }),
                x_offset_twips: None,
                y_offset_twips: None,
                text_height_twips: Some(240),
                glyphs: vec![TextGlyphEntry {
                    glyph_index: 0,
                    advance_twips: 140,
                }],
            }],
        }),
        Tag::DefineEditText(EditTextDef {
            id: 32,
            bounds: rect,
            font_id: Some(21),
            font_height_twips: 240,
            color: Rgba::rgb(0, 255, 0),
            variable_name: "score".to_owned(),
            initial_text: Some("0".to_owned()),
            word_wrap: true,
            multiline: false,
            read_only: true,
            html: false,
            use_outlines: true,
        }),
        Tag::CSMTextSettings {
            text_id: 32,
            use_flash_type: 1,
            grid_fit: 2,
            thickness_bits: 0,
            sharpness_bits: 0,
        },
        Tag::DefineButton(ButtonDef {
            id: 40,
            version: 1,
            track_as_menu: false,
            records: vec![ButtonRecord {
                state_hit_test: true,
                state_down: true,
                state_over: true,
                state_up: true,
                character_id: 1,
                depth: 1,
                matrix: Matrix::IDENTITY,
                color_transform: ColorTransform::IDENTITY,
            }],
            actions: vec![ButtonAction {
                condition: 0,
                actions: vec![0x07, 0x00],
            }],
        }),
        Tag::DefineButton(ButtonDef {
            id: 41,
            version: 2,
            track_as_menu: true,
            records: vec![ButtonRecord {
                state_hit_test: true,
                state_down: false,
                state_over: true,
                state_up: true,
                character_id: 2,
                depth: 1,
                matrix: Matrix::IDENTITY,
                color_transform: ColorTransform::IDENTITY,
            }],
            actions: vec![
                ButtonAction {
                    condition: 0x0001,
                    actions: vec![0x06, 0x00],
                },
                ButtonAction {
                    condition: 0x0004,
                    actions: vec![0x07, 0x00],
                },
            ],
        }),
        Tag::DefineButtonCxform {
            button_id: 40,
            cxform: ColorTransform::IDENTITY,
        },
        Tag::DefineButtonSound {
            button_id: 40,
            raw: vec![0, 0, 0, 0, 0, 0, 0, 0],
        },
        Tag::DefineSound(SoundDef {
            id: 50,
            format: AudioFormat::PcmLittleEndian,
            sample_rate_code: 3,
            is_16bit: true,
            is_stereo: false,
            sample_count: 2,
            data: vec![0, 0, 100, 0],
        }),
        Tag::StartSound {
            sound_id: 50,
            sound_info: SoundInfo {
                sync_stop: false,
                sync_no_multiple: true,
                in_point: Some(0),
                out_point: Some(2),
                loop_count: Some(1),
                envelope: vec![SoundEnvelopePoint {
                    pos44: 0,
                    left_level: 32768,
                    right_level: 32768,
                }],
            },
        },
        Tag::StartSound2 {
            class_name: "BgmSound".to_owned(),
            sound_info: SoundInfo {
                sync_stop: true,
                sync_no_multiple: false,
                in_point: None,
                out_point: None,
                loop_count: None,
                envelope: Vec::new(),
            },
        },
        Tag::SoundStreamHead(SoundStreamHeadDef {
            version: 1,
            playback_rate_code: 3,
            playback_is_16bit: true,
            playback_is_stereo: true,
            stream_format: AudioFormat::Adpcm,
            stream_rate_code: 3,
            stream_is_16bit: true,
            stream_is_stereo: false,
            stream_sample_count: 735,
            latency_seek: None,
        }),
        Tag::SoundStreamHead(SoundStreamHeadDef {
            version: 2,
            playback_rate_code: 3,
            playback_is_16bit: true,
            playback_is_stereo: true,
            stream_format: AudioFormat::Mp3,
            stream_rate_code: 3,
            stream_is_16bit: true,
            stream_is_stereo: true,
            stream_sample_count: 735,
            latency_seek: Some(0),
        }),
        Tag::SoundStreamBlock {
            data: vec![0, 1, 2, 3],
        },
        Tag::PlaceObject(PlaceObjectTag {
            character_id: 1,
            depth: 1,
            matrix: Matrix::IDENTITY,
            color_transform: Some(ColorTransform::IDENTITY),
        }),
        Tag::PlaceObject2(PlaceObject2Tag {
            is_move: false,
            depth: 2,
            character_id: Some(2),
            matrix: Some(Matrix::IDENTITY),
            color_transform: Some(ColorTransform::IDENTITY),
            ratio: Some(32768),
            name: Some("hero".to_owned()),
            clip_depth: Some(5),
            clip_actions_raw: None,
        }),
        Tag::PlaceObject3(PlaceObject2Tag {
            is_move: true,
            depth: 2,
            character_id: None,
            matrix: Some(Matrix::IDENTITY),
            color_transform: None,
            ratio: None,
            name: None,
            clip_depth: None,
            clip_actions_raw: None,
        }),
        Tag::RemoveObject {
            character_id: 1,
            depth: 1,
        },
        Tag::RemoveObject2 { depth: 2 },
        Tag::DefineSprite {
            id: 60,
            frame_count: 1,
            tags: vec![
                Tag::PlaceObject2(PlaceObject2Tag {
                    is_move: false,
                    depth: 1,
                    character_id: Some(1),
                    matrix: Some(Matrix::IDENTITY),
                    color_transform: None,
                    ratio: None,
                    name: None,
                    clip_depth: None,
                    clip_actions_raw: None,
                }),
                Tag::ShowFrame,
                Tag::End,
            ],
        },
        Tag::DoInitAction {
            sprite_id: 60,
            actions: vec![0x07, 0x00],
        },
        Tag::DoAction {
            actions: vec![0x06, 0x00],
        },
        Tag::ExportAssets {
            exports: vec![(60, "HeroClip".to_owned())],
        },
        Tag::ImportAssets {
            url: "shared.swf".to_owned(),
            imports: vec![(99, "SharedBtn".to_owned())],
        },
        Tag::SymbolClass {
            symbols: vec![(0, "MainTimeline".to_owned())],
        },
        Tag::Unknown {
            code: 60, // DefineVideoStream (unsupported in v1 per docs/notes/tag-coverage.md)
            raw: vec![0xAA, 0xBB, 0xCC],
        },
        Tag::ShowFrame,
        Tag::End,
    ]
}

#[test]
fn roundtrip_all_supported_swf_v19_tags() {
    let tags = all_supported_tags();
    let mut builder = SwfBuilder::new(10, 100, 80, 60.0, 1);
    for tag in &tags {
        builder.push_tag(tag);
    }
    let swf_bytes = builder.build();
    let movie = Movie::parse(&swf_bytes).expect("round-trip movie must parse cleanly");
    assert_eq!(movie.header.version, 10);
    assert_eq!(movie.header.frame_count, 1);
    assert_eq!(movie.tags, tags);
}

#[test]
fn roundtrip_compressions_fws_cws_zws_and_long_tags() {
    let tags = vec![
        Tag::SetBackgroundColor {
            r: 0x20,
            g: 0x40,
            b: 0x80,
        },
        Tag::DoAction {
            actions: vec![0x96; 128], // > 63 bytes triggers long tag header (0x3F)
        },
        Tag::ShowFrame,
        Tag::End,
    ];

    for comp in [
        Compression::Uncompressed,
        Compression::Zlib,
        Compression::Lzma,
    ] {
        let mut builder = SwfBuilder::new(13, 320, 240, 30.0, 1).with_compression(comp);
        for t in &tags {
            builder.push_tag(t);
        }
        // Also test a small tag explicitly written with a 6-byte long RECORDHEADER
        builder.push_long_raw_tag(9, vec![1, 2, 3]);
        let bytes = builder.build();
        let parsed = Movie::parse(&bytes).expect("compressed movie should parse");
        assert_eq!(parsed.header.compression, comp);
        assert_eq!(parsed.tags.first(), tags.first());
    }

    // Verify 10x decompression ratio limit rejects a highly compressible payload when capped tightly
    let mut bomb_builder =
        SwfBuilder::new(10, 320, 240, 60.0, 1).with_compression(Compression::Zlib);
    bomb_builder.push_raw_tag(77, vec![b'A'; 50_000]); // 50 KB of 'A' compresses to < 200 bytes (> 10x ratio)
    let bomb_bytes = bomb_builder.build();
    let res = Movie::parse(&bomb_bytes);
    assert!(
        res.is_err(),
        "decompression bomb exceeding 10x ratio must be rejected"
    );

    // Verify custom limits can also cap tag count
    let small_limits = Limits {
        max_tag_count: 2,
        ..Limits::default()
    };
    let mut many_tags = SwfBuilder::new(8, 100, 100, 60.0, 1);
    for _ in 0..5 {
        many_tags.push_tag(&Tag::ShowFrame);
    }
    let many_bytes = many_tags.build();
    assert!(Movie::parse_with_limits(&many_bytes, &small_limits).is_err());
}

#[test]
fn mutator_100k_iterations_never_panics() {
    let mut builder = SwfBuilder::new(10, 160, 120, 60.0, 1);
    for tag in all_supported_tags().iter().take(12) {
        builder.push_tag(tag);
    }
    let seed_swf = builder.build();
    let mut mutator = Mutator::new(0x5357_4652_4655_5A5A);

    for _ in 0..100_000 {
        let mutated = mutator.mutate(&seed_swf);
        let _ = Movie::parse(&mutated);
    }
}
