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

//! Times of day, datetimes and fixed offsets with `chrono` 0.4 semantics.
//!
//! A [`CivilTime`] can represent a leap second as a fractional part of
//! `1_000_000_000 ..= 1_999_999_999` nanoseconds on second 59, exactly as
//! `chrono::NaiveTime` did. `jiff` cannot represent leap seconds, which is
//! why parsing and formatting operate on these types.

use super::date::{write_hundreds, CivilDate, UNIX_EPOCH_DAY};
use std::fmt;

const NANOS_PER_SEC: u32 = 1_000_000_000;

/// A time of day, possibly within a leap second
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct CivilTime {
    secs: u32,
    frac: u32,
}

impl CivilTime {
    /// `00:00:00`
    pub const MIDNIGHT: CivilTime = CivilTime { secs: 0, frac: 0 };

    /// Makes a time from hour, minute, second and nanosecond, if valid
    ///
    /// `nano` may be `1_000_000_000 ..= 1_999_999_999` to denote a leap
    /// second, but only when `sec == 59`.
    pub const fn from_hms_nano_opt(hour: u32, min: u32, sec: u32, nano: u32) -> Option<Self> {
        if (hour >= 24 || min >= 60 || sec >= 60)
            || (nano >= NANOS_PER_SEC && sec != 59)
            || nano >= 2 * NANOS_PER_SEC
        {
            return None;
        }
        Some(Self {
            secs: hour * 3600 + min * 60 + sec,
            frac: nano,
        })
    }

    /// Makes a time from seconds since midnight and a fractional part, if valid
    pub const fn from_num_seconds_from_midnight_opt(secs: u32, nano: u32) -> Option<Self> {
        if secs >= 86_400 || nano >= 2 * NANOS_PER_SEC || (nano >= NANOS_PER_SEC && secs % 60 != 59)
        {
            return None;
        }
        Some(Self { secs, frac: nano })
    }

    /// The hour, 0-23
    pub const fn hour(&self) -> u32 {
        self.secs / 3600
    }

    /// The minute, 0-59
    pub const fn minute(&self) -> u32 {
        self.secs / 60 % 60
    }

    /// The second, 0-59 (a leap second reports 59)
    pub const fn second(&self) -> u32 {
        self.secs % 60
    }

    /// Nanoseconds within the second, `>= 1_000_000_000` within a leap second
    pub const fn nanosecond(&self) -> u32 {
        self.frac
    }

    /// Seconds since midnight, excluding any leap second
    pub const fn num_seconds_from_midnight(&self) -> u32 {
        self.secs
    }

    /// Returns `(is_pm, hour)` with the hour on a 12-hour clock (1-12)
    pub const fn hour12(&self) -> (bool, u32) {
        let hour = self.hour();
        let hour12 = match hour % 12 {
            0 => 12,
            h => h,
        };
        (hour >= 12, hour12)
    }

    /// Adds `offset` seconds, returning the new time and the day carry
    const fn overflowing_add_offset(self, offset: i32) -> (Self, i32) {
        let secs = self.secs as i32 + offset;
        let time = Self {
            secs: secs.rem_euclid(86_400) as u32,
            frac: self.frac,
        };
        (time, secs.div_euclid(86_400))
    }
}

impl fmt::Debug for CivilTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use fmt::Write;
        let (sec, nano) = match self.frac >= NANOS_PER_SEC {
            true => (self.second() + 1, self.frac - NANOS_PER_SEC),
            false => (self.second(), self.frac),
        };
        write_hundreds(f, self.hour() as u8)?;
        f.write_char(':')?;
        write_hundreds(f, self.minute() as u8)?;
        f.write_char(':')?;
        write_hundreds(f, sec as u8)?;
        write_auto_si_fraction(f, nano)
    }
}

impl fmt::Display for CivilTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// Writes `.mmm`, `.uuuuuu` or `.nnnnnnnnn`, whichever is shortest exact, or
/// nothing for zero
pub(crate) fn write_auto_si_fraction(w: &mut (impl fmt::Write + ?Sized), nano: u32) -> fmt::Result {
    if nano == 0 {
        Ok(())
    } else if nano % 1_000_000 == 0 {
        write!(w, ".{:03}", nano / 1_000_000)
    } else if nano % 1_000 == 0 {
        write!(w, ".{:06}", nano / 1_000)
    } else {
        write!(w, ".{:09}", nano)
    }
}

/// A date and time of day without a time zone
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CivilDateTime {
    /// The date
    pub date: CivilDate,
    /// The time of day
    pub time: CivilTime,
}

impl CivilDateTime {
    /// Combines a date and a time
    pub const fn new(date: CivilDate, time: CivilTime) -> Self {
        Self { date, time }
    }

    /// Makes a UTC datetime from seconds since the epoch and nanoseconds
    ///
    /// Returns `None` outside the valid date range, or for `nanos` that are
    /// not a valid fraction (leap seconds only on second 59).
    pub fn from_timestamp(secs: i64, nanos: u32) -> Option<Self> {
        let days = secs.div_euclid(86_400);
        let secs_of_day = secs.rem_euclid(86_400) as u32;
        let date = CivilDate::from_unix_days(days)?;
        let time = CivilTime::from_num_seconds_from_midnight_opt(secs_of_day, nanos)?;
        Some(Self { date, time })
    }

    /// Makes a UTC datetime from a timestamp in units of `1 / base` seconds
    pub fn from_timestamp_scaled(v: i64, base: i64) -> Option<Self> {
        let secs = v.div_euclid(base);
        let sub = v.rem_euclid(base) as u32;
        Self::from_timestamp(secs, sub * (NANOS_PER_SEC / base as u32))
    }

    /// Seconds since the epoch, treating `self` as UTC (leap second excluded)
    pub const fn timestamp(&self) -> i64 {
        let days = self.date.num_days_from_ce() as i64 - UNIX_EPOCH_DAY;
        days * 86_400 + self.time.num_seconds_from_midnight() as i64
    }

    /// Milliseconds since the epoch (leap second fraction included)
    pub const fn timestamp_millis(&self) -> i64 {
        self.timestamp() * 1000 + (self.time.nanosecond() / 1_000_000) as i64
    }

    /// Microseconds since the epoch (leap second fraction included)
    pub const fn timestamp_micros(&self) -> i64 {
        self.timestamp() * 1_000_000 + (self.time.nanosecond() / 1_000) as i64
    }

    /// Nanoseconds since the epoch, `None` on `i64` overflow
    pub const fn timestamp_nanos_opt(&self) -> Option<i64> {
        let mut timestamp = self.timestamp();
        let mut subsec_nanos = self.time.nanosecond() as i64;
        if timestamp < 0 {
            subsec_nanos -= NANOS_PER_SEC as i64;
            timestamp += 1;
        }
        match timestamp.checked_mul(NANOS_PER_SEC as i64) {
            Some(v) => v.checked_add(subsec_nanos),
            None => None,
        }
    }

    /// Adds an offset, allowing the result one day beyond the valid range
    pub fn overflowing_add_offset(self, offset: i32) -> Self {
        let (time, days) = self.time.overflowing_add_offset(offset);
        let date = match days {
            -1 => self
                .date
                .pred_opt()
                .unwrap_or(CivilDate::from_days_unchecked(
                    self.date.num_days_from_ce() - 1,
                )),
            1 => self
                .date
                .succ_opt()
                .unwrap_or(CivilDate::from_days_unchecked(
                    self.date.num_days_from_ce() + 1,
                )),
            _ => self.date,
        };
        Self { date, time }
    }

    /// Subtracts an offset, returning `None` if the result is out of range
    pub fn checked_sub_offset(self, offset: i32) -> Option<Self> {
        let (time, days) = self.time.overflowing_add_offset(-offset);
        let date = match days {
            -1 => self.date.pred_opt()?,
            1 => self.date.succ_opt()?,
            _ => self.date,
        };
        Some(Self { date, time })
    }

    /// Adds signed nanoseconds to a datetime that is not within a leap
    /// second, returning `None` if the result is out of range
    pub fn checked_add_nanos(self, nanos: i128) -> Option<Self> {
        debug_assert!(self.time.nanosecond() < NANOS_PER_SEC);
        let base =
            self.timestamp() as i128 * NANOS_PER_SEC as i128 + self.time.nanosecond() as i128;
        let total = base.checked_add(nanos)?;
        let secs = i64::try_from(total.div_euclid(NANOS_PER_SEC as i128)).ok()?;
        let sub = total.rem_euclid(NANOS_PER_SEC as i128) as u32;
        Self::from_timestamp(secs, sub)
    }
}

impl fmt::Debug for CivilDateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}T{:?}", self.date, self.time)
    }
}

impl fmt::Display for CivilDateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.date, self.time)
    }
}

/// Returns `secs` if it is a valid fixed offset (strictly within ±24h)
pub const fn fixed_offset_opt(secs: i32) -> Option<i32> {
    match -86_400 < secs && secs < 86_400 {
        true => Some(secs),
        false => None,
    }
}

/// Displays a fixed offset as `+HH:MM` or `+HH:MM:SS`
#[derive(Copy, Clone, Debug)]
pub struct OffsetDisplay(pub i32);

impl fmt::Display for OffsetDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (sign, offset) = match self.0 < 0 {
            true => ('-', -self.0),
            false => ('+', self.0),
        };
        let sec = offset.rem_euclid(60);
        let mins = offset.div_euclid(60);
        let min = mins.rem_euclid(60);
        let hour = mins.div_euclid(60);
        match sec {
            0 => write!(f, "{}{:02}:{:02}", sign, hour, min),
            _ => write!(f, "{}{:02}:{:02}:{:02}", sign, hour, min, sec),
        }
    }
}

/// A UTC datetime paired with the offset of the zone it is viewed in
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ZonedDateTime {
    /// The UTC datetime
    pub utc: CivilDateTime,
    /// The offset from UTC in seconds
    pub offset: i32,
}

impl ZonedDateTime {
    /// The local datetime, possibly one day beyond the valid range
    pub fn local(&self) -> CivilDateTime {
        self.utc.overflowing_add_offset(self.offset)
    }
}

/// Largest `|seconds|` a duration may hold (`i64::MAX` milliseconds)
const MAX_DURATION_SECS: i64 = i64::MAX / 1000;

/// A signed duration with `chrono::TimeDelta` range and normalisation
///
/// `secs` is floored and `nanos` is in `0..1_000_000_000`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Duration {
    secs: i64,
    nanos: u32,
}

impl Duration {
    /// Seconds, `None` beyond `±i64::MAX` milliseconds (as `TimeDelta::try_seconds`)
    pub const fn try_seconds(secs: i64) -> Option<Self> {
        match secs > MAX_DURATION_SECS || secs < -MAX_DURATION_SECS {
            true => None,
            false => Some(Self { secs, nanos: 0 }),
        }
    }

    /// Milliseconds, `None` only for `i64::MIN` (as `TimeDelta::try_milliseconds`)
    pub const fn try_milliseconds(ms: i64) -> Option<Self> {
        match ms == i64::MIN {
            true => None,
            false => Some(Self::from_scaled(ms, 1_000)),
        }
    }

    /// Microseconds (always in range)
    pub const fn microseconds(us: i64) -> Self {
        Self::from_scaled(us, 1_000_000)
    }

    /// Nanoseconds (always in range)
    pub const fn nanoseconds(ns: i64) -> Self {
        Self::from_scaled(ns, 1_000_000_000)
    }

    const fn from_scaled(v: i64, base: i64) -> Self {
        Self {
            secs: v.div_euclid(base),
            nanos: (v.rem_euclid(base) * (1_000_000_000 / base)) as u32,
        }
    }

    /// Whole seconds (floored) and non-negative nanosecond remainder
    pub const fn parts(&self) -> (i64, u32) {
        (self.secs, self.nanos)
    }

    fn abs(&self) -> Self {
        match (self.secs < 0, self.nanos) {
            (false, _) => *self,
            (true, 0) => Self {
                secs: -self.secs,
                nanos: 0,
            },
            (true, n) => Self {
                secs: -self.secs - 1,
                nanos: NANOS_PER_SEC - n,
            },
        }
    }
}

/// ISO 8601 rendering identical to `chrono::TimeDelta`'s `Display`
impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.secs < 0 { "-" } else { "" };
        let abs = self.abs();
        write!(f, "{}P", sign)?;
        if abs.secs == 0 && abs.nanos == 0 {
            return f.write_str("0D");
        }
        write!(f, "T{}", abs.secs)?;
        if abs.nanos > 0 {
            let mut figures = 9usize;
            let mut fraction = abs.nanos;
            while fraction % 10 == 0 {
                fraction /= 10;
                figures -= 1;
            }
            write!(f, ".{:01$}", fraction, figures)?;
        }
        f.write_str("S")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chrono_dt(v: &CivilDateTime) -> chrono::NaiveDateTime {
        let d = chrono::NaiveDate::from_num_days_from_ce_opt(v.date.num_days_from_ce()).unwrap();
        let t = chrono::NaiveTime::from_num_seconds_from_midnight_opt(
            v.time.num_seconds_from_midnight(),
            v.time.nanosecond(),
        )
        .unwrap();
        d.and_time(t)
    }

    #[test]
    fn test_from_timestamp_matches_chrono() {
        let max = chrono::NaiveDateTime::MAX.and_utc().timestamp();
        let min = chrono::NaiveDateTime::MIN.and_utc().timestamp();
        let secs = [
            i64::MIN,
            min - 1,
            min,
            -1,
            0,
            1,
            951_782_400,
            max,
            max + 1,
            i64::MAX,
        ];
        for s in secs {
            for n in [
                0,
                1,
                999_999_999,
                1_000_000_000,
                1_999_999_999,
                2_000_000_000,
            ] {
                let ours = CivilDateTime::from_timestamp(s, n);
                let theirs = chrono::DateTime::from_timestamp(s, n).map(|d| d.naive_utc());
                assert_eq!(ours.map(|d| chrono_dt(&d)), theirs, "{s} {n}");
                let Some(ours) = ours else { continue };
                let theirs = theirs.unwrap().and_utc();
                assert_eq!(ours.timestamp(), theirs.timestamp());
                assert_eq!(ours.timestamp_millis(), theirs.timestamp_millis());
                assert_eq!(ours.timestamp_micros(), theirs.timestamp_micros());
                assert_eq!(ours.timestamp_nanos_opt(), theirs.timestamp_nanos_opt());
                assert_eq!(format!("{ours:?}"), format!("{:?}", theirs.naive_utc()));
                assert_eq!(format!("{ours}"), format!("{}", theirs.naive_utc()));
            }
        }
    }

    #[test]
    fn test_time_matches_chrono() {
        for secs in [
            0,
            1,
            59,
            3599,
            43_199,
            43_200,
            86_339,
            86_399,
            86_400,
            u32::MAX,
        ] {
            for nano in [
                0,
                1,
                1000,
                1_000_000,
                999_999_999,
                1_000_000_000,
                1_500_000_000,
            ] {
                let ours = CivilTime::from_num_seconds_from_midnight_opt(secs, nano);
                let theirs = chrono::NaiveTime::from_num_seconds_from_midnight_opt(secs, nano);
                assert_eq!(ours.is_some(), theirs.is_some());
                let (Some(a), Some(b)) = (ours, theirs) else {
                    continue;
                };
                assert_eq!(format!("{a:?}"), format!("{b:?}"));
                use chrono::Timelike;
                assert_eq!(a.hour12(), b.hour12());
            }
        }
    }

    #[test]
    fn test_offset_display_matches_chrono() {
        for secs in [-86_399, -3600, -1, 0, 1, 59, 60, 3600, 19_800, 86_399] {
            let theirs = chrono::FixedOffset::east_opt(secs).unwrap();
            assert_eq!(OffsetDisplay(secs).to_string(), theirs.to_string());
        }
        for secs in [-86_400, 86_400, i32::MIN, i32::MAX] {
            assert_eq!(fixed_offset_opt(secs), None);
            assert_eq!(chrono::FixedOffset::east_opt(secs), None);
        }
    }

    #[test]
    fn test_duration_matches_chrono() {
        let values = [
            i64::MIN,
            i64::MIN + 1,
            -MAX_DURATION_SECS - 1,
            -MAX_DURATION_SECS,
            -1_500_000_001,
            -1,
            0,
            1,
            999,
            1_000,
            1_234_000,
            MAX_DURATION_SECS,
            MAX_DURATION_SECS + 1,
            i64::MAX,
        ];
        for v in values {
            let pairs = [
                (Duration::try_seconds(v), chrono::Duration::try_seconds(v)),
                (
                    Duration::try_milliseconds(v),
                    chrono::Duration::try_milliseconds(v),
                ),
                (
                    Some(Duration::microseconds(v)),
                    Some(chrono::Duration::microseconds(v)),
                ),
                (
                    Some(Duration::nanoseconds(v)),
                    Some(chrono::Duration::nanoseconds(v)),
                ),
            ];
            for (ours, theirs) in pairs {
                assert_eq!(ours.is_some(), theirs.is_some(), "{v}");
                let (Some(a), Some(b)) = (ours, theirs) else {
                    continue;
                };
                assert_eq!(a.to_string(), b.to_string(), "{v}");
            }
        }
    }
}
