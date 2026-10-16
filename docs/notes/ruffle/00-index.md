# Ruffle Study Index (`00-index.md`)

- **Pinned Upstream Repository:** `https://github.com/ruffle-rs/ruffle`
- **Pinned Release Tag:** `v0.7.1`
- **Pinned Upstream Commit SHA:** `89a7049965f3090e706265670df309a5128233ac`
- **Execution Order (per TSD 5.5 item 1):** `R1` -> `R2` -> `R3` -> `R5` -> `R4` -> `R7` -> `R10` -> `R12` -> `R6` -> `R8` -> `R9` -> `R11` -> `R13`

---

## Study Notes (`R1`–`R13`)

| Order | Work Package | Note File | Pinned Ruffle Commit SHA | `swfr` Commit | Overall Confidence | Summary |
|---:|---|---|---|---|---|---|
| 1 | **R1** | [00-inventory.md](./00-inventory.md) | `89a7049965f3090e706265670df309a5128233ac` (`v0.7.1`) | `42a6c77` | **High** | Workspace & crate inventory (36 `Cargo.toml` packages, LOC, dependency counts, licenses, `edition = "2024"`, Rust `>= 1.85.0`). |
| 2 | **R2** | [01-player-frame-loop.md](./01-player-frame-loop.md) | `89a7049965f3090e706265670df309a5128233ac` (`v0.7.1`) | `70d8f9b` | **High** | `Player::tick`, `run_frame`, 5-phase `FramePhase` (`Load` -> `Enter` -> `Update` -> `Construct` -> `Scripts`), 3-priority `ActionQueue`, `Timers`, execution limits, and mapping to `retro_run`. |
| 3 | **R3** | [02-swf-crate.md](./02-swf-crate.md) | `89a7049965f3090e706265670df309a5128233ac` (`v0.7.1`) | `f6068c8` | **High** | `swf` (`0.3.0`) crate architecture, 92 `TagCode`s, 102 AVM1 opcodes, `SwfStr`, `Twips`, `Matrix`, `ColorTransform`, malformed tag clamping, and gap comparison vs `swfr-swf`. |
| 4 | **R5** | [04-avm1.md](./04-avm1.md) | `89a7049965f3090e706265670df309a5128233ac` (`v0.7.1`) | `2c61e12` | **High** | AVM1 `Value`, `Object` (`NativeObject`), `PropertyMap` (SWF6 vs SWF7 case sensitivity), `Scope`, `Activation`, `DefineFunction2` register preload, `SuperObject`, `MovieClipReference`, and 62 built-in modules. |
| 5 | **R4** | [03-display-list.md](./03-display-list.md) | `89a7049965f3090e706265670df309a5128233ac` (`v0.7.1`) | `c47b487` | **High** | `DisplayObject` enum (11 variants), `ChildContainer` depth `BTreeMap` (`AVM_DEPTH_BIAS = -16384`), `PlaceObject` (`Place`/`Move`/`Replace`), `goto_frame` forward/backward rules, `Avm1Button`, `EditText`, and `Drawing`. |
| 6 | **R7** | [06-render.md](./06-render.md) | `89a7049965f3090e706265670df309a5128233ac` (`v0.7.1`) | `9a1f87f` | **High** | `RenderBackend` trait, `CommandList`, `ShapeConverter` dual-fill edge flipping/stitching, `shape_hit_test`, stencil mask stroke omission, and `tiny-skia` backend blueprint. |
| 7 | **R10** | [09-tests.md](./09-tests.md) | `89a7049965f3090e706265670df309a5128233ac` (`v0.7.1`) | `46be0f5` / `5ee69cd` | **High** | `ruffle_test_framework`, `test.toml`, `input.json` (`AutomatedEvent`), `DeterministicLocaleBackend`, `AvmRng`, 4,451 test suite breakdown, and per-directory license audit (`avm1/` MIT/Apache-2.0 vs `from_gnash/` GPL-3.0 & `from_avmplus/` MPL-2.0). |
| 8 | **R12** | [11-fit.md](./11-fit.md) | `89a7049965f3090e706265670df309a5128233ac` (`v0.7.1`) | `c3b55a6` / `5ee69cd` | **High** | All 11 Ruffle backend traits mapped to libretro callbacks, sandbox safety audit (0 threads, 0 sockets, 0 TLS), `wgpu` vs `tiny-skia`, and ARMv7 / Android API 21 / 6 MB binary / 32 MB RAM fit analysis. |
| 9 | **R6** | [05-gc-memory.md](./05-gc-memory.md) | `89a7049965f3090e706265670df309a5128233ac` (`v0.7.1`) | `507f558` | **High** | `gc-arena` (`GcRoot`, `Gc`, `GcRefLock`, `collect_debt`), `WStr`/`WString` (Latin-1 vs UCS-2 and Stacked Borrows `unsafe` caveat), `AvmStringInterner`, and comparison with `swfr-avm1`'s safe handle table. |
| 10 | **R8** | [07-audio.md](./07-audio.md) | `89a7049965f3090e706265670df309a5128233ac` (`v0.7.1`) | `808ab7b` | **High** | `AudioBackend`, `AudioMixer`, 6 audio decoders, `MAX_SOUNDS = 32`, event vs stream sounds, 44.1 kHz `in_sample`/`out_sample` scaling, and AVM1 `Sound` lazy `duration`/`position` + `onSoundComplete`. |
| 11 | **R9** | [08-input.md](./08-input.md) | `89a7049965f3090e706265670df309a5128233ac` (`v0.7.1`) | `763d02d` | **High** | `PlayerEvent` -> `InputEvent` -> `ClipEvent`, `Stage::build_matrices` letterbox/scale mapping, `update_mouse_state`, `update_drag` (`_droptarget` pick), `Key`, `Mouse`, `Selection`, and `FocusTracker`. |
| 12 | **R11** | [10-quirks.md](./10-quirks.md) | `89a7049965f3090e706265670df309a5128233ac` (`v0.7.1`) | `df5eab5` / `5ee69cd` | **High** | Catalogue of 102 concrete Flash Player behaviors/quirks across coercion, operators, closures, `DefineFunction2`, built-ins, depths, timeline, text, SWF19 errata, and rendering. |
| 13 | **R13** | [12-decision-memo.md](./12-decision-memo.md) | `89a7049965f3090e706265670df309a5128233ac` (`v0.7.1`) | *(this commit)* | **High** | Component-by-component decision table (`parser`, `display list`, `AVM1`, `GC`, `render`, `audio`, `input`, `tests`), Option 1–4 evaluation, and `docs/PORTED.md` ledger protocol. |
