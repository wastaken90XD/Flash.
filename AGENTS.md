# AGENTS.md — Rules for the Implementing AI (TSD 2.16 & Part 3)

Read this file first, then `STATUS.md`, then only the current milestone's section in `docs/TSD.md`.

## Core Operating Rules (Copy of TSD 2.16)

1. Read `AGENTS.md`, then `STATUS.md`, then only the current milestone's section. Read 2.19 when you touch CI. Do not read ahead.
2. Work on a branch named `mN/<slug>` (or the session's assigned working branch) and open one pull request per task. Merge only when the required checks are green and no protected path is touched. Protected-path changes wait for the owner.
3. CI is your compiler. Do not assume a local toolchain. Never try to download files in a shell (`curl`, `wget`, `cargo fetch`, `rustup`, `pip`). Anything that must be downloaded is a workflow job or a `references.lock` row.
4. Research by reading web pages. Write what you learn into `docs/notes/<topic>.md` in your own words, with the source URL and section. Do not paste long passages from copyrighted documents. Cite those notes in code comments.
5. Prefer primary sources. For libretro, `docs/reference/libretro.h` beats any web page. If it is missing, add a `references.lock` row, open a `needs-reference` issue and stop that task. Never copy a constant from a web page into code.
6. Walk the ladder before writing code: does it need to exist for this milestone, does `std` do it, does an existing workspace crate do it, does an allowed crate do it, can it be fewer lines. Only then write the minimum, with a test.
7. Use only crates in `deps-allow.txt`. To add one, edit the root `Cargo.toml` in a pull request, expect the allowlist check to fail until the owner updates the file, and pick a version at least 14 days old.
8. `unsafe` exists only in `swfr-libretro::ffi`, each block commented.
9. Write tests with each task. Never merge red. Never delete or weaken a test, and never loosen a tolerance or budget without the owner's approval in an issue.
10. Never edit `docs/reference/`, any `golden/` directory or `Cargo.lock` by hand.
11. If CI fails the same way three times, open an issue labeled `blocked` with what you tried, and stop that task.
12. When the spec is ambiguous, take the simpler reading and log it in `DECISIONS.md`.
13. Never add secrets, and never change a workflow in a way that widens its permissions without the owner's review.
14. A milestone ends when its exit criteria hold. Update `STATUS.md`, open the `human-test` issues, then stop and wait.

## Protected Paths (`CODEOWNERS`)

Changes to the following paths require review and approval from `@wastaken90XD`:
- `/docs/reference/`
- `**/golden/`
- `/Cargo.toml`
- `/deny.toml`
- `/deps-allow.txt`
- `/references.lock`
- `/corpus/real/MANIFEST.toml`
- `/.github/`

---

## Part 3: Autonomous completion directive (for the AI)

Where Part 3 conflicts with Part 2, **Part 3 wins** for the duration of this autonomous run.

### 3.1 Definition of done
Do not stop and wait for the owner between tasks or milestones. Continue until **all** of the following hold at the same time:
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

### 3.2 Rules that are relaxed for this run
- **Rule 14 (stop at milestone boundary):** Replaced by 3.8. Do not stop and wait for `human-test` issues V1–V7 to be closed by the owner before starting the next milestone. Open the issue, attach the artifact run link, mark the milestone `engineering-complete (awaiting human verification)` in `STATUS.md`, and continue immediately.
- **Protected-path waits (1.2, 2.16 rule 2):** Pre-authorized for this run to edit `.github/workflows/*.yml`, `Cargo.toml`, `deps-allow.txt`, and `references.lock` **only** within the bounds in 3.3.
- **Golden approval gate (1.6, 2.13):** Replaced by the provisional golden policy in 3.7 so M2 and M3 do not block on human review.

### 3.3 Pre-authorized changes to formerly protected files
- **`Cargo.toml` and `deps-allow.txt`:** Restricted to the pre-approved crate list in 1.4 (`miniz_oxide`, `lzma-rs`, `tiny-skia`, `png`, `zune-jpeg`, `fontdue`, `symphonia` with `mp3` only).
- **`.github/workflows/*.yml`:** Pinned actions by commit SHA, read-only default permissions (`contents: write` only in `lockfile.yml` and `release.yml`, `pull-requests: write` / `issues: write` only in `ci-report`).
- **`references.lock`:** Only specification documents and public-domain/CC0 fonts.
- **`docs/reference/` and `**/golden/`:** Remain strictly read-only to the AI; provisional goldens go in `**/golden-provisional/`.

### 3.4 Rules that remain strictly in force
1. **No `unsafe` outside `crates/swfr-libretro/src/ffi.rs`.**
2. **Never weaken, delete, `#[ignore]`, or `#[allow]` a failing test or lint** to make CI green.
3. **Never fabricate evidence.**
4. **Never download in a shell outside a workflow step.**
5. **Every commit must pass `fmt`, `clippy`, `doc`, `deny`, `build-test`, `abi`, `size-check`, and `cross-build`.**

### 3.5–3.13 Task 0, Milestone Substitutions, Provisional Goldens, Self-Audit, and Heartbeat
- Execute Task 0 (`3.5`), M1–M6 with autonomous substitutions (`3.6`), provisional goldens (`3.7`), milestone self-audits in `docs/audits/mN.md` (`3.8`), failure recovery (`3.9`), `WORKLOG.md` heartbeat (`3.10`), and scope discipline (`3.11`–`3.13`).
