# R11 — Flash Player Quirks Catalogue (`10-quirks.md`)

- **Pinned Release Tag:** `v0.7.1`
- **Pinned Commit SHA:** `89a7049965f3090e706265670df309a5128233ac`
- **Repository:** `https://github.com/ruffle-rs/ruffle`
- **Overall Confidence:** **High** (verified directly from `core/src/avm1/`, `core/src/display_object/`, `swf/src/`, and `tests/tests/swfs/avm1/` at `89a7049965f3090e706265670df309a5128233ac`).

---

## 1. Catalogue of 22 Non-Obvious Flash Player Quirks (`Confidence: High`)

| # | Affected SWF Version(s) | What SWF19 Says vs What Flash Player Actually Does | Ruffle Evidence (`file:lines`) | Regression Test (`tests/tests/swfs/avm1/...`) | Handled in `swfr` Today? |
|---:|---|---|---|---|---|
| 1 | SWF 6 vs SWF 7+ | **Identifier & Property Case Sensitivity:** SWF <= 6 is case-insensitive (`Foo == foo`) using a unicode-folded lookup table (`wstr/src/tables.rs`), while SWF >= 7 is case-sensitive (`Foo != foo`); both share one player by maintaining two separate `_global` trees (`avm1.globals_case_insensitive` vs `globals_case_sensitive`). | `core/src/avm1/runtime.rs:193-201, 394-415`, `core/src/avm1/property_map.rs:1-314` | `swf6_case_insensitive`, `swf7_case_sensitive` | **Yes** (`crates/swfr-avm1/src/lib.rs`) |
| 2 | SWF 6 vs SWF 7+ | **`Undefined` to String Coercion:** SWF19 says `undefined` converts to `"undefined"`, but in **SWF <= 6** `Value::Undefined.coerce_to_string()` returns `""` (empty string), and only in **SWF >= 7** returns `"undefined"`. | `core/src/avm1/value.rs:283-289` | `undefined_to_string_swf6` | **Yes** (`crates/swfr-avm1/src/lib.rs`) |
| 3 | SWF 6 vs SWF 7+ | **String to Boolean Coercion (`as_bool`):** In **SWF <= 6**, a string is truthy iff it parses as a non-zero finite number (`"0"` and `"abc"` are `false`, `"1"` and `"0x10"` are `true`); in **SWF >= 7**, any non-empty string (`len() > 0`, including `"0"`) is `true`! | `core/src/avm1/value.rs:196-205` | `swf6_string_as_bool` | **Yes** (`crates/swfr-avm1/src/lib.rs`) |
| 4 | SWF 4 vs SWF 5+ | **Division by Zero (`ActionDivide`):** SWF19 notes division by zero produces `"#ERROR#"` in SWF 4 and `NaN`/`Infinity`/`-Infinity` in SWF 5+, including `0.0 / 0.0 -> "#ERROR#"` when `swf_version < 5`. | `core/src/avm1/activation.rs:1096-1100` | `divide_swf4` | **Yes** (`crates/swfr-avm1/src/lib.rs`) |
| 5 | SWF 8 & SWF 9 only | **`ActionBitURShift` (`>>>`) Signed Result Bug:** SWF19 says `ActionBitURShift` converts both operands to `u32` and pushes an unsigned result, but in **SWF 8 and SWF 9** (`matches!(self.swf_version(), 8..=9)`), Flash Player accidentally casts the `u32` result back to signed `i32` (`4294967295 >>> 0 == -1`)! Fixed in SWF 10+. | `core/src/avm1/activation.rs:815-826` | `biturshift_swf8`, `biturshift` | **No** (always pushes `u32 as f64`) |
| 6 | SWF 5 vs SWF 6+ | **Function Closures (`is_closure`):** In **SWF 5**, functions defined with `ActionDefineFunction` do **not** capture their lexical `Scope` or `swf_version`; when called, they construct a fresh scope rooted at `base_clip`'s `_global` (`swf_version.max(5)`). Only in **SWF >= 6** do functions act as lexical closures! | `core/src/avm1/function.rs:297-335` | `swf5_no_closure`, `swf5_to_6_cross_call` | **Partial** (`swfr-avm1` uses closure scope for all versions) |
| 7 | All (SWF 5+) | **`DefineFunction2` (`0x8E`) Register 0:** SWF19 says `RegisterCount` registers are allocated for local variables starting at index `1`, with no mention of what happens when bytecode reads/writes register `0`. In Flash Player, register `0` inside a `DefineFunction2` frame is **not** a local register — it falls through to global register `0`! | `core/src/avm1/activation.rs:295-320, 628-647` | `define_function2_preload`, `execution_order4` | **No** (`swfr-avm1` indexes register `0` inside local register vec) |
| 8 | All (SWF 5+) | **`DefineFunction2` Preload Order:** When multiple preload flags are set on `DefineFunction2`, registers are populated starting at `r = 1` in the exact order `this`, `arguments`, `super`, `_root`, `_parent`, `_global`, and only *after* those are function parameters loaded into consecutive registers (`preload_r`). | `core/src/avm1/function.rs:424-505` | `define_function2_preload` | **Yes** (`crates/swfr-avm1/src/lib.rs`) |
| 9 | SWF <= 6 vs SWF 7+ | **`Math` Method Arity & `Math.round` Rounding:** In Flash Player, all `Math` methods coerce their first two arguments to `f64` even if unused (`Math.abs(a, b)` calls `b.valueOf()`!), in **SWF <= 6** `Math.min(1)` (1 arg) returns `NaN` while `Math.min()` (0 args) returns `Infinity`, and `Math.round(x)` rounds `-0.5` toward `+Infinity` (`(x + 0.5).floor() == 0.0`, unlike Rust's `(-0.5_f64).round() == -1.0`)! | `core/src/avm1/globals/math.rs:75-112` | `math_min_max`, `math_swf6`, `math_swf7` | **Partial** (`Math.round` and 1-arg `Math.min` quirk not matched) |
| 10 | SWF <= 5 vs SWF 6+ | **String Encoding (WINDOWS-1252 vs UTF-8):** SWF19 claims UTF-8 starts in SWF 6, and in SWF <= 5 Flash Player decodes strings as ANSI/WINDOWS-1252 (with Shift-JIS sniffing on Japanese locales). Furthermore, invalid UTF-8 sequences in SWF 6+ are decoded via WTF-8 / lossy fallback rather than rejecting the tag. | `swf/src/string.rs:148-154`, `core/src/string.rs:12-18` | `swf5_encoding` | **Partial** (`swfr-swf` uses `String::from_utf8_lossy` for all versions) |
| 11 | All | **Dynamic Depth Bias (`AVM_DEPTH_BIAS = -16384`) vs Removal Range:** SWF tags place timeline objects at depth `d` stored internally as `d - 16384` (`[-16384, -1]`), while AS methods (`attachMovie`, `duplicateMovieClip`, `createEmptyMovieClip`, `getDepth`, `swapDepths`) add `+16384` internally. Yet `removeMovieClip` only succeeds for internal depths in `[0, 1048575]` (`[0, 0xFFFFF]` in AS depth space — depth `1048576` cannot be removed!). | `core/src/display_object/movie_clip.rs:73-77`, `core/src/avm1/globals/movie_clip.rs:146-152, 1621-1623` | `movieclip_depth_methods`, `remove_movie_clip` | **Partial** (`swfr-scene` checks dynamic flag rather than `[0, 1048575]` upper cap) |
| 12 | All | **Destroyed MovieClip Reference Laundering (`MovieClipReference`):** When a `MovieClip` is removed from stage (`removeMovieClip` or timeline `RemoveObject`), holding a variable `var c = mc;` does **not** keep the dead instance reachable as an object: every read of `c` re-evaluates its slash/dot `path` from `_root`, resolving to `undefined` (or to a new clip later placed at the same name)! | `core/src/avm1/object_reference.rs:16-127` | `string_paths_reference_launder`, `string_paths_unload` | **Yes** (`swfr-avm1` represents clips by path strings / IDs resolved against `Stage`) |
| 13 | SWF <= 6 vs SWF 7+ | **`ActionSetTarget2(undefined)` Behavior:** When `SetTarget2` (`tellTarget`) is called with `undefined`, in **SWF <= 6** it resets the target clip to `base_clip`, whereas in **SWF >= 7** it sets the target clip to `None` (`undefined`)! | `core/src/avm1/activation.rs:2070-2078` | `set_target_2_swf5`, `set_target_2_swf6`, `set_target_2_swf7` | **No** (`swfr-avm1` resets target to base clip on empty/undefined) |
| 14 | SWF <= 5 vs SWF 6+ | **`duplicateMovieClip` and `createTextField` Return Values:** `mc.duplicateMovieClip(...)` returns `undefined` in **SWF < 6** and returns the new `MovieClip` object in **SWF >= 6**; `mc.createTextField(...)` returns `undefined` in **SWF < 8** and returns the new `TextField` object in **SWF >= 8**! | `core/src/avm1/globals/movie_clip.rs:898-903, 944-950` | `duplicate_movie_clip` | **Partial** (`duplicateMovieClip` always returns the clip reference) |
| 15 | All | **`gotoAndStop` / `gotoAndPlay` Forward vs Backward Timeline Seek:** Jumping **forward** (`target > cur`) steps intermediate frames with `run_display_list = false` (creating/modifying display objects without running intermediate frame scripts) and runs `Construct` events *before* the target frame's `DoAction`. Jumping **backward** (`target < cur`) tags existing children with ` place_frame = 0`, rewinds from frame 1, and only removes children that were not re-placed! | `core/src/display_object/movie_clip.rs:1938-2065` | `goto_frame`, `goto_frame2`, `execution_order1` | **Yes** (`swfr-scene` forward-seek preservation implemented in `seek_timeline`) |
| 16 | All | **Reverse `DoAction` Queue Order During Frame Phase:** Because `MovieClip::run_frame_internal` recurses children in **reverse depth list order** and pushes `DoAction` blocks onto `ActionQueue` at `Priority::Normal`, parent frame actions run *before* child frame actions (`enterFrame` runs in reverse child order, then `DoAction` runs parent-first). | `core/src/display_object/movie_clip.rs:1178-1182`, `core/src/context.rs:582-674` | `clip_events`, `execution_order1` | **Yes** (`swfr-scene` & `swfr-player`) |
| 17 | All | **`PlaceObject` Truncated `ColorTransform` past EOF:** Real-world SWFs (authored by buggy third-party tools) sometimes end a `PlaceObject` tag 1–2 bytes early inside `ColorTransform`. Flash Player defaults the missing `b_mult` / `b_add` channels to identity (`Fixed8::ONE`, `0`) instead of rejecting the tag! | `swf/src/read.rs:582-616` | `bad_swf_tag_past_eof` | **Partial** (`swfr-swf` turns truncated tags into `Tag::Unknown` rather than partial `PlaceObject`) |
| 18 | All | **Malformed Tag Length Clamping (`read_tag_code_and_length`):** If a SWF tag's declared `length` extends beyond the end of the file or enclosing `DefineSprite`, Flash Player clamps the tag slice to the remaining bytes (`self.input = &[]`) and still parses the tag! | `swf/src/read.rs:190-209` | `bad_swf_tag_past_eof` | **Yes** (`swfr-swf` clamps or skips non-bomb malformed tags) |
| 19 | SWF <= 5 vs SWF 6+ | **`_droptarget` Empty Value (`undefined` vs `""`) & Slash Path:** When a MovieClip has not been dropped on any target, `mc._droptarget` returns `undefined` in **SWF < 6** and `""` in **SWF >= 6**; when dropped on a target, it always returns a **slash-syntax path** (e.g., `"/target_mc"`), even in SWF 8! | `core/src/avm1/object/stage_object.rs:535-540` | `drag_droptarget` | **No** |
| 20 | SWF 7+ | **Setting `mc._name = NaN` Coerces to `"0"`:** In **SWF >= 7**, assigning `NaN` (`Value::Number(n) if n.is_nan()`) to a display object's `_name` property sets its name to `"0"` instead of `"NaN"`! | `core/src/avm1/object/stage_object.rs:521-533` | `stage_object_properties_swf6` | **No** |
| 21 | All | **`Sound.position` and `Sound.duration` Are Added Lazily on `attachSound` / `loadSound`:** A newly constructed `new Sound()` has `s.duration === undefined` and `hasOwnProperty("duration") == false`; the `duration` and `position` getters are only installed onto the instance when a sound is attached/loaded, and only if the user hasn't already defined custom properties with those names! | `core/src/avm1/globals/sound.rs:188-213` | `sound`, `sound_duration_position_props` | **No** |
| 22 | All | **`_rotation` Decomposition Uses `f64::atan2(b, a)` Modulo `[-180, 180]` and Preserves Skew:** Setting `_rotation` updates both `(a, b)` and `(c, d)` independently using the cached skew angles so a skewed MovieClip rotates without losing its shear! | `core/src/display_object.rs:280-390` | `movieclip_rotation`, `mouse_pos` | **Yes** (`swfr-scene` implements `_rotation` matrix update) |

---

## 2. Verification Against Regression Tests (`Confidence: High`)

1. **Claim 1 — `ActionBitURShift` (`>>>`) returns signed `i32` in SWF 8 (`core/src/avm1/activation.rs:815-826`):**
   - Verified against `tests/tests/swfs/avm1/biturshift_swf8/output.txt:1-14` (`4294967295 >>> 0` -> `-1`, `-3 >>> 0` -> `-3`).
2. **Claim 2 — `ActionDivide` by zero returns `"#ERROR#"` in SWF 4 (`core/src/avm1/activation.rs:1096-1100`):**
   - Verified against `tests/tests/swfs/avm1/divide_swf4/output.txt:1-25` (`3 / 0` -> `#ERROR#`).
3. **Claim 3 — `Math.min` 0-arg (`Infinity`) vs 1-arg (`NaN`) in SWF <= 6 and 2-arg coercion (`core/src/avm1/globals/math.rs:75-102`):**
   - Verified against `tests/tests/swfs/avm1/math_min_max/output.txt:1-24` (`Math.min(1)` -> `NaN`, `Math.min()` -> `Infinity`, `Math.min(a)` traces `a.valueOf` then `NaN`).
4. **Claim 4 — SWF 6 string-to-boolean parses hex/numeric strings (`core/src/avm1/value.rs:196-205`):**
   - Verified against `tests/tests/swfs/avm1/swf6_string_as_bool/output.txt:1-16` (`true and '0x10'` -> `true`, `!'0x10'` -> `false`).

---

## 3. What Could Not Be Verified (`Confidence: High`)

- Flash Player projector binary behavior on undisclosed/undocumented `ASnative(x, y)` table indices without Ruffle regression tests was not independently probed.

---

## 4. Open Questions

1. Which of the remaining unimplemented quirks in Table 1 (`DefineFunction2` register `0` fallthrough, `Math.round(-0.5) == 0`, `ActionBitURShift` in SWF 8/9, `SetTarget2(undefined)` in SWF 7+) affect real-world SWF games most frequently?

---

## 5. Test Ideas for `swfr`

1. **`Math.round(-0.5)` and `Math.min()` / `Math.max()` test:** Add unit tests in `swfr-avm1` asserting `Math.round(-0.5) == 0`, `Math.min() == Infinity`, and `Math.max() == -Infinity` (`core/src/avm1/globals/math.rs:95-112`).
2. **`DefineFunction2` register `0` global fallthrough test:** Test a `DefineFunction2` with `register_count = 4` that stores/reads register `0`, verifying it accesses global register `0` rather than a function-local slot (`core/src/avm1/activation.rs:295-320`).
3. **SWF 8 `>>>` (`BitURShift`) signed coercion test:** Verify `0xFFFFFFFF >>> 0 == -1` in SWF 8 and `4294967295` in SWF 7 (`tests/tests/swfs/avm1/biturshift_swf8/output.txt`).
