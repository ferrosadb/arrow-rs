// MIT License
//
// Copyright (c) 2020-2022 Oliver Margetts
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

// Tests adapted from the chronoutil crate

//! Calendar shifts by months and days, with the semantics of the `chrono`
//! operations arrow previously used (`NaiveDate + Months`,
//! `DateTime::checked_add_months`, `DateTime::checked_add_days`, ...).

use crate::calendar::{CivilDate, ZonedDateTime};
use crate::timezone::Tz;
use std::cmp::Ordering;

/// Shift a date by the given number of months.
///
/// # Panics
///
/// If the result is out of range, as `chrono`'s `NaiveDate + Months` did
pub(crate) fn shift_months(date: CivilDate, months: i32) -> CivilDate {
    match months.cmp(&0) {
        Ordering::Equal => date,
        Ordering::Greater => date
            .checked_add_months(months as i64)
            .expect("`NaiveDate + Months` out of range"),
        Ordering::Less => date
            .checked_add_months(-(months.unsigned_abs() as i64))
            .expect("`NaiveDate - Months` out of range"),
    }
}

/// Adds (`op == "+"`) or subtracts (`op == "-"`) whole days.
///
/// # Panics
///
/// If the result is out of range, as `chrono`'s `NaiveDate + TimeDelta` did
pub(crate) fn add_signed_days(date: CivilDate, days: i64, op: &str) -> CivilDate {
    let delta = match op {
        "+" => Some(days),
        _ => days.checked_neg(),
    };
    let result = delta
        .filter(|d| i32::try_from(*d).is_ok())
        .and_then(|d| date.checked_add_days_i64(d));
    match result {
        Some(d) => d,
        None => panic!("`NaiveDate {op} TimeDelta` overflowed"),
    }
}

/// Whole days in a duration of `ms` milliseconds, truncated toward zero
pub(crate) fn date64_days(ms: i64) -> i64 {
    ms / 86_400_000
}

/// Whole days in a duration of `ns` nanoseconds, truncated toward zero
pub(crate) fn nanos_days(ns: i64) -> i64 {
    ns / 86_400_000_000_000
}

/// Shifts the local datetime of `z` by `months`, resolving it again in `tz`.
///
/// Returns `None` when the result is out of range or not unique in `tz`.
fn shift_months_zoned(z: ZonedDateTime, months: i64, tz: &Tz) -> Option<ZonedDateTime> {
    let mut local = z.local();
    local.date = local.date.checked_add_months(months)?;
    tz.from_local(local)
}

/// Shifts the local datetime of `z` by `days`, resolving it again in `tz`.
fn shift_days_zoned(z: ZonedDateTime, days: i64, tz: &Tz) -> Option<ZonedDateTime> {
    let days = i32::try_from(days).ok()?;
    let mut local = z.local();
    local.date = local.date.add_days_headroom(days)?;
    tz.from_local(local).filter(|z| z.utc.date.is_valid())
}

/// Add the given number of months to the given datetime.
///
/// Returns `None` when it will result in overflow.
pub(crate) fn add_months_zoned(z: ZonedDateTime, months: i32, tz: &Tz) -> Option<ZonedDateTime> {
    match months.cmp(&0) {
        Ordering::Equal => Some(z),
        _ => shift_months_zoned(z, months as i64, tz),
    }
}

/// Add the given number of days to the given datetime.
///
/// Returns `None` when it will result in overflow.
pub(crate) fn add_days_zoned(z: ZonedDateTime, days: i32, tz: &Tz) -> Option<ZonedDateTime> {
    match days.cmp(&0) {
        Ordering::Equal => Some(z),
        _ => shift_days_zoned(z, days as i64, tz),
    }
}

/// Subtract the given number of months from the given datetime.
///
/// Returns `None` when it will result in overflow.
pub(crate) fn sub_months_zoned(z: ZonedDateTime, months: i32, tz: &Tz) -> Option<ZonedDateTime> {
    match months.cmp(&0) {
        Ordering::Equal => Some(z),
        _ => shift_months_zoned(z, -(months as i64), tz),
    }
}

/// Subtract the given number of days from the given datetime.
///
/// Returns `None` when it will result in overflow.
pub(crate) fn sub_days_zoned(z: ZonedDateTime, days: i32, tz: &Tz) -> Option<ZonedDateTime> {
    match days.cmp(&0) {
        Ordering::Equal => Some(z),
        _ => shift_days_zoned(z, -(days as i64), tz),
    }
}

#[cfg(test)]
mod tests {

    use crate::calendar::{CivilDate as NaiveDate, CivilDateTime, CivilTime};

    use super::*;

    #[test]
    fn test_shift_months() {
        let base = NaiveDate::from_ymd_opt(2020, 1, 31).unwrap();

        assert_eq!(
            shift_months(base, 0),
            NaiveDate::from_ymd_opt(2020, 1, 31).unwrap()
        );
        assert_eq!(
            shift_months(base, 1),
            NaiveDate::from_ymd_opt(2020, 2, 29).unwrap()
        );
        assert_eq!(
            shift_months(base, 2),
            NaiveDate::from_ymd_opt(2020, 3, 31).unwrap()
        );
        assert_eq!(
            shift_months(base, 3),
            NaiveDate::from_ymd_opt(2020, 4, 30).unwrap()
        );
        assert_eq!(
            shift_months(base, 4),
            NaiveDate::from_ymd_opt(2020, 5, 31).unwrap()
        );
        assert_eq!(
            shift_months(base, 5),
            NaiveDate::from_ymd_opt(2020, 6, 30).unwrap()
        );
        assert_eq!(
            shift_months(base, 6),
            NaiveDate::from_ymd_opt(2020, 7, 31).unwrap()
        );
        assert_eq!(
            shift_months(base, 7),
            NaiveDate::from_ymd_opt(2020, 8, 31).unwrap()
        );
        assert_eq!(
            shift_months(base, 8),
            NaiveDate::from_ymd_opt(2020, 9, 30).unwrap()
        );
        assert_eq!(
            shift_months(base, 9),
            NaiveDate::from_ymd_opt(2020, 10, 31).unwrap()
        );
        assert_eq!(
            shift_months(base, 10),
            NaiveDate::from_ymd_opt(2020, 11, 30).unwrap()
        );
        assert_eq!(
            shift_months(base, 11),
            NaiveDate::from_ymd_opt(2020, 12, 31).unwrap()
        );
        assert_eq!(
            shift_months(base, 12),
            NaiveDate::from_ymd_opt(2021, 1, 31).unwrap()
        );
        assert_eq!(
            shift_months(base, 13),
            NaiveDate::from_ymd_opt(2021, 2, 28).unwrap()
        );

        assert_eq!(
            shift_months(base, -1),
            NaiveDate::from_ymd_opt(2019, 12, 31).unwrap()
        );
        assert_eq!(
            shift_months(base, -2),
            NaiveDate::from_ymd_opt(2019, 11, 30).unwrap()
        );
        assert_eq!(
            shift_months(base, -3),
            NaiveDate::from_ymd_opt(2019, 10, 31).unwrap()
        );
        assert_eq!(
            shift_months(base, -4),
            NaiveDate::from_ymd_opt(2019, 9, 30).unwrap()
        );
        assert_eq!(
            shift_months(base, -5),
            NaiveDate::from_ymd_opt(2019, 8, 31).unwrap()
        );
        assert_eq!(
            shift_months(base, -6),
            NaiveDate::from_ymd_opt(2019, 7, 31).unwrap()
        );
        assert_eq!(
            shift_months(base, -7),
            NaiveDate::from_ymd_opt(2019, 6, 30).unwrap()
        );
        assert_eq!(
            shift_months(base, -8),
            NaiveDate::from_ymd_opt(2019, 5, 31).unwrap()
        );
        assert_eq!(
            shift_months(base, -9),
            NaiveDate::from_ymd_opt(2019, 4, 30).unwrap()
        );
        assert_eq!(
            shift_months(base, -10),
            NaiveDate::from_ymd_opt(2019, 3, 31).unwrap()
        );
        assert_eq!(
            shift_months(base, -11),
            NaiveDate::from_ymd_opt(2019, 2, 28).unwrap()
        );
        assert_eq!(
            shift_months(base, -12),
            NaiveDate::from_ymd_opt(2019, 1, 31).unwrap()
        );
        assert_eq!(
            shift_months(base, -13),
            NaiveDate::from_ymd_opt(2018, 12, 31).unwrap()
        );

        assert_eq!(
            shift_months(base, 1265),
            NaiveDate::from_ymd_opt(2125, 6, 30).unwrap()
        );
    }

    #[test]
    fn test_shift_months_with_overflow() {
        let base = NaiveDate::from_ymd_opt(2020, 12, 31).unwrap();

        assert_eq!(shift_months(base, 0), base);
        assert_eq!(
            shift_months(base, 1),
            NaiveDate::from_ymd_opt(2021, 1, 31).unwrap()
        );
        assert_eq!(
            shift_months(base, 2),
            NaiveDate::from_ymd_opt(2021, 2, 28).unwrap()
        );
        assert_eq!(
            shift_months(base, 12),
            NaiveDate::from_ymd_opt(2021, 12, 31).unwrap()
        );
        assert_eq!(
            shift_months(base, 18),
            NaiveDate::from_ymd_opt(2022, 6, 30).unwrap()
        );

        assert_eq!(
            shift_months(base, -1),
            NaiveDate::from_ymd_opt(2020, 11, 30).unwrap()
        );
        assert_eq!(
            shift_months(base, -2),
            NaiveDate::from_ymd_opt(2020, 10, 31).unwrap()
        );
        assert_eq!(
            shift_months(base, -10),
            NaiveDate::from_ymd_opt(2020, 2, 29).unwrap()
        );
        assert_eq!(
            shift_months(base, -12),
            NaiveDate::from_ymd_opt(2019, 12, 31).unwrap()
        );
        assert_eq!(
            shift_months(base, -18),
            NaiveDate::from_ymd_opt(2019, 6, 30).unwrap()
        );
    }

    #[test]
    fn test_shift_months_datetime() {
        let tz: Tz = "+00:00".parse().unwrap();
        let date = NaiveDate::from_ymd_opt(2020, 1, 31).unwrap();
        let o_clock = CivilTime::from_hms_nano_opt(1, 2, 3, 0).unwrap();
        let base = tz.from_local(CivilDateTime::new(date, o_clock)).unwrap();

        let shifted = |m| add_months_zoned(base, m, &tz).unwrap().local();
        assert_eq!(
            shifted(0).date,
            NaiveDate::from_ymd_opt(2020, 1, 31).unwrap()
        );
        assert_eq!(
            shifted(1).date,
            NaiveDate::from_ymd_opt(2020, 2, 29).unwrap()
        );
        assert_eq!(
            shifted(2).date,
            NaiveDate::from_ymd_opt(2020, 3, 31).unwrap()
        );
        assert_eq!(shifted(0).time, o_clock);
        assert_eq!(shifted(1).time, o_clock);
        assert_eq!(shifted(2).time, o_clock);
    }

    /// Zoned month/day shifts reproduce chrono's `DateTime` arithmetic
    #[test]
    fn test_zoned_shifts_match_chrono() {
        use chrono::TimeZone;
        let max = chrono::NaiveDateTime::MAX.and_utc().timestamp();
        let min = chrono::NaiveDateTime::MIN.and_utc().timestamp();
        let secs = [
            min,
            min + 3_600,
            -1,
            0,
            951_825_600,
            1_582_934_400,
            max - 3_600,
            max,
        ];
        let deltas = [0, 1, -1, 12, -13, 400, 1_000_000, i32::MAX, i32::MIN];
        for offset in [0, 7_200, -7_200, 86_340, -86_340] {
            let tz: Tz = crate::calendar::OffsetDisplay(offset)
                .to_string()
                .parse()
                .unwrap();
            let ctz = chrono::FixedOffset::east_opt(offset).unwrap();
            for s in secs {
                let utc = CivilDateTime::from_timestamp(s, 0).unwrap();
                let ours = tz.from_utc(utc).unwrap();
                let theirs = ctz.from_utc_datetime(
                    &chrono::DateTime::from_timestamp(s, 0).unwrap().naive_utc(),
                );
                let key = |z: Option<ZonedDateTime>| z.map(|z| z.utc.timestamp());
                let ckey =
                    |d: Option<chrono::DateTime<chrono::FixedOffset>>| d.map(|d| d.timestamp());
                for d in deltas {
                    let ctx = format!("{offset} {s} {d}");
                    assert_eq!(
                        key(add_months_zoned(ours, d, &tz)),
                        ckey(chrono_add_months(theirs, d)),
                        "{ctx}"
                    );
                    assert_eq!(
                        key(sub_months_zoned(ours, d, &tz)),
                        ckey(chrono_sub_months(theirs, d)),
                        "{ctx}"
                    );
                    assert_eq!(
                        key(add_days_zoned(ours, d, &tz)),
                        ckey(chrono_add_days(theirs, d)),
                        "{ctx}"
                    );
                    assert_eq!(
                        key(sub_days_zoned(ours, d, &tz)),
                        ckey(chrono_sub_days(theirs, d)),
                        "{ctx}"
                    );
                }
            }
        }
    }

    type ChronoDt = chrono::DateTime<chrono::FixedOffset>;

    fn chrono_add_months(dt: ChronoDt, months: i32) -> Option<ChronoDt> {
        use chrono::Months;
        match months.cmp(&0) {
            Ordering::Equal => Some(dt),
            Ordering::Greater => dt.checked_add_months(Months::new(months as u32)),
            Ordering::Less => dt.checked_sub_months(Months::new(months.unsigned_abs())),
        }
    }

    fn chrono_sub_months(dt: ChronoDt, months: i32) -> Option<ChronoDt> {
        use chrono::Months;
        match months.cmp(&0) {
            Ordering::Equal => Some(dt),
            Ordering::Greater => dt.checked_sub_months(Months::new(months as u32)),
            Ordering::Less => dt.checked_add_months(Months::new(months.unsigned_abs())),
        }
    }

    fn chrono_add_days(dt: ChronoDt, days: i32) -> Option<ChronoDt> {
        use chrono::Days;
        match days.cmp(&0) {
            Ordering::Equal => Some(dt),
            Ordering::Greater => dt.checked_add_days(Days::new(days as u64)),
            Ordering::Less => dt.checked_sub_days(Days::new(days.unsigned_abs() as u64)),
        }
    }

    fn chrono_sub_days(dt: ChronoDt, days: i32) -> Option<ChronoDt> {
        use chrono::Days;
        match days.cmp(&0) {
            Ordering::Equal => Some(dt),
            Ordering::Greater => dt.checked_sub_days(Days::new(days as u64)),
            Ordering::Less => dt.checked_add_days(Days::new(days.unsigned_abs() as u64)),
        }
    }

    #[test]
    fn test_date_arithmetic_matches_chrono() {
        let epoch = chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
        for days in [-1_000_000i64, -1, 0, 1, 20_000, 1_000_000] {
            let ours = add_signed_days(NaiveDate::UNIX_EPOCH, days, "+");
            let theirs = epoch + chrono::Duration::try_days(days).unwrap();
            assert_eq!(ours.unix_days(), (theirs - epoch).num_days());
        }
        for ms in [
            i64::MIN + 1,
            -86_400_001,
            -86_399_999,
            -1,
            0,
            86_400_000,
            i64::MAX,
        ] {
            let ours = date64_days(ms);
            let theirs = chrono::Duration::try_milliseconds(ms).unwrap().num_days();
            assert_eq!(ours, theirs, "{ms}");
        }
        for ns in [
            i64::MIN,
            -86_400_000_000_001,
            -1,
            0,
            86_400_000_000_000,
            i64::MAX,
        ] {
            assert_eq!(
                nanos_days(ns),
                chrono::Duration::nanoseconds(ns).num_days(),
                "{ns}"
            );
        }
    }

    #[test]
    #[should_panic(expected = "`NaiveDate + Months` out of range")]
    fn test_shift_months_overflow_panics() {
        shift_months(NaiveDate::MAX, 1);
    }

    #[test]
    #[should_panic(expected = "`NaiveDate - TimeDelta` overflowed")]
    fn test_sub_days_overflow_panics() {
        add_signed_days(NaiveDate::MIN, 1, "-");
    }
}
