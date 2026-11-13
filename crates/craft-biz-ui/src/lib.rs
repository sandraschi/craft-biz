//! craft-biz-ui: the shared egui kit of the back-office craft suite.
//!
//! - [`theme`]: high-contrast light/dark tokens, fonts, type scale (contrast is tested).
//! - [`shell`]: settings, Ctrl+wheel zoom, toasts, top bar, nav rail, status bar, screenshots.
//! - [`kanban`]: board with workflow rules, swimlanes, WIP limits, drag and drop, keyboard.
//! - [`table`]: sortable, filterable, virtual-scrolling data table.
//! - [`palette`]: Ctrl+K command palette with forms generated from MCP tool schemas.
//! - [`signals`]: bus dock (waiting, applied, rejected, sent) with drain and retry.
//! - [`coverage`]: test helper proving every MCP tool has a place in the window.
//! - [`menu`]: Craft Menu Bar Standard bar, shortcuts, smoke parity, tofu guard.
//! - [`bookend`]: the same File / Edit / View / Window / Help bar for a window that does not use the shell.
//! - [`help`]: Help suite dialogs (shortcuts, docs, licenses, about, preferences).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod app;
pub mod bookend;
pub mod coverage;
pub mod help;
pub mod kanban;
pub mod menu;
pub mod palette;
pub mod shell;
pub mod signals;
pub mod table;
pub mod theme;
pub mod widgets;

pub use eframe;
pub use egui;
pub use theme::Tone;

#[cfg(test)]
mod tests;
