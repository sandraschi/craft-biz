# AGENTS.md - craft-biz

Shared library for the back-office craft apps. Read
`mcp-central-docs/standards/RUST_ARTCRAFT_STANDARD.md` and
`mcp-central-docs/projects/craft-fleet-expansion/BACKOFFICE_CRAFT_SUITE.md` first.

## UI kit (crates/craft-biz-ui)

- Every suite window uses this kit: one look, one place to fix things.
- High contrast is a hard rule: never light grey text on white. New colours go into `theme.rs` and must pass `text_contrast_is_high_in_both_modes`.
- Ctrl+mouse-wheel zoom must keep working in every window (`shell::handle_zoom` each frame).
- Widgets return events; the app turns them into `Command`s. No business logic in the kit.
- egui 0.37: `Id::unique` (not `Id::new`); named font families need `theme::install_fonts` before the first drawn frame (tests too).
- UI behaviour is tested headless with egui_kittest (real frames, simulated pointer and keys).

## Rules

- Library only. No UI crates, no clap, no network, no ports.
- Zero-panic: `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!` are denied
  outside tests. Every fallible path returns `craft_biz::Result`.
- Money is integer cents. Never introduce `f64` for amounts.
- Treat all external input as hostile: length caps, checked arithmetic, no
  raw indexing, names validated before they become path components.
- Every bug fix ships with a regression test.
- Changing a public signature means checking every back-office app that
  depends on this crate (`D:\Dev\repos\*craft` with a `craft-biz` path dep).

## Repo hygiene

- Line endings: LF only (`.gitattributes` enforces it). CRLF breaks commit hooks.
- Single trunk: work on `main`. No agent branches, no leftover WIP branches.
- No em dash characters anywhere (code, docs, commit messages). Use a hyphen.
- No `*.bak`, `target/`, or binaries in git.

## Gates (all must pass before commit)

1. `cargo clippy --workspace --all-targets --all-features -- -D warnings`
2. `cargo test --workspace`
3. `cargo fmt --all --check`
4. `Cargo.lock` committed
5. `CHANGELOG.md` line added
