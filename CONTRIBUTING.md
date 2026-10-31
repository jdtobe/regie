# Contributing

Regie is a macOS-first drop-down terminal written in Rust. Contributions are welcome, including bug reports and documentation fixes.

## Before you start

- Only macOS is tested. Code for other platforms is untested and unfinished, so do not claim it works.
- Read [AGENTS.md](AGENTS.md) for the working rules and [docs/principles.md](docs/principles.md) for why they exist. The same rules apply to people and to coding agents.
- For anything larger than a small fix, open an issue first so the approach can be agreed before you write it.

## Workflow

Work test-first:

1. Write a failing test that defines the behaviour.
2. Write the minimum code to make it pass.
3. Refactor with the tests green.
4. Run the checks below.
5. Commit one logical change at a time, with its tests.

Behaviour that can be a pure function belongs in `src/lib.rs` or another library module, where it can be tested without a GUI. `src/main.rs` stays thin because the event loop cannot be unit tested.

## Checks

CI runs these on every push and pull request; run them before asking for review:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo test --doc
```

## Commits and pull requests

- Commit messages start with `feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `ci:` or `chore:`, followed by the capability and what changed, for example `feat: terminal session - connect the login shell to xterm.js`.
- One logical change per commit, with its tests.
- Describe what changed, why, and how you verified it. Say plainly what is untested.
- Keep documentation honest: describe what exists, and mark plans as plans.
- Add an architecture decision record in `docs/adr/` for any significant decision. Accepted ADRs are never edited; correct them with a dated amendment.
- Pull requests must pass the full CI checks before merging.

## Licence

Regie is released under the [MIT licence](LICENSE). By contributing you agree that your contribution is licensed under the same terms.
