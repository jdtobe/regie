# Agent Instructions

Regie is a Rust, macOS-first Quake-style drop-down terminal. Read [README.md](README.md) for status and [docs/](docs/) for design, principles, roadmap and open questions.

## Commands
- `cargo run`: run the app (menubar icon)
- `cargo test`: run all tests
- `cargo clippy --all-targets`: lint (must be clean)
- `cargo fmt`: format (must be clean)

## Workflow (TDD)
1. Write a failing test that defines the behavior.
2. Write the minimum code to pass.
3. Refactor with tests green.
4. Run clippy and fmt.
5. Commit: `feat: <capability> - <what was implemented>` (or `fix:`, `refactor:`, `docs:`, `chore:`).

One logical change per commit, with its tests. Details in [docs/principles.md](docs/principles.md).

## Layout
- `src/main.rs`: event loop and wiring. Keep it thin.
- `src/lib.rs`: pure, unit-testable logic.
- `docs/`: design, roadmap, principles, open questions.
- `docs/adr/`: architecture decision records. Add one for any significant decision, and never edit an accepted one.

## Rules
- Build only what the current milestone needs. No stubs, placeholder modules or unused dependencies.
- Platform-specific code goes behind `cfg(target_os)` in small functions.
- Keep docs honest: describe what exists, and mark plans as plans.
- Only macOS is tested. Do not claim other platforms work.
