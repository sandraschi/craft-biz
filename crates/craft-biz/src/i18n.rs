//! Locale handling shared by the back-office apps.
//!
//! These apps encode Austrian law, so German and English are the two locales
//! with real content. Each app keeps its own dictionary in `<app>-core/src/i18n.rs`
//! keyed by this `Locale`.

use crate::Money;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    #[default]
    En,
    De,
}

impl Locale {
    /// `CRAFT_LOCALE` (`de`/`en`) wins, then `LANG`, then English.
    pub fn from_env() -> Locale {
        for var in ["CRAFT_LOCALE", "LANG"] {
            if let Ok(v) = std::env::var(var) {
                if let Some(l) = Locale::parse(&v) {
                    return l;
                }
            }
        }
        Locale::En
    }

    pub fn parse(s: &str) -> Option<Locale> {
        let s = s.trim().to_ascii_lowercase();
        if s.starts_with("de") {
            Some(Locale::De)
        } else if s.starts_with("en") {
            Some(Locale::En)
        } else {
            None
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::De => "de",
        }
    }

    /// De: "09.11.2026", En: "2026-11-09" (ISO, unambiguous for an international reader).
    pub fn format_date(self, d: NaiveDate) -> String {
        match self {
            Locale::De => d.format("%d.%m.%Y").to_string(),
            Locale::En => d.format("%Y-%m-%d").to_string(),
        }
    }

    /// De: "€ 1.250,50", En: "EUR 1,250.50".
    pub fn format_money(self, m: Money) -> String {
        match self {
            Locale::De => m.format_eur(),
            Locale::En => format!(
                "EUR {}",
                m.format_de()
                    .replace('.', "\u{0}")
                    .replace(',', ".")
                    .replace('\u{0}', ",")
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_format() {
        assert_eq!(Locale::parse("de_AT.UTF-8"), Some(Locale::De));
        assert_eq!(Locale::parse("EN"), Some(Locale::En));
        assert_eq!(Locale::parse("ja"), None);
        let d = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
        assert_eq!(Locale::De.format_date(d), "09.11.2026");
        assert_eq!(Locale::En.format_date(d), "2026-11-09");
        let m = Money::from_cents(125_050);
        assert_eq!(Locale::De.format_money(m), "\u{20ac} 1.250,50");
        assert_eq!(Locale::En.format_money(m), "EUR 1,250.50");
        assert_eq!(Locale::En.format_money(Money::from_cents(-5)), "EUR -0.05");
    }
}
