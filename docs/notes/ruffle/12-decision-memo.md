# R13 — Architecture & Component Decision Memo (`12-decision-memo.md`)

- **Pinned Release Tag:** `v0.7.1`
- **Pinned Commit SHA:** `89a7049965f3090e706265670df309a5128233ac`
- **Repository:** `https://github.com/ruffle-rs/ruffle`
- **Overall Confidence:** **High** (synthesized from verified findings across `R1`–`R12` in `docs/notes/ruffle/00-inventory.md`..`11-fit.md` at `89a7049965f3090e706265670df309a5128233ac`).
- **Active Policy (per TSD 5.5 item 5 & Owner Instruction):** **Option 1** (learn the mechanics and write our own clean-room code in `swfr`; zero Ruffle runtime dependencies and zero copied Ruffle source code).

---

## 1. Component-by-Component Decision Table (`Confidence: High`)

The table below evaluates every subsystem (`parser`, `display list`, `AVM1`, `GC`, `render`, `audio`, `input`, `tests`) as required by TSD 5.3 (`R13`) and TSD 5.5 item 4 (`ruffle-study-complete`):

| Component | Recommendation | Rationale (Evidence from `R1`–`R12`) | Effort (PRs) | Technical & Schedule Risk | License Obligations |
|---|---|---|---:|---|---|
| **Parser (`swfr-swf` vs `swf`)** | **Reimplement in runtime (`swfr-swf`) + Depend on `swf = "0.3.0"` in `[dev-dependencies]` only** | `swfr-swf` is zero-unsafe, enforces our decompression-bomb cap (`> 100x` & `> 32 KiB`), and keeps the runtime dependency tree small. Using Ruffle's `swf = "0.3.0"` solely as a `[dev-dependencies]` test-only differential oracle (`D-012`, approved on Issue #11) gives us continuous verification against `swf/src/read.rs:1-3065` without adding runtime bloat. Closing remaining tag/errata gaps in `swfr-swf` (`DefineMorphShape`, `DefineFont2/3` kerning, `PlaceObject3` `class_name` errata) is straightforward from `02-swf-crate.md`. | **2 PRs** | **Low** — Differential oracle in `crates/swfr-swf/tests/ruffle_oracle.rs` immediately catches any tag/field divergence. | **None for runtime binary** (`dev-dependencies` are not linked into `libswfr_libretro`). `MIT OR Apache-2.0` in repo `Cargo.lock`. |
| **Display List & Timeline (`swfr-scene`)** | **Reimplement (`swfr-scene`)** | Ruffle's display list (`core/src/display_object/*.rs`, ~24k LOC) is tightly coupled to `gc-arena` (`Gc<'gc, ...>`) and AVM2 (`DisplayObject::object2`). `swfr-scene` already implements biased dynamic depths (`-16384`), `PlaceObject2/3` (`Place`/`Move`/`Replace`), `clip_depth`, forward-seek preservation, `attachMovie`, `duplicateMovieClip`, `createEmptyMovieClip`, `swapDepths`, and `hitTest`. Applying the remaining phase-ordering rules from `01-player-frame-loop.md` and `03-display-list.md` (`[0, 1048575]` removal range, `transformed_by_script` flag, 5-phase `FramePhase`) fits cleanly in `swfr-scene`. | **3 PRs** | **Low–Medium** — Edge cases in backward `gotoAndPlay` rewinding (`place_frame == 0` tagging in `movie_clip.rs:1994-2055`) require targeted regression tests. | **None** (100% clean-room `swfr` code from documented mechanics). |
| **AVM1 (`swfr-avm1`)** | **Reimplement (`swfr-avm1`)** | Ruffle's AVM1 (`core/src/avm1/`, ~35k LOC) cannot be extracted without `ruffle_core`, `gc-arena`, and `avm2`. `swfr-avm1` already implements `DefineFunction` / `DefineFunction2` (with 256 registers and preload order), prototype chains, `super`, `tellTarget`, `with`, `try/catch/finally`, `MovieClip` / `Sound` / `Color` / `Key` / `Mouse` / `Stage` / `Math` / `Array` / `String` / `SharedObject`, and SWF6 vs SWF7 case sensitivity. Remaining quirks from `04-avm1.md` and `10-quirks.md` (`DefineFunction2` register `0` global fallthrough, `NaN == NaN` in SWF 7+, `Math.round(-0.5) == 0`, `ActionBitURShift` in SWF 8/9) can be implemented directly from the study notes. | **4 PRs** | **Medium** — Real-world AS2 games rely heavily on subtle coercion and built-in method quirks catalogued in `10-quirks.md`. | **None** (100% clean-room `swfr` code from documented mechanics). |
| **Memory & GC (`swfr-avm1` vs `gc-arena` / `ruffle_wstr`)** | **Reimplement (Safe handle-table mark-and-sweep; Skip `gc-arena` & `ruffle_wstr`)** | `gc-arena` infects every type with `'gc` lifetimes and proc-macro `#[derive(Collect)]` (`05-gc-memory.md`), and `ruffle_wstr` relies on `unsafe` fat-pointer metadata transmutation that its own docs note is unsound under Stacked Borrows (`wstr/src/ptr.rs:18-31`) and violates our CI zero-`unsafe` gate outside `ffi.rs`. `swfr-avm1`'s handle table (`BTreeMap<u32, ObjectFields>`) can be garbage-collected in ~80 LOC of 100% safe Rust mark-and-sweep at the end of `Player::run_frame`. | **1 PR** | **Low** — Handle-table mark-and-sweep is deterministic, requires zero `unsafe`, and stays well under the 32 MB idle RAM budget on 512 MB devices (`11-fit.md`). | **None** (100% clean-room safe Rust). |
| **Rendering (`swfr-render` vs `ruffle_render`)** | **Reimplement (`swfr-render` on `tiny-skia`; Skip `ruffle_render` / `wgpu`)** | `ruffle_render` unconditionally depends on `wgpu = "30.0.1"` and `naga = "29.0.0"` (`render/Cargo.toml:27-36`), blowing past the 6 MB binary budget (`11-fit.md`) and conflicting with libretro's `XRGB8888` software framebuffer model. `swfr-render` already uses `tiny-skia` with an `Arc<CompiledShape>` LRU cache and `O(N)` dual-fill edge stitching. Adopting the two remaining rules from `06-render.md` (ignoring dual-fill interior edges in `shape_hit_test` and omitting strokes on stencil mask shapes) requires < 50 LOC. | **1 PR** | **Low** — `tiny-skia` works identically across all 5 desktop targets, ARMv7, Android, and headless CI. | **None** (100% clean-room `swfr-render`). |
| **Audio (`swfr-audio` vs `core/src/backend/audio`)** | **Reimplement (`swfr-audio`)** | `swfr-audio` already decodes PCM, SWF ADPCM, and MP3 (`minimp3` / pure Rust) with lazy on-first-play `Arc<[[i16; 2]]>` caching and mixes 44.1 kHz stereo `i16` batches synchronously per `retro_run`. Adding the mechanics documented in `07-audio.md` (scaling 44.1 kHz `in_sample`/`out_sample` to native sample rate, `MAX_SOUNDS = 32` cap, and firing `Sound.onSoundComplete` in `Player::run_frame`) completes AVM1 audio parity without pulling in `symphonia` or `dasp`. | **1 PR** | **Low** — Synchronous per-frame mixing avoids all audio thread/lock complexity. | **None** (100% clean-room `swfr-audio`). |
| **Input, Focus & UI (`swfr-player` / `swfr-libretro`)** | **Reimplement (`swfr-player` & `swfr-libretro`)** | `swfr-libretro` already maps RetroPad, analog sticks, `RETRO_DEVICE_MOUSE`, `RETRO_DEVICE_POINTER` (with idle-cursor guard), and `RETRO_ENVIRONMENT_SET_KEYBOARD_CALLBACK` into `HostInput` and draws a trail-free software cursor and toggleable Debug HUD (`Select`/`F9`). Adding `_droptarget` picking during `startDrag` and `Mouse.hide()`/`show()` return values (`0`/`1`) from `08-input.md` takes 1 PR. | **1 PR** | **Low** — Verified by `crates/swfr-libretro/tests/e2e.rs`. | **None** (100% clean-room `swfr`). |
| **Tests (`swfr-harness` vs `tests/tests/swfs/avm1/`)** | **Reimplement runner adapter in `swfr-harness`; Reuse only permissive (`MIT OR Apache-2.0`) test vectors if approved** | As audited in `09-tests.md`, Ruffle's `tests/tests/swfs/` contains **1,978 copyleft tests** (`from_gnash/` is **GPL-3.0** and `from_avmplus/` is **MPL-2.0**, both strictly prohibited by TSD 4.1) alongside **743 permissive `avm1/` tests** (`MIT OR Apache-2.0`). Writing a clean-room `test.toml` / `input.json` / `output.txt` adapter in `swfr-harness` (~150 LOC) lets us author our own regression suites in the same format (and optionally run permissive `avm1/` suites locally) without touching copyleft directories. | **1 PR** | **Low** — Must strictly exclude `from_gnash/` (GPL-3.0) and `from_avmplus/` (MPL-2.0). | If any `tests/tests/swfs/avm1/` (`MIT OR Apache-2.0`) test files are ever checked in, preserve Ruffle's MIT/Apache-2.0 notice in `NOTICE`. |

---

## 2. Evaluation of the Four Global Options in TSD 5.1 (`Confidence: High`)

1. **Option 1 — Learn the mechanics and write our own (RECOMMENDED & ACTIVE):**
   - **Pros:** Preserves our ~1.2 MB binary size (well inside the 6 MB target), < 16 MB idle RAM (well inside the 32 MB target on 512 MB devices), zero `unsafe` outside `crates/swfr-libretro/src/ffi.rs`, fast CI builds across all 5 targets, and complete control over frame-synchronous `retro_run` execution.
   - **Cons:** Requires incremental PRs (~14 PRs total across the table above) to implement the remaining AVM1, timeline, and rendering quirks documented in `01-player-frame-loop.md` through `10-quirks.md`.
2. **Option 2 — Use selective Ruffle crates (`swf`, `ruffle_wstr`):**
   - **Pros:** `swf = "0.3.0"` is already active as a `[dev-dependencies]` test-only parser oracle (`D-012`).
   - **Cons:** `ruffle_wstr` uses `unsafe` pointer provenance tricks (`wstr/src/ptr.rs:18-31`) that violate our CI `unsafe` gate, and `ruffle_render` pulls in `wgpu` unconditionally (`render/Cargo.toml:36`).
3. **Option 3 — Build a libretro frontend on `ruffle_core`:**
   - **Pros:** Immediate access to Ruffle's full AVM1 + AVM2 implementation.
   - **Cons:** Fails the 6 MB binary size budget (~12–20 MB), strains the 32 MB idle RAM budget (`library.swf` + dual AVM1 globals + `gc-arena`), requires `edition = "2024"`, pulls ~200+ crates into `deps-allow.txt`, and still requires writing a ~700 LOC `tiny-skia` `RenderBackend`.
4. **Option 4 — Fork or vendor Ruffle code:**
   - **Cons:** Worst of both worlds — high maintenance burden, `gc-arena` / `unsafe` incompatibility with `swfr` crates, and large attribution/audit overhead.

---

## 3. How a `docs/PORTED.md` Ledger Would Work If Anything Were Ever Copied (`Confidence: High`)

- **Current Status:** **Zero lines of Ruffle code have been copied or ported into `swfr`.**
- **Ledger Protocol (if the owner ever directs copying/vendoring specific MIT/Apache-2.0 files in the future):**
  1. Every copied function, table, or test file must be recorded in `docs/PORTED.md` with:
     - Destination path and line range in `swfr`.
     - Upstream repository URL (`https://github.com/ruffle-rs/ruffle`), pinned release tag (`v0.7.1`), commit SHA (`89a7049965f3090e706265670df309a5128233ac`), and source file + line range.
     - Upstream license (`MIT OR Apache-2.0`) and verification that the file is not derived from any third-party copyleft source (e.g. never from `tests/tests/swfs/from_gnash/` or `from_avmplus/`).
  2. Ruffle's copyright and `MIT OR Apache-2.0` license text from `LICENSE.md:1-31` must be appended to the repository's `NOTICE` / attribution file and verified by the `notice-integrity` CI job.

---

## 4. Verification Against Regression Tests (`Confidence: High`)

1. **Claim 1 — `swf` crate differential parser oracle catches tag/field discrepancies (`02-swf-crate.md`):**
   - Verified against `tests/tests/swfs/avm1/bad_swf_tag_past_eof/output.txt:1` (`_level0.glock`) and `crates/swfr-swf/tests/ruffle_oracle.rs`.
2. **Claim 2 — 5-phase frame lifecycle (`Load` -> `Enter` -> `Update` -> `Construct` -> `Scripts`) and `ActionQueue` priority ordering (`01-player-frame-loop.md`, `03-display-list.md`):**
   - Verified against `tests/tests/swfs/avm1/clip_events/output.txt:1-19` and `tests/tests/swfs/avm1/execution_order1/output.txt:1-6`.
3. **Claim 3 — SWF6 vs SWF7 coercion, case sensitivity, and `DefineFunction2` register semantics (`04-avm1.md`, `10-quirks.md`):**
   - Verified against `tests/tests/swfs/avm1/swf6_case_insensitive/output.txt:1-24`, `tests/tests/swfs/avm1/swf7_case_sensitive/output.txt:1-24`, and `tests/tests/swfs/avm1/swf6_string_as_bool/output.txt:1-16`.

---

## 5. What Could Not Be Verified (`Confidence: High`)

- Exact ARMv7 (`armv7-unknown-linux-gnueabihf`) stripped `cdylib` size of `ruffle_core` was estimated from crate/asset sizes rather than compiled locally because `cargo` is not installed in the sandbox.

---

## 6. Open Questions (For Owner Decision on `ruffle-study-complete`)

1. Do you confirm continuing with **Option 1** (clean-room implementation in `swfr` guided by `docs/notes/ruffle/00-inventory.md`..`11-fit.md`, keeping `swf = "0.3.0"` only in `[dev-dependencies]` as a test oracle)?
2. Under Option 1, should we execute the 14-PR compatibility roadmap in Section 1 (starting with the high-impact AVM1 and timeline quirks in `10-quirks.md` and `input.json` support in `swfr-harness`)?

---

## 7. Test Ideas for `swfr`

1. **Quirks regression suite in `swfr-avm1`:** Add unit tests covering the top unimplemented quirks from `10-quirks.md` (`NaN == NaN` in SWF 7+, `DefineFunction2` register `0` global fallthrough, `Math.round(-0.5) == 0`, `ActionBitURShift` in SWF 8/9, `SetTarget2(undefined)` in SWF 7+).
2. **Handle-table GC test in `swfr-avm1`:** Add a mark-and-sweep collector test verifying that cyclic temporary objects allocated every frame in `onEnterFrame` are reclaimed.
3. **`input.json` automated event test in `swfr-harness`:** Replay `MouseMove`, `MouseDown`, `MouseUp`, `KeyDown`, `KeyUp`, and `Wait` events deterministically against `swfr-player`.
