# Golden Image Registry (`GOLDENS.md`)

Per TSD 3.6 and 3.7 (Provisional golden policy):
- Owner-approved goldens live under `crates/swfr-render/tests/golden/`.
- AI-generated provisional goldens live under `crates/swfr-render/tests/golden-provisional/` and are verified on every test run both by pixel-level comparison (tolerance `2` per channel, `<= 1%` outlier pixels per TSD 2.13) and by explicit coordinate/bounding-box color sanity checks in `crates/swfr-render/tests/render_goldens.rs`.

## Provisional Goldens (`crates/swfr-render/tests/golden-provisional/`)

- `m2_solid_rect.png`: `64x64` dark blue stage (`#102030`) with a centered `32x32` solid red square (`#E02020`) outlined by a `2px` white stroke (`#FFFFFF`).
- `m2_linear_gradient.png`: `64x64` charcoal stage (`#181820`) with a `40x40` square filled by a left-to-right green (`#00FF40`) to blue (`#0040FF`) linear gradient.
- `m2_radial_gradient.png`: `64x64` midnight stage (`#101018`) with a `44x44` square filled by a centered yellow (`#FFFF00`) to magenta (`#FF0080`) radial gradient.
- `m2_lossless_bitmap_fill.png`: `64x64` gray stage (`#202020`) with a `40x40` square filled by a repeating `4x4` `DefineBitsLossless` red/green checkerboard bitmap.
- `m2_clip_depth_mask.png`: `64x64` dark gray stage (`#303030`) where a wide `56x24` bright green bar (`#00E040`) at depth 2 is masked by a narrow `20x48` vertical window at depth 1 (`clip_depth = 3`), leaving only the central `20x24` green intersection visible.
- `m2_morph_shape_mid.png`: `64x64` navy stage (`#151525`) showing a `DefineMorphShape` at `ratio = 32768` (50% interpolation between a left red square `#FF0000` and a right blue square `#0000FF`), producing a centered purple square (`#7F007F`).
- `m3_button_and_text.png`: `64x64` slate stage (`#1E2430`) showing a `DefineButton2` hovered in `Over` state (`#FFAA00` top button bar) and a `DefineEditText` dynamic text field rendering `"88"` in bright green (`#40FF80`) in the lower half.
