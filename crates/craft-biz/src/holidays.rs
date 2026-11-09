//! Austrian statutory public holidays (Feiertagsruhegesetz section 7(2)) and
//! working-day counting. National holidays only: regional patron-saint days
//! (e.g. Leopold in Lower Austria/Vienna) are not statutory rest days.

use crate::{BizError, Result};
use chrono::{Datelike, Duration, NaiveDate, Weekday};

/// Easter Sunday (Gregorian, anonymous/Meeus algorithm).
pub fn easter_sunday(year: i32) -> Result<NaiveDate> {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    NaiveDate::from_ymd_opt(year, month as u32, day as u32)
        .ok_or_else(|| BizError::Invalid(format!("no Easter date for {year}")))
}

/// The 13 national public holidays of a year, sorted, with German names.
pub fn public_holidays(year: i32) -> Result<Vec<(NaiveDate, &'static str)>> {
    if !(1900..=2200).contains(&year) {
        return Err(BizError::Invalid(format!(
            "year {year} out of range 1900-2200"
        )));
    }
    let fixed = |m: u32, d: u32| {
        NaiveDate::from_ymd_opt(year, m, d)
            .ok_or_else(|| BizError::Invalid(format!("{year}-{m}-{d}")))
    };
    let easter = easter_sunday(year)?;
    let mut v = vec![
        (fixed(1, 1)?, "Neujahr"),
        (fixed(1, 6)?, "Heilige Drei Könige"),
        (easter + Duration::days(1), "Ostermontag"),
        (fixed(5, 1)?, "Staatsfeiertag"),
        (easter + Duration::days(39), "Christi Himmelfahrt"),
        (easter + Duration::days(50), "Pfingstmontag"),
        (easter + Duration::days(60), "Fronleichnam"),
        (fixed(8, 15)?, "Mariä Himmelfahrt"),
        (fixed(10, 26)?, "Nationalfeiertag"),
        (fixed(11, 1)?, "Allerheiligen"),
        (fixed(12, 8)?, "Mariä Empfängnis"),
        (fixed(12, 25)?, "Christtag"),
        (fixed(12, 26)?, "Stefanitag"),
    ];
    v.sort_by_key(|(d, _)| *d);
    Ok(v)
}

pub fn holiday_name(d: NaiveDate) -> Result<Option<&'static str>> {
    Ok(public_holidays(d.year())?
        .into_iter()
        .find(|(h, _)| *h == d)
        .map(|(_, n)| n))
}

/// Monday to Friday and not a public holiday.
pub fn is_working_day(d: NaiveDate) -> Result<bool> {
    if matches!(d.weekday(), Weekday::Sat | Weekday::Sun) {
        return Ok(false);
    }
    Ok(holiday_name(d)?.is_none())
}

/// Working days in `from..=to`. Errors if `to < from` or the span exceeds 3 years.
pub fn working_days(from: NaiveDate, to: NaiveDate) -> Result<u32> {
    if to < from {
        return Err(BizError::Invalid(format!("{to} is before {from}")));
    }
    if (to - from).num_days() > 3 * 366 {
        return Err(BizError::Invalid("span longer than 3 years".into()));
    }
    let mut n = 0;
    let mut d = from;
    while d <= to {
        if is_working_day(d)? {
            n += 1;
        }
        d += Duration::days(1);
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn easter_known_years() {
        assert_eq!(easter_sunday(2024).unwrap(), d(2024, 3, 31));
        assert_eq!(easter_sunday(2025).unwrap(), d(2025, 4, 20));
        assert_eq!(easter_sunday(2026).unwrap(), d(2026, 4, 5));
        assert_eq!(easter_sunday(2027).unwrap(), d(2027, 3, 28));
    }

    #[test]
    fn holidays_2026() {
        let h = public_holidays(2026).unwrap();
        assert_eq!(h.len(), 13);
        assert_eq!(holiday_name(d(2026, 4, 6)).unwrap(), Some("Ostermontag"));
        assert_eq!(
            holiday_name(d(2026, 5, 14)).unwrap(),
            Some("Christi Himmelfahrt")
        );
        assert_eq!(holiday_name(d(2026, 6, 4)).unwrap(), Some("Fronleichnam"));
        assert_eq!(holiday_name(d(2026, 11, 15)).unwrap(), None);
    }

    #[test]
    fn working_day_counts() {
        // Week of 2026-10-26 (Nationalfeiertag Monday): 4 working days.
        assert_eq!(working_days(d(2026, 10, 26), d(2026, 11, 1)).unwrap(), 4);
        // 2026-12-21..2027-01-06: Christtag Fri, Stefanitag Sat, Neujahr Fri, Drei Koenige Wed.
        assert_eq!(working_days(d(2026, 12, 21), d(2027, 1, 6)).unwrap(), 10);
        assert!(working_days(d(2026, 2, 2), d(2026, 2, 1)).is_err());
    }
}
