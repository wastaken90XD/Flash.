# R1 — Workspace & Crate Inventory (`00-inventory.md`)

- **Pinned Release Tag:** `v0.7.1`
- **Pinned Commit SHA:** `89a7049965f3090e706265670df309a5128233ac`
- **Repository:** `https://github.com/ruffle-rs/ruffle`
- **Overall Confidence:** **High** (verified directly from `Cargo.toml`, `Cargo.lock`, `LICENSE.md`, `README.md`, and all 35 crate manifests at `89a7049965f3090e706265670df309a5128233ac`).

---

## 1. Licensing & Legal Posture (`Confidence: High`)

- **Dual License:** The workspace root declares `license = "MIT OR Apache-2.0"` (`Cargo.toml:45` at `89a7049965f3090e706265670df309a5128233ac`) and `LICENSE.md:1-202` provides the full MIT and Apache-2.0 texts (`Copyright (c) 2018-2022 Ruffle LLC <ruffle@ruffle.rs> and Ruffle contributors`).
- **Clean-Room Policy:** Contributions are dual-licensed under MIT/Apache-2.0 (`README.md:187-199`), and third-party crates in `Cargo.lock` use permissive licenses (`MIT`, `Apache-2.0`, `BSD`, `Zlib`, `ISC`, `MPL-2.0` where applicable).

---

## 2. Rust Edition, Minimum Toolchain, and Build Prerequisites (`Confidence: High`)

- **Rust Edition:** `edition = "2024"` (`Cargo.toml:43` at `89a7049965f3090e706265670df309a5128233ac`), with `resolver = "2"` (`Cargo.toml:39`). Rust Edition 2024 requires **`rustc >= 1.85.0`** (`README.md:45` specifies the latest stable Rust channel).
- **No Explicit `rust-version` Field:** Neither the root `Cargo.toml` (`Cargo.toml:41-47`) nor any member `Cargo.toml` sets `rust-version`; the effective MSRV is governed by `edition = "2024"` (`1.85+`) and `wgpu = "30.0.1"` / `naga = "30.0.1"` (`Cargo.toml:55-56`).
- **Build-Time Java Prerequisite (`core/build.rs`):**
  - `ruffle_core` runs a custom build script (`core/build.rs:1-23`) that invokes `build_playerglobal::build_avm1_playerglobal` and `build_playerglobal::build_avm2_playerglobal` (`core/build_playerglobal/src/lib.rs:1-120`).
  - While AVM1 intrinsic/builtin classes (`core/src/avm1/globals/*.as`) are compiled by `rascal = "0.3.5"` (`Cargo.toml:105`, a pure-Rust AS2 compiler), AVM2 builtin classes (`core/src/avm2/globals/*.as`) are compiled into `playerglobal_import.abc` by `tools/asc` (`tools/asc/src/lib.rs:1-76`), which spawns `java` on `PATH` (`README.md:46`).
  - Furthermore, `core/build.rs:4` hardcodes `let repo_root = Path::new("../");`, assuming `core/` is built inside the Ruffle repository layout where `../core/src/avm1/globals/` and `../core/src/avm2/globals/` exist.

---

## 3. Complete Crate Inventory (`Confidence: High`)

At commit `89a7049965f3090e706265670df309a5128233ac`, the repository contains **35 Cargo packages** (`31` listed in `[workspace].members` at `Cargo.toml:2-37`, plus `core/common`, `web/common`, `render/naga-pixelbender` pulled via relative `path = ...` dependencies, and `fuzz` as an excluded standalone workspace):

| Path | Package Name | Version | `.rs` Files | Rust LOC | `crates.io` Status | Primary Role |
|---|---|---|---:|---:|---|---|
| `core` | `ruffle_core` | `0.7.1` | 423 | 162,449 | **Unpublished** (uses git + unversioned path deps) | Main emulator (`Player`, AVM1, AVM2, display list, audio mixer, HTML/text layout) |
| `core/common` | `ruffle_common` | `0.7.1` | 4 | 645 | **Unpublished** (path-only) | Shared XML, buffer, and avm string utilities (`core/common/Cargo.toml:1-25`) |
| `core/macros` | `ruffle_macros` | `0.7.1` | 1 | 577 | **Unpublished** (path-only) | Proc-macros (`#[enum_trait_object]`, intrinsic helpers; `core/macros/src/lib.rs:1-577`) |
| `core/build_playerglobal` | `build_playerglobal` | `0.7.1` | 5 | 967 | **Unpublished** (path-only) | Build-time compiler for AVM1 (`rascal`) and AVM2 (`asc` + Java) builtin classes |
| `swf` | `swf` | `0.3.0` | 33 | 18,553 | **Published (`0.3.0`)** | Standalone SWF & AVM1/AVM2 bytecode reader/writer (`swf/Cargo.toml:1-31`) |
| `wstr` | `ruffle_wstr` | `0.7.1` | 10 | 5,062 | **Published (`0.1.0` old)** | Flash UCS-2 / Latin-1 string types (`WStr`, `WString`; `wstr/Cargo.toml:1-20`) |
| `flv` | `flv-rs` | `0.7.1` | 8 | 1,661 | **Unpublished** (`0.7.1`) | Flash Video (`.flv`) container demuxer (`flv/Cargo.toml:1-15`) |
| `render` | `ruffle_render` | `0.7.1` | 22 | 7,559 | **Unpublished** (uses git `jpegxr`) | `RenderBackend` trait, `ShapeConverter` tessellation (`lyon`), matrix/transform types |
| `render/wgpu` | `ruffle_render_wgpu` | `0.7.1` | 31 | 11,043 | **Unpublished** (path-only) | GPU renderer using `wgpu = "30.0.1"`, WGSL shaders, filter pipelines (`render/wgpu/Cargo.toml:1-42`) |
| `render/canvas` | `ruffle_render_canvas` | `0.7.1` | 1 | 1,397 | **Unpublished** (path-only) | Web HTML5 2D Canvas renderer (`render/canvas/src/lib.rs:1-1397`) |
| `render/webgl` | `ruffle_render_webgl` | `0.7.1` | 1 | 1,875 | **Unpublished** (path-only) | WebGL2 renderer (`render/webgl/src/lib.rs:1-1875`) |
| `render/naga-agal` | `naga-agal` | `0.7.1` | 4 | 2,378 | **Unpublished** (path-only) | Stage3D AGAL bytecode to `naga` IR translator (`render/naga-agal/Cargo.toml:1-18`) |
| `render/pixel_bender` | `pixel_bender` | `0.7.1` | 5 | 2,314 | **Unpublished** (path-only) | Adobe Pixel Bender shader parser/disassembler (`render/pixel_bender/Cargo.toml:1-22`) |
| `render/naga-pixelbender` | `naga-pixelbender` | `0.7.1` | 2 | 1,042 | **Unpublished** (path-only) | Pixel Bender to `naga` WGSL compiler (`render/naga-pixelbender/Cargo.toml:1-16`) |
| `render/pixel_bender/assembly_tests` | `pixel_bender_assembly_tests` | `0.7.1` | 1 | 175 | **Unpublished** | Pixel Bender roundtrip tests |
| `video` | `ruffle_video` | `0.7.1` | 5 | 239 | **Unpublished** (path-only) | `VideoBackend` trait and frame handle definitions (`video/src/lib.rs:1-120`) |
| `video/software` | `ruffle_video_software` | `0.7.1` | 6 | 813 | **Unpublished** (uses git `h263-rs`, `nihav`) | Pure-Rust H.263, VP6, and ScreenVideo decoders (`video/software/Cargo.toml:1-31`) |
| `video/external` | `ruffle_video_external` | `0.7.1` | 6 | 2,428 | **Unpublished** (path-only) | Cisco OpenH264 dynamic loader & WebCodecs H.264 decoder (`video/external/Cargo.toml:1-39`) |
| `frontend-utils` | `ruffle_frontend_utils` | `0.7.1` | 27 | 5,960 | **Unpublished** (path-only) | Shared desktop/CLI helpers: `cpal` audio, `reqwest` navigator, LSO disk storage, bundle loader |
| `desktop` | `ruffle_desktop` | `0.7.1` | 41 | 11,788 | **Unpublished** (binary app) | Desktop GUI player (`winit`, `egui`, `wgpu`, `cpal`, `gilrs`; `desktop/Cargo.toml:1-84`) |
| `web` | `ruffle_web` | `0.7.1` | 11 | 4,310 | `publish = false` | WebAssembly browser player (`web/Cargo.toml:1-68`) |
| `web/common` | `ruffle_web_common` | `0.7.1` | 2 | 118 | **Unpublished** (path-only) | Shared JS/Wasm error and console helpers |
| `web/packages/extension/safari` | `ruffle_web_safari` | `0.7.1` | 1 | 46 | `publish = false` | Safari extension stub |
| `scanner` | `ruffle_scanner` | `0.7.1` | 8 | 805 | **Unpublished** (binary tool) | Bulk SWF parser/scanner CLI (`scanner/Cargo.toml:1-25`) |
| `exporter` | `exporter` | `0.7.1` | 6 | 608 | **Unpublished** (binary tool) | Headless `wgpu` PNG frame exporter (`exporter/Cargo.toml:1-27`) |
| `exporter/integration_tests` | `exporter_integration_tests` | `0.7.1` | 1 | 294 | **Unpublished** | Integration tests for `exporter` |
| `tests` | `tests` | `0.7.1` | 8 | 413 | **Unpublished** | Main regression test binary (`tests/Cargo.toml:1-31`) |
| `tests/framework` | `ruffle_test_framework` | `0.7.1` | 27 | 3,682 | **Unpublished** | Test runner, `test.toml` parser, mock backends (`tests/framework/Cargo.toml:1-42`) |
| `tests/fs-tests-runner` | `ruffle_fs_tests_runner` | `0.7.1` | 1 | 267 | **Unpublished** | Parallel filesystem test discovery using `libtest-mimic` |
| `tests/input-format` | `ruffle_input_format` | `0.7.1` | 3 | 312 | **Unpublished** | JSON input script deserializer (`input.json`) |
| `tests/socket-format` | `ruffle_socket_format` | `0.7.1` | 1 | 35 | **Unpublished** | Socket replay format for `XMLSocket` / `Socket` tests |
| `tests/mocket` | `mocket` | `0.7.1` | 1 | 107 | **Unpublished** | Mock TCP socket server CLI |
| `tools/asc` | `asc` | `0.7.1` | 1 | 76 | **Unpublished** | Wrapper around Adobe `asc.jar` Java compiler (`tools/asc/src/lib.rs:1-76`) |
| `tools/fuzzer` | `ruffle_fuzzer` | `0.7.1` | 2 | 581 | **Unpublished** | Corpus fuzzer runner |
| `tools/stub-report` | `stub-report` | `0.7.1` | 1 | 43 | **Unpublished** | Generates JSON report of `avm1_stub!` / `avm2_stub!` annotations |
| `fuzz` | `ruffle-fuzz` | `0.0.0` | 2 | 64 | `publish = false` | `libfuzzer-sys` targets (`fuzz/Cargo.toml:1-25`) |

---

## 4. Dependency Graph, Features, and Native/C Dependencies (`Confidence: High`)

### 4.1 Internal Crate Dependency DAG
- **Leaf Crates:** `swf` (depends only on `bitflags`, `bitstream-io`, `encoding_rs`, `num-derive`, `num-traits`, `log`, `flate2`, optional `lzma-rs`, `enum-map`, `simple_asn1`; `swf/Cargo.toml:13-24`), `ruffle_wstr` (zero runtime dependencies; `wstr/Cargo.toml:12`), `flv-rs` (`bitflags`, `thiserror`), `pixel_bender`, and `ruffle_macros`.
- **Mid-Tier Trait Crates:**
  - `ruffle_render` depends on `swf`, `ruffle_wstr`, `pixel_bender`, `naga-agal`, `lyon`, `wgpu`, `image`, `png`, `gif`, `jpeg-decoder`, and optional git `jpegxr` (`render/Cargo.toml:14-42`). Note that `wgpu = "30.0.1"` is a **non-optional** dependency of `ruffle_render` (`render/Cargo.toml:36`) even when `ruffle_render_wgpu` is not selected!
  - `ruffle_video` depends on `swf`, `ruffle_render`, `slotmap`, `thiserror` (`video/Cargo.toml:12-17`).
- **Core Emulator (`ruffle_core`):**
  - Depends on `swf`, `ruffle_wstr`, `ruffle_common`, `ruffle_macros`, `ruffle_render` (with `tessellator` feature), `ruffle_video`, `naga-agal`, `flv-rs`, `gc-arena = "0.7.0"`, `slotmap`, `indexmap`, `hashbrown`, `quick-xml`, `ttf-parser`, `realfft`, `fluent-templates`, `image`, and three git dependencies (`regress`, `flash-lso`, optional `nellymoser-rs` and `jpegxr`) (`core/Cargo.toml:12-67`).
  - **Features on `ruffle_core` (`core/Cargo.toml:75-91`):**
    - `default = []` (minimal build without audio resampling or MP3/AAC/LZMA)
    - `lzma = ["lzma-rs", "swf/lzma"]`
    - `audio = ["dasp"]`
    - `mp3 = ["symphonia", "symphonia/mp3"]`
    - `aac = ["symphonia", "symphonia/aac"]`
    - `nellymoser = ["nellymoser-rs"]`
    - `jpegxr = ["dep:jpegxr", "ruffle_render/jpegxr", "lzma"]`
    - `default_font = []` (embeds Noto Sans fallback font bytes)
    - `default_compatibility_rules = []`
    - `known_stubs = ["linkme", "serde"]`
    - `egui = ["dep:egui", "dep:egui_extras", "png"]`
    - `avm_debug = []`, `tracy_avm = ["dep:tracy-client"]`, `timeline_debug = []`, `serde = ["serde/derive"]`

### 4.2 Total Lockfile Footprint & Native C/`-sys` Dependencies
- `Cargo.lock` resolves **753 crates** across the workspace (`Cargo.lock:1-9850`).
- **C / Native `-sys` crates in `ruffle_core`:**
  - When `ruffle_core` is built with `default-features = false` and `features = ["lzma", "mp3", "audio", "default_font"]`, all code is pure Rust except optional C bindings if `jpegxr` (`jpegxr` wraps a C JPEG-XR library via `cc`; `Cargo.toml:106`) or `tracy-client` (`tracy-client-sys`) or `known_stubs` (`linkme` linker sections) are enabled.
  - However, `ruffle_frontend_utils` and `ruffle_desktop` pull in C/OS system libraries: `alsa-sys`, `libudev-sys`, `freetype-sys`, `yeslogic-fontconfig-sys`, `openssl-sys` / `aws-lc-sys`, `wayland-sys`, `bzip2-sys` (`Cargo.lock`).

---

## 5. Verification Against Regression Tests (`Confidence: High`)

Per TSD 5.2 Rule 7, three inventory claims were verified against Flash-Player-derived regression tests in `tests/tests/swfs/` at `89a7049965f3090e706265670df309a5128233ac`:

1. **Claim 1 — `build_playerglobal` compiles scripted AVM1 classes (`flash.geom.Point` in `core/src/avm1/globals/point.as`) via `rascal` at build time (`core/build.rs:7-8`):**
   - Verified against `tests/tests/swfs/avm1/point/output.txt:1-10`, where `new Point()`, `new Point(1)`, and `new Point(1, 2)` trace `(x=0, y=0)`, `(x=1, y=undefined)`, and `(x=1, y=2)` using the scripted AS2 `Point` class compiled into `playerglobal_avm1`.
2. **Claim 2 — `ruffle_core` uses `flash-lso` (`core/Cargo.toml:40`) to serialize and reload AMF0 `.sol` `SharedObject` files (`core/src/avm1/globals/shared_object.rs`):**
   - Verified against `tests/tests/swfs/avm1/shared_object/RuffleTest.sol`, `output1.txt:1-4` (`No data found. Initializing...`, `size: 0`, `size: 279`), and `output2.txt:1-19` (`saved: true`, `num: 10`, `str: hello`, `array: elem0,undefined,undefined,undefined,elem4`).
3. **Claim 3 — `ruffle_core` uses `regress` (`ruffle-rs/regras3`, `core/Cargo.toml:39`) to implement AS3 `RegExp` flags including `extended` and `dotall`:**
   - Verified against `tests/tests/swfs/avm2/regexp_constr/output.txt:1-9`, which verifies `new RegExp()` property defaults (`dotall false`, `extended false`, `global false`, `ignoreCase false`, `multiline false`).

---

## 6. What Could Not Be Verified (`Confidence: High`)

- Live `crates.io` publication timestamps for historical versions of `ruffle_wstr` (`0.1.0`) were not queried because `Cargo.toml` in `v0.7.1` shows `swf = "0.3.0"` is the only crate maintained with an independent crates.io release version (`swf/Cargo.toml:3`).

---

## 7. Open Questions

1. Can `ruffle_render`'s unconditional `wgpu = "30.0.1"` dependency (`render/Cargo.toml:36`) be feature-gated out if someone only wants `RenderBackend` and `ShapeConverter` (`lyon` tessellation) for a CPU renderer?
2. Would `ruffle_core` build outside the Ruffle monorepo if pulled via `git = "https://github.com/ruffle-rs/ruffle"` given `core/build.rs:4` (`Path::new("../")`) and the Java `asc.jar` requirement? (Investigated in detail in R12 / `11-fit.md`.)

---

## 8. Test Ideas for `swfr`

1. **`flash.geom.Point` & `flash.geom.Rectangle` AS2 builtins:** Add an AVM1 test modeled on `tests/tests/swfs/avm1/point` verifying `new flash.geom.Point(x, y)`, `add`, `subtract`, `distance`, and `toString()` -> `(x=..., y=...)`.
2. **Sparse array properties in `SharedObject`:** Add a test modeled on `tests/tests/swfs/avm1/shared_object/test.as` verifying that sparse array indices (`array[0] = "elem0"; array[4] = "elem4"`) and negative/named keys on arrays survive `SharedObject.getLocal` round-trips.
3. **Strict direct-dependency budget test:** Keep `ci.yml`'s `deps-allow.txt` gate so `swfr` never accidentally pulls in heavy GPU or windowing crates (`wgpu`, `naga`, `egui`) into the core crate graph.
