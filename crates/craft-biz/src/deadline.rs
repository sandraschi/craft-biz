//! Notice-period (Kuendigungsfrist) and contract-term math.
//!
//! Conventions, stated so they can be checked against a real contract:
//! - A term of N months starting on S ends the day before S + N months
//!   (start 01.01.2026, 12 months -> ends 31.12.2026).
//! - "N months notice to term end E" means notice must be received by
//!   E minus N months. If E is the last day of its month, the result is the
//!   last day of the month N months earlier (31.12 -> 30.09, 30.06 -> 31.03).
//! - Days are inclusive: notice received on the returned date is in time.

use crate::{BizError, Result};
use chrono::{Datelike, Months, NaiveDate};
use serde::{Deserialize, Serialize};

/// On which days a termination may take effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminationRule {
    AnyDay,
    EndOfMonth,
    EndOfQuarter,
    EndOfYear,
}

fn months(n: u32) -> Months {
    Months::new(n)
}

/// Last day of the month containing `d`.
pub fn month_end(d: NaiveDate) -> Result<NaiveDate> {
    let first = d.with_day(1).ok_or(BizError::Overflow)?;
    first
        .checked_add_months(months(1))
        .and_then(|n| n.pred_opt())
        .ok_or(BizError::Overflow)
}

pub fn is_month_end(d: NaiveDate) -> bool {
    d.succ_opt().is_some_and(|n| n.month() != d.month())
}

/// The period end (per `rule`) on or after `d`.
pub fn period_end(d: NaiveDate, rule: TerminationRule) -> Result<NaiveDate> {
    match rule {
        TerminationRule::AnyDay => Ok(d),
        TerminationRule::EndOfMonth => month_end(d),
        TerminationRule::EndOfQuarter => {
            let q_last_month = ((d.month() - 1) / 3 + 1) * 3;
            let in_q_last =
                NaiveDate::from_ymd_opt(d.year(), q_last_month, 1).ok_or(BizError::Overflow)?;
            month_end(in_q_last)
        }
        TerminationRule::EndOfYear => {
            NaiveDate::from_ymd_opt(d.year(), 12, 31).ok_or(BizError::Overflow)
        }
    }
}

/// Last day notice can be received to end the contract on `term_end`.
pub fn last_notice_date(term_end: NaiveDate, notice_months: u32) -> Result<NaiveDate> {
    let back = term_end
        .checked_sub_months(months(notice_months))
        .ok_or(BizError::Overflow)?;
    if is_month_end(term_end) {
        month_end(back)
    } else {
        Ok(back)
    }
}

/// Earliest date a termination takes effect if notice is received on
/// `received`, with `notice_months` notice and effect only on `rule` days.
pub fn earliest_termination(
    received: NaiveDate,
    notice_months: u32,
    rule: TerminationRule,
) -> Result<NaiveDate> {
    let mut candidate = period_end(received, rule)?;
    // Walk period ends forward until the notice period fits. Bounded: at most
    // notice_months + 13 steps for monthly rules, far fewer for yearly.
    for _ in 0..=(notice_months.saturating_add(13)) {
        if last_notice_date(candidate, notice_months)? >= received {
            return Ok(candidate);
        }
        let next_day = candidate.succ_opt().ok_or(BizError::Overflow)?;
        candidate = period_end(next_day, rule)?;
        if rule == TerminationRule::AnyDay {
            // Direct answer for AnyDay: received + notice months.
            return received
                .checked_add_months(months(notice_months))
                .ok_or(BizError::Overflow);
        }
    }
    Err(BizError::Invalid("no termination date found".into()))
}

/// A fixed initial term that renews automatically unless terminated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoRenewal {
    pub start: NaiveDate,
    pub initial_months: u32,
    pub renewal_months: u32,
    pub notice_months: u32,
}

/// The next term end whose notice deadline has not passed, and that deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NextDeadline {
    pub term_end: NaiveDate,
    pub last_notice: NaiveDate,
}

impl AutoRenewal {
    pub fn validate(&self) -> Result<()> {
        if self.initial_months == 0 || self.renewal_months == 0 {
            return Err(BizError::Invalid(
                "term lengths must be at least one month".into(),
            ));
        }
        if self.notice_months > 120 || self.initial_months > 1200 || self.renewal_months > 1200 {
            return Err(BizError::Invalid(
                "implausible term or notice length".into(),
            ));
        }
        Ok(())
    }

    fn term_end_after(&self, total_months: u32) -> Result<NaiveDate> {
        self.start
            .checked_add_months(months(total_months))
            .and_then(|d| d.pred_opt())
            .ok_or(BizError::Overflow)
    }

    /// First term end whose last notice date is on or after `today`.
    pub fn next_deadline(&self, today: NaiveDate) -> Result<NextDeadline> {
        self.validate()?;
        let mut total = self.initial_months;
        // Bounded loop: 1,000 renewals is far beyond any real contract.
        for _ in 0..1000 {
            let term_end = self.term_end_after(total)?;
            let last_notice = last_notice_date(term_end, self.notice_months)?;
            if last_notice >= today {
                return Ok(NextDeadline {
                    term_end,
                    last_notice,
                });
            }
            total = total
                .checked_add(self.renewal_months)
                .ok_or(BizError::Overflow)?;
        }
        Err(BizError::Invalid(
            "contract renews more than 1000 times".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn last_notice_month_end_logic() {
        assert_eq!(
            last_notice_date(d(2026, 12, 31), 3).unwrap(),
            d(2026, 9, 30)
        );
        assert_eq!(last_notice_date(d(2026, 6, 30), 3).unwrap(), d(2026, 3, 31));
        assert_eq!(last_notice_date(d(2026, 5, 31), 3).unwrap(), d(2026, 2, 28));
        assert_eq!(last_notice_date(d(2028, 5, 31), 3).unwrap(), d(2028, 2, 29));
        assert_eq!(last_notice_date(d(2026, 7, 15), 1).unwrap(), d(2026, 6, 15));
        assert_eq!(
            last_notice_date(d(2026, 12, 31), 0).unwrap(),
            d(2026, 12, 31)
        );
    }

    #[test]
    fn period_ends() {
        assert_eq!(
            period_end(d(2026, 2, 3), TerminationRule::EndOfMonth).unwrap(),
            d(2026, 2, 28)
        );
        assert_eq!(
            period_end(d(2026, 2, 3), TerminationRule::EndOfQuarter).unwrap(),
            d(2026, 3, 31)
        );
        assert_eq!(
            period_end(d(2026, 11, 9), TerminationRule::EndOfQuarter).unwrap(),
            d(2026, 12, 31)
        );
        assert_eq!(
            period_end(d(2026, 4, 1), TerminationRule::EndOfQuarter).unwrap(),
            d(2026, 6, 30)
        );
        assert_eq!(
            period_end(d(2026, 4, 1), TerminationRule::EndOfYear).unwrap(),
            d(2026, 12, 31)
        );
    }

    #[test]
    fn earliest_termination_quarter() {
        // Notice on 09.11.2026, 3 months to quarter end: 31.12 needs notice by 30.09 (missed),
        // 31.03.2027 needs notice by 31.12.2026 (ok).
        assert_eq!(
            earliest_termination(d(2026, 11, 9), 3, TerminationRule::EndOfQuarter).unwrap(),
            d(2027, 3, 31)
        );
        // Notice exactly on the deadline counts.
        assert_eq!(
            earliest_termination(d(2026, 9, 30), 3, TerminationRule::EndOfQuarter).unwrap(),
            d(2026, 12, 31)
        );
        assert_eq!(
            earliest_termination(d(2026, 11, 9), 1, TerminationRule::EndOfMonth).unwrap(),
            d(2026, 12, 31)
        );
        assert_eq!(
            earliest_termination(d(2026, 11, 9), 2, TerminationRule::AnyDay).unwrap(),
            d(2027, 1, 9)
        );
    }

    #[test]
    fn auto_renewal_rolls_forward() {
        let c = AutoRenewal {
            start: d(2025, 1, 1),
            initial_months: 12,
            renewal_months: 12,
            notice_months: 3,
        };
        // Before 30.09.2025 the first term is still cancellable.
        assert_eq!(
            c.next_deadline(d(2025, 9, 30)).unwrap(),
            NextDeadline {
                term_end: d(2025, 12, 31),
                last_notice: d(2025, 9, 30)
            }
        );
        // On 01.10.2025 it has auto-renewed to 2026.
        assert_eq!(
            c.next_deadline(d(2025, 10, 1)).unwrap(),
            NextDeadline {
                term_end: d(2026, 12, 31),
                last_notice: d(2026, 9, 30)
            }
        );
        let bad = AutoRenewal {
            renewal_months: 0,
            ..c
        };
        assert!(bad.next_deadline(d(2025, 1, 1)).is_err());
    }
}
