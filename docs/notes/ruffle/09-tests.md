# R10 — Test Framework and AVM1 Regression Suite (`09-tests.md`)

- **Pinned Release Tag:** `v0.7.1`
- **Pinned Commit SHA:** `89a7049965f3090e706265670df309a5128233ac`
- **Repository:** `https://github.com/ruffle-rs/ruffle`
- **Overall Confidence:** **High** (verified directly from `tests/framework/src/`, `tests/input-format/src/`, `core/src/{backend/locale.rs,avm_rng.rs}`, and `tests/tests/swfs/` at `89a7049965f3090e706265670df309a5128233ac`).

---

## 1. `test.toml` Format (`TestOptions`) (`Confidence: High`)

- Defined in `tests/framework/src/options.rs:95-123` (`TestOptions`):
  - `num_frames: Option<u32>` / `num_ticks: Option<u32>`: Number of frames or ticks to run (`tests/framework/src/runner.rs:71-76, 137-141`).
  - `tick_rate: Option<f64>`: Overrides the movie's `1000.0 / frame_rate()` frame duration in milliseconds (`tests/framework/src/runner.rs:80-86`).
  - `sleep_to_meet_frame_rate: bool`: Sleeps `frame_time_duration` between frames when a test relies on wall-clock `getTimer()` consistency (`tests/framework/src/runner.rs:205-219`).
  - `output_path: String`: Path to expected `trace()` log (defaults to `"output.txt"`).
  - `approximations: Option<Approximations>` (`tests/framework/src/options/approximations.rs:1-67`): Allows `epsilon` / `max_relative` numeric tolerances on `bare_numbers` or regex `number_patterns`.
  - `image_comparisons: HashMap<String, ImageComparison>` (`tests/framework/src/options/image_comparison.rs:7-46`): Per-trigger (`last_frame`, specific frame, or `fscommand:captureImage`) visual comparison with `tolerance: u8` (per-channel delta) and `max_outliers: usize`.
  - `audio_assertions: HashMap<String, AudioAssertion>` (`tests/framework/src/options.rs:77-85`): Checks `max_amplitude` and `min_max_amplitude` over selected frames.
  - `shared_objects: HashMap<String, SharedObjectConfig>` (`tests/framework/src/options.rs:87-93`): Compares persisted `.sol` files after execution.
  - `known_failure: KnownFailure` (`tests/framework/src/options/known_failure.rs:1-58`): Boolean, panic substring (`KnownFailure::Panic { message }`), or expected wrong output (`output.ruffle.txt`).
  - `player_options: PlayerOptions` (`tests/framework/src/options/player.rs:1-114`): Configures `viewport_dimensions`, `with_renderer`, `with_audio`, `with_video`, `max_execution_duration`, `player_version`, `player_runtime`, `frame_rate`.
  - Subtests (`merge_into_subtest`, `tests/framework/src/options.rs:26-59`): Supports `[subtests.<name>]` table inheritance in a single `test.toml`.

---

## 2. `input.json` Format (`AutomatedEvent`) (`Confidence: High`)

- Defined in `ruffle_input_format` (`tests/input-format/src/format.rs:116-184`, `#[serde(tag = "type")] pub enum AutomatedEvent`):
  - `Wait`: Ends the current frame's input injection and advances to the next frame (`tests/input-format/src/injector.rs:34-47`).
  - `MouseMove { pos: MousePosition(f64, f64), assert_cursor: Option<MouseCursor> }`.
  - `MouseDown { pos: MousePosition, btn: MouseButton, index: Option<usize>, assert_handled: Option<EventHandledAssertion> }`.
  - `MouseUp { pos: MousePosition, btn: MouseButton }`.
  - `MouseWheel { lines: Option<f64>, pixels: Option<f64> }`.
  - `KeyDown { key: AutomatedKey }` / `KeyUp { key: AutomatedKey }` (`AutomatedKey` has 46 variants including `Char(char)`, `Numpad(char)`, arrows, function keys, modifiers, `Space`, `Tab`, `Enter`, `Escape`).
  - `TextInput { codepoint: char }` / `TextControl { code: TextControlCode }` (`MoveLeft`, `SelectAll`, `Copy`, `Paste`, `Cut`, `Backspace`, `Enter`, `Delete`, etc.).
  - `SetClipboardText { text: String }`, `FocusGained`, `FocusLost`, `ImePreedit`, `ImeCommit`.

---

## 3. Trace Comparison, Image Comparison, and Deterministic Time / RNG (`Confidence: High`)

- **Trace Comparison (`compare_trace_output`, `tests/framework/src/runner/trace.rs:75-234`):**
  - Compares captured `TestLogBackend` `avm_trace` lines (`tests/framework/src/backends/log.rs:1-38`) against `output.txt`.
  - When `approximations` is absent, asserts exact line-by-line string equality (`tests/framework/src/runner/trace.rs:64-73`).
  - When `approximations` is present (`tests/framework/src/runner/trace.rs:88-165`), compares line counts, parses bare `f64` lines (treating `NaN == NaN` as equal, lines 108–110) or regex capture groups, and verifies `approx.compare(actual, expected)` within `epsilon` / `max_relative`.
- **Image Comparison (`capture_and_compare_image`, `tests/framework/src/runner/image_test.rs:1-303`):**
  - Captures RGBA pixels from `RenderInterface` at the configured `ImageTrigger`, computes per-channel `|actual - expected|` difference, and counts pixels exceeding `check.tolerance`; fails if outliers exceed `check.max_outliers`.
- **Deterministic Time and RNG (`core/src/backend/locale.rs:35-66`, `core/src/avm_rng.rs:13-66`):**
  - `DeterministicLocaleBackend::default()` (`core/src/backend/locale.rs:57-66`) fixes the system clock to **2001-02-03T04:05:06+05:45** (Nepal timezone `+05:45` = `20700` seconds east, chosen because Nepal has a non-integer offset and has never used DST).
  - `AvmRng` (`core/src/avm_rng.rs:13-66`) implements Flash Player's exact PRNG (`avmplus/core/MathUtils.cpp`), lazily seeding `u_value` on first call from `locale.get_current_date_time().timestamp_micros() as u32` (`core/src/avm_rng.rs:50-53, 64-66`). Because `DeterministicLocaleBackend` fixes that timestamp, `random(n)` and `Math.random()` are 100% deterministic in tests!

---

## 4. Count of Regression Tests and AVM1 Breakdown (`Confidence: High`)

- Across `/tmp/ruffle-study/tests/tests/swfs/**/test.toml`, there are **4,451 `test.toml` suites**:
  - `from_avmplus`: 1,574 | `avm2`: 1,284 | **`avm1`: 743** | `from_gnash`: 404 | `from_shumway`: 229 | `visual`: 147 | `timeline`: 17 | `mixed_avm`: 12 | `text`: 11 | `fonts`: 8 | `stage3d`: 7 | `swf`: 7 | `audio`: 5 | `import_assets`: 3.
- **Breakdown of the 743 `tests/tests/swfs/avm1/` suites by category:**

| AVM1 Category | Count | Representative Suites |
|---|---:|---|
| Core VM, Coercion, Scope, Functions, Prototypes, Operators | **306** | `execution_order1..4`, `register_class`, `super`, `tell_target`, `with`, `try_catch_finally`, `swf6_case_insensitive`, `swf7_case_sensitive` |
| MovieClip, Timeline, DisplayList, Loader | **138** | `clip_events`, `goto_frame`, `goto_frame2`, `movieclip_depth_methods`, `remove_movie_clip`, `duplicate_movie_clip`, `mcl_*` |
| Built-ins (`XML`, `SharedObject`, `Math`, `Date`, Input, Events) | **109** | `xml_*`, `shared_object*`, `date*`, `math*`, `set_interval`, `mouse_pos`, `button_order`, `asbroadcaster` |
| Text, `EditText`, HTML, Fonts | **84** | `edittext_align`, `edittext_tag_indent`, `edittext_html_*`, `textformat*`, `stylesheet*` |
| Graphics, Filters, Geometry, `hitTest`, Focus | **81** | `hittest_winding_rule`, `movieclip_hittest_shapeflag`, `bitmap_data_*`, `point`, `rectangle`, `matrix`, `color`, `focus_*` |
| Audio, `Sound`, Video | **25** | `sound`, `sound_id3`, `sound_load`, `netstream_*`, `video_*` |
| **Total `avm1/`** | **743** | |

---

## 5. Can `swfr-harness` Implement a Clean-Room Adapter for `test.toml` / `input.json` / `output.txt`? (`Confidence: High`)

- **Yes (`~150–200 LOC` in `swfr-harness`, zero Ruffle code copied):**
  1. `swfr-harness` already parses minimalist `num_frames` / `output.txt` test cases.
  2. Extending `swfr-harness` to parse `num_frames` / `num_ticks`, `[approximations]` (`bare_numbers`, `epsilon`), and `input.json` (`Wait`, `MouseMove`, `MouseDown`, `MouseUp`, `KeyDown`, `KeyUp`) maps directly onto `swfr_player::Player::run_frame(&HostInput)`!

---

## 6. Verification Against Regression Tests (`Confidence: High`)

1. **Claim 1 — `input.json` `MouseMove` + `Wait` frame-by-frame injection (`tests/input-format/src/format.rs:118-128`):**
   - Verified against `tests/tests/swfs/avm1/mouse_pos/{test.toml,input.json,output.txt}` (`num_frames = 75`, injecting `{"type": "MouseMove", "pos": [308, 340]}, {"type": "Wait"}` on frame 1 and tracing `_root 308 340`).
2. **Claim 2 — `[approximations]` `bare_numbers = true` and `epsilon` (`tests/framework/src/runner/trace.rs:104-116`):**
   - Verified against `tests/tests/swfs/avm1/edittext_align/{test.toml,output.txt}` (`bare_numbers = true`, `epsilon = 3.0` for `field.textWidth` / `field.textHeight` lines `158`, `18`).
3. **Claim 3 — `[image_comparisons.output]` visual verification (`tests/framework/src/options/image_comparison.rs:9-15`):**
   - Verified against `tests/tests/swfs/avm1/bitmap_data_fillrect/test.toml` (`num_frames = 1`, `[image_comparisons.output] tolerance = 0`, `with_renderer = { optional = false, quality = "low" }`).

---

## 7. What Could Not Be Verified (`Confidence: High`)

- Running `cargo test --package ruffle_test` directly was not executed because the sandbox does not have `cargo` installed.

---

## 8. Open Questions

1. Should `swfr-avm1` implement `AvmRng` (`core/src/avm_rng.rs:13-66`) and the fixed mock timestamp `2001-02-03T04:05:06+05:45` in `swfr-harness` so deterministic `random()` / `Math.random()` / `Date` regression tests match Flash Player output verbatim?

---

## 9. Test Ideas for `swfr`

1. **`input.json` replay support in `swfr-harness`:** Add clean-room `input.json` event replay (`Wait`, `MouseMove`, `MouseDown`, `MouseUp`, `KeyDown`, `KeyUp`) to `swfr-harness` so interactive button/mouse/key tests can be replayed deterministically.
2. **`[approximations]` support in `swfr-harness`:** Support `bare_numbers = true` and `epsilon` in `swfr-harness` trace comparisons for floating-point and font-metric tests.
3. **Deterministic `AvmRng` + mock `Date` test:** Verify `random(n)` and `Math.random()` against Flash Player's seeded PRNG output.
