# Technical Specification Document (`swfr`)

## Part 1: Preparing the GitHub Repository

- **1.1 How the AI works here:** CI is the only trusted build; workflow jobs handle all downloads; job summaries and sticky PR comments provide text feedback; goldens and hardware tests go to the human maintainer.
- **1.2 Repository configuration:** Protected `main` (`lints`, `build-test`, `abi`, `cross-build`), `.github/CODEOWNERS`, read-only default token, pinned SHA actions, labels (`human-test`, `goldens-review`, `goldens-approved`, `needs-reference`, `blocked`), issue templates (`human-test.md`, `need.md`).
- **1.3 Bootstrap skeleton:** Workspace with 11 crates (`swfr-reader`, `swfr-swf`, `swfr-scene`, `swfr-render`, `swfr-avm1`, `swfr-avm2`, `swfr-audio`, `swfr-player`, `swfr-libretro`, `swfr-harness`, `swfr-testkit`), `rust-toolchain.toml`, `deny.toml`, `deps-allow.txt`, `references.lock`.
- **1.4 Allowed direct dependencies:** `miniz_oxide`, `lzma-rs`, `tiny-skia`, `png`, `zune-jpeg`, `fontdue`, `symphonia` (`mp3` only).
- **1.5 References:** `references.lock` and `docs/notes/<topic>.md`.
- **1.6 Test corpus:** Hand-derived expectations, programmatic test movies in `swfr-testkit`, and human-approved goldens.
- **1.8 Frontend baseline (V0):** Recorded in `docs/platform-notes.md`.

---

## Part 2: Technical Specification

### 2.1 Goal
`swfr` is a libretro core named `swfr_libretro` that loads `.swf` files and plays them inside any libretro frontend, written from scratch in Rust.

### 2.2 What libretro changes
- Window, GPU, audio device: Owned by the frontend (`no winit`, `no softbuffer`, `no cpal`).
- Threads: Single-threaded on the frontend's thread inside `retro_run`.
- Audio: Pushed once per frame through `retro_audio_sample_batch_t` at 44,100 Hz stereo `i16`.
- Video: CPU-rendered 32-bit `XRGB8888` (`0x00RRGGBB`) frame handed to the frontend each frame.
- Content: Frontend passes SWF bytes at load (`need_fullpath = false`).
- Save states: Not supported in v1 (`retro_serialize_size` returns 0).

### 2.5 Libretro Integration
- Every constant, struct layout, and function signature is verified against `docs/reference/libretro.h` (see `docs/notes/libretro-abi.md`).
- Core options: `swfr_scale` (`1x`, `2x`, `3x`, `4x`), `swfr_quality` (`low`, `medium`, `high`), `swfr_pad_mode` (`cursor`, `keys`), `swfr_cursor_speed` (`1`..`10`), `swfr_script_timeout` (`1`, `2`, `5`, `15`), `swfr_trace_to_log` (`on`, `off`).
- Panic safety: Every exported `extern "C"` function wraps its body in `catch_unwind`. After a panic, the core is marked dead and subsequent `retro_run` calls emit the last good frame (or a solid dark red frame `0x00800000` if none) with silent audio and log the panic once.

### 2.6 Workspace & Player Contract
```rust
Player::new(swf_bytes: &[u8], base_dir: Option<String>, opts: &Options) -> Result<Player, PlayerError>
Player::av_info(&self) -> AvInfo
Player::run_frame(&mut self, input: &Input) -> FrameOut<'_>
Player::reset(&mut self)
```

### 2.11 Safety and Limits
- `Input file size`: 64 MiB
- `Decompressed size`: 64 MiB
- `Tag count`: 2,000,000
- `Edges in one shape`: 1,000,000
- `Movie clip nesting depth`: 256
- `AVM call depth`: 256
- `Bitmap side length`: 8,192 px
- `Total bitmap memory`: 64 MiB
- `String length`: 16 MiB
- `Array length`: 1,048,576
- `Script time per frame`: option, default 2 s
- `Script heap, desktop`: 128 MiB
- `Script heap, Reference B`: 48 MiB

### 2.12 Milestones
- **M0:** Bring-up with a stub player (320x240 color bars, 440 Hz tone, `libretro.h` constant verification test, mock frontend ABI test, `swfr_libretro.info`, Linux & Windows builds in `cross.yml`, `swfr-harness` `AvInfo` CLI, V1 human-test issue).
- **M1:** Container and tags (`swfr-reader`, `swfr-swf`, `swfr-testkit` builder & mutator).
- **M2:** Shapes and rendering (`swfr-scene`, `swfr-render`, caches, dirty rectangles, goldens).
- **M3:** Timeline, input, text, and libretro wiring.
- **M4:** AVM1 (AS1 & AS2 interpreter, generational arena heap & incremental collector).
- **M5:** Audio (PCM, ADPCM, MP3 via `symphonia`, resampler, mixer).
- **M6:** Packaging and platform verification (`release.yml`, V5–V7).
- **M7:** AVM2 (ActionScript 3).

---

## Part 3: Autonomous completion directive (for the AI)

Where Part 3 conflicts with Part 2, **Part 3 wins** for the duration of this autonomous run.

### 3.1 Definition of done
1. Every required CI job on `main` (and the active session branch) is green.
2. Milestones **M1 through M6** are implemented as specified in 2.12, with the substitutions in 3.6 where an exit criterion in 2.12 depends on owner hardware or owner approval.
3. `cross.yml` builds the core for **every target in 2.4** (Linux `x86_64`, `x86`, `armv7`, `aarch64`; Android `armv7`, `thumbv7neon`, `aarch64`; Windows `x86_64`, `x86` GNU and MSVC; macOS `arm64`, `x86_64`; iOS `arm64` build-only).
4. `release.yml` produces a draft release (or workflow artifact) with one zip per platform containing the library, `.info`, `README.md`, and licenses.
5. Every tag in the SWF specification v19 either has a builder round-trip test or is listed in `docs/notes/tag-coverage.md` with the reason it is unsupported.
6. The AS1/AS2 conformance suite (M4) has at least 150 generated movies with expected logs derived from ES3 semantics, all passing.
7. Audio tests (M5, including the marker-pulse stream sync test) and rendering tests (M2, M3) pass against approved or provisional goldens (see 3.7).
8. Release core size is under 6 MB and `bench` shows no regression above 30%.
9. `README.md`, `docs/INSTALL.md`, and `docs/KNOWN_LIMITATIONS.md` exist and match the code.
10. All `human-test` issues (V1 through V7) are open with artifact links ready for the owner to run when they return, and a final summary issue titled `v1 engineering-complete report` is open with the checklist from 3.1 and links to every piece of evidence.

### 3.5 Task 0: Fix M0 audit gaps
1. Add a `cross-build` step that lists exported `retro_*` symbols (`nm -D` on Linux, `llvm-nm` or `dumpbin` on other platforms) and fails the job unless all 25 symbols in 2.5 are present.
2. Pin every action by commit SHA.
3. Fix the `ci-report` table rendering and include every job.
4. Apply the `human-test` label to Issue #2 and fix its checklist formatting.
5. Add the missing `windows-latest` and `macos-latest` matrix entries for `build-test` and `abi`, and the native Windows MSVC build.
6. Mark `docs/platform-notes.md` as unverified.
7. Report any environment constraint in a `question` issue.

### 3.6–3.13 Milestone Exit Criteria Substitutions, Provisional Goldens, Audits, and Heartbeat
- **M1:** Tag coverage table in `docs/notes/tag-coverage.md`; 100k-iteration deterministic mutator test.
- **M2:** Provisional goldens in `crates/swfr-render/tests/golden-provisional/` + `GOLDENS.md`; open V2 `human-test` issue.
- **M3:** ABI tests for `retro_input_state_t` (`JOYPAD`, `ANALOG`, `MOUSE`, `POINTER`), keyboard callback, and live `swfr_scale` `SET_GEOMETRY`; open V3 and V4 `human-test` issues.
- **M4:** Benchmark flat `match` vs decoded `Vec<Op>` in `DECISIONS.md`; >= 150 generated ES3 conformance movies in `crates/swfr-avm1/tests/conformance.rs`; open V5 `human-test` issue.
- **M5:** PCM, ADPCM, MP3, resampler, and 300-frame marker-pulse stream sync test; open V6 `human-test` issue.
- **M6:** Full `cross.yml` matrix (including QEMU user-mode ABI test on `armv7`), `release.yml`, `bench` per-frame cost in `ci-report`, `README.md`, `docs/INSTALL.md`, `docs/KNOWN_LIMITATIONS.md`, open V7 `human-test` issue and `v1 engineering-complete report` issue.

3.14 End-to-end acceptance (added after the first run failed it)
The first autonomous run reported completion while the released core still played the M0 stub (color bars and a 440 Hz tone) for every file. Unit tests passed because no test drove a real movie through the finished path. This section overrides 3.1 and 3.10. The run is not complete, and no milestone is audited as complete, until all of the following hold. The status in STATUS.md is reset to not complete until then.
1. The stub is gone. The color-bar and tone generators exist only behind a cargo feature m0-stub, which is off by default. A CI step fails cross.yml and release.yml if that feature is enabled in any shipped build.
2. No silent fallback. If parsing or player construction fails, retro_load_game logs the error and returns false. It never substitutes a placeholder frame or tone.
3. Real dependency chain. A CI step runs cargo tree -p swfr-libretro and fails unless it lists swfr-swf, swfr-scene, swfr-render, swfr-avm1 and swfr-audio.
4. End-to-end ABI tests. In swfr-libretro/tests/e2e.rs, using movies from the swfr-testkit builder and only the exported retro_* symbols:
    ◦ A 1-frame movie with a red 100x80 rectangle centered on a blue stage: after load and one run, the center pixel is red, the corner is blue, and retro_get_system_av_info reports the stage size and the header frame rate.
    ◦ A 10-frame movie where a square moves 10 pixels per frame: successive frames differ and the square is at the expected position in each.
    ◦ A movie with a PCM sound event: the audio buffer contains the sound's exact samples. A silent movie yields all-zero samples. Neither is a 440 Hz sine.
    ◦ An AS2 movie calling trace('hello'): the log callback receives a line containing hello.
    ◦ A button movie with scripted pointer input: the click changes the frame.
    ◦ A malformed file: retro_load_game returns false.
5. Real files. The corpus-real job loads every manifest file through the exported ABI and runs 120 frames. It fails if any output matches the M0 stub's frame hash.
6. Honest benchmarks. bench runs the 10-frame movie and the largest real file through the full path. A benchmark of the stub does not count.
7. Evidence rule. Every audit item must name the test that proves it and the CI run. A criterion with no test is unmet.
After the gates pass, re-audit M1 to M6 against the new end-to-end tests. Fix whatever fails, then rewrite docs/audits/ and the final report.

Part 4: Prior art and research notes (for the AI)
Researched on 14 October 2026 from public pages. This is not legal advice. Save this part as docs/notes/prior-art.md. Every rule in Parts 2 and 3 still applies, including 2.3: do not translate any other player's source code.
How to use this part
1. Read documentation, changelogs, wikis and issue discussions to learn what behavior matters.
2. Do not read copyleft source code and then reimplement it. That includes GPL, LGPL, AGPL and MPL projects.
3. Do not port code from any player, even a permissive one. Learn the behavior, then write your own.
4. External tools run in CI only, with pinned versions and hashes. Never vendor them into the repository.
5. If a prior-art page changed a decision, say so in DECISIONS.md.
4.1 Projects
Project
License
State
How to use it
Gnash
GPL-3.0
Mirror last pushed 2017
Cautionary tale only. It descends from gameswf and covers AVM1 up to about SWF 7, with some SWF 8 classes. Do not read its source
Lightspark
LGPL-3.0 and GPL-3.0
Active, release 0.9.0 in February 2025
Its release notes are a checklist of hard areas (4.6). Do not read its source
Shumway
Apache-2.0
Archived July 2019
JavaScript, AVM1 and AVM2. Read its test setup and documentation. Not portable, do not port
swfdec
LGPL-2.1 or later
Dormant, mirror at open-flash/swfdec
Handled Flash 3, 4 and many Flash 7 files. Its test/ folder organizes automated tests by area. Layout idea only
gameswf
Public domain
Maintained on SourceForge (tu-testbed)
The only code that is free to copy, but 2.3 still says write your own. Useful as a reference for a small, portable renderer boundary: its rendering module is factored out so other graphics APIs can replace it
Tamarin and avmplus
MPL, GPL and LGPL tri-license
Discontinued, repository adobe/avmplus
AVM2 reference for M7 only. The license makes copying into an MIT or Apache project awkward. Use the AVM2 overview instead
Autodesk Scaleform
Proprietary
No longer sold
Do not use
Ruffle
MIT OR Apache-2.0
Active
The owner excluded it as a base and as a dependency. Its public test corpus and contributor docs are useful as data (4.4)
ruffle_libretro (JesseTG)
Not checked
Experimental, Rust, 184 commits, updated 2023
Proves a Flash libretro core is feasible. Read its public README and issues for libretro pitfalls only. My fetch of its README returned nothing useful
open-flash swf-types
MIT OR Apache-2.0
Version 0.14, 2022
Type definitions only. Not needed
open-flash swf-parser
AGPL-3.0-or-later
Version 0.14, 2022
Never use it and never copy from it
Ruffle's swf crate
MIT OR Apache-2.0
Active
Reads and writes SWF. Possible test-only oracle (4.5), needs owner approval
FFDec (JPEXS)
App GPL-3.0, library LGPL-3.0
Active
Java decompiler and editor. Run in CI as a tool (4.5)
swfmill
GPL
Maintained
XML to SWF and back, as a tool
flasm
BSD
Old
Assembler and disassembler for AVM1 bytecode, Flash 8 and earlier. Tool and syntax reference
Ming
LGPL
Old
Library for writing SWFs. Gnash's early tests used its compiler
Apache Flex SDK
Apache-2.0
Maintained
mxmlc and the AS3 compiler, for M7 test movies
4.2 Legal ground rules
• swfr is MIT or Apache-2.0. Anything copied from GPL, LGPL, AGPL or MPL code would change that. Do not do it.
• Running a GPL tool and using its output as test input does not make swfr derivative. Keep the tool outside the repository.
• Test files from another project are data under that project's license. Record the license and source for each in the manifest, and never commit them.
• If unsure, do not use it. Open a question issue and continue.
4.3 What the predecessors teach
• The version treadmill kills projects. Gnash was still implementing SWF 7 while SWF 8 spread, and never got AVM2. Lightspark started AS3-first, fell back to Gnash for AVM1, and only added direct AVM1 support much later. swfr does the reverse: AVM1 first, with the version targeting in 2.9 and 3.13.
• Late-found gaps are the usual cost. Lightspark's release notes list DefineScalingGrid, an erase blend mode, ADPCM fixes, morph shapes, Nellymoser and even PlaceObject and RemoveObject for SWF 1 and 2 as recent additions. Plan tests for the boring old cases, not only the common ones.
• Test infrastructure decided who survived. Shumway ran three suites on every change: AVM1 trace tests, AVM2 acceptance tests and screenshot reference tests. Gnash began with per-class existence tests built with Ming. The model to copy is trace-based tests plus image tests, as in 2.13.
• Native SWF code is a security surface. A Mozilla engineer argued that processing SWF in C or C++ carries the same security and device-compatibility problems as the original player. Your answer is the limits in 2.11, forbid(unsafe_code) and mutation fuzzing.
• Keep the renderer behind a small boundary. gameswf's design made porting to other graphics APIs possible. Your boundary is swfr-render producing an XRGB frame.
4.4 Ground truth for expected results
The M4 rule in 3.6 has you derive expected logs by reasoning about ES3. That can encode your own misunderstanding. Real Flash Player output is the only trustworthy source.
• How others obtain it. Ruffle's contributor guide says its tests compare trace() output against output recorded from the official debug Flash Player. To capture it, create mm.cfg with ErrorReportingEnable=1 and TraceOutputFileEnable=1, run the movie, and read flashlog.txt from %APPDATA%/Macromedia/Flash Player/Logs/ on Windows, ~/Library/Preferences/Macromedia/Flash Player/Logs/ on macOS or $HOME/.macromedia/Flash_Player/Logs/ on Linux. That guide links an archived copy of Adobe's debug player download page. Only the owner can do this.
• Rule: every conformance test's meta says expectation = "verified" (from real Flash Player output) or "derived" (your reasoning). ci-report shows both counts. Only verified tests support claims of conformance.
• Third-party corpus (owner decision). Ruffle's repository keeps regression SWFs with Flash-Player-derived output.txt files under tests/tests/swfs/avm1 and avm2, each with a test.toml giving frame counts and image checks. The repository is MIT OR Apache-2.0. If the owner approves, add a corpus-ext CI job that sparse-checks-out a pinned commit of the AVM1 directory, runs each movie through swfr-harness, compares traces with output.txt, and writes pass and fail counts plus a known-failure list to docs/COMPAT.md. Nothing is committed. Check the license of each test asset first.
• Making test movies. The same guide compiles AS2 with mtasc -main -header 200:150:30 test.as -swf test.swf and AS3 with mxmlc -output test.swf -compiler.debug=true Test.as. Whether those install on hosted runners is unverified. Your builder, flasm-style assembly and FFDec's XML round trip are alternatives.
4.5 Oracles and tools in CI (all optional until verified)
Tool
Use
Notes
FFDec command line
Dump the tag list of any movie and compare it with swfr-harness dump. Convert SWF to XML and back. Compress and decompress to check container handling
Needs Java on the runner. Pin the release and its hash. Read the exact command names from its -help output
Kaitai swf.ksy
Cross-check header and tag layouts
Already in docs/reference/
swfmill
Write test movies as XML
GPL tool, outside the repo
flasm
Assemble AVM1 test bytecode
BSD
Ruffle's swf crate
Parse your builder's output in tests and compare tag counts
Test-only. Needs the owner to add it to deps-allow.txt. Not shipped
The point of an oracle is to avoid grading your own homework. Use one for every tag decoder you can.
4.6 Hard-areas checklist
Each item becomes a test or a line in docs/KNOWN_LIMITATIONS.md.
1. SWF 1 and 2 files, with PlaceObject and RemoveObject instead of the later tags.
2. Morph shapes, including mixed fill types.
3. ADPCM at every bit depth. Nellymoser and Speex are out of scope: log and play silence.
4. Bitmap smoothing exists only from SWF 8.
5. DefineScalingGrid (nine-slice scaling).
6. Blend modes, including erase.
7. Script order: root timeline against child clips, playhead changes made by a script, and clip constructors running before first use. Frame scripts must not run for a frame that was never reached.
8. AVM1 details: local variable scoping, prototype and class chains, cloning movie clips, goto by label. Reference counting bugs were a recurring problem in a mature player, which supports the tracing collector in 2.9.
9. Mixed AVM1 and AVM2 files and obfuscated AVM1 bytecode.
10. Event order, from an AS1-era reference. Button, mouse and key handlers run immediately when their event happens. The load handler runs in the first frame the clip is on stage, after the parent timeline's code and before the clip's own code and before rendering. The unload handler runs in the first frame the clip is gone, before the parent timeline's code. Test every one of these.
11. The AS3-era frame loop (the elastic racetrack): events run first, queued events wait for the next frame, then enterFrame, then rendering. A widely cited update for AVM2 says some details were inferred and not confirmed by Adobe, so treat them as hypotheses.
12. Streaming sound sync and the AS Sound object.
13. Collector pauses on weak CPUs.
14. Version-gated behavior such as string encoding and case sensitivity by SWF version. Confirm each in your spec notes before coding it. Do not rely on recall.
4.7 Actions for this run
1. Save this part as docs/notes/prior-art.md.
2. Add the 4.6 items to docs/notes/tag-coverage.md or the relevant test plans, and write tests for each one that fits M1 to M5.
3. M3: write event-order tests for item 10 and record the result in DECISIONS.md.
4. M4: add expectation = verified or derived to every conformance test's meta, and show the counts in ci-report.
5. M1: if Java is available on the runner, add an optional ffdec-crosscheck job that compares tag sequences. Report mismatches and do not fail the build until the job has been stable for a while.
6. Open a question issue asking the owner to decide on the corpus-ext job and the Ruffle swf test oracle. Continue without them.
7. Check the public issues of ruffle_libretro for input, audio and timing problems, and add anything relevant to docs/KNOWN_LIMITATIONS.md.
4.8 What I could not verify
• Whether FFDec, Java, mtasc or the Flex SDK install cleanly on hosted runners.
• The license of each individual asset in Ruffle's test directories.
• The content and license of ruffle_libretro.
• Gnash's state beyond a 2017 mirror.
• Whether Adobe's debug player is still obtainable.
• The pages on frame order are old and one is a third party's inference. Real Flash Player output outranks all of them.

Part 5: Ruffle study, split of the research (for the AI)
The owner wants to understand how Ruffle works before deciding how much of it to reuse. The work is split. Claude did about 10%: the map in 5.1 and the access hints in 5.2. You do the other 90%: the work packages in 5.3, written up as notes. This is research only. Do not copy code and do not change swfr crates as part of it.
Order: finish the end-to-end gates in 3.14 first. Then do the study. Then continue Part 3 with the owner's decision on the memo (5.5). If the owner has not decided, the default is option 1 from the earlier discussion: learn the mechanics and write your own.
5.1 The map (verified by Claude)
• Repository layout. Top-level parts are core (the emulator), desktop, render (backends), swf (SWF and ActionScript reading and writing), video, web and wstr (Flash-compatible UCS-2 strings). Player in core is the main entry point. Each platform feature is a trait passed into Player when it is created (render, audio, UI and so on), and a no-op null backend exists for each. Source: the Project Layout page of Ruffle's wiki.
• Licenses. Ruffle is MIT OR Apache-2.0, and its third-party libraries are under compatible licenses. Its contributor guide says it uses no proprietary knowledge or code and is built from inspecting Flash Player output and from license-compatible libraries such as avmplus.
• Frame loop. A FramePhase enum tracks where the player is in a frame: Idle, Enter, Construct, Update, FrameScripts, Exit. In AVM1 only the Update phase does work. It walks an execution list of clips, runs each clip's frame, then runs queued on-load actions. In AVM2 the phases run in order: enter frame, construct frame, run frame, frame scripts, exit frame. Source: pull request 7048 and core/src.
• Modules seen in core/src. player.rs (events, drag, hover, with AVM1 and AVM2 differences), avm1/activation.rs (the opcode interpreter), avm1/function.rs, avm1/globals/, avm1/object/stage_object, display_object/movie_clip.rs, display_object/avm1_button.rs, display_object/container and interactive, context.rs (UpdateContext, RenderContext, an action queue), the library (instantiates characters by id), tag_utils.rs (SwfMovie, SwfSlice), and focus_tracker.rs.
• Memory. Contexts carry a 'gc lifetime, and the garbage collector is the gc-arena crate (bumped to 0.7.0 in the v0.6.0 release notes).
• Rendering. The RenderBackend trait takes shapes as DrawCommand lists from ruffle_render::shape_utils. The shipped backends use wgpu, and for the web a canvas or WebGL path. A software backend appears on the wiki only as an open question.
• Tests. Regression movies live under tests/tests/swfs/avm1 and avm2. Each directory has test.swf, a source file, output.txt (recorded from the official debug Flash Player) and test.toml. Image checks need the imgtests feature. A scanner tool bulk-parses SWFs and an exporter tool writes PNG frames.
• Build. Building the AS3 core needs Java on the path, to build the library of built-in classes.
• Code comments are a data source. The interpreter records places where it deviates from the spec and where Flash Player behaves unexpectedly. For example, a comment says one opcode returns a boolean even in SWF 4, which differs from the spec. Another notes that AVM1 and AVM2 use separate random number generators.
• Other useful pages. The wiki's Helpful Resources page links to Flash Player Oddities (behavior nobody fully understands), a SWF version chart, a debug Flash Player download page, an AS2 language reference and an AMF0 spec.
5.2 Ground rules
1. Read only. Your outputs are notes in docs/notes/ruffle/. Nothing else changes.
2. Pin a version. Use the latest release tag and record its commit SHA at the top of every note. Ruffle moves fast.
3. Own words. Describe behavior and structure. Do not paste code. A single identifier or a path with a line range is fine. The reason is that the owner has not yet decided whether to copy anything.
4. Evidence. Every claim names a file path, the pinned commit and a symbol or line range, or else a wiki or issue link.
5. Confidence. Mark each section high, medium or low, and say what you could not read.
6. Access routes. GitHub blocks some page types for automated tools. Try the blob page, then raw.githubusercontent.com, then the docs.rs source viewer for published crates such as swf, then issues, pull requests and the wiki. If every route fails for a file, say so and move on. Do not guess its content.
7. Verify by tests. For each note, pick at least three claims and confirm them against a regression test's output.txt or a documented Flash Player behavior.
5.3 Work packages
ID
Topic
Where to look
Questions to answer
Output
R1
Inventory
Root Cargo.toml, each member's Cargo.toml, LICENSE.md, README.md
Which crates exist, their sizes, the dependency graph, minimum Rust version, features, any C dependencies. Which crates are published on crates.io and which are not
00-inventory.md
R2
Player and frame loop
core/src/player.rs, context.rs, the frame-phase code, timer and action queue code
How a tick turns elapsed time into frames. How many frames can run per tick. What triggers a render. How timers, the action queue and the exec list are ordered. How events from mouse and keyboard enter. How this maps onto one retro_run per frame
01-player-frame-loop.md
R3
The swf crate
swf/src (read via docs.rs if needed)
Module layout, the tag enum, how AVM1 actions and shapes are parsed, handling of malformed lengths and truncated data, version gating, how compression is handled, how writing works. Compare with swfr-swf and list gaps
02-swf-crate.md
R4
Display list and timeline
core/src/display_object/*, tag_utils.rs, library code
How tags are processed per frame. Place and replace and move semantics, ratio, clip depth, names. Preloading. Goto frame behavior. Buttons and hit testing. Text objects
03-display-list.md
R5
AVM1
core/src/avm1/*, activation.rs, function.rs, globals/, object/
Value and object model, prototype chains, property flags, scopes, registers, constant pools, function calls (this, arguments, super), coercion rules, SWF-version-gated behavior, recursion and time limits, how built-ins are registered and which exist
04-avm1.md
R6
Memory and GC
gc-arena use in core, Collect derives
How the arena and the collector are used, when collection runs, cost of the 'gc lifetime design, what it means for a 512 MB device, comparison with the handle-based design in 2.9
05-gc-memory.md
R7
Rendering
render/*, shape_utils, RenderBackend trait
The trait's methods, shape registration, how fills and strokes become draw commands, bitmaps, masks, blend modes, filters, hairline handling, what a tiny-skia backend would need
06-render.md
R8
Audio
Audio backend trait, mixer, sound instances, decoders
Event versus streaming sound, sync to the timeline, resampling, supported codecs, how stream blocks tie to frames
07-audio.md
R9
Input and UI
Input events, key code tables, focus tracker, UI backend trait
Event types, key code mapping, text input, cursor handling, what a gamepad-to-cursor layer must provide
08-input.md
R10
Tests
tests/, regression_tests.rs, tests/README.md, scanner, exporter, fuzz tests
Harness design, the test.toml fields, how expected output is produced, known-failure handling, counts per directory, which directories look safest to reuse as a corpus, and each directory's license status
09-tests.md
R11
Quirks
Code comments on deviations from the spec, the Flash Player Oddities page, issues labeled as accuracy
A catalog of at least 100 behaviors: what Flash Player does, which SWF versions, where it was found, and a test idea for each
10-quirks.md
R12
Fit for legacy targets
Dependency lists from R1
Could ruffle_core build for ARMv7 and Android API 21 and stay within 6 MB and 32 MB idle? Is it usable as a library from a git dependency? What breaks without a GPU? Optionally write ruffle-probe.yml, a manual workflow that builds a pinned ruffle_core for x86_64 and ARMv7 and reports size, time and dependency counts. The owner reviews the workflow before it runs
11-fit.md
R13
Decision memo
All notes
For each component (parser, display list, AVM1, GC, render, audio, input, tests): reimplement, vendor, depend on, or skip. Give rationale, effort in pull requests, risk, license obligations, and how a docs/PORTED.md ledger would work if anything is copied
12-decision-memo.md
5.4 Quality bar
• A note is done when a developer who has never opened Ruffle could implement the behavior from it.
• Prefer concrete rules over summaries. For example: in this phase, this list is walked in this order, and these things are queued.
• Each note ends with a list of open questions and a list of test ideas for swfr.
• docs/notes/ruffle/00-index.md links every note with its commit SHA and confidence.
5.5 Order, time-box and decision
1. Do R1, then R2, R3, R5, R4, R7, R10 and R12, then R6, R8, R9 and R11, then R13.
2. One pull request per note, merged into dev.
3. Time-box each package. If a package is not finished after its pull request, write what is missing and move on.
4. When R13 is merged, open an issue titled ruffle-study-complete with the memo's table in the body, and label it question. Then return to Part 3.
5. Until the owner replies, apply option 1. Do not vendor or depend on Ruffle code.
5.6 What Claude did not check
• The contents of most files in core/src. The map is built from search results, the wiki and pull request pages.
• Whether the AI's tools can read GitHub source pages. Claude's own fetch of GitHub directory pages was refused.
• Test license status, binary sizes, minimum Rust version and build behavior on ARMv7 or Android 5.
• Whether the unpublished crates in the repository can be used as git dependencies.
