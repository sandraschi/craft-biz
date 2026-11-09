//! Austrian Einheitskontenrahmen (EKR) seed chart and VAT tax keys with their
//! U30 Kennzahlen.
//!
//! The seed chart is a starting point for a small business, not an authority.
//! Account numbers are editable data in every app that uses them; check them
//! against the chart your Steuerberater uses before relying on postings.

use crate::{BizError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountKind {
    Asset,
    Liability,
    Equity,
    Revenue,
    Expense,
}

impl AccountKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AccountKind::Asset => "asset",
            AccountKind::Liability => "liability",
            AccountKind::Equity => "equity",
            AccountKind::Revenue => "revenue",
            AccountKind::Expense => "expense",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "asset" => Ok(AccountKind::Asset),
            "liability" => Ok(AccountKind::Liability),
            "equity" => Ok(AccountKind::Equity),
            "revenue" => Ok(AccountKind::Revenue),
            "expense" => Ok(AccountKind::Expense),
            other => Err(BizError::Invalid(format!("unknown account kind {other:?}"))),
        }
    }
}

/// EKR class (first digit) of a four-digit account number.
pub fn class_of(account: u32) -> Result<u8> {
    if !(0..=9999).contains(&account) {
        return Err(BizError::Invalid(format!(
            "account {account} is not a 4-digit EKR number"
        )));
    }
    u8::try_from(account / 1000).map_err(|_| BizError::Overflow)
}

/// Human name of an EKR class.
pub fn class_name(class: u8) -> &'static str {
    match class {
        0 => "Anlagevermoegen",
        1 => "Vorraete",
        2 => "Sonstiges Umlaufvermoegen",
        3 => "Rueckstellungen, Verbindlichkeiten",
        4 => "Betriebliche Ertraege",
        5 => "Material- und Leistungsaufwand",
        6 => "Personalaufwand",
        7 => "Abschreibungen, sonstige Aufwendungen",
        8 => "Finanzerfolg, Steuern",
        9 => "Eigenkapital, Abschluss",
        _ => "unbekannt",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedAccount {
    pub number: u32,
    pub name: &'static str,
    pub kind: AccountKind,
    pub default_tax_key: Option<TaxKey>,
}

const fn acc(
    number: u32,
    name: &'static str,
    kind: AccountKind,
    key: Option<TaxKey>,
) -> SeedAccount {
    SeedAccount {
        number,
        name,
        kind,
        default_tax_key: key,
    }
}

/// Small-business seed chart. Editable data, see module docs.
pub fn seed_accounts() -> Vec<SeedAccount> {
    use AccountKind::*;
    use TaxKey::*;
    vec![
        acc(620, "EDV-Anlagen (Hardware)", Asset, Some(V20)),
        acc(630, "PKW", Asset, Some(V20)),
        acc(660, "Betriebs- und Geschaeftsausstattung", Asset, Some(V20)),
        acc(680, "Geringwertige Wirtschaftsgueter", Asset, Some(V20)),
        acc(1600, "Handelswaren", Asset, None),
        acc(2000, "Lieferforderungen Inland", Asset, None),
        acc(2500, "Vorsteuer", Asset, None),
        acc(2510, "Vorsteuer ig. Erwerb", Asset, None),
        acc(2700, "Kassa", Asset, None),
        acc(2800, "Bank", Asset, None),
        acc(3300, "Lieferverbindlichkeiten Inland", Liability, None),
        acc(3500, "Umsatzsteuer", Liability, None),
        acc(3510, "Umsatzsteuer ig. Erwerb", Liability, None),
        acc(3520, "Verrechnungskonto Finanzamt", Liability, None),
        acc(
            3540,
            "Verbindlichkeiten Sozialversicherung",
            Liability,
            None,
        ),
        acc(3600, "Verbindlichkeiten Lohnabgaben", Liability, None),
        acc(4000, "Umsatzerloese 20%", Revenue, Some(U20)),
        acc(4010, "Umsatzerloese 10%", Revenue, Some(U10)),
        acc(4013, "Umsatzerloese 13%", Revenue, Some(U13)),
        acc(5000, "Wareneinsatz", Expense, Some(V20)),
        acc(5800, "Fremdleistungen", Expense, Some(V20)),
        acc(6200, "Gehaelter", Expense, None),
        acc(
            6560,
            "Gesetzlicher Sozialaufwand Angestellte",
            Expense,
            None,
        ),
        acc(7000, "Planmaessige Abschreibung", Expense, None),
        acc(7010, "Sofortabschreibung GWG", Expense, None),
        acc(7200, "Instandhaltung", Expense, Some(V20)),
        acc(7380, "Telefon und Internet", Expense, Some(V20)),
        acc(7400, "Miete", Expense, Some(V20)),
        acc(7600, "Bueromaterial", Expense, Some(V20)),
        acc(7650, "Software und Lizenzen", Expense, Some(V20)),
        acc(7700, "Versicherungen", Expense, None),
        acc(7750, "Rechts- und Beratungsaufwand", Expense, Some(V20)),
        acc(7790, "Spesen des Geldverkehrs", Expense, None),
        acc(8100, "Zinsertraege", Revenue, None),
        acc(8300, "Zinsaufwand", Expense, None),
        acc(9000, "Kapital", Equity, None),
        acc(9600, "Privat", Equity, None),
        acc(9800, "Eroeffnungsbilanzkonto", Equity, None),
        acc(9850, "Schlussbilanzkonto", Equity, None),
    ]
}

/// Whether the tax is charged to customers or reclaimed from the Finanzamt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaxDirection {
    /// Umsatzsteuer owed.
    Output,
    /// Vorsteuer reclaimable.
    Input,
    /// Intra-community acquisition: output and input at once (net zero for a
    /// fully deductible business).
    ReverseCharge,
}

/// VAT keys. Callers book net amounts with a key; the ledger engine generates
/// the tax line. Callers never book the tax line themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaxKey {
    U20,
    U10,
    U13,
    V20,
    V10,
    V13,
    /// Innergemeinschaftlicher Erwerb, 20%.
    IGE20,
}

/// U30 Kennzahlen a key contributes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct U30Fields {
    /// Kennzahl that receives the net base, if any.
    pub base_kz: Option<&'static str>,
    /// Kennzahl that receives the tax amount, if any (input tax keys).
    pub tax_kz: Option<&'static str>,
    /// Whether the base also counts toward KZ 000 (total supplies).
    pub in_kz_000: bool,
}

impl TaxKey {
    pub const ALL: [TaxKey; 7] = [
        TaxKey::U20,
        TaxKey::U10,
        TaxKey::U13,
        TaxKey::V20,
        TaxKey::V10,
        TaxKey::V13,
        TaxKey::IGE20,
    ];

    pub fn rate_pct(self) -> u32 {
        match self {
            TaxKey::U20 | TaxKey::V20 | TaxKey::IGE20 => 20,
            TaxKey::U10 | TaxKey::V10 => 10,
            TaxKey::U13 | TaxKey::V13 => 13,
        }
    }

    pub fn direction(self) -> TaxDirection {
        match self {
            TaxKey::U20 | TaxKey::U10 | TaxKey::U13 => TaxDirection::Output,
            TaxKey::V20 | TaxKey::V10 | TaxKey::V13 => TaxDirection::Input,
            TaxKey::IGE20 => TaxDirection::ReverseCharge,
        }
    }

    /// Account(s) the generated tax line(s) go to: (output account, input account).
    pub fn tax_accounts(self) -> (Option<u32>, Option<u32>) {
        match self.direction() {
            TaxDirection::Output => (Some(3500), None),
            TaxDirection::Input => (None, Some(2500)),
            TaxDirection::ReverseCharge => (Some(3510), Some(2510)),
        }
    }

    pub fn u30(self) -> U30Fields {
        match self {
            TaxKey::U20 => U30Fields {
                base_kz: Some("022"),
                tax_kz: None,
                in_kz_000: true,
            },
            TaxKey::U10 => U30Fields {
                base_kz: Some("029"),
                tax_kz: None,
                in_kz_000: true,
            },
            TaxKey::U13 => U30Fields {
                base_kz: Some("006"),
                tax_kz: None,
                in_kz_000: true,
            },
            TaxKey::V20 | TaxKey::V10 | TaxKey::V13 => U30Fields {
                base_kz: None,
                tax_kz: Some("060"),
                in_kz_000: false,
            },
            TaxKey::IGE20 => U30Fields {
                base_kz: Some("072"),
                tax_kz: Some("065"),
                in_kz_000: false,
            },
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            TaxKey::U20 => "U20",
            TaxKey::U10 => "U10",
            TaxKey::U13 => "U13",
            TaxKey::V20 => "V20",
            TaxKey::V10 => "V10",
            TaxKey::V13 => "V13",
            TaxKey::IGE20 => "IGE20",
        }
    }

    pub fn parse(s: &str) -> Result<TaxKey> {
        TaxKey::ALL
            .into_iter()
            .find(|k| k.as_str().eq_ignore_ascii_case(s.trim()))
            .ok_or_else(|| {
                BizError::Invalid(format!(
                    "unknown tax key {s:?} (U20 U10 U13 V20 V10 V13 IGE20)"
                ))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes() {
        assert_eq!(class_of(2800).unwrap(), 2);
        assert_eq!(class_of(620).unwrap(), 0);
        assert!(class_of(10_000).is_err());
        assert_eq!(class_name(4), "Betriebliche Ertraege");
    }

    #[test]
    fn seed_chart_is_consistent() {
        let seed = seed_accounts();
        let mut numbers: Vec<u32> = seed.iter().map(|a| a.number).collect();
        numbers.sort_unstable();
        numbers.dedup();
        assert_eq!(numbers.len(), seed.len(), "duplicate account numbers");
        for a in &seed {
            let class = class_of(a.number).unwrap();
            match class {
                0..=2 => assert_eq!(a.kind, AccountKind::Asset, "{}", a.number),
                3 => assert_eq!(a.kind, AccountKind::Liability, "{}", a.number),
                4 => assert_eq!(a.kind, AccountKind::Revenue, "{}", a.number),
                5..=7 => assert_eq!(a.kind, AccountKind::Expense, "{}", a.number),
                9 => assert_eq!(a.kind, AccountKind::Equity, "{}", a.number),
                _ => {}
            }
        }
        // Every tax account a key posts to exists in the seed.
        for k in TaxKey::ALL {
            let (out, inp) = k.tax_accounts();
            for n in [out, inp].into_iter().flatten() {
                assert!(
                    seed.iter().any(|a| a.number == n),
                    "missing tax account {n}"
                );
            }
        }
    }

    #[test]
    fn tax_keys() {
        assert_eq!(TaxKey::parse("v20").unwrap(), TaxKey::V20);
        assert!(TaxKey::parse("X99").is_err());
        assert_eq!(TaxKey::U13.u30().base_kz, Some("006"));
        assert_eq!(TaxKey::V10.u30().tax_kz, Some("060"));
        assert_eq!(TaxKey::IGE20.direction(), TaxDirection::ReverseCharge);
        assert_eq!(AccountKind::parse("revenue").unwrap(), AccountKind::Revenue);
    }
}
