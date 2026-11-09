//! Austrian identifiers and date formats: UID (ATU), IBAN, DD.MM.YYYY.

use crate::{BizError, Result};
use chrono::NaiveDate;

/// Validate and normalise an Austrian VAT number (UID). Accepts spaces and
/// lower case; returns the canonical "ATU12345678".
///
/// Check digit: for digits d1..d7 of the 8-digit body, S = d1 + q(2*d2) + d3 +
/// q(2*d4) + d5 + q(2*d6) + d7 where q is the digit sum; the 8th digit is
/// (10 - (S + 4) mod 10) mod 10.
pub fn validate_uid(input: &str) -> Result<String> {
    let s: String = input
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_uppercase)
        .collect();
    let body = s
        .strip_prefix("ATU")
        .ok_or_else(|| BizError::Invalid(format!("UID must start with ATU: {input:?}")))?;
    if body.len() != 8 || !body.bytes().all(|b| b.is_ascii_digit()) {
        return Err(BizError::Invalid(format!(
            "UID needs 8 digits after ATU: {input:?}"
        )));
    }
    let digits: Vec<u32> = body.bytes().map(|b| u32::from(b - b'0')).collect();
    let mut sum = 0u32;
    for (i, d) in digits.iter().take(7).enumerate() {
        sum += if i % 2 == 1 {
            let x = d * 2;
            x / 10 + x % 10
        } else {
            *d
        };
    }
    let check = (10 - (sum + 4) % 10) % 10;
    match digits.get(7) {
        Some(&last) if last == check => Ok(s),
        _ => Err(BizError::Invalid(format!(
            "UID check digit wrong: {input:?}"
        ))),
    }
}

/// Validate and normalise an IBAN (ISO 13616 mod-97). Austrian IBANs must be 20
/// characters. Returns the compact upper-case form.
pub fn validate_iban(input: &str) -> Result<String> {
    let s: String = input
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_uppercase)
        .collect();
    if s.len() < 15 || s.len() > 34 || !s.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err(BizError::Invalid(format!("not an IBAN: {input:?}")));
    }
    let bytes = s.as_bytes();
    let country_ok = bytes.first().is_some_and(u8::is_ascii_uppercase)
        && bytes.get(1).is_some_and(u8::is_ascii_uppercase)
        && bytes.get(2).is_some_and(u8::is_ascii_digit)
        && bytes.get(3).is_some_and(u8::is_ascii_digit);
    if !country_ok {
        return Err(BizError::Invalid(format!(
            "IBAN must start with country + check digits: {input:?}"
        )));
    }
    if s.starts_with("AT") && s.len() != 20 {
        return Err(BizError::Invalid(format!(
            "Austrian IBAN must have 20 characters: {input:?}"
        )));
    }
    let rearranged = s.chars().skip(4).chain(s.chars().take(4));
    let mut rem: u32 = 0;
    for c in rearranged {
        let v = c
            .to_digit(36)
            .ok_or_else(|| BizError::Invalid(format!("bad IBAN character {c:?}")))?;
        rem = if v >= 10 {
            (rem * 100 + v) % 97
        } else {
            (rem * 10 + v) % 97
        };
    }
    if rem == 1 {
        Ok(s)
    } else {
        Err(BizError::Invalid(format!("IBAN checksum wrong: {input:?}")))
    }
}

/// "09.11.2026".
pub fn format_date_de(d: NaiveDate) -> String {
    d.format("%d.%m.%Y").to_string()
}

/// Accepts "09.11.2026", "9.11.2026" and ISO "2026-11-09".
pub fn parse_date(input: &str) -> Result<NaiveDate> {
    let s = input.trim();
    if s.len() > 10 {
        return Err(BizError::Parse(format!("date too long: {input:?}")));
    }
    NaiveDate::parse_from_str(s, "%d.%m.%Y")
        .or_else(|_| NaiveDate::parse_from_str(s, "%Y-%m-%d"))
        .map_err(|_| BizError::Parse(format!("not a date (DD.MM.YYYY or YYYY-MM-DD): {input:?}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uid_known_valid_and_normalised() {
        assert_eq!(validate_uid("ATU13585627").unwrap(), "ATU13585627");
        assert_eq!(validate_uid(" atu 1358 5627 ").unwrap(), "ATU13585627");
    }

    #[test]
    fn uid_rejects_wrong_check_and_shape() {
        assert!(validate_uid("ATU13585628").is_err());
        assert!(validate_uid("ATU1358562").is_err());
        assert!(validate_uid("DE123456789").is_err());
        assert!(validate_uid("ATUABCDEFGH").is_err());
        assert!(validate_uid("").is_err());
    }

    #[test]
    fn iban_valid_examples() {
        assert_eq!(
            validate_iban("AT61 1904 3002 3457 3201").unwrap(),
            "AT611904300234573201"
        );
        assert!(validate_iban("DE89370400440532013000").is_ok());
        assert!(validate_iban("GB82WEST12345698765432").is_ok());
    }

    #[test]
    fn iban_rejects() {
        assert!(validate_iban("AT611904300234573202").is_err()); // checksum
        assert!(validate_iban("AT61190430023457320").is_err()); // AT length
        assert!(validate_iban("1T611904300234573201").is_err());
        assert!(validate_iban("AT61-1904").is_err());
        assert!(validate_iban(&"A".repeat(50)).is_err());
    }

    #[test]
    fn dates() {
        let d = parse_date("09.11.2026").unwrap();
        assert_eq!(d, NaiveDate::from_ymd_opt(2026, 11, 9).unwrap());
        assert_eq!(parse_date("2026-11-09").unwrap(), d);
        assert_eq!(format_date_de(d), "09.11.2026");
        assert!(parse_date("31.02.2026").is_err());
        assert!(parse_date("tomorrow").is_err());
    }
}
