# AGENTS.md — Operating Rules for `swfr`

Read this file first before making any changes in the repository.

## 1. Environment & Feedback Loop

1. **CI is the only trusted build.** Every task ends with a commit, push, and wait for a green GitHub Actions run (`lints`, `build-test`, `abi`, `cross-build`).
2. **No local downloads assumed.** Do not rely on `curl`, `wget`, `cargo fetch`, `rustup`, or `pip` in the local shell. Anything that downloads external crates, reference documents, toolchains, or real-world SWF files runs in a GitHub Actions workflow job.
3. **Read text reports.** Every CI job writes a summary to `$GITHUB_STEP_SUMMARY`, and pull requests receive a sticky `ci-report` comment. Read job logs and PR comments via `gh`.
4. **Visual and hardware checks go to the human maintainer:**
   - **Golden image reviews:** Apply the `goldens-review` label and wait for the human maintainer to inspect and add `goldens-approved`.
   - **Hardware / frontend testing:** Open an issue using `.github/ISSUE_TEMPLATE/human-test.md` (`human-test` label).
   - **Missing references or dependencies:** Open an issue using `.github/ISSUE_TEMPLATE/need.md` (`needs-reference` or `blocked` label).

## 2. Rust Code & Safety Rules

1. **Strict `unsafe` boundary:**
   - `unsafe_code = "forbid"` is enforced across the entire workspace EXCEPT `swfr-libretro`.
   - `swfr-libretro` sets `unsafe_code = "deny"` at the crate level and permits `#[allow(unsafe_code)]` ONLY on its `ffi` module (`crates/swfr-libretro/src/lib.rs`).
2. **No panics or unchecked indexing in library code:**
   - Clippy lints `unwrap_used`, `expect_used`, `indexing_slicing`, and `panic` are set to `"deny"`.
   - `missing_docs` and `arithmetic_side_effects` are set to `"warn"` (which fail CI under `-D warnings`). Document all public items and use checked/saturating/wrapping arithmetic as appropriate.
3. **Malformed SWF resilience:**
   - Never panic, abort, or allocate unbounded memory on truncated, corrupt, or adversarial SWF data. Return structured errors or follow documented Flash Player recovery semantics.

## 3. Dependencies & Lockfile Rules

1. **Allowed direct dependencies (`deps-allow.txt`):**
   - All allowed third-party crates are listed in `deps-allow.txt` and `[workspace.dependencies]` in the root `Cargo.toml`.
   - The `lints` CI job verifies that every workspace crate's direct external dependencies are present in `deps-allow.txt`.
   - No external crate is permitted for `libretro`; its C ABI types are written by hand in `swfr-libretro::ffi` and verified against `docs/reference/libretro.h`.
2. **Lockfile maintenance:**
   - Never hand-edit `Cargo.lock`. When `Cargo.toml` changes on a branch, the `lockfile` workflow runs `cargo generate-lockfile` / `cargo fetch` and commits the updated `Cargo.lock`.

## 4. Specifications, Research Notes & Citations

1. **Working notes (`docs/notes/<topic>.md`):**
   - Summarize binary layouts, tag tables, bit-packing rules, and opcode semantics in your own words in `docs/notes/<topic>.md`, citing the source URL and section number.
   - Cite `docs/notes/<topic>.md` in code comments. Never copy long verbatim passages from proprietary specifications.
2. **Reference lockfile (`references.lock`):**
   - You may propose new `[[ref]]` entries in `references.lock` after verifying URLs.
   - Set `commit = false` unless redistribution in a repository is explicitly permitted by the document's license.
   - Only the repository owner runs `fetch-references` and merges changes to `docs/reference/`.

## 5. Protected Paths (`CODEOWNERS`)

Changes to the following paths require review and approval from `@wastaken90XD`:
- `/docs/reference/`
- `**/golden/`
- `/Cargo.toml`
- `/deny.toml`
- `/deps-allow.txt`
- `/references.lock`
- `/corpus/real/MANIFEST.toml`
- `/.github/`
