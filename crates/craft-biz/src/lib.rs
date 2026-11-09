//! craft-biz: shared kernel for the fleet back-office craft apps.
//!
//! Every module here is pure logic with no UI, CLI or transport framing except
//! `mcp`, which is the one shared MCP 2024-11-05 stdio loop so all apps speak
//! the protocol identically. Secrets live in `craft-secrets` (splatcraft repo),
//! not here.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod at;
pub mod bus;
pub mod confine;
pub mod deadline;
pub mod ekr;
pub mod i18n;
pub mod mcp;
pub mod money;

pub use money::Money;

/// One error type for the whole kernel. Apps wrap it in their own error enums.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BizError {
    #[error("parse error: {0}")]
    Parse(String),
    #[error("invalid value: {0}")]
    Invalid(String),
    #[error("arithmetic overflow")]
    Overflow,
    #[error("io error: {0}")]
    Io(String),
    #[error("json error: {0}")]
    Json(String),
}

impl From<std::io::Error> for BizError {
    fn from(e: std::io::Error) -> Self {
        BizError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for BizError {
    fn from(e: serde_json::Error) -> Self {
        BizError::Json(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, BizError>;
