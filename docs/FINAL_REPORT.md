# `swfr` v1 Engineering-Complete Report (Re-Audited per TSD Part 3.14 & Part 3.1)

All autonomous implementation tasks (**Task 0**, **Milestones M0 through M6**, and **TSD Part 3.14 End-to-End Acceptance**) are complete, verified, and re-audited on branch [`arena/a4bfe02a-flash`](https://github.com/wastaken90XD/Flash./tree/arena/a4bfe02a-flash) ([PR #1](https://github.com/wastaken90XD/Flash./pull/1)).

---

## 1. TSD Part 3.14 End-to-End Acceptance Verification Table

Every item below names the exact Rust `#[test]` function identifier(s) and passing GitHub Actions CI run(s) per **TSD 3.14 item 7**.

| # | TSD 3.14 Requirement | Proving Test(s) (`#[test]` identifier) & CI Check | Passing CI Run(s) | Status |
| :--- | :--- | :--- | :--- | :--- |
| **3.14.1** | **The stub is gone:** Color-bar and 440 Hz tone generators exist only behind off-by-default Cargo feature `m0-stub`; CI fails `ci.yml`, `cross.yml`, and `release.yml` if `m0-stub` is enabled in any shipped build | `default_player_rejects_stub_and_malformed_bytes`, `stub_player_renders_320x240_color_bars_and_440hz_tone` (with `--features m0-stub`), `harness_formats_real_movie_av_info_and_rejects_stub`, CI `cargo tree -p swfr-libretro -e features` check | `ci.yml` [#38486527709](https://github.com/wastaken90XD/Flash./actions/runs/38486527709), `cross.yml` [#38486527624](https://github.com/wastaken90XD/Flash./actions/runs/38486527624), `release.yml` [#38486527622](https://github.com/wastaken90XD/Flash./actions/runs/38486527622) | PASS |
| **3.14.2** | **No silent fallback:** If parsing or `Player` construction fails, `retro_load_game` logs the error via `retro_log_printf_t` and returns `false`; never substitutes a placeholder frame or tone | `e2e_malformed_files_return_false_from_retro_load_game`, `default_player_rejects_stub_and_malformed_bytes`, `full_libretro_abi_lifecycle_and_fault_containment` | `ci.yml` [#38486527709](https://github.com/wastaken90XD/Flash./actions/runs/38486527709), `cross.yml` [#38486527624](https://github.com/wastaken90XD/Flash./actions/runs/38486527624) | PASS |
| **3.14.3** | **Real dependency chain:** CI runs `cargo tree -p swfr-libretro` and fails unless it lists `swfr-swf`, `swfr-scene`, `swfr-render`, `swfr-avm1`, and `swfr-audio` | `e2e_1frame_red_rect_on_blue_stage_and_av_info`, `end_to_end_swf_movie_executes_scene_render_avm1_and_audio`, CI step `Verify real dependency chain and m0-stub off by default (TSD 3.14.1 & 3.14.3)` | `ci.yml` [#38486527709](https://github.com/wastaken90XD/Flash./actions/runs/38486527709) | PASS |
| **3.14.4.1** | **E2E ABI test 1:** 1-frame movie with a red 100x80 rectangle centered on a blue stage: after `retro_load_game` and one `retro_run`, center pixel is red (`0x00FF0000`), corner is blue (`0x000000FF`), and `retro_get_system_av_info` reports stage size and header frame rate | `e2e_1frame_red_rect_on_blue_stage_and_av_info` (`crates/swfr-libretro/tests/e2e.rs`) | `ci.yml` [#38486527709](https://github.com/wastaken90XD/Flash./actions/runs/38486527709), `cross.yml` [#38486527624](https://github.com/wastaken90XD/Flash./actions/runs/38486527624) | PASS |
| **3.14.4.2** | **E2E ABI test 2:** 10-frame movie where a square moves 10 pixels per frame: successive frames differ and the square is at the expected position (`x = 10 + 10 * frame`) in each frame | `e2e_10frame_moving_square_differs_and_tracks_position` (`crates/swfr-libretro/tests/e2e.rs`) | `ci.yml` [#38486527709](https://github.com/wastaken90XD/Flash./actions/runs/38486527709), `cross.yml` [#38486527624](https://github.com/wastaken90XD/Flash./actions/runs/38486527624) | PASS |
| **3.14.4.3** | **E2E ABI test 3:** Movie with a PCM sound event: audio buffer contains the sound's exact samples; silent movie yields all-zero samples; neither is a 440 Hz sine | `e2e_pcm_sound_event_exact_samples_and_silent_movie_zero_not_440hz` (`crates/swfr-libretro/tests/e2e.rs`) | `ci.yml` [#38486527709](https://github.com/wastaken90XD/Flash./actions/runs/38486527709), `cross.yml` [#38486527624](https://github.com/wastaken90XD/Flash./actions/runs/38486527624) | PASS |
| **3.14.4.4** | **E2E ABI test 4:** AS2 movie calling `trace('hello')`: `retro_log_printf_t` callback receives a line containing `hello` | `e2e_as2_trace_hello_delivers_log_callback` (`crates/swfr-libretro/tests/e2e.rs`) | `ci.yml` [#38486527709](https://github.com/wastaken90XD/Flash./actions/runs/38486527709), `cross.yml` [#38486527624](https://github.com/wastaken90XD/Flash./actions/runs/38486527624) | PASS |
| **3.14.4.5** | **E2E ABI test 5:** Button movie with scripted pointer input: click changes the frame | `e2e_button_movie_scripted_pointer_click_changes_frame` (`crates/swfr-libretro/tests/e2e.rs`) | `ci.yml` [#38486527709](https://github.com/wastaken90XD/Flash./actions/runs/38486527709), `cross.yml` [#38486527624](https://github.com/wastaken90XD/Flash./actions/runs/38486527624) | PASS |
| **3.14.4.6** | **E2E ABI test 6:** Malformed file: `retro_load_game` returns `false` | `e2e_malformed_files_return_false_from_retro_load_game` (`crates/swfr-libretro/tests/e2e.rs`) | `ci.yml` [#38486527709](https://github.com/wastaken90XD/Flash./actions/runs/38486527709), `cross.yml` [#38486527624](https://github.com/wastaken90XD/Flash./actions/runs/38486527624) | PASS |
| **3.14.5** | **Real files (`corpus-real`):** Loads all 20 manifest SWFs (`corpus/real/MANIFEST.toml`) through the exported ABI and runs 120 frames; fails if any output matches the M0 stub's frame hash (`0xd18a4968fa7fa262`) | `e2e_real_corpus_manifest_runs_120_frames_without_m0_stub` (`ci.yml` job `corpus-real`) | `ci.yml` [#38486527709](https://github.com/wastaken90XD/Flash./actions/runs/38486527709) | PASS |
| **3.14.6** | **Honest benchmarks (`bench`):** Runs the 10-frame moving-square movie (`0.026 ms/frame`) and the largest real corpus SWF (`ruffle-sound.swf`, `6,336 B`, `0.029 ms/frame`) through the full `Player` path | `harness_formats_full_movie_info_and_runs_honest_bench`, `harness_formats_real_movie_av_info_and_rejects_stub`, CI job `bench` | `ci.yml` [#38486527709](https://github.com/wastaken90XD/Flash./actions/runs/38486527709) | PASS |
| **3.14.7** | **Evidence rule:** Every milestone audit item in `docs/audits/m0.md`–`m6.md` names the exact proving `#[test]` and CI run | [`docs/audits/m0.md`](../blob/arena/a4bfe02a-flash/docs/audits/m0.md) – [`docs/audits/m6.md`](../blob/arena/a4bfe02a-flash/docs/audits/m6.md) | `ci.yml` [#38486527709](https://github.com/wastaken90XD/Flash./actions/runs/38486527709) | PASS |

---

## 2. TSD 3.1 Definition of Done Checklist

- [x] **1. Every required CI job is green:**
  - `ci.yml`: [Run #38486527709](https://github.com/wastaken90XD/Flash./actions/runs/38486527709) (push) / [Run #38486532489](https://github.com/wastaken90XD/Flash./actions/runs/38486532489) (PR) — `lints`, `build-test` (`ubuntu`, `macos`, `windows`), `abi` (`ubuntu`, `macos`, `windows` + `e2e` + `fault-inject`), `corpus-real` (`20` real SWFs x `120` frames), `size-check`, `bench`, `cross-build`, and `ci-report` all **green**.
  - `lockfile.yml`: [Run #38486527615](https://github.com/wastaken90XD/Flash./actions/runs/38486527615) — **green**.
  - `cross.yml`: [Run #38486527624](https://github.com/wastaken90XD/Flash./actions/runs/38486527624) (push) / [Run #38486532480](https://github.com/wastaken90XD/Flash./actions/runs/38486532480) (PR) — all target jobs including QEMU `armv7-unknown-linux-gnueabihf` `abi` + `e2e` execution **green**.
  - `release.yml`: [Run #38486527622](https://github.com/wastaken90XD/Flash./actions/runs/38486527622) — all release packaging jobs **green**.
- [x] **2. Milestones M0 through M6 implemented and re-audited under TSD 3.14:**
  - Self-audits: [`docs/audits/m0.md`](../blob/arena/a4bfe02a-flash/docs/audits/m0.md), [`docs/audits/m1.md`](../blob/arena/a4bfe02a-flash/docs/audits/m1.md), [`docs/audits/m2.md`](../blob/arena/a4bfe02a-flash/docs/audits/m2.md), [`docs/audits/m3.md`](../blob/arena/a4bfe02a-flash/docs/audits/m3.md), [`docs/audits/m4.md`](../blob/arena/a4bfe02a-flash/docs/audits/m4.md), [`docs/audits/m5.md`](../blob/arena/a4bfe02a-flash/docs/audits/m5.md), [`docs/audits/m6.md`](../blob/arena/a4bfe02a-flash/docs/audits/m6.md).
  - Status & worklog: [`STATUS.md`](../blob/arena/a4bfe02a-flash/STATUS.md) and [`WORKLOG.md`](../blob/arena/a4bfe02a-flash/WORKLOG.md).
- [x] **3. `cross.yml` builds the core, verifies `m0-stub` is off, verifies all 25 exported `retro_*` symbols, and runs `abi` + `e2e` under QEMU on `armv7-unknown-linux-gnueabihf`:**
  - Verified in [Run #38486527624](https://github.com/wastaken90XD/Flash./actions/runs/38486527624).
- [x] **4. `release.yml` produces release zip artifacts per platform containing the library, `.info`, `README.md`, and licenses, with `m0-stub` verified off:**
  - Verified in [Run #38486527622](https://github.com/wastaken90XD/Flash./actions/runs/38486527622).
- [x] **5. Every tag in SWF specification v19 has a builder round-trip test (`roundtrip_all_supported_swf_v19_tags`) or is documented in `docs/notes/tag-coverage.md`:**
  - Round-trip & 100k-iteration mutator suite (`roundtrip_all_supported_swf_v19_tags`, `roundtrip_compressions_fws_cws_zws_and_long_tags`, `mutator_100k_iterations_never_panics`): [`crates/swfr-testkit/tests/tag_roundtrip.rs`](../blob/arena/a4bfe02a-flash/crates/swfr-testkit/tests/tag_roundtrip.rs).
  - Full SWF v19 tag coverage table: [`docs/notes/tag-coverage.md`](../blob/arena/a4bfe02a-flash/docs/notes/tag-coverage.md) and [`docs/notes/swf-format.md`](../blob/arena/a4bfe02a-flash/docs/notes/swf-format.md).
- [x] **6. AS1/AS2 conformance suite (M4) has at least 150 generated movies with expected logs derived from ES3 semantics, all passing:**
  - 160 generated movies passing in `avm1_es3_conformance_160_generated_movies` ([`crates/swfr-avm1/tests/conformance.rs`](../blob/arena/a4bfe02a-flash/crates/swfr-avm1/tests/conformance.rs)) plus `e2e_as2_trace_hello_delivers_log_callback` and `e2e_m3_avm1_closures_edit_text_and_movieclip_props_via_abi` ([`crates/swfr-libretro/tests/e2e.rs`](../blob/arena/a4bfe02a-flash/crates/swfr-libretro/tests/e2e.rs)).
- [x] **7. Audio tests (M5, including 300-frame marker-pulse stream sync test and PCM E2E test) and rendering tests (M2, M3) pass:**
  - Audio E2E & unit tests: `e2e_pcm_sound_event_exact_samples_and_silent_movie_zero_not_440hz`, `e2e_m4_savestate_roundtrip_and_streaming_audio_via_abi`, `pcm_and_resampler_known_output`, `adpcm_2_3_4_5_bit_decodes_cleanly`, `stream_sync_300_frames_marker_pulse_within_one_frame`.
  - Rendering E2E & golden tests: `e2e_1frame_red_rect_on_blue_stage_and_av_info`, `e2e_10frame_moving_square_differs_and_tracks_position`, `e2e_m2_gradients_morphs_and_sprites_via_abi`, `render_m2_solid_rect_and_dirty_rect_skip`, `render_m2_linear_and_radial_gradients`, `render_m2_lossless_bitmap_clip_mask_and_morph`, `render_m3_button_states_and_dynamic_edit_text`.
- [x] **8. Release core size is under 6 MB and `bench` runs both the 10-frame movie and largest real corpus file with no regression above 30%:**
  - Stripped release `libswfr_libretro.so` size: **~1.06 MiB** (`< 6.00 MiB` budget).
  - `10-Frame Moving Square (200x100)`: **`0.026 ms/frame`** (`26 us`, `38,461.5 fps`, gate `<= 5.200 ms`).
  - `Largest Real Corpus SWF (ruffle-sound.swf, 6336 B)`: **`0.029 ms/frame`** (`29 us`, `34,482.8 fps`, gate `<= 5.200 ms`).
- [x] **9. `README.md`, `docs/INSTALL.md`, and `docs/KNOWN_LIMITATIONS.md` exist and match the code:**
  - [`README.md`](../blob/arena/a4bfe02a-flash/README.md), [`docs/INSTALL.md`](../blob/arena/a4bfe02a-flash/docs/INSTALL.md), [`docs/KNOWN_LIMITATIONS.md`](../blob/arena/a4bfe02a-flash/docs/KNOWN_LIMITATIONS.md).
- [x] **10. All `human-test` issues (V1 through V7) are open with artifact links:**
  - **V1 (M0 stub core):** #2
  - **V2 (M2 vectors, gradients, bitmaps, masks, morphs):** #4
  - **V3 (M3 timeline, nested sprites, buttons, input):** #5
  - **V4 (M3 static/dynamic text & fonts):** #6
  - **V5 (M4 AVM1 interactive scripting & timers):** #7
  - **V6 (M5 PCM/ADPCM/MP3 audio & stream sync):** #8
  - **V7 (M6 release packages & hardware verification):** #9
  - **Environment / branch constraints (`question`):** #3

---

## 3. Items Flagged for Owner Sign-Off on Return (TSD 3.6 & 3.7)

1. **Provisional Goldens (`GOLDENS.md`):**
   - Visual inspection of the 6 provisional PNG goldens in `crates/swfr-render/tests/golden-provisional/` (`m2_solid_rect.png`, `m2_linear_gradient.png`, `m2_radial_gradient.png`, `m2_lossless_bitmap_fill.png`, `m2_clip_depth_mask.png`, `m3_button_and_text.png`) and promotion to `crates/swfr-render/tests/golden/`.
2. **Third-Party License Sign-Off (`THIRD_PARTY_LICENSES.md`, `deny.toml`):**
   - All direct dependencies (`miniz_oxide 0.9.1`, `lzma-rs 0.3.0`, `tiny-skia 0.12.0`, `png 0.18.1`, `zune-jpeg 0.5.15`, `fontdue 0.9.4`, `symphonia 0.5.5`) are recorded in `Cargo.lock`, `deps-allow.txt`, `deny.toml`, and `THIRD_PARTY_LICENSES.md`. `symphonia 0.5.5` is licensed under `MPL-2.0` (weak file-level copyleft); owner sign-off is requested for distributing prebuilt binary archives containing `symphonia`.
3. **Physical Hardware Verification (`docs/platform-notes.md`):**
   - `docs/platform-notes.md` is marked `UNVERIFIED` pending owner testing of V1–V7 on physical Reference A/B/C hardware.
