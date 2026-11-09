//! Money as signed integer cents (EUR only in v1).
//!
//! Formatting follows ONORM A 1080 conventions: `.` groups thousands, `,`
//! separates cents ("1.250,50"). Rounding is commercial (half away from zero),
//! which is what Austrian invoices and the U30 expect.

use crate::{BizError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Signed amount in euro cents.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Money(i64);

/// Longest amount string `parse_de` accepts. Hostile-input fence.
const MAX_PARSE_LEN: usize = 32;

impl Money {
    pub const ZERO: Money = Money(0);

    pub const fn from_cents(cents: i64) -> Self {
        Money(cents)
    }

    pub const fn cents(self) -> i64 {
        self.0
    }

    pub fn from_euros(euros: i64) -> Result<Self> {
        euros.checked_mul(100).map(Money).ok_or(BizError::Overflow)
    }

    pub fn checked_add(self, other: Money) -> Result<Money> {
        self.0
            .checked_add(other.0)
            .map(Money)
            .ok_or(BizError::Overflow)
    }

    pub fn checked_sub(self, other: Money) -> Result<Money> {
        self.0
            .checked_sub(other.0)
            .map(Money)
            .ok_or(BizError::Overflow)
    }

    pub fn checked_neg(self) -> Result<Money> {
        self.0.checked_neg().map(Money).ok_or(BizError::Overflow)
    }

    pub fn checked_abs(self) -> Result<Money> {
        self.0.checked_abs().map(Money).ok_or(BizError::Overflow)
    }

    pub fn is_zero(self) -> bool {
        self.0 == 0
    }

    pub fn is_negative(self) -> bool {
        self.0 < 0
    }

    /// Sum with overflow detection.
    pub fn sum<I: IntoIterator<Item = Money>>(items: I) -> Result<Money> {
        items
            .into_iter()
            .try_fold(Money::ZERO, |acc, m| acc.checked_add(m))
    }

    /// Tax on a net amount at `rate_pct` percent, commercially rounded.
    pub fn tax_from_net(net: Money, rate_pct: u32) -> Result<Money> {
        let num = i128::from(net.0)
            .checked_mul(i128::from(rate_pct))
            .ok_or(BizError::Overflow)?;
        to_money(round_div(num, 100)?)
    }

    /// Split a gross amount into (net, tax) at `rate_pct`. net + tax == gross exactly.
    pub fn split_gross(gross: Money, rate_pct: u32) -> Result<(Money, Money)> {
        let num = i128::from(gross.0)
            .checked_mul(i128::from(rate_pct))
            .ok_or(BizError::Overflow)?;
        let den = 100i128
            .checked_add(i128::from(rate_pct))
            .ok_or(BizError::Overflow)?;
        let tax = to_money(round_div(num, den)?)?;
        let net = gross.checked_sub(tax)?;
        Ok((net, tax))
    }

    /// Split into `parts` amounts that differ by at most one cent and sum exactly
    /// to `self`. Leftover cents go to the earliest parts.
    pub fn allocate(self, parts: u32) -> Result<Vec<Money>> {
        if parts == 0 {
            return Err(BizError::Invalid("allocate into zero parts".into()));
        }
        let n = i64::from(parts);
        let base = self.0.checked_div(n).ok_or(BizError::Overflow)?;
        let rem = self.0.checked_rem(n).ok_or(BizError::Overflow)?;
        let step = if rem < 0 { -1 } else { 1 };
        let extra = rem.unsigned_abs();
        let mut out = Vec::with_capacity(parts as usize);
        for i in 0..u64::from(parts) {
            let bump = if i < extra { step } else { 0 };
            out.push(Money(base.checked_add(bump).ok_or(BizError::Overflow)?));
        }
        Ok(out)
    }

    /// "1.250,50", "-0,05". No currency sign.
    pub fn format_de(self) -> String {
        let abs = self.0.unsigned_abs();
        let euros = abs / 100;
        let cents = abs % 100;
        let sign = if self.0 < 0 { "-" } else { "" };
        format!("{sign}{},{cents:02}", group_thousands(euros))
    }

    /// "€ 1.250,50", "€ -1.250,50".
    pub fn format_eur(self) -> String {
        format!("\u{20ac} {}", self.format_de())
    }

    /// Parse an Austrian/German formatted amount: optional sign, optional `€` or
    /// `EUR`, thousands groups with `.`, cents after `,` (one or two digits).
    /// "1.250,50", "1250,5", "-12", "€ 3.000,00", "1.000 EUR".
    pub fn parse_de(input: &str) -> Result<Money> {
        if input.len() > MAX_PARSE_LEN {
            return Err(BizError::Parse("amount string too long".into()));
        }
        let mut s = input.trim();
        for prefix in ["\u{20ac}", "EUR"] {
            if let Some(rest) = s.strip_prefix(prefix) {
                s = rest.trim_start();
            }
        }
        for suffix in ["\u{20ac}", "EUR"] {
            if let Some(rest) = s.strip_suffix(suffix) {
                s = rest.trim_end();
            }
        }
        let (negative, body) = match s.strip_prefix('-') {
            Some(rest) => (true, rest.trim_start()),
            None => (false, s.strip_prefix('+').unwrap_or(s).trim_start()),
        };
        if body.is_empty() {
            return Err(BizError::Parse(format!("not an amount: {input:?}")));
        }
        let (int_part, frac_part) = match body.split_once(',') {
            Some((i, f)) => (i, Some(f)),
            None => (body, None),
        };
        let euros = parse_grouped_int(int_part)
            .ok_or_else(|| BizError::Parse(format!("bad euro part in {input:?}")))?;
        let cents: u64 = match frac_part {
            None => 0,
            Some(f) if f.len() == 1 && f.bytes().all(|b| b.is_ascii_digit()) => {
                f.parse::<u64>()
                    .map_err(|e| BizError::Parse(e.to_string()))?
                    * 10
            }
            Some(f) if f.len() == 2 && f.bytes().all(|b| b.is_ascii_digit()) => f
                .parse::<u64>()
                .map_err(|e| BizError::Parse(e.to_string()))?,
            Some(_) => return Err(BizError::Parse(format!("bad cent part in {input:?}"))),
        };
        let total = euros
            .checked_mul(100)
            .and_then(|e| e.checked_add(cents))
            .ok_or(BizError::Overflow)?;
        let signed = i64::try_from(total).map_err(|_| BizError::Overflow)?;
        Ok(Money(if negative { -signed } else { signed }))
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.format_eur())
    }
}

fn to_money(v: i128) -> Result<Money> {
    i64::try_from(v).map(Money).map_err(|_| BizError::Overflow)
}

/// Integer division rounding half away from zero. `den` must be positive.
fn round_div(num: i128, den: i128) -> Result<i128> {
    if den <= 0 {
        return Err(BizError::Invalid("non-positive divisor".into()));
    }
    let half = den / 2;
    let q = if num >= 0 {
        num.checked_add(half).ok_or(BizError::Overflow)? / den
    } else {
        -(num
            .checked_neg()
            .ok_or(BizError::Overflow)?
            .checked_add(half)
            .ok_or(BizError::Overflow)?
            / den)
    };
    Ok(q)
}

fn group_thousands(mut v: u64) -> String {
    if v < 1000 {
        return v.to_string();
    }
    let mut groups: Vec<String> = Vec::new();
    while v >= 1000 {
        groups.push(format!("{:03}", v % 1000));
        v /= 1000;
    }
    groups.push(v.to_string());
    groups.reverse();
    groups.join(".")
}

/// "1250" or "1.250" or "12.500.000". Groups after the first must be 3 digits.
fn parse_grouped_int(s: &str) -> Option<u64> {
    if s.is_empty() {
        return None;
    }
    if !s.contains('.') {
        if !s.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        return s.parse().ok();
    }
    let mut parts = s.split('.');
    let first = parts.next()?;
    if first.is_empty() || first.len() > 3 || !first.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut digits = String::from(first);
    for p in parts {
        if p.len() != 3 || !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        digits.push_str(p);
    }
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_onorm() {
        assert_eq!(Money::from_cents(125_050).format_de(), "1.250,50");
        assert_eq!(Money::from_cents(-5).format_de(), "-0,05");
        assert_eq!(Money::from_cents(100_000_000).format_de(), "1.000.000,00");
        assert_eq!(Money::from_cents(99_999).format_de(), "999,99");
        assert_eq!(Money::from_cents(125_050).format_eur(), "\u{20ac} 1.250,50");
        assert_eq!(
            Money::from_cents(i64::MIN).format_de(),
            "-92.233.720.368.547.758,08"
        );
    }

    #[test]
    fn parses_de() {
        assert_eq!(Money::parse_de("1.250,50").unwrap().cents(), 125_050);
        assert_eq!(Money::parse_de("1250,5").unwrap().cents(), 125_050);
        assert_eq!(Money::parse_de("-12").unwrap().cents(), -1200);
        assert_eq!(
            Money::parse_de("\u{20ac} 3.000,00").unwrap().cents(),
            300_000
        );
        assert_eq!(Money::parse_de("1.000 EUR").unwrap().cents(), 100_000);
        assert_eq!(Money::parse_de("+0,01").unwrap().cents(), 1);
    }

    #[test]
    fn rejects_bad_amounts() {
        for bad in [
            "",
            "-",
            "1,234",
            "12.34",
            "1.25,00",
            "abc",
            "1,2,3",
            "1.2345,00",
        ] {
            assert!(Money::parse_de(bad).is_err(), "should reject {bad:?}");
        }
        assert!(Money::parse_de("99999999999999999999,00").is_err());
        assert!(Money::parse_de(&"9".repeat(40)).is_err());
    }

    #[test]
    fn round_trips() {
        for c in [0, 1, -1, 99, 100, 123_456_789, -98_765] {
            let m = Money::from_cents(c);
            assert_eq!(Money::parse_de(&m.format_de()).unwrap(), m);
        }
    }

    #[test]
    fn tax_math_is_commercially_rounded() {
        // 0,05 net at 10% is 0,005 -> rounds half away from zero to 0,01
        assert_eq!(
            Money::tax_from_net(Money::from_cents(5), 10)
                .unwrap()
                .cents(),
            1
        );
        assert_eq!(
            Money::tax_from_net(Money::from_cents(-5), 10)
                .unwrap()
                .cents(),
            -1
        );
        assert_eq!(
            Money::tax_from_net(Money::from_cents(10_000), 20)
                .unwrap()
                .cents(),
            2_000
        );
        let (net, tax) = Money::split_gross(Money::from_cents(12_000), 20).unwrap();
        assert_eq!((net.cents(), tax.cents()), (10_000, 2_000));
        let (net, tax) = Money::split_gross(Money::from_cents(999), 13).unwrap();
        assert_eq!(net.cents() + tax.cents(), 999);
        assert_eq!(tax.cents(), 115); // 999 * 13 / 113 = 114.93
    }

    #[test]
    fn allocates_exactly() {
        let parts = Money::from_cents(100_000).allocate(3).unwrap();
        assert_eq!(
            parts.iter().map(|m| m.cents()).collect::<Vec<_>>(),
            vec![33_334, 33_333, 33_333]
        );
        assert_eq!(Money::sum(parts).unwrap().cents(), 100_000);
        let neg = Money::from_cents(-10).allocate(3).unwrap();
        assert_eq!(Money::sum(neg.clone()).unwrap().cents(), -10);
        assert_eq!(neg[0].cents(), -4);
        assert!(Money::from_cents(1).allocate(0).is_err());
    }

    #[test]
    fn overflow_is_an_error_not_a_panic() {
        assert!(Money::from_cents(i64::MAX)
            .checked_add(Money::from_cents(1))
            .is_err());
        assert!(Money::from_cents(i64::MIN).checked_neg().is_err());
        assert!(Money::from_euros(i64::MAX).is_err());
        assert!(Money::sum([Money::from_cents(i64::MAX), Money::from_cents(1)]).is_err());
    }
}
