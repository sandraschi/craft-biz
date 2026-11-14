# craft-biz

![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)
![Rust 1.90+](https://img.shields.io/badge/rust-1.90%2B-orange?logo=rust)
![MCP stdio](https://img.shields.io/badge/MCP-stdio-purple)
![ArtCraft harness](https://img.shields.io/badge/ArtCraft-harness-green)

Shared kernel for the fleet's back-office craft apps (buchcraft, taskcraft,
teamcraft, assetcraft, contractcraft, deskcraft, gaincraft, formcraft,
wikicraft). Two library crates, no binary, no ports: `craft-biz` (the kernel
below) and `craft-biz-ui` (the shared egui window every suite app runs in).

Plan and rationale: `mcp-central-docs/projects/craft-fleet-expansion/BACKOFFICE_CRAFT_SUITE.md`.

| Module | What it gives the apps |
|---|---|
| `money` | `Money` as i64 cents, ONORM A 1080 format/parse ("1.250,50"), commercial rounding, net/gross/tax split, exact allocation |
| `at` | Austrian UID (ATU) check-digit validation, IBAN mod-97, DD.MM.YYYY dates |
| `deadline` | Kuendigungsfrist math: last notice date, earliest termination by month/quarter/year end, auto-renewing terms |
| `ekr` | EKR seed chart (editable starting point), tax keys U20/U10/U13/V20/V10/V13/IGE20 with U30 Kennzahlen |
| `bus` | File outbox/inbox/ack event bus between apps, atomic writes, path-traversal fence, junk quarantine |
| `mcp` | The single MCP 2024-11-05 stdio loop (`ToolHost` trait), pure `handle_line` for tests |
| `i18n` | `Locale` En/De, date and money formatting |

`craft-biz-ui` (egui 0.37, wgpu):

| Module | What it gives the apps |
|---|---|
| `app` | `Shell` + `SuiteApp`: the whole window frame (nav rail, quick view row, top bar, status bar, palette, signals dock, confirmations, `ui.json`, `--smoke`, `--screenshot`); `main_with` is an app's entire `main` |
| `menu` | Craft Menu Bar Standard bar: File / Edit / View / app menus / Window / Help, switch-on-hover, shortcuts, smoke parity, tofu guard against the loaded fonts |
| `help` | Help suite dialogs: keyboard shortcuts, project docs, licenses, about, preferences |
| `theme`, `shell`, `widgets` | High-contrast light/dark tokens (contrast tested), Ctrl+wheel zoom, toasts, pills, meters, cards |
| `kanban`, `table`, `palette`, `signals`, `coverage` | Board with rules and drag and drop, virtual table, Ctrl+K forms from MCP schemas, bus dock, "every tool has a place" test helper |

Secrets are not here: apps depend on `craft-secrets` from the splatcraft repo
(env var first, then Windows Credential Manager).

## Use from an app

```toml
[dependencies]
craft-biz = { path = "../craft-biz/crates/craft-biz" }
```

```rust
use craft_biz::{Money, ekr::TaxKey};
let net = Money::parse_de("1.000,00")?;
let tax = Money::tax_from_net(net, TaxKey::V20.rate_pct())?; // 200,00
```

## Status

v0.1.0, 51 tests (33 kernel, 18 UI incl. headless drag and drop and menu parity), clippy `-D warnings` clean with unwrap/expect/panic denied and `unsafe_code` forbidden.
Real logic throughout; nothing here is a demo or stub. The EKR seed chart is
data to be checked against your Steuerberater's chart, not an authority.

## Planned: skills and in-app chat (2026-11-14, not built)

Spec: `mcp-central-docs/standards/rust-artcraft/CRAFT_SKILLS_AND_CHAT_STANDARD.md`.

- `mcp`: `ToolHost::skills()` (default empty). When an app bundles skills, the loop also advertises `resources`, answers `resources/list` / `resources/read` with `skill://<name>/SKILL.md`, and adds the tool `<app>_skill_read`. Tool tables gain a `read_only` flag (missing = not read-only), which the chat uses to decide what needs confirmation.
- `craft-biz-ui`: Window > Chat (`Ctrl+J`) through `craft-ai-ui::ChatPanel` when the app implements `ChatHost`, and a Skills tab in Help > Project Files & Docs.
- New path dependencies: `craft-skills` and `craft-ai-ui` from the craft-ai repo.
- Pilot app: taskcraft.

## Gates

```powershell
just check; just test; just fmt-check
```
