# R7 — `ruffle_render` and Shape Pipeline (`06-render.md`)

- **Pinned Release Tag:** `v0.7.1`
- **Pinned Commit SHA:** `89a7049965f3090e706265670df309a5128233ac`
- **Repository:** `https://github.com/ruffle-rs/ruffle`
- **Overall Confidence:** **High** (verified directly from `render/src/{backend,commands,shape_utils,tessellator,filters,blend}.rs`, `render/canvas/src/lib.rs`, and `render/wgpu/src/{backend,surface/commands}.rs` at `89a7049965f3090e706265670df309a5128233ac`).

---

## 1. `RenderBackend` Trait and `CommandList` (`Confidence: High`)

- **`RenderBackend` (`render/src/backend.rs:28-129`):**
  - Abstracts all GPU/2D rendering behind a single trait:
    - Shape registration: `register_shape(DistilledShape, &dyn BitmapSource) -> ShapeHandle` (`render/src/backend.rs:33-37`) and `register_shape_with_scale` (`render/src/backend.rs:39-47`).
    - Texture lifecycle: `register_bitmap`, `update_texture`, `create_empty_texture` (`render/src/backend.rs:90-102`).
    - Frame submission: `submit_frame(clear: Color, commands: CommandList, cache_entries: Vec<BitmapCacheEntry>)` (`render/src/backend.rs:83-88`).
    - Offscreen & filters (`BitmapData.draw`, `cacheAsBitmap`): `is_offscreen_supported`, `render_offscreen`, `is_filter_supported`, `apply_filter`, `resolve_sync_handle` (`render/src/backend.rs:49-80, 124-128`).
    - Stage3D & PixelBender: `create_context3d`, `compile_pixelbender_shader`, `run_pixelbender_shader` (`render/src/backend.rs:104, 112-123`).
- **`CommandList` & `Command` (`render/src/commands.rs:8-242`):**
  - Display objects do not call `RenderBackend` draw methods directly; instead they record 12 high-level `Command` variants into `RenderContext.commands: CommandList` (`render/src/commands.rs:205-242`):
    - `RenderShape { shape, transform }`, `RenderBitmap { bitmap, transform, smoothing, pixel_snapping, region }`, `RenderStage3D { bitmap, transform }`, `DrawRect { color, matrix }`, `DrawLine { color, matrix }`, `DrawLineRect { color, matrix }`.
    - Stencil masking: `PushMask`, `ActivateMask`, `DeactivateMask`, `PopMask`.
    - Alpha masking (`cacheAsBitmap`): `RenderAlphaMask { maskee_commands, mask_commands }`.
    - Blend modes: `Blend(CommandList, RenderBlendMode)`.
  - **Nested Mask Suppression (`render/src/commands.rs:46-48, 165-201`):**
    - Flash Player does not support nested maskers inside a masker. `CommandList` tracks `maskers_in_progress: u32`: if a masker child itself tries to `push_mask()`, `maskers_in_progress` increments above `1` and all drawing commands inside the nested masker are discarded (`if self.maskers_in_progress <= 1`, `render/src/commands.rs:93-163`)!

---

## 2. Flash Dual-Fill Edge Conversion to Simple Paths (`ShapeConverter`) (`Confidence: High`)

- **The Problem:** Flash SWF shapes store edges as an unordered "edge soup" where each directed edge can have both a left fill (`fill_style_0`, negative/CCW side) and a right fill (`fill_style_1`, positive/CW side), plus a `line_style`, and a single shape can contain multiple drawing layers separated by `StyleChangeRecord.new_styles`.
- **Ruffle's Solution (`ShapeConverter` in `render/src/shape_utils.rs:181-576`):**
  1. Maintains three `ActivePath` accumulators (`fill_style0`, `fill_style1`, `line_style`) and per-layer vectors `fills: Vec<PendingPath>` and `strokes: Vec<PendingPath>` (`render/src/shape_utils.rs:386-394`).
  2. As edges (`StraightEdge`, `CurvedEdge`) are visited, points are appended to each active style's `PathSegment` (`render/src/shape_utils.rs:509-524`).
  3. Whenever `move_to` or a style change occurs, `flush_fill` flushes the `ActivePath` into `fills[style_id - 1]`:
     - For `fill_style_1`, `flip = false` (`render/src/shape_utils.rs:457, 530`).
     - For `fill_style_0`, **`flip = true` reverses the points in the segment** (`self.segment.flip()`, `render/src/shape_utils.rs:226-228, 358-360, 532`) so both `fill_style_0` and `fill_style_1` segments share consistent clockwise winding!
  4. `PendingPath::add_segment` (`render/src/shape_utils.rs:302-327`) stitches segments whose endpoints match (`other.end() == new_segment.start()` or `new_segment.end() == other.start()`) using `swap_remove` until closed contours form.
  5. When a `StyleChangeRecord` has `new_styles: Some(...)` or the shape ends, `flush_layer()` (`render/src/shape_utils.rs:537-575`) emits:
     - All `DrawPath::Fill` paths for that layer in **ascending `fill_style` ID order** (`render/src/shape_utils.rs:542-555`).
     - Followed by all `DrawPath::Stroke` paths for that layer in **ascending `line_style` ID order** (`render/src/shape_utils.rs:560-574`), because strokes in a layer always render above fills in the same layer.

---

## 3. Winding Rules, Strokes, Gradients, Bitmap Fills, Masks, Blend Modes, Filters (`Confidence: High`)

- **Winding Rules (`FillRule`, `render/src/shape_utils.rs:11-62`):**
  - Default is `FillRule::EvenOdd`; `DefineShape4` sets `FillRule::NonZero` when `swf::ShapeFlag::NON_ZERO_WINDING_RULE` is present (`render/src/shape_utils.rs:48-54`).
  - Notice that in `shape_hit_test` (`render/src/shape_utils.rs:660-691`), interior edges where **both** `has_fill_style0` and `has_fill_style1` are true are **skipped** (`if has_fill_style1 { if !has_fill_style0 { ... } } else if has_fill_style0 { ... }`), because an edge with fills on both sides is internal to the filled region!
- **Strokes & Mask Omission (`render/src/tessellator.rs:155-190`):**
  - Flash enforces a **minimum stroke width of 1 pixel (20 twips)** (`width.max(1.0 / scale)`, `render/src/tessellator.rs:180-186`).
  - When a shape is used as a stencil mask, **strokes are omitted** (`self.mask_index_count`, `render/src/tessellator.rs:156-160`).
- **Gradients & Bitmap Fills (`render/src/tessellator.rs:78-146`, `render/canvas/src/lib.rs:106-146`):**
  - Supports `LinearGradient`, `RadialGradient`, `FocalGradient` (with `focal_point` and `Spread`/`Interpolation` modes), and `Bitmap` fills (`is_smoothed`, `is_repeating`).
- **Stencil Masks (`apply_standard_mask_and_scroll`, `core/src/display_object.rs:1226-1290`):**
  - Uses a 4-phase stencil protocol (`PushMask` -> draw masker -> `ActivateMask` -> draw maskee -> `DeactivateMask` -> re-draw masker to decrement stencil -> `PopMask`).

---

## 4. Could a `tiny-skia` `RenderBackend` Be Written, and What Would It Take? (`Confidence: High`)

- **Feasibility:** **Yes — `render/canvas/src/lib.rs` (`WebCanvasRenderBackend`, 1,397 lines) is a direct blueprint for a 2D path-based `RenderBackend`!**
- **Why `WebCanvasRenderBackend` is the exact template:**
  - Unlike `ruffle_render_wgpu` (which uses `ShapeTessellator` / `lyon` to triangulate shapes into GPU vertex buffers), `WebCanvasRenderBackend::register_shape` (`render/canvas/src/lib.rs:490-650`) consumes `DistilledShape` (`DrawPath::Fill` and `DrawPath::Stroke`) directly and builds 2D vector paths (`Path2d::move_to`, `line_to`, `quadratic_curve_to`, `bezier_curve_to`) without `lyon`!
- **What a `tiny-skia` `RenderBackend` would require (~600–900 LOC):**
  1. `register_shape`: Convert `DistilledShape` (`Vec<DrawPath>`) into pre-built `tiny_skia::Path` + style records (just like `swfr-render`'s `CompiledShape`).
  2. `register_bitmap` / `update_texture` / `create_empty_texture`: Store `Arc<Mutex<tiny_skia::Pixmap>>` inside `BitmapHandle(Arc<dyn BitmapHandleImpl>)`.
  3. `submit_frame` & `CommandHandler`: Execute `RenderShape`, `RenderBitmap`, `DrawRect`, `DrawLine`, `PushMask`/`ActivateMask`/`DeactivateMask`/`PopMask` (using a `tiny_skia::Mask` stack), `RenderAlphaMask`, and `Blend` (rendering the sub-`CommandList` into an offscreen `Pixmap` and compositing with `tiny_skia::BlendMode`).
  4. `render_offscreen`: Execute the offscreen `CommandList` onto the target `BitmapHandle`'s `Pixmap` (enabling `BitmapData.draw()` and `cacheAsBitmap` on CPU!).
- **Caveat in `ruffle_render/Cargo.toml`:** `ruffle_render` currently has an unconditional dependency on `wgpu = "30.0.1"` (`render/Cargo.toml:36`) even though only `render/src/backend.rs:160-169` (`#[cfg(feature = "wgpu")] impl RawTexture for wgpu::Texture`) uses it.

---

## 5. Verification Against Regression Tests (`Confidence: High`)

1. **Claim 1 — `FillRule::EvenOdd` vs `FillRule::NonZero` (`swf::ShapeFlag::NON_ZERO_WINDING_RULE`, `render/src/shape_utils.rs:48-61`):**
   - Verified against `tests/tests/swfs/avm1/hittest_winding_rule/output.txt:1-12`, where `hitTest in hole (even-odd)` is `false` and `hitTest in hole (non-zero)` is `true`.
2. **Claim 2 — `shape_hit_test` (`hitTest(x, y, true)`) tests exact contours and holes in global stage coordinates (`render/src/shape_utils.rs:603-704`):**
   - Verified against `tests/tests/swfs/avm1/movieclip_hittest_shapeflag/output.txt:1-24`, where `hitTest(400, 400, true)` inside the donut's center hole is `false`, while points on the ring (`(400, 500)`, `(500, 400)`, `(400, 300)`, `(300, 400)`) return `true`.
3. **Claim 3 — `Drawing` bounds (`MovieClip.lineTo`/`beginFill`) and `duplicateMovieClip` reset (`core/src/drawing.rs:1-484`):**
   - Verified against `tests/tests/swfs/avm1/duplicate_movie_clip_drawing/output.txt:1-2` (`100`, `400`).

---

## 6. What Could Not Be Verified (`Confidence: High`)

- WGSL shader execution for `Stage3D` (`render/wgpu/src/context3d/`) and `PixelBender` (`render/wgpu/src/pixel_bender.rs`) was not tested because no GPU or `cargo` runtime is present in the sandbox.

---

## 7. Open Questions

1. Should `swfr-scene`'s `hit_test_shape` (`shapeFlag = true`) adopt Ruffle's `shape_hit_test` rule of ignoring interior edges where **both** `fill_style_0 > 0` and `fill_style_1 > 0` (`render/src/shape_utils.rs:660-691`)?
2. Should `swfr-render` omit strokes when rendering a `clip_depth` mask shape, matching Ruffle's `mask_index_count` (`render/src/tessellator.rs:156-160`)?

---

## 8. Test Ideas for `swfr`

1. **Donut `hitTest(x, y, true)` (`shapeFlag = true`) test:** Model a test on `tests/tests/swfs/avm1/movieclip_hittest_shapeflag` with a hollow ring shape, asserting `hitTest(center_x, center_y, true) == false` and `hitTest(ring_x, ring_y, true) == true`.
2. **Adjacent dual-fill interior edge `hitTest` test:** Create a shape split down the middle by an interior edge with `fill_style_0 = 1` and `fill_style_1 = 2`, and assert `hitTest(x, y, true)` returns `true` across both halves and on the seam (`render/src/shape_utils.rs:660-667`).
3. **Multi-layer shape (`StyleChangeRecord.new_styles`) render order test:** Verify that in a single `DefineShape` with two layers separated by `new_styles`, layer 1's strokes are drawn *below* layer 2's fills (`render/src/shape_utils.rs:442-453, 537-575`).
