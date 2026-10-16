TSD: swfr, a SWF player as a libretro core, written from scratch in Rust
This document has two parts. Part 1 is for you: set it up once before the AI starts. Part 2 is the TSD the AI works from.
The AI works entirely on GitHub. It edits code on branches and pull requests, and GitHub Actions is its compiler, test runner and release builder. It can read the web for research, but it cannot download files from a shell. I am assuming it can read workflow results and pull request comments through its GitHub access, and that it can fetch and read web pages. Confirm both in 1.9, because the loop below breaks without them.
Part 1: Preparing the GitHub repository (you)
1.1 How the AI works here
• It can: read and write repository files, open branches, pull requests and issues, read CI results and pull request comments, and read web pages (specs, docs, crate pages).
• It cannot: download anything in a shell (no curl, wget, cargo fetch, rustup, pip). So it probably cannot build with dependencies locally, cannot run RetroArch, and cannot reliably look at images.
Design rules that follow from this:
1. CI is the only trusted build. Every task ends with a push and a wait for a green run.
2. Anything that needs downloading is a workflow job: crates, reference documents, real-world SWFs, toolchains.
3. Results are readable as text. Every job writes a summary, and each pull request gets one sticky ci-report comment. Artifacts are for you, not for the AI.
4. Things that need eyes or hardware go to you: goldens by label, device tests by issue.
5. No local toolchain is assumed. The local shell is for text work.
6. Keep the loop short. Push, wait, read the report, fix. Required jobs target under 10 minutes, cheapest checks first.
1.2 Create and configure the repository
• Make it private until you have checked the redistribution terms of the reference files (1.5).
• Protect main: pull request required, required status checks lints, build-test, abi and cross-build (names must match the workflow jobs exactly), code owner review required, no force pushes.
• Add .github/CODEOWNERS so you must approve changes to the sensitive paths:
/docs/reference/             @you
**/golden/                   @you
/Cargo.toml                  @you
/deny.toml                   @you
/deps-allow.txt              @you
/references.lock             @you
/corpus/real/MANIFEST.toml   @you
/.github/                    @you
• Actions settings: the default token is read-only; workflows ask for write access only where 2.19 says so. Allow GitHub-authored actions plus a short list of third-party ones, each pinned by commit SHA. Store no secrets.
• Create labels: human-test, goldens-review, goldens-approved, needs-reference, blocked.
• Add issue templates human-test.md and need.md.
• Hosted runners: ubuntu-latest, windows-latest, macos-latest. Check the current image versions and architectures when the workflows are written.
1.3 What you commit before the AI starts
The bootstrap gives the AI a working feedback loop from its first push.
swfr/
  AGENTS.md              rules the AI reads first (copy of 2.16)
  STATUS.md              one line per milestone
  DECISIONS.md           decisions log
  THIRD_PARTY.md         crates and licenses
  Cargo.toml             workspace, lints, dependencies
  Cargo.lock             maintained by the lockfile workflow
  rust-toolchain.toml
  deny.toml
  deps-allow.txt         the allowed direct dependencies
  references.lock        reference documents (1.5)
  .github/
    CODEOWNERS
    ISSUE_TEMPLATE/
    workflows/ci.yml     bootstrap only, the rest is built in 2.19
  docs/
    platform-notes.md    your V0 answers (1.8)
    notes/               the AI's research notes
    reference/           filled by the fetch-references workflow
  corpus/real/MANIFEST.toml
  bench/baseline.json
  crates/
    swfr-reader  swfr-swf  swfr-scene  swfr-render  swfr-avm1
    swfr-avm2 (empty placeholder)  swfr-audio  swfr-player
    swfr-libretro (cdylib and rlib, the only crate allowed unsafe)
    swfr-harness (development CLI)
    swfr-testkit (test helpers and the SWF builder, see 1.6)
Every crate is an empty lib.rs that compiles. Files you write by hand:
rust-toolchain.toml, pinned so CI and everyone else agree. Jobs add cross targets themselves:
[toolchain]
channel = "<pinned stable version>"
components = ["rustfmt", "clippy"]
Workspace lints in the root Cargo.toml, inherited with [lints] workspace = true in every crate except swfr-libretro:
[workspace.lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"

[workspace.lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
indexing_slicing = "deny"
panic = "deny"
arithmetic_side_effects = "warn"
swfr-libretro gets its own [lints] table with unsafe_code = "deny", copying the rest, and a single #[allow(unsafe_code)] on its ffi module.
deny.toml: allowed licenses (MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode), crates.io as the only source, advisories checked. Confirm the key names against the cargo-deny documentation.
Bootstrap ci.yml:
name: ci
on:
  pull_request:
  push:
    branches: [main]
permissions:
  contents: read
jobs:
  build-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@<pinned commit sha>
      - run: rustup show
      - run: cargo fmt --all --check
      - run: cargo clippy --workspace --locked -- -D warnings
      - run: cargo test --workspace --locked
The --locked flag needs a committed Cargo.lock. Generate it once yourself.
1.4 Dependencies without a local fetch
The AI cannot run cargo add, so crates arrive through pull requests and CI.
Need
Crate (verify each before allowing it)
First used
Inflate
miniz_oxide
M1
LZMA (ZWS files)
lzma-rs
M1
Rasterizer
tiny-skia
M2
PNG decode and encode
png
M2
JPEG decode
one pure-Rust decoder, your pick
M2
Device-font glyphs
one small pure-Rust font crate, your pick
M3
MP3 decode
symphonia with only the mp3 feature, default features off
M5



• Put the whole list in the root Cargo.toml and deps-allow.txt now, so the first lockfile is complete.
• A lints job compares the direct dependencies of every workspace crate (from cargo metadata) with deps-allow.txt. Anything else fails the check until you edit that file, which you own.
• The lockfile workflow (2.19) runs cargo fetch on branches that change a Cargo.toml and commits the updated Cargo.lock as a bot. The AI never edits the lockfile by hand.
• New crate versions should be at least 14 days old. You review lockfile diffs in pull requests.
• No crate is needed for libretro itself. Its FFI types are written by hand inside the one crate allowed to use unsafe, and a test checks them against the header (M0).
1.5 References and research
The AI reads documents on the web but cannot save them. Two mechanisms cover that.
references.lock lists documents. The fetch-references workflow (manual run, 2.19) downloads each one, checks its hash, converts PDFs to text, and opens a pull request that you review and merge. The AI may add rows after finding URLs through research. Only you run the workflow and merge the result.
[[ref]]
name = "libretro-h"
url = "<url>"
sha256 = ""                 # empty on the first run, then pinned
dest = "docs/reference/libretro.h"
commit = true               # true only if redistribution is allowed
license_note = "<check before committing>"
Rows with commit = false are only checked for reachability and a stable hash. The AI then reads those documents straight from the web.
docs/notes/<topic>.md are the AI's own working notes: layouts, tables and rules in its own words, each with a source URL and section. Code comments cite these notes. No long verbatim passages.
Redistribution. The AVM2 overview carries an all-rights-reserved notice, and the SWF specification's license text only lets you print one copy, even though Adobe publishes the format as open for implementing. I have not confirmed what is allowed for copies in a repository. Until you have, keep both out of any public repo and set commit = false. libretro.h and the core info example are open source, but confirm their licenses too.
Reference
Why
Candidate source (verify URL and license)
Needed by
libretro.h
Exact constants and struct layouts
The libretro-common repository
M0, blocking
libretro docs
Environment calls, options, input, VFS, core info files
docs.libretro.com
M0
Core info example
.info field names
The libretro core-info collection
M0
SWF specification v19
Every tag and action
Adobe's SWF and AMF page (archived copies exist), or the mirror referenced from the Kaitai SWF page
M1, blocking
swf.ksy
Cross-check of tag layouts
formats.kaitai.io/swf
M1
AVM2 overview
ABC and opcodes
Adobe, university-hosted copies exist
M7
ECMA-262, 3rd edition
AS1 and AS2 semantics
Ecma International
M4
LZMA specification
SWF LZMA header details
LZMA SDK documentation
M1
One open-licensed font
Device-font fallback
Your choice, with its license text
M3
1.6 Test corpus without downloads
There is no reference Flash player to compare against. Expectations therefore come from three places: values derived by hand for simple shapes, expected trace() output known from how each test movie is built, and goldens you approve by eye (2.13). Say so in corpus/README.md.
• Generated in code. swfr-testkit contains a SWF builder that writes movies byte by byte: header, tags, bit-packed records, an assembler for AS1 and AS2 bytecode, PCM and ADPCM sounds, and deliberately malformed variants. The AI writes the builder and the movies as Rust code, so there are no binary SWFs to download. Expected results sit beside each generator.
• Real-world files. corpus/real/MANIFEST.toml lists files to download in CI, each with URL, SHA-256, license and source page. The corpus-real job fetches them into the cache and runs them. Real files are never committed. A check rejects any row missing a license. You review each license before merging a row.
[[file]]
name = "example-game"
url = "<url>"
sha256 = "<hash>"
license = "<SPDX id or text>"
source_page = "<url>"
swf_version = 8
frames_to_capture = [1, 30]
• Compiled content. Compiling AS2 or AS3 sources with a SWF compiler in CI is possible but depends on a compiler being installable on the runners. I have not verified that, so treat it as optional. AS1 and AS2 can be covered with the bytecode assembler instead. AS3 (M7) will need a compiler.
• Goldens. Approved through a pull request label (2.19).
1.7 What stays with you
• Review and merge pull requests that touch protected paths.
• Approve goldens with the goldens-approved label.
• Run device tests with CI-built artifacts and report in the human-test issues.
• Run fetch-references and merge its pull request.
• Check license terms of anything that gets committed.
• Supply anything neither CI nor the AI can get, such as paywalled documents.
1.8 Verify the frontends (test V0)
Install RetroArch on a Windows 10 machine, a Linux machine and an Android 5 device. Check whether a current RetroArch build still runs on that Android version. Write the result and the exact RetroArch versions in docs/platform-notes.md. If a floor device cannot run any current RetroArch, the floor in Part 2 changes before M0.
1.9 Preflight checklist
[ ] Repository created, private, main protected with the required check names
[ ] CODEOWNERS active on the listed paths
[ ] Actions enabled, default token read-only, allowed actions pinned by SHA
[ ] Skeleton committed, every crate compiles, bootstrap ci.yml green on main
[ ] Root Cargo.toml, deps-allow.txt and deny.toml agree, and Cargo.lock is committed
[ ] references.lock seeded with at least libretro.h, the SWF spec and the core info example
[ ] Redistribution of each reference checked, commit flags set
[ ] fetch-references run once and merged
[ ] Labels and issue templates created
[ ] docs/platform-notes.md filled in (V0)
[ ] AGENTS.md committed with the rules from 2.16
[ ] Confirmed that the AI can read workflow runs, job logs and pull request comments through its GitHub access, and can fetch web pages
[ ] Tag bootstrap-complete created
Part 2: Technical specification (for the AI)
2.1 Goal
swfr is a libretro core named swfr_libretro that loads .swf files and plays them inside any libretro frontend. It is written from scratch in Rust. Success means the fixed corpus renders, behaves and sounds correct when driven through the real libretro C ABI, with the frontends listed in 2.4, inside the budgets in 2.14.
2.2 What libretro changes
The frontend owns the window, the GPU, the audio device and the input devices. The core only produces one frame of video and audio per call.
Concern
Decision
Window, GPU, audio device
Frontend. The core has none. No winit, no softbuffer, no cpal anywhere in the repo
Threads
None. Everything runs on the frontend's thread inside retro_run. No audio thread, no ring buffer
Audio
Pushed once per frame through the batch callback
Video
CPU-rendered 32-bit XRGB frame handed to the frontend each frame
Content
The frontend passes the SWF bytes at load. need_fullpath is false
Save states
Not supported in v1
Standalone player
Out of scope. swfr-harness exists only for development and tests
Distribution
One shared library per platform and architecture, plus one .info file
2.3 Scope
Written in-house: the SWF reader and tag decoder, display list and timeline, shape-to-path conversion and compositing glue, AVM1, AVM2 and the ABC verifier, the VM object model and collector, the audio mixer, the player loop, the libretro layer.
Dependencies allowed: only the crates listed in deps-allow.txt (1.4). Rasterization, inflate, LZMA, PNG, JPEG, MP3 and font glyphs come from them.
Sources of truth: the references in references.lock, read from the web or from docs/reference/ where they are fetched, and your own notes in docs/notes/. Do not translate any other player's source code. Behavior the spec leaves open is decided by a test movie and recorded in DECISIONS.md.
Non-goals for v1: networking, LocalConnection, camera and microphone, DRM, Stage3D, video codecs, Nellymoser and Speex audio, filters, save states, hardware rendering, a standalone window, SWF versions above 19 except as corpus files require.
2.4 Platforms and reference hardware
Platform
Artifact
Notes
Windows 10, x86_64 and x86
swfr_libretro.dll
Built by you on a Windows machine
Linux, x86_64, x86, ARMv7, AArch64
swfr_libretro.so
The AI builds and tests the x86_64 one directly
macOS, arm64 and x86_64
swfr_libretro.dylib
Built by you
Android 5.0 (API 21), ARMv7 baseline, ARMv7 NEON, AArch64
swfr_libretro.so
Linked with the NDK API 21 wrappers
iOS, AArch64
static library
Packaging unverified. Gate in M6
Web
none
Deferred. Needs an Emscripten build of the core. Not in v1
The default armv7-linux-androideabi target compiles with NEON off. thumbv7neon-linux-androideabi turns it on. Build both. M2 measures them, and if the baseline build cannot meet budget, ship NEON only and record the dropped devices in DECISIONS.md. No runtime CPU detection.
Reference hardware (all performance gates are measured here, not in emulators):
• A: Core 2 Duo-class, 2 GB RAM, Windows 10 or Linux.
• B: ARMv7 Cortex-A9-class at 1 GHz, 512 MB RAM, Android 5.0.
Whether current RetroArch builds run on your floor devices is test V0 (section 1.8). The AI assumes the answer is recorded in docs/platform-notes.md and reads it before M0. Hosted CI runners are newer than your floors, so CI proves the core builds and passes tests, not that it runs on Windows 10 or Android 5. People verify that.
2.5 Libretro integration
Rule zero: every constant, struct layout and function signature is copied from docs/reference/libretro.h. The names in this section orient you. If a name here is not in the header, drop it from the design and log it in DECISIONS.md. Never type a numeric value from memory.
Exports. swfr-libretro exports every symbol the header marks as required, with C ABI and unmangled names:
Function
Behavior in swfr
retro_api_version
Returns the version constant from the header
retro_set_environment
Stores the callback, then declares core options and the pixel format (see environment table)
retro_set_video_refresh, retro_set_audio_sample, retro_set_audio_sample_batch, retro_set_input_poll, retro_set_input_state
Store the callbacks. The single-sample audio callback is stored and never used
retro_init
Creates the global state. Does not touch content
retro_deinit
Drops the global state
retro_get_system_info
Name swfr, version from Cargo.toml, extensions swf, need_fullpath = false, block_extract = false
retro_load_game
Null info returns false. Otherwise parse the bytes, build the Player, return true. Parse errors are logged and return false
retro_load_game_special
Returns false
retro_get_system_av_info
Geometry and timing from the loaded movie (see Video and Timing)
retro_run
One frame: poll input, run the player once, submit video, submit audio
retro_reset
Calls Player::reset: back to frame 1, VM state cleared
retro_unload_game
Drops the player, keeps the global state
retro_serialize_size
Returns 0. retro_serialize and retro_unserialize return false
retro_cheat_reset, retro_cheat_set
No-ops
retro_get_region
The NTSC constant from the header
retro_get_memory_data, retro_get_memory_size
Null and 0
retro_set_controller_port_device
Stores the device for port 0, which switches pad mode (see Input)
Environment calls. Check each against the header before using it:
Purpose
Used for
Set pixel format
Request 32-bit XRGB. If the frontend refuses, retro_load_game fails with a log line
Get log interface
trace() output and diagnostics. Fall back to standard error when absent
Set core options (v2 if the header has it, else the older variables call) and get variable
Options table below. Read in retro_load_game and again whenever the update flag is set
Get system and save directory
Stored for future use. v1 writes nothing
Set input descriptors
Names for the pad buttons so frontends show sensible labels
Set keyboard callback
Receives key down and up events with character codes
Set geometry
When the scale option changes at runtime and the new size fits within the declared maximum
Get VFS interface
Reading sibling files for relative loadMovie in M4 or later. Optional
Get can-dupe
Decides whether unchanged frames may be sent as a null buffer
Set performance level
Tells the frontend this core needs little
Set support no game
Set to false
Video. The core owns one buffer of width * height unsigned 32-bit pixels, each 0x00RRGGBB in native endianness, with a pitch of width * 4 bytes. The stage background is opaque, so the final composited frame has alpha 255 everywhere. Convert from the rasterizer's premultiplied RGBA by writing (r << 16) | (g << 8) | b per pixel. Geometry:
• Base size = stage size in pixels (header rectangle in twips divided by 20) times the scale option.
• Maximum size = stage size times 4, capped at 4,194,304 pixels total. The scale option never exceeds what fits.
• Aspect ratio = stage width over stage height, ignoring scale.
• If nothing changed in the frame and can-dupe is true, call the video callback with a null buffer and the previous dimensions. Otherwise send the full buffer.
Timing. Frame rate comes from the SWF header (8.8 fixed point), clamped to 1 through 120. The core declares that rate and a fixed audio sample rate of 44,100. Samples per frame are 44100 / fps with the fractional remainder carried to the next frame, so over any second the totals are exact.
Audio. Every retro_run submits exactly the computed number of stereo 16-bit frames through the batch callback, silence included. The Player mixes into a buffer allocated once at load.
Input. retro_run calls the poll callback exactly once at the start, then reads state:
Device
Maps to
Pointer (touch)
Mouse position and left button. First touch only. Coordinates are converted from the frontend's normalized range to stage coordinates using the current geometry
Mouse
Relative movement accumulated into a cursor clamped to the stage. Left and right buttons. Wheel
Keyboard
Via the keyboard callback. Key codes go through the table in docs/keymap.md, written in M3 from flash-keycodes.md and the header's key list
Pad, cursor mode
Left stick and D-pad move the cursor. South button is click. Speed from the cursor-speed option
Pad, keys mode
D-pad sends arrow keys, south button sends space, east sends enter, start sends enter, others unmapped. Both modes are listed in docs/keymap.md
Content loading. The frontend hands over bytes. If a path is also provided, store its directory as the base for relative loads. Without the VFS interface, relative loads fail with a logged error.
Core options. All keys start with swfr_:
Key
Values
Default
swfr_scale
1x, 2x, 3x, 4x
1x
swfr_quality
low, medium, high (anti-aliasing and gradient step count)
medium
swfr_pad_mode
cursor, keys
cursor
swfr_cursor_speed
1 to 10
5
swfr_script_timeout
1, 2, 5, 15 seconds
2
swfr_trace_to_log
on, off
on
Logging. trace() output goes to the log interface at info level, prefixed [trace]. Diagnostics use warn and error.
Panic safety. A panic must never cross the C boundary. Every exported function wraps its body in catch_unwind. After a panic the core is marked dead: retro_run then shows the last good frame (or a solid dark red frame if none) with silent audio, and logs the panic message once. Never set panic = "abort" in any profile.
Global state. One static holding a Mutex around the core state and stored callbacks. Callback pointers sit inside that state. Exactly one unsafe module, ffi, contains every raw pointer, every extern "C" item and every cast. Everything it calls is safe Rust. Each unsafe block carries a comment saying why it is sound.
Core info file. swfr_libretro.info, built from docs/reference/example.info: display name, authors, supported extensions swf, core name, categories, license, display version, no-content support false, save states false. Field names come from the example file, not from memory.
2.6 Workspace and crate interfaces
Dependency direction, one way only: reader then swf then scene then render and avm1 and audio then player then libretro and harness.
Crate
Responsibility
Key public items
swfr-reader
Bounds-checked bit and byte reading
Reader<'a> with u8, u16, u32, i16, i32, ubits(n), sbits(n), fbits(n), align(), bytes(n), string(), rect(), matrix(), cxform(). Every method returns Result<_, ParseError>. ParseError carries the byte offset and a kind
swfr-swf
Header, decompression, tags
Movie { header, tags }, Header { version, compression, frame_size, frame_rate, frame_count }, Tag enum with an Unknown { code, raw } variant, Limits
swfr-scene
Library, display list, timeline
Library, DisplayList, MovieClip, Stage, event queue
swfr-render
Shapes to pixels
Renderer::new(w, h, quality), render(&Stage, &mut Frame), Frame { width, height, xrgb: Vec<u32> }, dirty rectangles
swfr-avm1
AS1 and AS2 interpreter
Vm, Value, Heap, Vm::run(budget)
swfr-audio
Decoders and mixer
Mixer::mix(out: &mut [i16], frames), sound decoders
swfr-player
Frame loop, no platform code
see below
swfr-libretro
C ABI layer
ffi, state, options, input_map, convert
swfr-harness
Development CLI
swfr-harness run file.swf --frames N --out dir [--input script] writes PNG frames, a WAV file and the trace log
swfr-testkit
Test helpers
PNG compare with tolerance, meta file loader, corpus walker
The player interface is the contract between the engine and the libretro layer:
Player::new(swf_bytes: &[u8], base_dir: Option<String>, opts: &Options) -> Result<Player, PlayerError>
Player::av_info(&self) -> AvInfo            // width, height, fps, sample_rate
Player::run_frame(&mut self, input: &Input) -> FrameOut<'_>
Player::reset(&mut self)

FrameOut { video: Option<&Frame>,  // None means unchanged since last frame
           audio: &[i16],          // interleaved stereo, exactly the frame's sample count
           logs: &[LogLine] }
Input { pointer: Option<(f32, f32)>, buttons: u8, wheel: i32, keys: &[KeyEvent] }
2.7 Rendering
• Pipeline: shape records become Path objects with fill and line styles. These are drawn with the rasterizer into a pixmap, then converted to XRGB for the frontend.
• Fills: solid, linear gradient, radial gradient, focal gradient, bitmap (clipped or repeating, smoothed or not). Spread and interpolation modes per the spec.
• Strokes: caps, joins and miter limit per the spec. A minimum hairline width keeps thin lines visible.
• Morph shapes: interpolate every record by the ratio before drawing.
• Masks: a layer with a clip depth is drawn into a temporary pixmap, then the mask's alpha is applied. Budget for the memory: temporary pixmaps are pooled and never allocated inside the frame loop.
• Text: static text uses glyph outlines from the font tags. Device fonts use the bundled font file. Rich dynamic text beyond basic edit text is M3 stretch work.
• Caches: rasterized shapes are cached by character id, scale and color transform. Evict least-recently-used when over a byte budget (default 8 MB).
• Dirty rectangles: each frame, union the bounds of every display object whose state changed. Repaint only that region. If it exceeds half the stage, repaint everything. If it is empty, report the frame as unchanged.
• Allocation: one bump arena for tessellation buffers and transforms, reset at the end of each frame. Pools for display objects. No growth of any Vec inside the frame loop after warm-up.
• Quality option: low turns anti-aliasing off and halves gradient steps. High raises both.
2.8 Audio engine
• Output is 44,100 Hz, stereo, signed 16-bit.
• Supported sound formats: uncompressed PCM, ADPCM, MP3. Others are logged and played as silence.
• Input sample rates 5,512, 11,025, 22,050 and 44,100 Hz are resampled by linear interpolation.
• Event sounds follow the start-sound flags in the spec: stop, no-multiple, loop count, in and out points, envelope.
• Streaming sounds follow the timeline. Frame n's stream block plays during frame n. The core never skips frames because the frontend calls retro_run once per frame.
• The mixer sums into i32, applies volume and pan, then clamps to 16-bit.
• The output buffer is allocated once at load, sized for the largest per-frame sample count at the lowest allowed frame rate.
2.9 Script virtual machines
AVM1 (AS1 and AS2, SWF 8 and below):
• Values: undefined, null, boolean, number (f64), string, object handle, movie clip handle. Coercions follow docs/reference/ecma-262-3.txt.
• Action byte blocks are decoded once, on first use, into a Vec<Op> where Op is an enum. Dispatch is a match over that vector. There is no JIT and no raw-byte dispatch.
• The watchdog counts instructions and checks the wall clock every 4,096 of them (standard Instant only). Past the script-timeout option, the current script aborts with a logged error and the movie keeps playing.
• Built-ins, in priority order: trace, _root, _level0, _global, Object, Function, Array, String, Number, Boolean, Math, Date (basic), the MovieClip methods, Key, Mouse, Stage (basic), TextField (basic), Sound (M5). Not built: XML, LoadVars, LocalConnection, anything networked.
• Timers (setInterval and friends) run on the frame clock: elapsed time is frames / fps, never the wall clock, so tests are deterministic.
• Math.random uses a seeded xorshift generator. The seed is a core option and defaults to a fixed value in tests.
Heap and collector (shared by AVM1 and AVM2): a generational-index arena. A handle is { index: u32, generation: u32 }. No Rc cycles. A tracing mark-and-sweep collector runs in slices, triggered by allocation volume, and stops when its time slice is spent. Target: no single slice over 2 ms on Reference B. Roots are the stage, globals, the operand stack and pending timers.
AVM2 (AS3): M7, own TSD. Until then swfr-avm2 is an empty crate. Do not scaffold it.
2.10 Timeline semantics
One Player::run_frame equals one SWF frame. Order within it:
1. Apply input: update pointer state, fire button transitions, mouse and key events to clips and listeners.
2. For every running movie clip, advance the playhead and process that frame's tags (place, remove, background, start sound, frame label).
3. Run that frame's DoAction blocks. Order: the root clip first, then children in creation order. The spec does not pin this down completely, so M3 writes trace-order test SWFs, decides, and records the result in DECISIONS.md.
4. Fire onEnterFrame handlers.
5. Render, then mix audio.
Buttons have four states (up, over, down, hit). Hit-testing uses a point-in-path winding test over the flattened hit-state shapes, implemented in swfr-scene. The rasterizer is not used for hit-testing.
2.11 Safety and limits
Every limit lives in one Limits struct with the defaults below. Exceeding a limit at parse time returns an error from load. Exceeding it at run time aborts the offending script or frame work and logs. Neither may panic.
Limit
Default
Input file size
64 MiB
Decompressed size
64 MiB
Tag count
2,000,000
Edges in one shape
1,000,000
Movie clip nesting depth
256
AVM call depth
256
Bitmap side length
8,192 px
Total bitmap memory
64 MiB
String length
16 MiB
Array length
1,048,576
Script time per frame
option, default 2 s
Script heap, desktop
128 MiB
Script heap, Reference B
48 MiB
Other rules: forbid(unsafe_code) everywhere except swfr-libretro::ffi; no unwrap, expect, indexing or panic! in non-test code (the lints enforce this); every as cast that can truncate is replaced by a checked conversion.
2.12 Milestones
Work one milestone at a time. Each lists tasks, tests and exit criteria. A milestone ends only when every exit criterion holds and STATUS.md says so.
M0: Bring-up with a stub player
• Tasks:
    1. Confirm the bootstrap CI is green on main. If not, fix the workflow first. Changes under .github/ wait for the owner's review.
    2. Read docs/reference/libretro.h and write docs/notes/libretro-abi.md: every struct, constant and callback signature used in 2.5, each with its line number in the header.
    3. Write the FFI types and constants in ffi. Add a test that scans the header text for each #define it uses and compares the value with the Rust constant, so no value rests on memory.
    4. Export every function in the 2.5 table. The stub Player returns a 320x240 color-bar frame and a 440 Hz tone.
    5. Write the mock frontend in crates/swfr-libretro/tests/abi.rs: Rust extern "C" callbacks that record video, audio and input calls.
    6. Write swfr_libretro.info and the Linux and Windows jobs of cross.yml (2.19).
    7. Make swfr-harness print the core's AvInfo.
    8. Open the V1 human-test issue: which CI artifact to load and what to report.
• Tests: the ABI test drives load, run, unload, reload and deinit through the exported symbols. It asserts: poll called once per run; retro_load_game with null returns false; pitch is at least width times 4; per-run audio frame counts follow the fractional carry rule (over 600 runs at 12 fps the total is exactly 2,205,000 frames); no panic crosses the boundary when a fault is injected (a fault-inject cargo feature). The header test passes.
• Exit: every required CI job is green; ABI and header tests pass; CI has built the Linux and Windows cores; you run V1.
M1: Container and tags
• Tasks:
    1. swfr-reader with all methods in 2.6 and tests for bit alignment edge cases.
    2. Header parse; decompression for uncompressed, zlib and LZMA files with Limits. The LZMA header details come from the LZMA specification and the SWF spec notes.
    3. Tag record parsing, short and long forms.
    4. Decoders for every tag in the spec's tag chapters. Shapes decode to records, fonts to glyph lists, sounds to format plus raw bytes, action blocks stay raw. Unknown tags are kept as Unknown. Each decoder cites its note in docs/notes/.
    5. swfr-harness dump file.swf prints the tag list.
    6. The SWF builder in swfr-testkit (1.6): it emits a header, tags and bit-packed records. Write one movie per tag type, plus malformed variants.
    7. The built-in mutator (2.13).
• Tests: for every tag type, the builder's movie parses and the result equals what the builder was given (a round trip); every malformed variant returns an error and does not panic; 100,000 mutator iterations per pull request, one million nightly, with no panic.
• Exit: all of the above green in CI; a lints step that fails on any unsafe outside ffi; header and tag layouts cross-checked against swf.ksy and noted in DECISIONS.md.
M2: Shapes and rendering
• Tasks:
    1. Minimal scene: build the display list for the first frame.
    2. Rendering pipeline per 2.7: fills, gradients, strokes, bitmaps (lossless and JPEG variants with alpha), morphs, masks, color transforms.
    3. Shape cache, pooled pixmaps, bump arena, dirty rectangles. Test dirty rectangles with a synthetic scene.
    4. XRGB conversion and wiring into swfr-libretro: the first frame of a loaded SWF appears on screen.
    5. Harness frame dump: --frames writes PNGs.
    6. Baseline and NEON builds measured on Reference B (you run them).
• Tests: hand-derived exact pixels for simple shapes; gradient endpoints within a tolerance of 2 per channel; golden comparison per 2.13.
• Exit: corpus/shapes/ goldens approved by you; a static file from corpus/real/ shows correctly in RetroArch; render time per frame reported for A and B.
M3: Timeline, input and libretro wiring
• Tasks:
    1. Timeline engine, frame labels, nested clips, place and remove with ratio and clip depth.
    2. Buttons: four states and hit-testing.
    3. Input mapping per 2.5, with docs/keymap.md.
    4. Core options, including live scale changes through the geometry call.
    5. Static text from font outlines; basic dynamic text with the bundled font.
    6. retro_reset.
    7. Timeline-ordering test SWFs and the decision recorded.
• Tests: frame-counter test over 600 frames; scripted input playback in both the harness and the ABI test; option changes at runtime.
• Exit: corpus/timeline/ goldens approved; frame-accurate pacing; Reference A and B budgets met; you run V2 (play five real files with touch, mouse and pad).
M4: AVM1
• Tasks: decoder and interpreter; heap and collector; built-ins in the 2.9 order; clip events and listeners; timers on the frame clock; watchdog; a conformance mini-suite written from the ES3 text.
• Tests: every corpus/as12/ program's log matches its .expect.log exactly; the mutator runs over script-heavy files with no panic; collector pause measured; dispatch micro-benchmark recorded in DECISIONS.md.
• Exit: all of the above; budgets met; you run V3 (three AS2 games).
M5: Audio
• Tasks: PCM, ADPCM and MP3 decoding, resampler, mixer, event sounds with flags, streaming sounds synced to the timeline, Sound object basics, per-frame sample accounting.
• Tests: deterministic mixes compared by hash against stored WAVs you approve; a marker-pulse test proving stream sync within one frame; silence frames counted correctly.
• Exit: corpus/audio/ approved; Reference B CPU cost within budget; you run V4 (listening test).
M6: Packaging and platform verification
• Tasks: finish release.yml for every platform; write up the baseline versus NEON decision; produce memory reports; tune low-memory behavior for Reference B; write install notes; collect licenses into the release packages; open every remaining human-test issue.
• Exit: using the release artifacts, you run the corpus through RetroArch on Windows 10, Linux, macOS and Android 5 (Reference B) and report in the issues. The iOS result is recorded as a pass or a documented limitation. Budgets are met on A and B.
M7: AVM2. Own TSD. Starts only after M1 to M6 are done and the AVM1 benchmark shows headroom on Reference B. The verifier comes first.
2.13 Test strategy
Layer
Where
What it proves
Unit
#[cfg(test)] in each crate
Local correctness
Corpus
swfr-testkit walks corpus/**/*.meta.toml
Parsing, logs, frames
Golden
candidates from scripts/goldens.sh
Rendering regressions
ABI
swfr-libretro/tests/abi.rs
The core obeys the libretro contract
Mutation
swfr-mutate in fuzz/
No panics on hostile input
Benchmarks
swfr-harness bench using Instant
Budgets
Human
HUMAN_TESTS.md
Real frontends, real devices
Goldens. The AI never writes into any golden/ directory.
1. The goldens CI job renders every tracked frame by running scripts/goldens.sh and compares it with corpus/**/golden/.
2. Missing or differing frames are uploaded as candidates with diff images. The ci-report comment lists their names and the pull request gets the goldens-review label.
3. You open the artifact, look at the images, and add the goldens-approved label. The approve-goldens workflow then commits the candidates into golden/. The AI cannot approve its own goldens.
4. Tests compare with a per-channel tolerance of 2 and at most 1% of pixels outside it. Failures write a diff image.
5. Loosening a tolerance needs your approval through an issue.
Mutator. A seeded xorshift generator with these mutations: bit flip, byte replace, truncate, duplicate a chunk, corrupt a tag length, set a count field to its maximum. Each mutated input goes through Movie::parse and 30 frames of Player inside catch_unwind. A panic, or a limit violation that was not reported as an error, saves the input as corpus/malformed/found-<hash>.swf in a pull request the job opens. If the nightly cargo-fuzz job works on the runners, it also runs for an hour.
Determinism. Nothing in the player reads the wall clock except the watchdog. Random numbers are seeded. The same input always yields the same frames.
2.14 Budgets
Metric
Reference A
Reference B
retro_run time
under 80% of the frame period
under 80% of the frame period
Frame rate, M3 corpus
60 fps at 800x600
30 fps at 480x320
Load time (retro_load_game)
under 300 ms
under 800 ms
Core memory, idle
under 32 MB
under 32 MB
Core library size
under 6 MB
under 6 MB
Audio underruns
0
0
Missing a budget blocks the milestone. Cut the feature, never the budget.
2.15 Human verification
The AI opens a GitHub issue labeled human-test for each test. It says which CI artifact to download, what to load, what to do and what to report. You answer in the issue with pass or fail, device, RetroArch version and a note, then close it.
ID
After
You do
V0
Before M0
Section 1.8: RetroArch on each floor device, answer in docs/platform-notes.md
V1
M0
Download the Linux and Windows 10 cores from the CI artifacts and load them in RetroArch. Color bars and a tone appear
V2
M3
Play five real files with touch, mouse and pad
V3
M4
Play three AS2 games for five minutes each
V4
M5
Listening test on the audio movies
V5
M6
Full corpus pass on Windows 10, Linux, macOS and Android 5, using the release artifacts
V6
M6
Reference A and B benchmark run
V7
M6
iOS static library attempt
2.16 Rules for the implementing AI
1. Read AGENTS.md, then STATUS.md, then only the current milestone's section. Read 2.19 when you touch CI. Do not read ahead.
2. Work on a branch named mN/<slug> and open one pull request per task. Merge only when the required checks are green and no protected path is touched. Protected-path changes wait for the owner.
3. CI is your compiler. Do not assume a local toolchain. Never try to download files in a shell (curl, wget, cargo fetch, rustup, pip). Anything that must be downloaded is a workflow job or a references.lock row.
4. Research by reading web pages. Write what you learn into docs/notes/<topic>.md in your own words, with the source URL and section. Do not paste long passages from copyrighted documents. Cite those notes in code comments.
5. Prefer primary sources. For libretro, docs/reference/libretro.h beats any web page. If it is missing, add a references.lock row, open a needs-reference issue and stop that task. Never copy a constant from a web page into code.
6. Walk the ladder before writing code: does it need to exist for this milestone, does std do it, does an existing workspace crate do it, does an allowed crate do it, can it be fewer lines. Only then write the minimum, with a test.
7. Use only crates in deps-allow.txt. To add one, edit the root Cargo.toml in a pull request, expect the allowlist check to fail until the owner updates the file, and pick a version at least 14 days old.
8. unsafe exists only in swfr-libretro::ffi, each block commented.
9. Write tests with each task. Never merge red. Never delete or weaken a test, and never loosen a tolerance or budget without the owner's approval in an issue.
10. Never edit docs/reference/, any golden/ directory or Cargo.lock by hand.
11. If CI fails the same way three times, open an issue labeled blocked with what you tried, and stop that task.
12. When the spec is ambiguous, take the simpler reading and log it in DECISIONS.md.
13. Never add secrets, and never change a workflow in a way that widens its permissions without the owner's review.
14. A milestone ends when its exit criteria hold. Update STATUS.md, open the human-test issues, then stop and wait.
2.17 Deliverables per milestone
Source, tests, notes in docs/notes/, a DECISIONS.md entry, an updated THIRD_PARTY.md if anything changed, new human-test issues, an updated STATUS.md, and a green CI run on the milestone's final commit, with the release artifacts attached to a draft release.
2.18 Risks and unverified items
1. Frontend support on old systems. Whether current RetroArch builds run on Android 5 and on your Windows 10 machines is unverified until V0. If not, the floor changes.
2. Slow feedback loop. The AI cannot compile locally, so every change costs a CI round trip. Small pull requests, cheap checks first and caching are the mitigation.
3. What the AI can see. The design assumes it can read workflow runs, pull request comments and web pages. If it cannot, the loop breaks. Confirm in 1.9.
4. Hosted runners are newer than your floors. CI cannot show the core runs on Windows 10 or Android 5, or meets budgets on Reference A and B. V5 and V6 do.
5. Reference redistribution. Adobe documents carry copyright notices. Do not commit them to a public repo until you have checked.
6. iOS. I believe cores must link statically into the app, and that a Rust static library can do this. Unverified until V7.
7. Web. Needs an Emscripten build of the core. Deferred beyond v1.
8. Libretro details. Exact environment command names, pointer coordinate ranges and core info fields must be confirmed against the header, the docs and the example file. Anything not found there is dropped.
9. Frame-script execution order. Not fully pinned down by the spec. Decided in M3 by test.
10. No reference renderer. Goldens are approved by eye, so subtle differences from the original player will exist.
11. Label-driven approvals. The golden approval workflow writes to branches. It must check who added the label and must never run code from the pull request.
12. Real-world test files. Each needs a license you have reviewed.
13. Spec gaps past SWF 19. Best effort.
14. ARMv7 speed without NEON. Unknown until the M2 measurement on Reference B.
15. AVM2. Still the largest single cost.
16. Default limits in 2.11 and budgets in 2.14 are starting values. Adjust them with measured data and log the change.
2.19 CI and release pipeline
Workflows live in .github/workflows/. The bootstrap ci.yml from 1.3 grows into the set below. Workflow files are a protected path and need the owner's review. Every third-party action is pinned by commit SHA.
Workflow
Trigger
What it does
ci.yml
Pull request, push to main
lints: format, clippy with warnings denied, cargo deny, the direct-dependency allowlist check, the unsafe-outside-ffi check, the manifest license check. build-test: unit and corpus tests on Linux, Windows and macOS (Linux required). abi: the mock frontend tests on all three. mutate: 100,000 mutator iterations. bench: harness benchmarks on Linux x86_64 against bench/baseline.json. Writes the ci-report comment
cross.yml
Pull request, push to main
cross-build: release builds of the core for Linux x86 and ARMv7 (ARMv7 tests under user-mode emulation), Android ARMv7 baseline, ARMv7 NEON and AArch64 (NDK, API 21 wrappers), Windows x86 and x86_64, macOS arm64 and x86_64, iOS AArch64 (build only). Checks the core size against the budget. Uploads each library with its .info file
goldens.yml
Pull request
Compares rendered frames with golden/. On differences or missing frames, uploads candidates and diffs, lists them in the report and adds goldens-review
approve-goldens.yml
Label goldens-approved added
Copies the candidate images from the latest run into golden/ on the branch, as a bot. It checks that the owner added the label and never runs code from the pull request branch
lockfile.yml
Push to a branch that changes a Cargo.toml
Runs cargo fetch and commits Cargo.lock if it changed
fetch-references.yml
Manual
Reads references.lock, downloads, checks hashes, converts PDFs to text, commits rows marked commit = true into docs/reference/, and opens a pull request
corpus-real.yml
Pull request touching the manifest, nightly
Downloads the manifest files into the cache, verifies hashes, runs them through swfr-harness and reports
nightly.yml
Schedule
A longer mutator run, cargo-fuzz on a nightly toolchain if it works on the runners, an advisory check, and benchmark history
release.yml
Tag v*, manual
Builds every target in release mode, packages one zip per platform with the library, .info, README and licenses, and attaches them to a draft release
Reports. Each job writes a plain-text summary. On pull requests one sticky comment named ci-report lists each job's result, the names of failed tests, the first 40 lines of each failure, library sizes and benchmark numbers. The AI reads this comment, not raw logs.
Permissions. Jobs default to read-only. lockfile, approve-goldens, fetch-references and the report comment each ask for contents: write or pull-requests: write individually.
Speed. Cache the cargo registry and build output with a pinned caching action. Cheap jobs run first and the matrix fails fast.
What CI cannot prove. Hosted runners are newer than your floors. CI shows that the core builds and passes tests. It does not show that the core runs on Windows 10 or Android 5, or that it meets the budgets on Reference A and B. Benchmarks on shared runners are noisy, so bench only flags regressions above 30% against the baseline. You check absolute budgets in V5 and V6.
Possible later, not required: running RetroArch under an Android 5 emulator in CI. I have not verified that this works on hosted runners.
Part 3: Autonomous completion directive (for the AI)
This part overrides Part 2 wherever they conflict. In particular it replaces the stop-and-wait rule (2.16 rule 14), the per-pull-request owner gating (2.16 rule 2) and every exit criterion that needs a person. Everything else in Parts 1 and 2 still applies.
Mission: keep working, milestone after milestone, without waiting for the owner, until the core is engineering-complete and built by CI for every target in 2.4. Do not ask for permission. Do not stop at milestone boundaries.
3.1 Definition of done
The run is complete when all of these hold on the head of dev, with evidence in the final report (3.12):
1. Every required CI job is green.
2. M1 to M6 are implemented as specified in 2.12, with the substitutions in 3.6.
3. cross.yml builds the core for every target in 2.4. iOS is build-only.
4. release.yml produces a draft release with one zip per platform, each holding the library, .info, README and licenses.
5. Every tag in SWF spec v19 either has a builder round-trip test or is listed in docs/notes/tag-coverage.md with the reason it is unsupported.
6. The AS1 and AS2 conformance suite (M4) has at least 150 generated movies with expected logs, all passing.
7. Audio tests (M5) and rendering tests (M2, M3) pass against approved or provisional goldens (3.7).
8. The release core is under 6 MB and bench shows no regression above 30%.
9. README.md, docs/INSTALL.md and docs/KNOWN_LIMITATIONS.md exist and match the code.
10. human-test issues V2 to V7 are open with their artifacts. They do not block completion.
Not required for done: AVM2 and AS3 (M7), the web build, iOS packaging beyond a successful build, measurements on Reference A and B, anything that needs a person.
3.2 Before the run (owner)
1. Run the export check from the M0 review (nm -D --defined-only libswfr_libretro.so | grep -c ' T retro_' should print 25) and merge PR #1. V1 can run in parallel and does not block the run.
2. Create the branch dev from main. Protect it: require the status checks lints, build-test, abi and cross-build, no code owner review, and let the AI's account merge.
3. Leave main protected as it is. You merge dev into main when you choose. CODEOWNERS applies then.
4. Put this document into docs/TSD.md and add Part 3 to AGENTS.md.
5. Check that the AI's account can push branches, open and merge pull requests into dev, create issues and labels, read CI comments, and trigger workflows. Public repositories have free Actions minutes, but each run still takes real time.
3.3 Branches and merging
• Integration branch: dev. Task branches mN/<slug> merge into dev by pull request. Squash merge when the required checks are green. You may merge your own pull requests.
• Keep pull requests small, ideally under 800 changed lines, and one per task.
• Tag the head of dev as mN-complete when milestone N is done.
• You may edit .github/workflows/ and the root Cargo.toml on dev. Never widen a workflow's permissions beyond what 2.19 lists.
• You may not edit: docs/reference/ by hand, any approved golden/ directory, Cargo.lock by hand, deps-allow.txt, deny.toml, branch protection, or secrets. If you need a crate that is not allowed, open an issue labeled needs-reference with the reason and continue with other work.
• You may add rows to references.lock and corpus/real/MANIFEST.toml. A manifest row needs license evidence, or the check rejects it. The owner reviews both when merging dev into main.
• A bot commit made with the default token does not trigger CI. After one, push a follow-up commit so the required checks run on the real head.
3.4 Work loop
Repeat until 3.1 holds:
1. Read STATUS.md, the open pull requests, open issues and the milestone section you are on.
2. Pick the next task. Write a failing test first when practical.
3. Branch, implement, push, read the ci-report comment, fix, merge.
4. Update STATUS.md and WORKLOG.md (3.9).
5. When a milestone's criteria hold, run the audit (3.10), tag it, open its human-test issues, and start the next milestone in the same session.
When something is ambiguous, decide, record it in DECISIONS.md, and keep going. Do not wait for an answer. If a question truly needs the owner, open an issue labeled question that states the default you are proceeding with.
3.5 Task 0: M0 cleanup (do this first)
1. Add a cross-build step that lists the shared library's exported retro_* symbols and fails unless all 25 in the 2.5 table are present.
2. Pin every action by commit SHA, not tag.
3. Fix the ci-report table so it renders, and include every job.
4. Apply the human-test label to issue #2 and fix its checklist formatting.
5. Add native windows-latest and macos-latest jobs for build-test and abi, as 2.19 specifies. Keep the cross-compiled Windows core, add a native MSVC build, and ship the native one in releases if both pass.
6. Mark docs/platform-notes.md as unverified unless the owner wrote it. Do not invent V0 results.
7. Tell the owner in a question issue if anything above cannot be done.
3.6 Milestones M1 to M6 with substitutions
Build each milestone as written in 2.12. Where an exit criterion needs a person or a device, use the substitute below. The milestone still counts as done.
Needs
Substitute
Owner approves goldens
Provisional goldens (3.7)
Owner runs a V test
Open the human-test issue with the artifact link and continue
Budget on Reference A or B
Core size under 6 MB, bench regression under 30%, and a per-frame cost report in ci-report
Owner reviews a license
Add the row with license evidence and flag it in the final report
Extra requirements for autonomous work:
• M1: produce docs/notes/tag-coverage.md, one row per tag code in the spec with status and test name.
• M2: cross.yml also builds ARMv7 baseline and NEON cores and records their sizes. Shape cache and dirty rectangles are required.
• M3: ARMv7 ABI tests run under user-mode emulation in cross.yml. Scripted input tests cover pointer, mouse, keyboard and both pad modes.
• M4: the conformance movies are generated by the builder and assembler. Derive each expected log from ES3 semantics in your notes, and record the reasoning in a comment so a wrong expectation can be found later. The dispatch benchmark goes in DECISIONS.md.
• M5: audio expectations are hashes of mixed output, provisional until approved. Include the marker-pulse stream sync test.
• M6: finish release.yml, README.md, docs/INSTALL.md and docs/KNOWN_LIMITATIONS.md. Run the release workflow against dev and fix whatever breaks.
3.7 Provisional goldens
The owner cannot approve goldens while you work, so rendering tests use two tiers:
• golden/ holds approved images. Tests compare against it first.
• golden-provisional/ holds images the goldens workflow generated and committed. Tests compare against it only when no approved image exists.
Rules:
• A provisional image may be created only for a frame whose test carries a one-line written expectation (for example: red square centered on white). The workflow also runs sanity checks: not all one color unless declared, plausible variance, correct dimensions.
• If a render changes on purpose, you may regenerate the provisional image in the same pull request and explain why.
• GOLDENS.md is generated and lists approved and provisional counts. The ci-report shows both.
• The owner promotes provisional images with the goldens-approved label later. Until then, every report says how many frames are provisional.
3.8 Blockers
• Never stop on one blocker. Open an issue labeled blocked with what you tried, then work on the next unblocked task.
• If the same CI failure survives three different fixes, open a blocked issue, move on, and come back after finishing other work.
• If a reference is missing, add a references.lock row, open a needs-reference issue, and implement what you can from the notes you already have.
• If web access to the SWF spec or any reference fails, retry with alternate sources and record which source you used.
3.9 Checkpointing and resume
Your session can end at any time. Keep state in the repository.
• STATUS.md holds: the current milestone and task, branch, open pull requests, the last green commit on dev, and the next three steps. Update it after every merged pull request.
• WORKLOG.md is append-only: one short entry per merged pull request.
• On every start, read STATUS.md, list open pull requests, branches and issues, and continue from there. Never restart a milestone that is tagged complete.
3.10 Milestone audit
Before tagging mN-complete, write docs/audits/mN.md. For each exit criterion in 2.12 and each substitute in 3.6, give the evidence: a CI run link, test names or file paths. Be honest. List criteria that are unmet or only provisional, say why, and continue anyway when the gap needs a person.
3.11 Limits that still apply
These do not relax in autonomous mode:
• unsafe only in swfr-libretro::ffi, each block commented.
• Only crates in deps-allow.txt. No secrets.
• Never delete or weaken a test, a lint or a budget to get green. A wrong expectation may be corrected with the reasoning recorded.
• Never rewrite history or force-push.
• Never claim a human test passed. Only the owner's reply in the issue counts.
• Do not copy long passages from copyrighted documents into the repository.
3.12 Stop conditions and final report
Stop only when one of these is true:
• 3.1 holds.
• Every remaining task is blocked on something only the owner can supply.
• CI itself is broken and three different repairs have failed. Write BLOCKED.md explaining it.
Then open an issue titled v1 engineering-complete report with: the 3.1 checklist with evidence links, the number of provisional goldens, open human-test issues, open blocked and needs-reference issues, known limitations, license rows awaiting review, and the draft release link. Set STATUS.md to match. Stop.
3.13 Out of scope for this run
AVM2 and AS3 (M7), the web build, iOS packaging beyond a successful build, networking, video codecs, save states, and anything in the 2.3 non-goals. If you finish early, you may write docs/TSD-M7.md as a plan. Do not start implementing it.
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
Part 6: M7, ActionScript 3 (AVM2) (for the AI)
This part is the own TSD that 2.12 promised for M7. It does not start until the owner posts m7-go on a GitHub issue. Until then, the core detects AS3 movies and fails them with a clear message (see the real-file triage instructions). Parts 2 to 5 still apply, and so do the Part 3 working rules (branches, small pull requests, audits, final report). Part 3.14's end-to-end rule applies to every stage here.
6.1 Goal and non-goals
Goal: run typical AS3 games and animations from the Flash 9 to 11 era: Sprite and MovieClip based, with events, timers, simple drawing, bitmaps, text, sound, XML and data classes. Success is measured by tests that load real AS3 movies through the exported retro_* symbols.
Non-goals: Stage3D and Molehill, AIR APIs, 3D transforms, NetStream video, camera and microphone, sockets, ExternalInterface, LocalConnection, native extensions, Pixel Bender shaders, the advanced text engine (flash.text.engine), printing, accessibility, and a JIT.
6.2 Preconditions
1. The 3.14 gates pass and real AS2 files play.
2. corpus-ext for AVM1 has a recorded baseline.
3. The collector from the Part 5 memo is in place: allocation-volume triggers, a defined root set, bounded slices.
4. The owner has posted m7-go.
6.3 Sources and legal rules
• Primary spec: the AVM2 Overview, read from the web. It carries a copyright notice, so do not copy it into the repository. Take notes in your own words.
• Behavior of the Flash API: Adobe's ActionScript 3 reference pages, read from the web.
• Do not read Tamarin or avmplus source (MPL, GPL, LGPL). Do not copy code from any player.
• Ruffle's AVM2 can be studied under the Part 5 rules. Add work packages R14 to R17: ABC and verifier mechanics, class and object model, display and event model, and the built-in library approach. Notes go in docs/notes/ruffle/ and docs/notes/avm2/.
• Apache Flex SDK's compiler (asc.jar) runs in CI only, pinned by version and hash, and is never committed.
6.4 Architecture
• swfr-heap (new, extracted first). The generational-handle heap and the incremental collector now shared by AVM1 and AVM2. Task 0 of S1 is the extraction, with no behavior change and all AVM1 tests still passing.
• swfr-avm2: modules abc (parser), verify, interp, object (traits, slots, vtables, prototypes), names (namespaces and multinames), native (registry of Rust natives) and lib (the core classes).
• swfr-flash (new): the Flash API classes that bridge AS3 objects to swfr-scene: display, events, geom, text, media, utils. Dependency direction: avm2 knows nothing about the scene. flash depends on both.
• Execution: after verification, each method body is decoded once into a typed Vec<Op> and run by a match loop. No JIT. Strings are interned so equality is cheap. Record the representation in DECISIONS.md.
• The verifier is mandatory. Nothing runs before it passes. A movie whose ABC fails verification fails to load with a readable error.
• Class library strategy: write thin AS3 classes in as3lib/ (clean-room, no copied Ruffle or Adobe source), with the hot core types (Array, String, Math, Vector, Date, RegExp) as Rust natives. CI compiles as3lib/ with asc.jar into playerglobal.abc. Because the AI cannot build locally, the compiled file is committed by a bot pull request from an as3lib-build workflow, and an as3lib-check job fails if the committed file differs from a fresh build. Verify in S1 how asc.jar handles native methods. If it cannot, use ordinary stubs that call one intrinsic, __native(id, args).
• Linking movies to classes: handle FileAttributes (the AS3 flag), DoABC and DoABC2 (including lazy initialization), SymbolClass, the document class, and the frame-script convention where a timeline's compiled class registers methods per frame. Study this first in S0.
6.5 Stages
Each stage ends with its tests green in CI, an audit in docs/audits/m7-sN.md that names the proving test for each criterion, and a tag.
Stage
Content
Acceptance
S0
Study: R14 to R17 notes, a list of AS3 APIs by priority from the corpus, docs/notes/avm2/*.md
Notes merged with commit SHAs and evidence. Priority list ranks APIs by how many corpus movies use them
S1
swfr-heap extraction. ABC parser, verifier and interpreter core. An ABC assembler in swfr-testkit
Movies built by the assembler run arithmetic, control flow, calls, closures, try and catch, classes, interfaces, getters and setters, and prototypes. trace works through a minimal native. Malformed ABC is rejected without a panic. Mutator extended to ABC for 100,000 iterations
S2
Core classes in the library: Object, Class, Function, Array, String, Number, int, uint, Boolean, Math, Date, RegExp, the error classes, Vector, Dictionary, Namespace, QName, JSON, ByteArray, and E4X XML and XMLList
One test file per class, built from AS3 sources compiled in CI with expected output labeled verified or derived
S3
flash.events with capture, target and bubble phases. flash.utils: Timer, getTimer, setTimeout, setInterval on the frame clock
Tests for dispatch order, stopPropagation, listener priority and weak references, timers driven by frames
S4
flash.display and flash.geom including the Graphics drawing API, Stage, Sprite, MovieClip, Shape, Bitmap, BitmapData basics, Loader for embedded content, and SymbolClass binding with the AS3 frame model (enter frame, construct, frame constructed, frame scripts, exit frame, added and removed from stage)
An end-to-end test: an AS3 movie draws with Graphics, moves a sprite on every frame, and updates on a MouseEvent. Pixel checks. BitmapData has a memory cap
S5
flash.text (basic TextField, TextFormat), flash.media (Sound, SoundChannel, SoundTransform), flash.ui, keyboard and mouse events
End-to-end tests with exact audio samples and text output
S6
Real-file loop: an AS3 subset of corpus-ext (permissive directories only, never committed), plus a missing-API report: at load and unload, the core lists every unimplemented class or method the movie touched, with counts, in the log and on the HUD
A baseline pass count. Failure clusters ranked by frequency and fixed in that order
S7
Performance and memory
Budgets in 6.7 measured on Reference A, with a report from CI for Reference B
6.6 Limits and safety
• All new limits go into Limits: total ABC bytes (default 8 MiB), methods and classes per movie, constant pool sizes, locals per method (65,535), scope depth, operand stack depth, call depth (default 512), Vector and Array lengths, ByteArray size (64 MiB), BitmapData pixels (16 million), and the script timeout option from 2.9.
• A limit violation logs and aborts the script or the load. It never panics.
• No unsafe outside swfr-libretro::ffi. Fuzz the ABC parser and verifier with the mutator.
• A native method that is missing logs once, returns the type's default value and does not crash. The missing-API report counts it.
6.7 Budgets for M7
These amend 2.14 for AS3 content. Measure and log growth per stage in ci-report.
• Core library: at most 8 MB. The owner approves anything above.
• Idle memory with an AS3 movie loaded: under 48 MB on Reference A.
• Frame rate: the M3 corpus budgets still hold. Simple AS3 games target 30 fps on Reference B. If that is not reachable, document the limit in docs/KNOWN_LIMITATIONS.md.
6.8 Ground truth
• Mark every test verified or derived, as in 4.4.
• Compile AS3 test sources with asc.jar in CI. Expected output comes from the owner's debug Flash Player run where available, or from a permissive third-party corpus.
• A directory of an external corpus is used only if its license is permissive. Exclude any directory derived from GPL, LGPL or MPL sources.
6.9 Human tests and completion
• Human tests: V8 (three AS3 games), V9 (an AS3 movie with sound and text), V10 (a performance run on Reference B).
• M7 is complete when S0 to S7 are done and audited, the 6.5 end-to-end tests pass, release.yml builds all targets with the AS3 library, and the final report is posted in an issue titled m7-complete report with evidence, the missing-API ranking and the budget numbers.
6.10 Risks and unverified items
1. The scope is very large. E4X, BitmapData and the frame model are the likeliest to run over.
2. asc.jar may not install cleanly on every hosted runner, and I have not confirmed how it handles native methods.
3. AS3 content is heavier than AS2, so slow devices may only run simple games.
4. The recursion and time limits above are starting values, not measurements of Flash Player.
5. License status of Ruffle's AVM2 test directories has not been checked.
6. The verifier rules must come from the AVM2 Overview, not from memory.
Part 7: Direction decisions and priorities (for the AI)
The owner has chosen the goal and the engine. This part records those decisions and sets the order of work. It overrides earlier parts where they conflict.
7.1 Decisions
ID
Decision
Reason
D1
The goal is to run on all the owner's devices, including Android 5.0 (API 21) on ARMv7 and AArch64, plus Windows 10, Linux, macOS, and an iOS build
Owner's choice
D2
The engine is the clean-room Rust core. The original Adobe Flash Player is not bundled and is not hosted as a plugin
Its binaries are x86 desktop only, it cannot be stepped one frame per retro_run, its audio cannot be captured per frame, and recent builds contain the kill switch. None of that serves Android 5
D3
This is a personal fork and will not be distributed
Copyleft obligations generally attach when you distribute, but the rules in 2.3 and Part 4 stay in force so the code remains shareable later and no copyleft code is mixed in. Any Adobe binary the owner uses is the owner's responsibility
D4
Ruffle study outcome: Option 1
Learn the mechanics, write your own. The swf crate stays a dev-dependency test oracle only
D5
The real Flash Player on the owner's PC may be used as a test oracle
Optional. The owner may capture flashlog.txt and screenshots to turn derived expectations into verified ones (4.4)
D6
AS3 (Part 6) starts only after the owner posts m7-go
It is the largest single piece of work
7.2 What working on Android 5 means
The Android work is done when, on a real Android 5 device (Reference B) with RetroArch:
1. The core loads and lists .swf files.
2. The three generated sample movies and at least five real AS2 files play with correct pictures.
3. Touch input works through RetroArch's pointer and overlay, and a pad can drive the cursor.
4. Simple movies hold 30 fps at 480x320, and idle memory stays under 32 MB.
5. A 30 minute soak shows no crash and no growth in memory.
Anything short of this is recorded with numbers in docs/KNOWN_LIMITATIONS.md.
7.3 Android engineering items
1. Build. cross.yml builds armv7-linux-androideabi, thumbv7neon-linux-androideabi and aarch64-linux-android with the API 21 clang wrappers. Add a step that lists the library's undefined symbols and fails if any is newer than API 21.
2. Page size. Newer Android versions can use 16 KB memory pages, and a library linked for 4 KB pages may fail to load there. Link the Android builds with 16 KB maximum page size. Verify the exact linker flag in the NDK documentation.
3. NEON. Ship both ARMv7 builds. The default target has NEON off and the thumbv7neon target has it on. Document in docs/ANDROID.md how to tell which one a device needs (the neon flag in /proc/cpuinfo). Never detect it at runtime.
4. Memory profile. Add the core option swfr_memory_profile (low or normal). The ARMv7 builds default to low: smaller caches, the 48 MiB script heap cap from 2.11, and a smaller maximum render scale.
5. Content. The frontend passes the file bytes at load. Relative loadMovie needs the frontend's file access, so it can fail on Android. Log that clearly and list it in the known limitations.
6. Input. Touch maps to the pointer. Write docs/ANDROID.md with a recommended RetroArch overlay and pad mapping, including cursor mode and keys mode.
7. Logging. trace() and the compatibility report go through the libretro log, which appears in adb logcat on Android. Say so in docs/ANDROID.md.
8. Testing. CI cannot run Android devices. Every milestone from now on produces an Android artifact and a human-test issue for it. Running RetroArch under an API 21 emulator in CI is optional and unverified.
7.4 Order of work
1. P0. Finish 3.14 and the real-file triage items: AS3 detection with a clear message, the preloader contract, delivering a real first frame, the compatibility report, and the three sample movies as CI artifacts.
2. P1. Android smoke test: the three sample movies play on ARMv7 and AArch64. Write docs/ANDROID.md. Open the Android human-test issue and wait for the owner's result before moving on, doing unblocked work in the meantime.
3. P2. The corpus-ext AVM1 baseline and failure clusters, from the Part 5 memo decision. The collector changes from 6.2.
4. P3. Performance on Reference B, with the baseline versus NEON decision recorded.
5. P4. M7 (Part 6) after m7-go.
7.5 Reporting
After P1 and after each later priority, post a short report in the matching issue: what passed, what failed, numbers from CI, and what the owner must test next.
Part 8: swfbox, the real Flash Player inside DOSBox Pure (for the AI and the owner)
The owner wants a way like DOSBox Pure: inside RetroArch, one core for all devices, easy to use, running the original Flash Player. DOSBox Pure already contains the hard part, so this part needs no emulator code to start. It replaces the earlier experiment E1 and does not replace swfr. It does not block Parts 3 to 7.
8.1 What DOSBox Pure already does
• It can install an operating system onto a hard disk image kept in the frontend's save area. Afterwards, loading any content, for example a ZIP, offers an option to run the installed operating system. The image boots as C: and the loaded content becomes D:.
• An option makes the operating system start automatically with the content, skipping the start menu.
• Ready-made Windows 98 images can be placed in RetroArch's system folder and then appear under the run-installed-OS option.
• Core options control the disk: modifications to C: can be kept, discarded, or saved as a difference per content, and the free space on D: is selectable.
• Save states work while running an installed operating system.
• It is a libretro core with binaries for Windows, Linux, Raspberry Pi (ARMv7) and RetroArch's core downloader. Android file access through the Storage Access Framework is supported from v1.0 preview 7.
• Windows 95 and 98 support is labeled experimental.
8.2 Design
• Core: stock DOSBox Pure, unmodified at first.
• Guest: a Windows 98 SE image supplied by the owner (the owner's license), with:
    ◦ a Flash projector installed: Flash Player 9 for AS3 and Flash 9 era content, and optionally a Flash 8 projector as the lighter choice for AS1 and AS2 content;
    ◦ working sound drivers, 640x480 or 800x600 at 16-bit color;
    ◦ a startup batch file C:\FLASH\RUN.BAT in the Windows Startup folder that finds the first *.SWF on D: and runs the projector on it, full screen if possible;
    ◦ boot logos, sounds, screensaver, power management and disk checks turned off.
• Content: each game is a ZIP containing one .swf. The loaded ZIP becomes D:. Name the ZIPs so RetroArch playlist scanning picks them up.
• Disk safety: set OS disk modifications to discard, or to save the difference per content, so Windows never corrupts the base image. Set the OS to start automatically.
• Boot time: cold boot every time at first. Later, evaluate a save state taken at the idle projector screen.
• Speed levers per device class: emulated CPU preset, Flash 8 instead of Flash 9, lower resolution and color depth, low Flash quality, and a trimmed Windows image.
• Input: pointer or touch to the emulated mouse, pad to keys, using DOSBox Pure's controller mapping and RetroArch overlays.
8.3 Optional patch, only if needed
Accept a raw .swf as content by treating it as a one-file D: drive. This means forking DOSBox Pure, which is GPL-2.0 and acceptable for a personal fork. A workflow vendors the source at a pinned commit by pull request, and CI builds it for every target with the existing cross.yml. DOSBox Pure needs only a C++ compiler to build. Decide on this after the decision gate in 8.6.
8.4 AI tasks (documentation and scripts first, no emulator code)
1. Research the DOSBox Pure documentation and forum threads and write docs/swfbox/GUIDE.md: installing Windows 98 SE under DOSBox Pure, drivers, installing the Flash projector, setting up the auto-run, the exact core option names, where images live, how content ZIPs map to D:, and known problems. Cite every source and mark what is unverified.
2. Write docs/swfbox/RUN.BAT using only batch features that Windows 98 supports, with a variant that waits until D: is ready. Check what the projector accepts on its command line, and how to start it full screen, and record the answers.
3. Write a script that wraps each .swf in a ZIP with a standard name: tools/make-swfbox-zips.sh and a PowerShell version.
4. Generate three benchmark movies with the SWF builder and publish them as CI artifacts: bench-raster.swf (30 fps, 550x400, a fixed number of moving shapes and an AS2 frame counter on screen), bench-audio.swf (a PCM tone with a visual pulse) and bench-scripts.swf (an AS2 loop that reports getTimer time on screen).
5. Create docs/swfbox/RESULTS.md as a results template with a device matrix.
6. Do not commit any Windows or Flash binaries. Do not modify DOSBox Pure.
8.5 Owner tasks
• Supply your own bootable Windows 98 SE image and the Flash projector copies, and prepare the image following the guide.
• Run the benchmarks, one real AS2 game and one real AS3 game from the Flash 9 era, on each device including Reference B.
• Record per device: the emulated CPU setting, measured frame rate against the declared 30 fps, audio glitches, input feel, boot time from pressing Load Content to a visible game, and the number of button presses needed. Post it in the swfbox issue.
8.6 Decision gate
• Good enough: a game holds at least 80% of its declared frame rate at 480x320 on Reference B, audio is clean, and the game appears within 90 seconds and 3 button presses. Then make swfbox a supported path for Flash 9 era content, and decide on the optional patch in 8.3.
• PC only: it works on a PC but not on Reference B. Keep it as a PC fallback and test oracle.
• Poor everywhere: drop it, and keep the real player only as an oracle.
8.7 Limits
• Flash Player 10 is not available for Windows 98 or ME, and Flash 9 is the last version for them. Content that needs Flash 10 or later will not run.
• The cost of emulating a Pentium class PC on an ARMv7 device is unmeasured. This is the main risk.
• Windows 98 and Flash Player are the owner's licensed copies for personal use. Nothing is distributed.
• DOSBox Pure is GPL, so keep any changes to it separate from the swfr code.
Part 9: winebox, BoxedWine as a libretro core (for the AI and the owner)
The owner wants the equivalent of DOSBox, but for Wine, inside RetroArch. The closest existing project is BoxedWine. This part plans a libretro port of it as a separate core, winebox, that plays .swf files with the real Flash Player projector. It is separate from swfr and from Part 8. It does not block Parts 3 to 7.
9.1 What BoxedWine is
• An emulator that runs Windows applications by running a 32-bit Wine and emulating the Linux kernel and the CPU. It is written in C++ with SDL. It is released under GPL-2.0.
• It runs 16-bit and 32-bit Windows programs, supports several Wine versions, up to Wine 11, and uses Tiny Core Linux as its base file system.
• Tested platforms listed in its README: Web (Emscripten), Linux x86-64 and ARM64, macOS on ARM, Windows x86-64 and ARM64. Older READMEs also list Raspberry Pi 4 in 32-bit and 64-bit. Android is not in the tested list I found.
• CPU emulation has two kinds. The normal core is an interpreter that works on every architecture. The JIT core is much faster but exists only for x86, x64 and ARMv8 (ARM64).
• One of its published figures: on a Raspberry Pi 4 in 32-bit mode a GDI benchmark scored 14, against 133 on 64-bit ARM with the JIT.
• On first run it downloads the Wine and Linux file system. A libretro core cannot do that at runtime.
• A libretro forum thread suggests wrapping its main loop in libretro functions, so that retro_run produces a frame of video and audio. I found no existing libretro port.
9.2 Fit
Need
BoxedWine
Runs the real Flash projector, up to Flash 32
Yes in principle: a 32-bit Windows program under Wine. Test it. Use a projector from before June 2020, the update that added the kill switch
Works inside RetroArch
Needs a port: replace its SDL video, audio and input with libretro
Modern ARM64 and x86-64 devices
Has a JIT, so it should be reasonable
Android 5 ARMv7
No JIT for ARMv7, so the slow interpreter. Likely too slow, unmeasured. Android itself is untested upstream
Light
No. A Linux file system plus Wine is hundreds of megabytes, and CPU cost is high
9.3 Design
• Source: a workflow vendors BoxedWine at a pinned commit into winebox/third_party/boxedwine/ by pull request. Keep it separate from the swfr crates. It is GPL-2.0, and this is a personal fork.
• Libretro layer: replace the SDL video, audio and input backends. The emulator runs on its own thread or threads. retro_run copies the newest frame into the frontend's XRGB buffer, delivers exactly the audio frames due (silence on underrun), and queues input events. retro_unload_game stops the threads cleanly.
• Content: a .swf file is placed in a temporary host folder that the guest sees as a drive. Wine starts the Flash projector on it. A core option chooses the projector.
• File system: a Wine and Tiny Core file system archive placed in system/winebox/ by the owner or by a workflow. It is unpacked once into the saves directory. Check its hash on every start. No downloads at runtime.
• Core options: CPU core (normal or JIT where available), emulated memory, screen size, Wine version, projector choice.
• Info file: winebox_libretro.info with the extension swf.
9.4 Stages
Stage
Work
Done when
W0
Research: read BoxedWine's docs and source layout, how SDL is used, its command line, how a program is started, which Wine components a Flash projector needs, memory needs, and the interpreter speed data. Write docs/winebox/NOTES.md with sources
Notes merged, unverified items marked
W1
CI builds unmodified BoxedWine for Linux x64, Windows x64 and Android ARMv7 and ARM64 (NDK). Run a Flash projector benchmark on Linux in CI if a headless mode works
Build artifacts from every target. A CI log of the headless run, or a documented reason it could not run
W2
The libretro port with the mock-frontend ABI tests (frames, audio counts, input, clean unload) and a sample .swf
The ABI tests pass. The core plays a sample .swf on Linux x64
W3
Packaging: the file system archive layout, first-run unpack, the integrity check, .info, install notes
Fresh install works from the notes
W4
Device benchmarks by the owner with the three benchmark SWFs from Part 8
Results in the winebox issue
9.5 Owner tasks
• Supply a Flash projector from before the kill switch and a Wine file system archive, and put them where the notes say.
• Test on a modern ARM64 phone or tablet, a Windows PC and the Android 5 device. Record frame rate, audio, input feel, memory use, start time and battery.
9.6 Decision gate
• Worth keeping: it holds at least 80% of the declared frame rate on the benchmarks on a modern device, and starts a game within about 90 seconds.
• ARMv7: report the result with numbers. If it is unusable there, state that winebox supports only devices with a JIT, and that swfr covers the old ones.
9.7 Limits and risks
1. No ARMv7 JIT, so old Android devices are probably out.
2. Android is not an upstream-tested platform.
3. The emulator's threads and the libretro single-thread model need careful handoff.
4. The file system is large and cannot be downloaded at runtime.
5. Older BoxedWine READMEs say that games after 2000 had limited success. Newer versions are faster. Flash under Wine is the thing to test first.
6. GPL-2.0: keep it separate from swfr.
7. The real projector is the owner's responsibility, and nothing is distributed.
9.8 Source access and the licensing firewall
• Authorization. The owner authorizes reading, modifying and building BoxedWine's GPL-2.0 source for the winebox work. For that work only, this overrides the rule against reading copyleft source in 2.3 and Part 4.
• What to read. Read what matters for the port, not every one of its roughly 5,000 files: platform/sdl (the window, events, OpenGL and audio), the platform interfaces it implements, the kernel scheduler, the CPU core selection, the docs folder including docs/CPUemulation.md, the file system root and ZIP options, the command line handling and the build files. List in docs/winebox/NOTES.md which files you read and which you did not.
• Firewall. All BoxedWine-derived code stays under winebox/, with its own build and tests. Never copy code from it into any swfr crate, and never the other way. No swfr crate may depend on it. When you work on swfr, do not consult BoxedWine source. Use separate branches named winebox/<slug>.
• License note. This is a personal fork and nothing is distributed. GPL-2.0 obligations attach when you distribute. If the owner ever shares the core, the whole winebox core must be offered under GPL-2.0 with its source. Record this in winebox/LICENSE-NOTE.md.
• Upstream status. Issue 17 in the BoxedWine repository, which asks about a libretro core, is still open. The author said in 2021 that it should be possible because the SDL layer in platform/sdl is well abstracted, and that there were no plans. No libretro port exists upstream.
9.9 Shape of the first prototype
• A separate experimental core that launches one user-supplied Win32 program (the Flash projector, given a .swf) from a packaged Wine and root file system.
• A virtual writable drive, and software-rendered output only. OpenGL, Vulkan and applications with several windows are a later step.
• The libretro constraints decide the design: all work is surfaced through retro_run, input is polled, and every guest window reaches the frontend as a single video output. The SDL backend currently owns window creation, event waits and GL or Vulkan windows, so the backend is replaced or adapted, not just recompiled as a library.
• Booting Windows XP is out of scope. BoxedWine runs Windows applications under Wine. It does not emulate a whole PC.
Part 10: No license policy (for the AI)
The owner has decided that this is a personal fork. It is not distributed, and license compliance is not a goal. Everything in Parts 1 to 9 that exists only for license reasons is void. This part overrides every earlier statement about licenses, including 2.3, 2.16, 3.11, Part 4, Part 5, 6.3, 7.1 (D3) and 9.8. Technical rules stay: budgets, tests, the unsafe policy, the dependency allowlist, security advisories, and the end-to-end gates in 3.14.
10.1 Rules that no longer apply
• The clean-room rule, including the ban on reading, porting or translating other players' source code, and the ban on reading copyleft code (GPL, LGPL, AGPL, MPL).
• The firewall between swfr and winebox. Code may move between them in either direction.
• The legal ground rules in 4.2 and every never-use-it entry in 4.1, including swf-parser, Tamarin and avmplus, Gnash and Lightspark.
• The own-words requirement and the no-copying rule for notes and for Ruffle study material, the docs/PORTED.md ledger, and the notice-integrity check.
• License checks in CI: the license allowlist in deny.toml (keep the security advisories check), the license-evidence requirement and its CI check for rows in corpus/real/MANIFEST.toml, THIRD_PARTY.md as a license list, and license notes in references.lock.
• License files and sections: LICENSE-MIT, LICENSE-APACHE, THIRD_PARTY_LICENSES.md, license text in README.md, and license files in release zips.
• Checking the license or origin of test files before using them. Test directories derived from GPL or MPL sources, such as from_gnash and from_avmplus, may be used.
10.2 What is allowed now
• Read any source code, in any license: Ruffle, Gnash, Lightspark, swfdec, gameswf, Tamarin, BoxedWine, DOSBox Pure and others.
• Copy, port or vendor code from any of them into the repository, whole modules or whole crates. Reference documents and test files from any source may be committed.
• Choosing how much to copy is a technical decision. Judge it by size, memory, speed, correctness, the unsafe policy and test results. When you copy code, add a one-line comment saying where it came from, only so that you can find and update it later.
• If ported code conflicts with a technical rule, for example unsafe outside ffi, open an issue that explains the conflict and proposes an exception. The owner decides.
10.3 One practical item that is not about licenses
The repository is currently public. A public repository is publication, and publishing other people's proprietary material, such as Adobe documents, Flash Player binaries or Windows images, invites takedown requests. The owner should make the repository private. Until then, keep proprietary binaries and disk images out of the repository, as 8.4 and 9.5 already say.
10.4 Actions
1. Edit AGENTS.md and docs/TSD.md in the repository: remove or mark void every license-driven rule listed in 10.1.
2. Remove the license checks from CI as listed in 10.1. Keep the advisories check and keep CI green.
3. Delete the license files and license text listed in 10.1.
4. Redo the R13 decision memo on technical merits only, since license was a reason for some of its verdicts. The earlier technical objections still stand: ruffle_render pulls in wgpu, gc-arena changes every type with lifetimes, and ruffle_wstr uses unsafe pointer tricks. Decide which components to port from Ruffle, such as the swf crate, shape handling, and AVM1 mechanics, to reach correctness fastest within the budgets. Post the updated memo in the ruffle-study-complete issue.
5. Continue with Part 7's order of work.
