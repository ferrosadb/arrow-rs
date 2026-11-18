// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

//! Proleptic Gregorian dates with the value range arrow has always exposed.
//!
//! The valid range is `-262143-01-01 ..= +262142-12-31`, the range of the
//! `chrono` 0.4 `NaiveDate` type this module replaces. Validation happens at
//! the same points `chrono` validated, so overflow / `None` behaviour is
//! unchanged.

use std::fmt;

/// Smallest valid year
pub const MIN_YEAR: i32 = -262_143;
/// Largest valid year
pub const MAX_YEAR: i32 = 262_142;

/// Days from 0001-01-01 (day 1) to 1970-01-01
pub(crate) const UNIX_EPOCH_DAY: i64 = 719_163;

/// Day of the week
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Weekday {
    /// Monday
    Mon = 0,
    /// Tuesday
    Tue = 1,
    /// Wednesday
    Wed = 2,
    /// Thursday
    Thu = 3,
    /// Friday
    Fri = 4,
    /// Saturday
    Sat = 5,
    /// Sunday
    Sun = 6,
}

impl Weekday {
    const ALL: [Weekday; 7] = [
        Weekday::Mon,
        Weekday::Tue,
        Weekday::Wed,
        Weekday::Thu,
        Weekday::Fri,
        Weekday::Sat,
        Weekday::Sun,
    ];

    /// Day number from Monday (`Mon == 0`)
    pub const fn num_days_from_monday(self) -> u32 {
        self as u32
    }

    /// Day number from Sunday (`Sun == 0`)
    pub const fn num_days_from_sunday(self) -> u32 {
        (self as u32 + 1) % 7
    }

    /// Day number from Monday, one-based (`Mon == 1`)
    pub const fn number_from_monday(self) -> u32 {
        self as u32 + 1
    }

    /// Days from `other` forward to `self`, in `0..7`
    pub const fn days_since(self, other: Weekday) -> u32 {
        (self as u32 + 7 - other as u32) % 7
    }

    /// Weekday from a day number counted from Monday (`0 == Mon`), modulo 7
    pub(crate) fn from_monday0(n: i64) -> Weekday {
        Self::ALL[n.rem_euclid(7) as usize]
    }
}

/// Converts a civil date to days since 1970-01-01 (H. Hinnant's algorithm)
const fn unix_days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (month as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Converts days since 1970-01-01 to a civil date (H. Hinnant's algorithm)
const fn civil_from_unix_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + (month <= 2) as i64;
    (year, month, day)
}

/// Returns true if `year` is a leap year in the proleptic Gregorian calendar
pub const fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

const fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        2 if is_leap_year(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

const MIN_DAYS: i32 = (unix_days_from_civil(MIN_YEAR as i64, 1, 1) + UNIX_EPOCH_DAY) as i32;
const MAX_DAYS: i32 = (unix_days_from_civil(MAX_YEAR as i64, 12, 31) + UNIX_EPOCH_DAY) as i32;

/// A proleptic Gregorian date without a time zone
///
/// Internally the number of days since `0001-12-31 BCE` (so `0001-01-01` is
/// day 1). Values one day beyond [`CivilDate::MIN`] / [`CivilDate::MAX`] may
/// exist transiently when a valid UTC datetime is shifted by an offset into
/// local time, exactly as `chrono` permitted.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CivilDate {
    days: i32,
}

impl CivilDate {
    /// The smallest valid date, `-262143-01-01`
    pub const MIN: CivilDate = CivilDate { days: MIN_DAYS };
    /// The largest valid date, `+262142-12-31`
    pub const MAX: CivilDate = CivilDate { days: MAX_DAYS };
    /// `1970-01-01`
    pub const UNIX_EPOCH: CivilDate = CivilDate {
        days: UNIX_EPOCH_DAY as i32,
    };

    /// Wraps a day number without range validation (headroom dates)
    pub(crate) const fn from_days_unchecked(days: i32) -> Self {
        Self { days }
    }

    /// Returns true if within [`CivilDate::MIN`] ..= [`CivilDate::MAX`]
    pub const fn is_valid(&self) -> bool {
        self.days >= MIN_DAYS && self.days <= MAX_DAYS
    }

    /// Makes a date from days since CE (`0001-01-01` is day 1), if in range
    pub const fn from_num_days_from_ce_opt(days: i32) -> Option<Self> {
        let date = Self { days };
        match date.is_valid() {
            true => Some(date),
            false => None,
        }
    }

    /// Makes a date from days since CE given as `i64`, if in range
    pub fn from_days_i64(days: i64) -> Option<Self> {
        Self::from_num_days_from_ce_opt(i32::try_from(days).ok()?)
    }

    /// Makes a date from days since `1970-01-01`, if in range
    pub fn from_unix_days(days: i64) -> Option<Self> {
        Self::from_days_i64(days.checked_add(UNIX_EPOCH_DAY)?)
    }

    /// Makes a date from year, month (1-12) and day (1-31), if valid
    pub fn from_ymd_opt(year: i32, month: u32, day: u32) -> Option<Self> {
        if !(MIN_YEAR..=MAX_YEAR).contains(&year) || !(1..=12).contains(&month) {
            return None;
        }
        if day == 0 || day > days_in_month(year, month) {
            return None;
        }
        let days = unix_days_from_civil(year as i64, month, day) + UNIX_EPOCH_DAY;
        Some(Self { days: days as i32 })
    }

    /// Makes a date from year and day of year (1-366), if valid
    pub fn from_yo_opt(year: i32, ordinal: u32) -> Option<Self> {
        if !(MIN_YEAR..=MAX_YEAR).contains(&year) {
            return None;
        }
        let ndays = 365 + is_leap_year(year) as u32;
        if ordinal == 0 || ordinal > ndays {
            return None;
        }
        let jan1 = unix_days_from_civil(year as i64, 1, 1) + UNIX_EPOCH_DAY;
        Some(Self {
            days: (jan1 + ordinal as i64 - 1) as i32,
        })
    }

    /// Makes a date from an ISO 8601 week date, if valid
    pub fn from_isoywd_opt(year: i32, week: u32, weekday: Weekday) -> Option<Self> {
        // The year itself is not range checked: the result may fall in an
        // adjacent, valid year (as in chrono).
        if week == 0 || week > iso_weeks_in_year(year) {
            return None;
        }
        let jan4 = unix_days_from_civil(year as i64, 1, 4) + UNIX_EPOCH_DAY;
        let jan4_wd = Self::from_days_unchecked(jan4 as i32).weekday();
        let week1_monday = jan4 - jan4_wd.num_days_from_monday() as i64;
        let days = week1_monday + (week as i64 - 1) * 7 + weekday.num_days_from_monday() as i64;
        Self::from_days_i64(days)
    }

    /// Days since CE, `0001-01-01` being day 1
    pub const fn num_days_from_ce(&self) -> i32 {
        self.days
    }

    /// Days since `1970-01-01`
    pub const fn unix_days(&self) -> i64 {
        self.days as i64 - UNIX_EPOCH_DAY
    }

    fn ymd(&self) -> (i32, u32, u32) {
        let (y, m, d) = civil_from_unix_days(self.unix_days());
        (y as i32, m, d)
    }

    /// The year (may be negative)
    pub fn year(&self) -> i32 {
        self.ymd().0
    }

    /// The month, 1-12
    pub fn month(&self) -> u32 {
        self.ymd().1
    }

    /// The day of month, 1-31
    pub fn day(&self) -> u32 {
        self.ymd().2
    }

    /// The day of year, 1-366
    pub fn ordinal(&self) -> u32 {
        let year = self.year() as i64;
        let jan1 = unix_days_from_civil(year, 1, 1);
        (self.unix_days() - jan1 + 1) as u32
    }

    /// The day of the week
    pub fn weekday(&self) -> Weekday {
        // 1970-01-01 was a Thursday
        Weekday::from_monday0(self.unix_days() + 3)
    }

    /// Returns `(iso_year, iso_week)` per ISO 8601
    pub fn iso_week(&self) -> (i32, u32) {
        let year = self.year();
        let wd = self.weekday().number_from_monday() as i32;
        let week = (self.ordinal() as i32 - wd + 10) / 7;
        if week < 1 {
            (year - 1, iso_weeks_in_year(year - 1))
        } else if week as u32 > iso_weeks_in_year(year) {
            (year + 1, 1)
        } else {
            (year, week as u32)
        }
    }

    /// Week number (0-53) where weeks start on `day`, as `%U` / `%W`
    pub fn weeks_from(&self, day: Weekday) -> i32 {
        (self.ordinal() as i32 - self.weekday().days_since(day) as i32 + 6) / 7
    }

    /// Adds days, returning `None` if the result is out of range
    pub fn checked_add_days_i64(self, days: i64) -> Option<Self> {
        Self::from_days_i64((self.days as i64).checked_add(days)?)
    }

    /// `chrono`'s `NaiveDate::add_days`: results that stay within the same
    /// year skip range validation (relevant only for headroom dates).
    pub(crate) fn add_days_headroom(self, days: i32) -> Option<Self> {
        let year = self.year();
        let ndays = 365 + is_leap_year(year) as i64;
        let ordinal = self.ordinal() as i64 + days as i64;
        if ordinal > 0 && ordinal <= ndays {
            return Some(Self::from_days_unchecked(self.days + days));
        }
        self.checked_add_days_i64(days as i64)
    }

    /// Next day; same-year results skip range validation like `chrono`
    pub(crate) fn succ_opt(self) -> Option<Self> {
        self.add_days_headroom(1)
    }

    /// Previous day; same-year results skip range validation like `chrono`
    pub(crate) fn pred_opt(self) -> Option<Self> {
        self.add_days_headroom(-1)
    }

    /// Shifts by `months`, clamping the day to the end of the target month
    ///
    /// Equivalent to `chrono`'s `NaiveDate::checked_{add,sub}_months`: a zero
    /// shift returns `self` unvalidated, and `|months| > i32::MAX` is `None`.
    pub fn checked_add_months(self, months: i64) -> Option<Self> {
        if months == 0 {
            return Some(self);
        }
        let months =
            i32::try_from(months.unsigned_abs())
                .ok()
                .map(|m| if months < 0 { -m } else { m })?;
        let (year, month, day) = self.ymd();
        let total = (year * 12 + month as i32 - 1).checked_add(months)?;
        let new_year = total.div_euclid(12);
        let new_month = total.rem_euclid(12) as u32 + 1;
        let day = day.min(days_in_month(new_year, new_month));
        Self::from_ymd_opt(new_year, new_month, day)
    }
}

/// Number of ISO weeks (52 or 53) in `year`
fn iso_weeks_in_year(year: i32) -> u32 {
    let jan1 = unix_days_from_civil(year as i64, 1, 1);
    let jan1_wd = Weekday::from_monday0(jan1 + 3);
    match (jan1_wd, is_leap_year(year)) {
        (Weekday::Thu, _) | (Weekday::Wed, true) => 53,
        _ => 52,
    }
}

/// Writes a two-digit number, failing for values >= 100 (`chrono` semantics)
pub(crate) fn write_hundreds(w: &mut (impl fmt::Write + ?Sized), n: u8) -> fmt::Result {
    if n >= 100 {
        return Err(fmt::Error);
    }
    w.write_char((b'0' + n / 10) as char)?;
    w.write_char((b'0' + n % 10) as char)
}

/// Writes an ISO 8601 year, with an explicit sign outside `0..=9999`
pub(crate) fn write_iso_year(w: &mut (impl fmt::Write + ?Sized), year: i32) -> fmt::Result {
    if (0..=9999).contains(&year) {
        write_hundreds(w, (year / 100) as u8)?;
        write_hundreds(w, (year % 100) as u8)
    } else {
        write!(w, "{:+05}", year)
    }
}

impl fmt::Debug for CivilDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use fmt::Write;
        let (year, month, day) = self.ymd();
        write_iso_year(f, year)?;
        f.write_char('-')?;
        write_hundreds(f, month as u8)?;
        f.write_char('-')?;
        write_hundreds(f, day as u8)
    }
}

impl fmt::Display for CivilDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    fn oracle(days: i32) -> Option<chrono::NaiveDate> {
        chrono::NaiveDate::from_num_days_from_ce_opt(days)
    }

    /// Sample day numbers densely around interesting points and sparsely elsewhere
    fn sample_days() -> Vec<i32> {
        let mut out = vec![];
        for centre in [MIN_DAYS, MAX_DAYS, 0, 1, UNIX_EPOCH_DAY as i32, 730_000] {
            for d in -800..800 {
                out.push(centre.saturating_add(d));
            }
        }
        out.extend((MIN_DAYS..MAX_DAYS).step_by(9_973));
        out
    }

    #[test]
    fn test_fields_match_chrono() {
        for days in sample_days() {
            let ours = CivilDate::from_num_days_from_ce_opt(days);
            let theirs = oracle(days);
            assert_eq!(ours.is_some(), theirs.is_some(), "{days}");
            let (Some(a), Some(b)) = (ours, theirs) else {
                continue;
            };
            assert_eq!(a.year(), b.year(), "{days}");
            assert_eq!(a.month(), b.month(), "{days}");
            assert_eq!(a.day(), b.day(), "{days}");
            assert_eq!(a.ordinal(), b.ordinal(), "{days}");
            let wd = a.weekday().num_days_from_monday();
            assert_eq!(wd, b.weekday().num_days_from_monday(), "{days}");
            let iso = b.iso_week();
            assert_eq!(a.iso_week(), (iso.year(), iso.week()), "{days}");
            assert_eq!(format!("{a:?}"), format!("{b:?}"), "{days}");
        }
    }

    #[test]
    fn test_constructors_match_chrono() {
        let years = [
            MIN_YEAR - 1,
            MIN_YEAR,
            -1,
            0,
            1,
            1969,
            2000,
            2024,
            MAX_YEAR,
            MAX_YEAR + 1,
        ];
        for year in years {
            for month in 0..=13 {
                for day in [0, 1, 28, 29, 30, 31, 32] {
                    let ours = CivilDate::from_ymd_opt(year, month, day);
                    let theirs = chrono::NaiveDate::from_ymd_opt(year, month, day);
                    let theirs = theirs.map(|d| d.num_days_from_ce());
                    assert_eq!(ours.map(|d| d.num_days_from_ce()), theirs);
                }
            }
            for ordinal in [0, 1, 59, 60, 365, 366, 367] {
                let ours = CivilDate::from_yo_opt(year, ordinal).map(|d| d.days);
                let theirs = chrono::NaiveDate::from_yo_opt(year, ordinal);
                assert_eq!(ours, theirs.map(|d| d.num_days_from_ce()));
            }
            for week in 0..=54 {
                for (wd, cwd) in Weekday::ALL.iter().zip([
                    chrono::Weekday::Mon,
                    chrono::Weekday::Tue,
                    chrono::Weekday::Wed,
                    chrono::Weekday::Thu,
                    chrono::Weekday::Fri,
                    chrono::Weekday::Sat,
                    chrono::Weekday::Sun,
                ]) {
                    let ours = CivilDate::from_isoywd_opt(year, week, *wd).map(|d| d.days);
                    let theirs = chrono::NaiveDate::from_isoywd_opt(year, week, cwd);
                    assert_eq!(ours, theirs.map(|d| d.num_days_from_ce()), "{year} {week}");
                }
            }
        }
    }

    #[test]
    fn test_months_match_chrono() {
        let months = [0, 1, -1, 11, 13, -13, 1200, -1200, i32::MAX, i32::MIN + 1];
        for days in sample_days().into_iter().step_by(7) {
            let (Some(a), Some(b)) = (CivilDate::from_num_days_from_ce_opt(days), oracle(days))
            else {
                continue;
            };
            for m in months {
                let ours = a.checked_add_months(m as i64).map(|d| d.days);
                let theirs = match m >= 0 {
                    true => b.checked_add_months(chrono::Months::new(m as u32)),
                    false => b.checked_sub_months(chrono::Months::new(m.unsigned_abs())),
                };
                assert_eq!(ours, theirs.map(|d| d.num_days_from_ce()), "{days} {m}");
            }
        }
    }
}
