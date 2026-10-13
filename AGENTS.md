# AGENTS.md — Rules for the Implementing AI (TSD 2.16)

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
