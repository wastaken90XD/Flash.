# Part 4: Prior Art and Research Notes (for the AI)

Researched on 14 October 2026 from public pages. This is not legal advice. Saved as `docs/notes/prior-art.md` per TSD 4.7.1. Every rule in Parts 2 and 3 still applies, including 2.3: do not translate any other player's source code.

## How to use this part

1. Read documentation, changelogs, wikis and issue discussions to learn what behavior matters.
2. Do not read copyleft source code and then reimplement it. That includes GPL, LGPL, AGPL and MPL projects.
3. Do not port code from any player, even a permissive one. Learn the behavior, then write your own.
4. External tools run in CI only, with pinned versions and hashes. Never vendor them into the repository.
5. If a prior-art page changed a decision, say so in `DECISIONS.md`.

---

## 4.1 Projects

| Project | License | State | How to use it |
| :--- | :--- | :--- | :--- |
| **Gnash** | GPL-3.0 | Mirror last pushed 2017 | Cautionary tale only. It descends from gameswf and covers AVM1 up to about SWF 7, with some SWF 8 classes. Do not read its source |
| **Lightspark** | LGPL-3.0 and GPL-3.0 | Active, release 0.9.0 in February 2025 | Its release notes are a checklist of hard areas (4.6). Do not read its source |
| **Shumway** | Apache-2.0 | Archived July 2019 | JavaScript, AVM1 and AVM2. Read its test setup and documentation. Not portable, do not port |
| **swfdec** | LGPL-2.1 or later | Dormant, mirror at `open-flash/swfdec` | Handled Flash 3, 4 and many Flash 7 files. Its `test/` folder organizes automated tests by area. Layout idea only |
| **gameswf** | Public domain | Maintained on SourceForge (`tu-testbed`) | The only code that is free to copy, but 2.3 still says write your own. Useful as a reference for a small, portable renderer boundary: its rendering module is factored out so other graphics APIs can replace it |
| **Tamarin and avmplus** | MPL, GPL and LGPL tri-license | Discontinued, repository `adobe/avmplus` | AVM2 reference for M7 only. The license makes copying into an MIT or Apache project awkward. Use the AVM2 overview instead |
| **Autodesk Scaleform** | Proprietary | No longer sold | Do not use |
| **Ruffle** | MIT OR Apache-2.0 | Active | The owner excluded it as a base and as a dependency. Its public test corpus and contributor docs are useful as data (4.4) |
| **ruffle_libretro (JesseTG)** | Not checked | Experimental, Rust, 184 commits, updated 2023 | Proves a Flash libretro core is feasible. Read its public README and issues for libretro pitfalls only. My fetch of its README returned nothing useful |
| **open-flash swf-types** | MIT OR Apache-2.0 | Version 0.14, 2022 | Type definitions only. Not needed |
| **open-flash swf-parser** | AGPL-3.0-or-later | Version 0.14, 2022 | Never use it and never copy from it |
| **Ruffle's `swf` crate** | MIT OR Apache-2.0 | Active | Reads and writes SWF. Possible test-only oracle (4.5), needs owner approval |
| **FFDec (JPEXS)** | App GPL-3.0, library LGPL-3.0 | Active | Java decompiler and editor. Run in CI as a tool (4.5) |
| **swfmill** | GPL | Maintained | XML to SWF and back, as a tool |
| **flasm** | BSD | Old | Assembler and disassembler for AVM1 bytecode, Flash 8 and earlier. Tool and syntax reference |
| **Ming** | LGPL | Old | Library for writing SWFs. Gnash's early tests used its compiler |
| **Apache Flex SDK** | Apache-2.0 | Maintained | `mxmlc` and the AS3 compiler, for M7 test movies |

---

## 4.2 Legal ground rules

- `swfr` is MIT or Apache-2.0. Anything copied from GPL, LGPL, AGPL or MPL code would change that. Do not do it.
- Running a GPL tool and using its output as test input does not make `swfr` derivative. Keep the tool outside the repository.
- Test files from another project are data under that project's license. Record the license and source for each in the manifest, and never commit them.
- If unsure, do not use it. Open a question issue and continue.

---

## 4.3 What the predecessors teach

- **The version treadmill kills projects.** Gnash was still implementing SWF 7 while SWF 8 spread, and never got AVM2. Lightspark started AS3-first, fell back to Gnash for AVM1, and only added direct AVM1 support much later. `swfr` does the reverse: AVM1 first, with the version targeting in 2.9 and 3.13.
- **Late-found gaps are the usual cost.** Lightspark's release notes list `DefineScalingGrid`, an erase blend mode, ADPCM fixes, morph shapes, Nellymoser and even `PlaceObject` and `RemoveObject` for SWF 1 and 2 as recent additions. Plan tests for the boring old cases, not only the common ones.
- **Test infrastructure decided who survived.** Shumway ran three suites on every change: AVM1 trace tests, AVM2 acceptance tests and screenshot reference tests. Gnash began with per-class existence tests built with Ming. The model to copy is trace-based tests plus image tests, as in 2.13.
- **Native SWF code is a security surface.** A Mozilla engineer argued that processing SWF in C or C++ carries the same security and device-compatibility problems as the original player. Your answer is the limits in 2.11, `forbid(unsafe_code)` and mutation fuzzing.
- **Keep the renderer behind a small boundary.** gameswf's design made porting to other graphics APIs possible. Your boundary is `swfr-render` producing an XRGB frame.

---

## 4.4 Ground truth for expected results

The M4 rule in 3.6 has you derive expected logs by reasoning about ES3. That can encode your own misunderstanding. Real Flash Player output is the only trustworthy source.

- **How others obtain it.** Ruffle's contributor guide says its tests compare `trace()` output against output recorded from the official debug Flash Player. To capture it, create `mm.cfg` with `ErrorReportingEnable=1` and `TraceOutputFileEnable=1`, run the movie, and read `flashlog.txt` from `%APPDATA%/Macromedia/Flash Player/Logs/` on Windows, `~/Library/Preferences/Macromedia/Flash Player/Logs/` on macOS or `$HOME/.macromedia/Flash_Player/Logs/` on Linux. That guide links an archived copy of Adobe's debug player download page. Only the owner can do this.
- **Rule:** every conformance test's meta says `expectation = "verified"` (from real Flash Player output) or `"derived"` (your reasoning). `ci-report` shows both counts. Only verified tests support claims of conformance.
- **Third-party corpus (owner decision).** Ruffle's repository keeps regression SWFs with Flash-Player-derived `output.txt` files under `tests/tests/swfs/avm1` and `avm2`, each with a `test.toml` giving frame counts and image checks. The repository is MIT OR Apache-2.0. If the owner approves, add a `corpus-ext` CI job that sparse-checks-out a pinned commit of the AVM1 directory, runs each movie through `swfr-harness`, compares traces with `output.txt`, and writes pass and fail counts plus a known-failure list to `docs/COMPAT.md`. Nothing is committed. Check the license of each test asset first.
- **Making test movies.** The same guide compiles AS2 with `mtasc -main -header 200:150:30 test.as -swf test.swf` and AS3 with `mxmlc -output test.swf -compiler.debug=true Test.as`. Whether those install on hosted runners is unverified. Your builder, flasm-style assembly and FFDec's XML round trip are alternatives.

---

## 4.5 Oracles and tools in CI (all optional until verified)

| Tool | Use | Notes |
| :--- | :--- | :--- |
| **FFDec command line** | Dump the tag list of any movie and compare it with `swfr-harness dump`. Convert SWF to XML and back. Compress and decompress to check container handling | Needs Java on the runner. Pin the release and its hash. Read the exact command names from its `-help` output |
| **Kaitai `swf.ksy`** | Cross-check header and tag layouts | Already in `docs/reference/` |
| **swfmill** | Write test movies as XML | GPL tool, outside the repo |
| **flasm** | Assemble AVM1 test bytecode | BSD |
| **Ruffle's `swf` crate** | Parse your builder's output in tests and compare tag counts | Test-only. Needs the owner to add it to `deps-allow.txt`. Not shipped |

The point of an oracle is to avoid grading your own homework. Use one for every tag decoder you can.

---

## 4.6 Hard-areas checklist

Each item becomes a test or a line in `docs/KNOWN_LIMITATIONS.md`.

1. SWF 1 and 2 files, with `PlaceObject` and `RemoveObject` instead of the later tags.
2. Morph shapes, including mixed fill types.
3. ADPCM at every bit depth. Nellymoser and Speex are out of scope: log and play silence.
4. Bitmap smoothing exists only from SWF 8.
5. `DefineScalingGrid` (nine-slice scaling).
6. Blend modes, including `erase`.
7. Script order: root timeline against child clips, playhead changes made by a script, and clip constructors running before first use. Frame scripts must not run for a frame that was never reached.
8. AVM1 details: local variable scoping, prototype and class chains, cloning movie clips, `goto` by label. Reference counting bugs were a recurring problem in a mature player, which supports the tracing collector in 2.9.
9. Mixed AVM1 and AVM2 files and obfuscated AVM1 bytecode.
10. Event order, from an AS1-era reference. Button, mouse and key handlers run immediately when their event happens. The `load` handler runs in the first frame the clip is on stage, after the parent timeline's code and before the clip's own code and before rendering. The `unload` handler runs in the first frame the clip is gone, before the parent timeline's code. Test every one of these.
11. The AS3-era frame loop (the elastic racetrack): events run first, queued events wait for the next frame, then `enterFrame`, then rendering. A widely cited update for AVM2 says some details were inferred and not confirmed by Adobe, so treat them as hypotheses.
12. Streaming sound sync and the AS `Sound` object.
13. Collector pauses on weak CPUs.
14. Version-gated behavior such as string encoding and case sensitivity by SWF version. Confirm each in your spec notes before coding it. Do not rely on recall.

---

## 4.7 Actions for this run

1. Save this part as `docs/notes/prior-art.md`.
2. Add the 4.6 items to `docs/notes/tag-coverage.md` or the relevant test plans, and write tests for each one that fits M1 to M5.
3. M3: write event-order tests for item 10 and record the result in `DECISIONS.md`.
4. M4: add `expectation = verified` or `derived` to every conformance test's meta, and show the counts in `ci-report`.
5. M1: if Java is available on the runner, add an optional `ffdec-crosscheck` job that compares tag sequences. Report mismatches and do not fail the build until the job has been stable for a while.
6. Open a question issue asking the owner to decide on the `corpus-ext` job and the Ruffle `swf` test oracle. Continue without them.
7. Check the public issues of `ruffle_libretro` for input, audio and timing problems, and add anything relevant to `docs/KNOWN_LIMITATIONS.md`.

---

## 4.8 What I could not verify

- Whether FFDec, Java, `mtasc` or the Flex SDK install cleanly on hosted runners.
- The license of each individual asset in Ruffle's test directories.
- The content and license of `ruffle_libretro`.
- Gnash's state beyond a 2017 mirror.
- Whether Adobe's debug player is still obtainable.
- The pages on frame order are old and one is a third party's inference. Real Flash Player output outranks all of them.
