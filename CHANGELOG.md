# Changelog

## Unreleased

- New crate `craft-biz-ui`: shared egui kit for the back-office suite. High-contrast light and dark theme (contrast ratios enforced by a test: body text >= 12:1, secondary text and status pills >= 7:1), Segoe UI fonts, Ctrl+mouse-wheel zoom 50-300% with Ctrl+0 reset and saved zoom level, toasts, top bar, nav rail, status bar, screenshot helper.
- `craft-biz-ui::kanban`: board widget with workflow rules (a refused drop shows the reason), swimlanes, WIP limits, collapsible columns, insertion line, right-click Move to, keyboard moves, undo event. No usable egui kanban crate exists; built on egui's own drag and drop.
- `craft-biz-ui::table` (sort, filter, virtual scrolling, pills, amounts, sparklines, CSV), `palette` (Ctrl+K, forms generated from MCP tool schemas), `signals` (bus dock: waiting, applied, rejected with retry, sent), `coverage` (test helper: every MCP tool needs a place in the window).
- `bus`: `history`, `outgoing` and `retry` so a window can show applied, rejected and sent events.
- 46 tests (33 kernel, 13 kit incl. headless drag-and-drop with simulated pointer input).

## 0.1.0 - 2026-11-09

- Initial kernel: `money`, `at`, `deadline`, `ekr`, `bus`, `mcp`, `i18n`.
- 26 tests; clippy `-D warnings`, fmt clean.
