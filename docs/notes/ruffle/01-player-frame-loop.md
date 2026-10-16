# R2 — `Player`, Frame Loop, Action Queue, and Timers (`01-player-frame-loop.md`)

- **Pinned Release Tag:** `v0.7.1`
- **Pinned Commit SHA:** `89a7049965f3090e706265670df309a5128233ac`
- **Repository:** `https://github.com/ruffle-rs/ruffle`
- **Overall Confidence:** **High** (verified directly from `core/src/player.rs`, `core/src/context.rs`, `core/src/frame_lifecycle.rs`, `core/src/timer.rs`, `core/src/avm1/runtime.rs`, and `core/src/display_object/movie_clip.rs` at `89a7049965f3090e706265670df309a5128233ac`).

---

## 1. How a Tick Turns Elapsed Time Into Frames (`Confidence: High`)

- **Accumulator & Frame Duration:**
  - `Player` stores the movie's target `frame_rate: f64` (`core/src/player.rs:344` at `89a7049965f3090e706265670df309a5128233ac`) and `frame_accumulator: FloatDuration` (`core/src/player.rs:351`).
  - `Player::frame_time(time_unit)` computes `time_unit / self.frame_rate` (`core/src/player.rs:534-541`), and `Player::frame_duration()` returns `FloatDuration::from_millis(self.frame_time(1000.0))` (`core/src/player.rs:544-546`).
- **`Player::tick(dt: FloatDuration)` (`core/src/player.rs:548-614`):**
  1. Returns immediately if `!self.is_playing()` (`RunState::Suspended`; `core/src/player.rs:549-551, 639-644`).
  2. Adds `dt` to `self.frame_accumulator` (`core/src/player.rs:553`).
  3. Loops while `frame < max_frames_per_tick && self.frame_accumulator >= frame_duration` (`core/src/player.rs:559-581`):
     - Measures wall-clock duration around `self.run_frame()` (`core/src/player.rs:560-564`) and records the last 10 timings in `recent_run_frame_timings` (`core/src/player.rs:527-532`).
     - Subtracts `frame_duration` from `self.frame_accumulator` (`core/src/player.rs:566`).
     - If `self.time_offset > 0` (set when a script polls `getTimer()` repeatedly in a busy-wait FPS-limiter loop), subtracts `FloatDuration::from_millis(self.time_offset as f64)` from `frame_accumulator` (`core/src/player.rs:572-574`) and resets `self.time_offset = 0` after the loop (`core/src/player.rs:590`).
     - If `self.run_state == RunState::Stepping`, transitions to `RunState::Suspended` and breaks after one frame (`core/src/player.rs:577-580`).
  4. **Anti-Turbo Clamp:** If `self.frame_accumulator >= frame_duration` after exhausting `max_frames_per_tick`, resets `self.frame_accumulator = FloatDuration::ZERO` so the player drops frames rather than running in fast-forward (`core/src/player.rs:594-596`).
  5. **Timeline Stream Audio Sync:** Queries `context.audio_manager.audio_skew_time(context.audio, cur_frame_offset)` and adds `FloatDuration::from_secs(audio_skew)` to `self.frame_accumulator` so timeline animation stays locked to `SoundStreamHead`/`SoundStreamBlock` audio (`core/src/player.rs:598-605`).
  6. **Post-Frame Subsystem Updates (Once per `tick`, regardless of whether 0 or N frames ran):**
     - `self.update_sockets()` (`core/src/player.rs:607`)
     - `self.update_net_connections()` (`core/src/player.rs:608`)
     - `self.update_timers(dt)` (`core/src/player.rs:609`)
     - `StreamManager::tick(context, dt)` (`core/src/player.rs:610-612`)
     - `self.audio.tick()` (`core/src/player.rs:613`)

---

## 2. How Many Frames Can Run per Tick (`Confidence: High`)

- **Hard Cap (`MAX_FRAMES_PER_TICK = 5`):**
  - `Player::max_frames_per_tick(&self) -> u32` (`core/src/player.rs:514-525`) caps the number of `run_frame()` calls per `tick()` at **`5`**.
  - **Adaptive Down-Throttling:** Once `recent_run_frame_timings` is non-empty, Ruffle computes `(frame_time / average_run_frame_time) as u32` clamped to `1..=5` (`core/src/player.rs:520-524`). If `run_frame()` averages more than half a frame's budget, Ruffle runs at most 1 frame per tick to prevent spiral-of-death lag.

---

## 3. What Triggers a Render (`Confidence: High`)

- `self.needs_render: bool` (`core/src/player.rs:384, 950-952`) is set to `true`:
  1. At the end of `Player::run_frame()` (`core/src/player.rs:2086`).
  2. When stage properties change (`set_background_color`, `set_letterbox`, `set_viewport_dimensions`, `set_quality`, `set_window_mode`, `set_scale_mode`, `set_forced_scale_mode`; `core/src/player.rs:958-1041`).
  3. During `handle_input_event` when mouse state/button states or `EditText` selection changes (`core/src/player.rs:1449`, `1940`).
  4. When an AVM1 timer callback invokes `updateAfterEvent()` or an AVM2 `TimerEvent.updateAfterEvent()` / `MouseEvent.updateAfterEvent()` sets `context.needs_render = true` (`core/src/context.rs:99`).
- `Player::render(&mut self)` (`core/src/player.rs:2090-2148`):
  - If `stage.invalidated()` is true, broadcasts the AVM2 `Event.RENDER` event via `stage.broadcast_render(context)` before building the `CommandList` (`core/src/player.rs:2091-2098`).
  - Traverses `stage.render_viewport(&mut render_context)`, sweeps font caches, calls `self.renderer.submit_frame(background_color, commands, cache_draws)`, and clears `self.needs_render = false` (`core/src/player.rs:2119-2147`).

---

## 4. Ordering of Frame Phases, `clip_exec_list`, `ActionQueue`, and `Timers` (`Confidence: High`)

### 4.1 `Player::run_frame()` Top-Level Sequence (`core/src/player.rs:2052-2087`)
1. **Chunked Preload:** Runs `self.preload(&mut execution_limit)` (`core/src/player.rs:2066`), which parses definition tags (`DefineShape`, `DefineSprite`, `DefineSound`, `ExportAssets`, etc.) ahead of timeline playback (`core/src/display_object/movie_clip.rs:479-620`).
2. **Inside `self.update(|context| ...)` (`core/src/player.rs:2072-2084`):**
   - `run_all_phases_avm2(context)` (`core/src/frame_lifecycle.rs:73-114`): No-op if `!stage.movie().is_action_script_3()`. For AS3 movies, runs `FramePhase::Enter` -> `FramePhase::Construct` (plus `frameConstructed` broadcast) -> `FramePhase::FrameScripts` -> `FramePhase::Exit` (plus `exitFrame` broadcast) -> `FramePhase::Idle`.
   - `Avm1::run_frame(context)` (`core/src/avm1/runtime.rs:518-555`).
   - `AudioManager::update_sounds(context)` (`core/src/player.rs:2076`).
   - `LocalConnections::update_connections(context)` (`core/src/player.rs:2077`).
   - Drains `context.post_frame_callbacks` (`core/src/player.rs:2081-2083`).
   - **Important:** Because all of step 2 runs inside `Player::update`, `Player::update` then calls `Self::run_actions(context)` (`core/src/player.rs:2431`), followed by `Self::update_drag(context)` (`core/src/player.rs:2438`), `self.update_mouse_state(...)` (`core/src/player.rs:2440`), and `self.gc_arena.borrow_mut().collect_debt()` (`core/src/player.rs:2443`).

### 4.2 AVM1 Execution List (`clip_exec_list`) (`core/src/avm1/runtime.rs:518-567`)
- Ruffle maintains an intrusive singly-linked list of active `MovieClip`s headed at `Avm1.clip_exec_list: Option<MovieClip<'gc>>` (`core/src/avm1/runtime.rs:92`), with next pointers stored per clip (`clip.next_avm1_clip()`).
- Whenever a `MovieClip` is instantiated, `Avm1::add_to_exec_list` **prepends** it to `clip_exec_list` (`core/src/avm1/runtime.rs:561-567`), so newer clips appear before older clips during `Avm1::run_frame` traversal (`core/src/avm1/runtime.rs:529-544`), while within a single frame's `PlaceObject2` stream tags execute in tag order and queue actions into `ActionQueue`.
- At the start of `Avm1::run_frame`, `Self::remove_pending(context)` removes clips that were kept alive at negative depth for one extra frame due to `onUnload` (`core/src/avm1/runtime.rs:485-520`).
- Then for each non-removed `clip` in `clip_exec_list`, `clip.run_frame_avm1(context)` (`core/src/display_object/movie_clip.rs:448-465`):
  1. Dispatches `ClipEvent::Load` (if `!INITIALIZED`, then sets `INITIALIZED`) or `ClipEvent::EnterFrame` (`core/src/display_object/movie_clip.rs:451-457`).
  2. If `self.playing()`, calls `self.run_frame_internal(context, true, true, false)` (`core/src/display_object/movie_clip.rs:461-463`), which increments `current_frame` (or loops to frame 1 via `run_goto`) and decodes all tags up to `TagCode::ShowFrame` (`DoAction`, `PlaceObject*`, `RemoveObject*`, `SetBackgroundColor`, `StartSound`, `SoundStreamBlock`; `core/src/display_object/movie_clip.rs:1401-1449`).
- Finally, `Avm1::run_frame` fires `movie_clip_on_load` for completed loaders and calls `context.avm1.clear()` to empty the shared operand stack and reset the 4 global registers (`core/src/avm1/runtime.rs:547-554`).

### 4.3 `ActionQueue` Three-Bucket Priority Drain (`core/src/context.rs:509-560`, `core/src/player.rs:2205-2284`)
- `ActionQueue<'gc>` holds an `EnumMap<ActionPriority, VecDeque<QueuedAction<'gc>>>` with three FIFO priority buckets (`core/src/context.rs:509-524`):
  1. `ActionPriority::Initialize` — `ActionType::Initialize` (`onClipEvent(initialize)`). Always drained first (`core/src/context.rs:510-511, 695`).
  2. `ActionPriority::Construct` — `ActionType::Construct` (AS2 `Object.registerClass` constructor + `onClipEvent(construct)`). Always drained second (`core/src/context.rs:512-513, 696`).
  3. `ActionPriority::Default` — `ActionType::Normal` (`DoAction` frame scripts, `onClipEvent(load/enterFrame/unload/...)`), `ActionType::Method` (`onEnterFrame`, `onLoad`), and `ActionType::NotifyListeners` (`AsBroadcaster` system listeners). Always drained last (`core/src/context.rs:514-515, 697`).
- Because `ActionQueue::pop_action()` checks buckets in declaration order (`Initialize`, then `Construct`, then `Default`) inside `while let Some(action) = context.action_queue.pop_action()` (`core/src/player.rs:2207`), if a `Default` action dynamically attaches a new MovieClip that queues an `Initialize` or `Construct` action, the newly queued high-priority action runs on the very next iteration before remaining `Default` actions!
- Before running any popped `QueuedAction`, `Player::run_actions` skips the action if `!action.is_unload && (action.clip.avm1_removed() || action.clip.avm1_pending_removal())` (`core/src/player.rs:2209-2214`).

### 4.4 `Timers::update_timers` (`core/src/timer.rs:24-260`)
- Timers (`setInterval` / `setTimeout` and AS3 `flash.utils.Timer`) are stored in a `BinaryHeap<Timer<'gc>>` ordered as a min-heap by `(tick_time, id)` (`core/src/timer.rs:26, 360-370`).
- `Timers::MIN_INTERVAL = 10` ms (`core/src/timer.rs:214`), `Timers::MAX_TICKS = 10` per `update_timers` call (`core/src/timer.rs:217`), and `Timers::TIMER_SCALE = 1000.0` (microseconds; `core/src/timer.rs:220`).
- Timer IDs start at `1` (`self.timer_counter = self.timer_counter.wrapping_add(1)` before assignment; `core/src/timer.rs:256-259`) because Flash games treat `0` as "no timer".
- Each fired callback invokes `Player::run_actions(context)` immediately (`core/src/timer.rs:176`). If an `Avm1Method` timer targets a `MovieClip` where `mc.avm1_removed()` is true, the timer does not fire and cancels itself (`core/src/timer.rs:110-141`).

---

## 5. How Events From Mouse and Keyboard Enter (`Confidence: High`)

- External input enters synchronously via `Player::handle_event(event: PlayerEvent) -> bool` (`core/src/player.rs:1045-1061`), which delegates input events to `Player::handle_input_event` (`core/src/player.rs:1096-1466`):
  1. Converts `GamepadButtonDown`/`Up` to `KeyDown`/`KeyUp` via `self.input.process_event(event)` (`core/src/player.rs:1099`).
  2. For key presses/text input, dispatches `ClipEvent::KeyPress { key_code }` to stage buttons/clips first (`core/src/player.rs:1166-1208`), then `EditText` text control/input if unhandled (`core/src/player.rs:1210-1280`).
  3. Dispatches the event through the stage (`ClipEvent::KeyDown`, `ClipEvent::KeyUp`, `ClipEvent::MouseDown`, `ClipEvent::MouseUp`, `ClipEvent::MouseMove`) and notifies AVM1 global broadcasters (`Key`, `Mouse`, `Selection`; `core/src/player.rs:1282-1444`).
  4. Because step 3 runs inside `self.mutate_with_update_context`, it calls `Self::run_actions(context)` at the end of that closure (`core/src/player.rs:1443`).
  5. Calls `self.update_mouse_state(changed_mouse_buttons, is_mouse_moved, &mut handled)` (`core/src/player.rs:1447`), which performs hit-testing (`run_mouse_pick`), dispatches button/MovieClip `RollOver`/`RollOut`/`Press`/`Release`/`ReleaseOutside`/`DragOver`/`DragOut` events, and drains `Self::run_actions(context)` again (`core/src/player.rs:1560-1947`).

---

## 6. How This Maps Onto One `retro_run` per Frame in `swfr` (`Confidence: High`)

- In Libretro, the frontend drives timing by calling `retro_run` at the core's reported `fps` (`retro_system_av_info.timing.fps = movie.frame_rate`).
- Therefore, in `swfr`, one `retro_run` call corresponds to `dt = 1.0 / frame_rate` seconds (i.e. **1 frame per `retro_run`** without variable-`dt` multi-frame catch-up):
  1. Poll libretro input (`input_poll_cb` / `input_state_cb` + buffered `retro_keyboard_event_t`) and dispatch input events (`KeyDown`/`KeyUp`, `MouseMove`, `MouseDown`/`MouseUp` + button state transitions) and drain any actions they queue.
  2. Run one timeline frame (`Avm1::run_frame` + `ActionQueue` drain in `Initialize` -> `Construct` -> `Default` priority order).
  3. Advance timers by `1000.0 / frame_rate` ms (`Timers::update_timers` with `MIN_INTERVAL = 10` ms, `MAX_TICKS = 10`, IDs starting at `1`) and drain actions.
  4. Render the stage to the `XRGB8888` framebuffer and mix `sample_rate / frame_rate` stereo audio frames.

---

## 7. Verification Against Regression Tests (`Confidence: High`)

1. **Claim 1 — Three-bucket `ActionPriority` (`Initialize` -> `Construct` -> `Default`) and `EnterFrame` order (`core/src/context.rs:509-516`, `core/src/display_object/movie_clip.rs:448-465`):**
   - Verified against `tests/tests/swfs/avm1/clip_events/output.txt:1-19`:
     - Frame 1 traces `frame 1` -> `clip initialize1` -> `clip initialize2` -> `child initialize` -> `clip construct` -> `child construct` -> `frame 2` -> `clip load` -> `clip frame 1` -> `child load` -> `child frame 1`.
     - Frame 2 traces `child enterframe` -> `clip enterFrame` -> `clip onEnterFrame` -> `frame 3` (confirming newer clips in `clip_exec_list` fire `enterFrame` before older clips).
2. **Claim 2 — Parent `DoAction` and child `DoAction` execution order during placement (`core/src/display_object/movie_clip.rs:1401-1449`):**
   - Verified against `tests/tests/swfs/avm1/execution_order1/output.txt:1-5`:
     - Traces `root 1` -> `child 1` -> `child 2` -> `root 2` -> `root 3`, confirming that `PlaceObject2` queues the child's frame-1 actions at the moment the `PlaceObject2` tag is encountered between `root 1` and `root 2` `DoAction` tags.
3. **Claim 3 — `Timers::add_timer` starts IDs at `1` and fires `setInterval`/`setTimeout` callbacks ordered by `tick_time` (`core/src/timer.rs:247-265`):**
   - Verified against `tests/tests/swfs/avm1/set_interval/output.txt:1-29`:
     - Traces `// setInterval(tick, 200): 1`, `// setInterval(tick, 150, 'A' 'test'): 2`, ..., and fires `TICK` callbacks in deadline order while returning `undefined` when interval or callback is invalid (`// setInterval(tick, undefined): undefined`).

---

## 8. What Could Not Be Verified (`Confidence: High`)

- Only AVM1 and general `Player` frame lifecycle paths were verified against test outputs; AS3 `Stage3D` (`check_requested_context3ds`, `core/src/frame_lifecycle.rs:104`) is outside `swfr`'s AVM1 scope.

---

## 9. Open Questions

1. Should `swfr` adopt Ruffle's `time_offset` busy-loop detection (`core/src/player.rs:568-574`) if an AVM1 game busy-waits on `getTimer()` inside a single frame script?
2. Does `swfr` currently keep removed clips with `onUnload` handlers at negative depth (`-16385 - depth`) for one extra frame like `Avm1::remove_pending` (`core/src/avm1/runtime.rs:462-514`), or fire `unload` immediately on removal? (Investigated in R4 / `03-display-list.md`.)

---

## 10. Test Ideas for `swfr`

1. **`ActionPriority` bucket test (`Initialize` vs `Construct` vs `Default`):** Build a synthetic SWF with a parent clip placing two children that each have `onClipEvent(initialize)`, `onClipEvent(construct)`, `onClipEvent(load)`, and frame-1 `DoAction`, asserting that all `initialize` handlers across siblings/children run before any `construct` or `load`/`DoAction` handler (`tests/tests/swfs/avm1/clip_events/output.txt:1-11`).
2. **Interleaved `DoAction` and `PlaceObject2` test:** Build a synthetic SWF modeled on `tests/tests/swfs/avm1/execution_order1` where `_root` has `DoAction("root 1")`, `PlaceObject2(child)`, `DoAction("root 2")`, asserting trace order `root 1`, `child 1`, `root 2`.
3. **Removed-clip timer auto-cancellation test:** Register `setInterval(mc, "tick", 20)` on a dynamically created MovieClip, call `mc.removeMovieClip()`, step 3 frames, and assert that `tick` is never called after removal (`core/src/timer.rs:110-141`).
