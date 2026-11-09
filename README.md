# craft-biz

Shared kernel for the fleet's back-office craft apps (buchcraft, taskcraft,
teamcraft, assetcraft, contractcraft, deskcraft, gaincraft, formcraft,
wikicraft). Library only: no binary, no UI, no ports.

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

v0.1.0, 26 tests, clippy `-D warnings` clean with unwrap/expect/panic denied.
Real logic throughout; nothing here is a demo or stub. The EKR seed chart is
data to be checked against your Steuerberater's chart, not an authority.

## Gates

```powershell
just check; just test; just fmt-check
```
