# R4 — Display List and Timeline (`03-display-list.md`)

- **Pinned Release Tag:** `v0.7.1`
- **Pinned Commit SHA:** `89a7049965f3090e706265670df309a5128233ac`
- **Repository:** `https://github.com/ruffle-rs/ruffle`
- **Overall Confidence:** **High** (verified directly from `core/src/display_object.rs`, `core/src/display_object/{container,movie_clip,avm1_button,avm2_button,edit_text,text,morph_shape,graphic,interactive}.rs`, `core/src/drawing.rs`, and `core/src/library.rs` at `89a7049965f3090e706265670df309a5128233ac`).

---

## 1. `DisplayObject` Enum and Trait Hierarchy (`Confidence: High`)

- **Zero-Cost Enum Dispatch (`#[enum_trait_object]`):**
  - Ruffle uses its custom proc-macro `#[enum_trait_object]` (`core/macros/src/lib.rs:1-577`) to generate static enum dispatch for three nested display-object traits:
    1. `TDisplayObject<'gc>` / `DisplayObject<'gc>` (`core/src/display_object.rs:1300-1317`): 12 variants (`Stage`, `Bitmap`, `Avm1Button`, `Avm2Button`, `EditText`, `TextLine`, `Graphic`, `MorphShape`, `MovieClip`, `Text`, `Video`, `LoaderDisplay`).
    2. `TDisplayObjectContainer<'gc>` / `DisplayObjectContainer<'gc>` (`core/src/display_object/container.rs:117-135`): 4 variants (`Stage`, `Avm1Button`, `MovieClip`, `LoaderDisplay`).
    3. `TInteractiveObject<'gc>` / `InteractiveObject<'gc>` (`core/src/display_object/interactive.rs:58-76`): 7 variants (`Stage`, `Avm1Button`, `Avm2Button`, `EditText`, `TextLine`, `LoaderDisplay`, `MovieClip`).
- **Prefix-Aligned `DisplayObjectBase<'gc>` (`core/src/display_object.rs:260-322`):**
  - Every variant wraps a `Gc<'gc, ...>` whose first field (transitively via `#[derive(HasPrefixField)]`) is an 8-byte-aligned `DisplayObjectBase<'gc>`. That allows `TDisplayObject::as_ptr(self)` (`core/src/display_object.rs:1323-1325`) to cast `Gc::as_ptr(self.base())` directly for pointer-identity comparisons (`DisplayObject::ptr_eq`).

---

## 2. Depth Model, `AVM_DEPTH_BIAS` (`16384`), and Delayed Unload Removal (`Confidence: High`)

- **Two Lists inside `ChildContainer<'gc>` (`core/src/display_object/container.rs:684-693`):**
  1. `depth_list: BTreeMap<Depth, DisplayObject<'gc>>` — keyed by internal SWF depth (`Depth = i32`).
  2. `render_list: Rc<Vec<DisplayObject<'gc>>>` — ordered back-to-front for rendering (`Rc::make_mut` copy-on-write allows safe `RenderIter` snapshots while script code mutates children; `core/src/display_object/container.rs:1164-1182`).
- **`AVM_DEPTH_BIAS = 16384` (`core/src/avm1/globals.rs:821`):**
  - Timeline `PlaceObject` tags in SWF files use unsigned depths starting at `1` (e.g. `1, 3, 5`).
  - When ActionScript queries `mc.getDepth()` (`core/src/avm1/globals.rs:831-841`), Ruffle subtracts `AVM_DEPTH_BIAS` (`depth.wrapping_sub(16384)`), so timeline depth `1` reports **`-16383`** to ActionScript (`tests/tests/swfs/avm1/movieclip_depth_methods/output.txt:5`).
  - Conversely, when ActionScript calls `attachMovie`, `createEmptyMovieClip`, `duplicateMovieClip`, or `swapDepths(d)`, Ruffle adds `AVM_DEPTH_BIAS` (`d.wrapping_add(16384)`, `core/src/avm1/globals/movie_clip.rs:279, 894`), so AS depth `0` maps to internal depth `16384`.
- **`removeMovieClip` Depth Gate (`core/src/avm1/globals.rs:825-855`):**
  - `remove_display_object` only removes a clip if its internal depth is in `AVM_DEPTH_BIAS .. AVM_MAX_REMOVE_DEPTH` (`16384 .. 2_130_706_416`, i.e., AS depth `0 ..= 2_130_690_031` or `0 .. 0x7EFF_FFF0`) and `!this.avm1_removed()` (`core/src/avm1/globals.rs:849`). Static timeline clips (`depth < 16384`, AS depth `< 0`) cannot be removed by `removeMovieClip` unless first moved to AS depth `>= 0` via `swapDepths`!
- **Delayed Unload Removal at Negative Depth (`ChildContainer::queue_removal`, `core/src/display_object/container.rs:1108-1157`):**
  - In AVM1, if a removed clip (or any descendant) has an `onClipEvent(unload)` or `onUnload` property (`should_delay_removal`), it is **not** immediately removed from the container!
  - Instead, `queue_removal` moves it to internal depth **`-cur_depth - 1`** (which reports as AS depth `-16385 - cur_depth`, i.e. below `-16384`), sets `AVM1_PENDING_REMOVAL`, and queues `ClipEvent::Unload` (`core/src/display_object/container.rs:1148-1156`). The clip remains accessible at that negative depth until `Avm1::remove_pending` deletes it at the start of the **next** frame (`core/src/avm1/runtime.rs:485-520`).

---

## 3. `PlaceObject` 1/2/3/4 and `RemoveObject` 1/2 (`Confidence: High`)

- `MovieClip::place_object` (`core/src/display_object/movie_clip.rs:2970-3065`) decodes `PlaceObject` (v1–v4) and handles `PlaceObjectAction::Place(id)`, `Modify`, and `Replace(id)`:
  - When a new child is instantiated by timeline (`Place`), it sets `INSTANTIATED_BY_TIMELINE` and `place_frame = self.current_frame()`, inserts it via `replace_at_depth`, and calls `post_instantiation` (`core/src/display_object/movie_clip.rs:3120-3195`).
  - When modifying an existing child (`Modify`), if `child.transformed_by_script()` is **true** (set whenever ActionScript mutates `_x`, `_y`, `_xscale`, `_yscale`, `_rotation`, `_alpha`, or `_matrix`; `core/src/display_object.rs:455, 659`), timeline `PlaceObject` matrix and color-transform updates are **ignored** (`core/src/display_object.rs:2218`, `core/src/display_object/movie_clip.rs:4235-4255`) so script-driven movement is not overwritten by timeline tweens!

---

## 4. `MovieClip` Timeline, Scenes, Labels, and `run_goto` (`Confidence: High`)

- **Delta Aggregation in `MovieClip::run_goto` (`core/src/display_object/movie_clip.rs:1644-1960`):**
  - Because SWF timelines are delta-encoded across frames:
    - For a **forward goto** (`frame > self.current_frame()`), `is_rewind = false`: Ruffle steps from `self.current_frame()` up to `clamped_frame`, aggregating `PlaceObject*` and `RemoveObject*` tags into `goto_commands: Vec<GotoPlaceObject>` without running intermediate frame scripts (`core/src/display_object/movie_clip.rs:1686-1764`).
    - For a **backward goto / rewind** (`frame <= self.current_frame()`), `is_rewind = true`: Ruffle resets `tag_stream_pos = 0` and `current_frame = 0`, scans frames `1..=clamped_frame` into `goto_commands`, and then removes existing children that do not pass `survives_rewind` (`core/src/display_object/movie_clip.rs:1678-1793`).
  - **`survives_rewind` (`core/src/display_object/movie_clip.rs:1909-1960`):**
    - In AVM1, dynamic clips at depth `>= AVM_DEPTH_BIAS` (`16384`, AS depth `>= 0`) **always survive timeline rewinds** (`core/src/display_object/movie_clip.rs:1921-1926`)!
    - Timeline clips (`depth < AVM_DEPTH_BIAS`) survive a rewind to `frame` if and only if they were not placed by script, exist in `final_placements` at the same depth with the same `character_id`, and were originally placed on or before `frame` (`old_object.place_frame() <= final_placement.frame`).
  - **Execution Order on Target Frame (`core/src/display_object/movie_clip.rs:1863-1895`):**
    1. Apply `goto_commands` for children placed **before** the target frame (`params.frame < frame`).
    2. Run the target frame's non-display tags (`DoAction`, `StartSound` if `frame != frame_before_rewind`, `SetBackgroundColor`) via `self.run_frame_internal(context, false, frame != frame_before_rewind, ...)` (`core/src/display_object/movie_clip.rs:1873-1886`). Notice that if `gotoAndStop(9999)` overshoots `totalframes`, `hit_target_frame` is `false` and **no target-frame `DoAction` runs** (`core/src/display_object/movie_clip.rs:1871-1888`)!
    3. Apply `goto_commands` for children placed **on** the target frame (`params.frame >= frame`).

---

## 5. `Avm1Button` and `Avm2Button` (`Confidence: High`)

- `Avm1Button<'gc>` (`core/src/display_object/avm1_button.rs:32-684`) is a `TDisplayObjectContainer` with:
  - `state: Cell<ButtonState>` (`Up`, `Over`, `Down`) and `tracking: Cell<ButtonTracking>` (`Push` vs `Menu`; `core/src/display_object/avm1_button.rs:50-51, 109-113`).
  - `hit_area: BTreeMap<Depth, DisplayObject<'gc>>` instantiated from `ButtonRecord`s that have `ButtonState::HIT_TEST` (`core/src/display_object/avm1_button.rs:58, 224-255`).
  - When `set_state` transitions between `Up`, `Over`, and `Down`, it rebuilds active state children in `container` (preserving existing children whose record is active in both the old and new state!) and plays `ButtonSounds` (`core/src/display_object/avm1_button.rs:163-222`).
  - Maps `ClipEvent` (`RollOver`, `RollOut`, `Press`, `Release`, `ReleaseOutside`, `DragOver`, `DragOut`, `KeyPress`) to `swf::ButtonActionCondition` bits and queues matching `action_data` slices on the **parent** `MovieClip` (`self.avm1_parent()`) (`core/src/display_object/avm1_button.rs:300-385`)!

---

## 6. `Text` and `EditText` (`Confidence: High`)

- `Text<'gc>` (`core/src/display_object/text.rs:1-443`) renders static `DefineText`/`DefineText2` glyph runs and supports `TextSnapshot`.
- `EditText<'gc>` (`core/src/display_object/edit_text.rs:1-3682`) implements dynamic/input text fields:
  - **Variable Binding (`Avm1TextFieldBinding`, `core/src/display_object.rs:3084-3135`, `core/src/display_object/edit_text.rs:1620-1755`):** Supports two-way binding between `EditText.variable` (which can be a relative/slash/dot path like `_root.score` or `parent.msg`) and the target stage object's property. Unbound text fields wait in `context.unbound_text_fields` and bind automatically when their target clip/variable is created.
  - Supports HTML subset parsing (`<p>`, `<font>`, `<b>`, `<i>`, `<u>`, `<a href="asfunction:...">`, `<li>`, `<textformat>`, `<img>`), `autoSize`, `wordWrap`, scrolling (`scroll`, `hscroll`), password masking, and cursor/selection rendering.

---

## 7. `MorphShape` and `Drawing` API (`Confidence: High`)

- `MorphShape<'gc>` (`core/src/display_object/morph_shape.rs:1-642`) interpolates `DefineMorphShape` / `DefineMorphShape2` start and end shapes by `ratio: u16` (`0..=65535`), caching tessellated `ShapeHandle`s by ratio in a `fnv::FnvHashMap<u16, Frame>` (`core/src/display_object/morph_shape.rs:64-120`).
- `Drawing` (`core/src/drawing.rs:1-484`) implements `MovieClip.beginFill`, `beginGradientFill`, `beginBitmapFill`, `lineStyle`, `moveTo`, `lineTo`, `curveTo`, `endFill`, and `clear`, lazily registering a `ShapeHandle` with `RenderBackend` when dirty (`core/src/drawing.rs:350-420`).

---

## 8. Hit Testing and Bounds (`Confidence: High`)

- **Bounds Modes (`BoundsMode::Engine` vs `BoundsMode::Script`, `core/src/display_object.rs:1357-1432`):**
  - `self_bounds` returns leaf bounds; `bounds_with_transform` recursively transforms each child's AABB before unioning (producing a tighter AABB than transforming the union).
- **Two Hit-Test Methods (`TDisplayObject::hit_test_bounds` and `hit_test_shape`, `core/src/display_object.rs:1890-1985`):**
  - `hit_test_bounds` (`shapeFlag = false`): tests whether `world_bounds` contains the point (or intersects the target clip's `world_bounds`).
  - `hit_test_shape` (`shapeFlag = true`): transforms the global point into each leaf object's local coordinate space and calls `ruffle_render::shape_utils::shape_hit_test` (`Graphic::hit_test_shape`, `Drawing::hit_test`, `MorphShape::hit_test_shape`) while respecting `clip_depth` stencil masks and `masker`!

---

## 9. Verification Against Regression Tests (`Confidence: High`)

1. **Claim 1 — `AVM_DEPTH_BIAS = 16384` offsets timeline depths to `-16383..` and `getNextHighestDepth()` starts at `0` (`core/src/avm1/globals.rs:821-841`):**
   - Verified against `tests/tests/swfs/avm1/movieclip_depth_methods/output.txt:1-20`, which traces `getNextHighestDepth()` -> `0`, `clip1.getDepth()` -> `-16383`, `clip2.getDepth()` -> `-16381`, `clip1.swapDepths(0)` -> `0`, `getNextHighestDepth()` -> `1`.
2. **Claim 2 — `run_goto` executes target frame `DoAction` in-place unless target frame > `totalframes` (`core/src/display_object/movie_clip.rs:1871-1888`):**
   - Verified against `tests/tests/swfs/avm1/goto_frame/output.txt:1-12`, where `gotoAndStop(4)` traces `4` immediately, `gotoAndStop(5)` traces `5`, and `gotoAndStop(999)` clamps to the final frame without re-running actions.
3. **Claim 3 — `removeMovieClip` only removes clips in internal depth `16384 .. 2_130_706_416` (`core/src/avm1/globals.rs:849`):**
   - Verified against `tests/tests/swfs/avm1/remove_movie_clip/output.txt:1-18`, where static timeline clip `_level0.clip` (depth `< 0`) and out-of-range depths (`clip3`, `clip5`) survive `removeMovieClip`, while dynamic clips (`clip1`, `clip2`, `clip4`) become `undefined`.

---

## 10. What Could Not Be Verified (`Confidence: High`)

- `Avm2Button` (`core/src/display_object/avm2_button.rs`) and `TextLine` (`flash.text.engine` FTE in `core/src/display_object/text_line.rs`) are AS3-only display objects and were only skimmed for structural completeness.

---

## 11. Open Questions

1. Does `swfr-scene` currently set a `transformed_by_script` flag on display objects when ActionScript modifies `_x`, `_y`, `_xscale`, `_yscale`, `_rotation`, or `_alpha`, so subsequent timeline `PlaceObject2(Modify)` tags don't snap script-moved clips back to their authoring coordinates?
2. Does `swfr-scene` execute `DefineButton2` `ButtonAction` bytecode in the scope of the button's **parent** `MovieClip` (`button.avm1_parent()`), as Flash Player and Ruffle (`core/src/display_object/avm1_button.rs:345-380`) do?

---

## 12. Test Ideas for `swfr`

1. **`transformed_by_script` vs timeline `PlaceObject2(Modify)` test:** Place a clip on frame 1, set `mc._x = 200` in frame 1 `DoAction`, have frame 2 contain a timeline `PlaceObject2(Modify)` moving that depth to `x = 50`, and assert `mc._x` stays `200` on frame 2!
2. **Button action parent-scope test:** Place a `DefineButton2` inside a child `MovieClip` (`_root.panel.btn`) whose `on(release)` action sets `x = 42`, and assert that `_root.panel.x == 42` (parent scope, not button scope).
3. **`gotoAndStop(9999)` past-EOF clamping test:** Assert that `gotoAndStop(9999)` on a 5-frame MovieClip stops at frame 5 without re-running frame 5's `DoAction` (`tests/tests/swfs/avm1/goto_frame/output.txt:11-12`).
