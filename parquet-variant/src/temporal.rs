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

//! Conversions between the Variant temporal encodings and [`jiff`] types.
//!
//! The Variant binary format stores
//! - `Date` as days since the Unix epoch (`i32`),
//! - `Timestamp*Micros` as microseconds since the Unix epoch (`i64`),
//! - `Timestamp*Nanos` as nanoseconds since the Unix epoch (`i64`),
//! - `Time` as microseconds since midnight (`i64`, valid range `0..86_400_000_000`).
//!
//! jiff supports years `-9999..=9999` (a [`Timestamp`] is slightly narrower:
//! `-009999-01-02T01:59:59Z..=9999-12-30T22:00:00.999999999Z`). Encoded values
//! outside that range are rejected with an explicit error rather than clamped.
//!
//! The `Debug` wrappers at the bottom reproduce the text that `chrono`'s
//! `Debug` implementations produced before this crate moved to jiff, so the
//! `Debug` output of [`crate::Variant`] is unchanged.

use arrow_schema::ArrowError;
use jiff::civil::{Date, DateTime, Time};
use jiff::{SignedDuration, Timestamp};
use std::fmt;

/// `1970-01-01T00:00:00` as a civil datetime: the origin of the NTZ encodings.
const UNIX_EPOCH_DATETIME: DateTime = DateTime::constant(1970, 1, 1, 0, 0, 0, 0);

const SECONDS_PER_DAY: i64 = 86_400;
const MICROS_PER_DAY: u64 = 86_400_000_000;
const NANOS_PER_MICRO: i128 = 1_000;

// ---- decode -----------------------------------------------------------------------

/// Days since the Unix epoch to a civil date.
pub(crate) fn date_from_days(days_since_epoch: i32) -> Result<Date, ArrowError> {
    let offset = SignedDuration::from_secs(i64::from(days_since_epoch) * SECONDS_PER_DAY);
    UNIX_EPOCH_DATETIME
        .checked_add(offset)
        .map(|dt| dt.date())
        .map_err(|e| {
            ArrowError::CastError(format!(
                "Could not cast `{days_since_epoch}` days into a jiff::civil::Date: {e}"
            ))
        })
}

/// Microseconds since the Unix epoch to a UTC timestamp.
///
/// Goes through `from_nanosecond` rather than `from_microsecond`: jiff 0.2.46's
/// `from_microsecond` caps at `9999-12-30T22:00:00Z` exactly, rejecting the last
/// second of microseconds before `Timestamp::MAX` that a `Timestamp` can hold.
pub(crate) fn timestamp_from_micros(micros_since_epoch: i64) -> Result<Timestamp, ArrowError> {
    let nanos = i128::from(micros_since_epoch) * NANOS_PER_MICRO;
    Timestamp::from_nanosecond(nanos).map_err(|e| {
        ArrowError::CastError(format!(
            "Could not cast `{micros_since_epoch}` microseconds into a jiff::Timestamp: {e}"
        ))
    })
}

/// Nanoseconds since the Unix epoch to a UTC timestamp.
///
/// Every `i64` nanosecond count (years 1677..=2262) is inside jiff's range, so
/// this does not fail in practice; the error path is kept rather than unwrapped.
pub(crate) fn timestamp_from_nanos(nanos_since_epoch: i64) -> Result<Timestamp, ArrowError> {
    Timestamp::from_nanosecond(i128::from(nanos_since_epoch)).map_err(|e| {
        ArrowError::CastError(format!(
            "Could not cast `{nanos_since_epoch}` nanoseconds into a jiff::Timestamp: {e}"
        ))
    })
}

/// Microseconds since the Unix epoch to a civil (no time zone) datetime.
pub(crate) fn datetime_from_micros(micros_since_epoch: i64) -> Result<DateTime, ArrowError> {
    UNIX_EPOCH_DATETIME
        .checked_add(SignedDuration::from_micros(micros_since_epoch))
        .map_err(|e| {
            ArrowError::CastError(format!(
                "Could not cast `{micros_since_epoch}` microseconds into a jiff::civil::DateTime: {e}"
            ))
        })
}

/// Nanoseconds since the Unix epoch to a civil (no time zone) datetime.
///
/// Like [`timestamp_from_nanos`], this cannot fail for an `i64` input in practice.
pub(crate) fn datetime_from_nanos(nanos_since_epoch: i64) -> Result<DateTime, ArrowError> {
    UNIX_EPOCH_DATETIME
        .checked_add(SignedDuration::from_nanos(nanos_since_epoch))
        .map_err(|e| {
            ArrowError::CastError(format!(
                "Could not cast `{nanos_since_epoch}` nanoseconds into a jiff::civil::DateTime: {e}"
            ))
        })
}

/// Microseconds since midnight to a civil time. Rejects values of a day or more.
pub(crate) fn time_from_micros(micros_since_midnight: u64) -> Result<Time, ArrowError> {
    let cast_error = || {
        ArrowError::CastError(format!(
            "Could not cast {micros_since_midnight} microseconds into a jiff::civil::Time"
        ))
    };
    if micros_since_midnight >= MICROS_PER_DAY {
        return Err(cast_error());
    }
    // Every component below is bounded by the check above, so the narrowing
    // conversions cannot fail; `try_from` keeps that assumption checked.
    let secs = micros_since_midnight / 1_000_000;
    let subsec_nanos = (micros_since_midnight % 1_000_000) * 1_000;
    let hour = i8::try_from(secs / 3_600).map_err(|_| cast_error())?;
    let minute = i8::try_from(secs / 60 % 60).map_err(|_| cast_error())?;
    let second = i8::try_from(secs % 60).map_err(|_| cast_error())?;
    let nanos = i32::try_from(subsec_nanos).map_err(|_| cast_error())?;
    Time::new(hour, minute, second, nanos).map_err(|_| cast_error())
}

// ---- encode -----------------------------------------------------------------------

/// Civil date to days since the Unix epoch.
pub(crate) fn date_to_days(value: Date) -> i32 {
    let secs = value
        .to_datetime(Time::midnight())
        .duration_since(UNIX_EPOCH_DATETIME)
        .as_secs();
    debug_assert_eq!(
        secs % SECONDS_PER_DAY,
        0,
        "a date is a whole number of days"
    );
    // jiff dates span about +-4.4 million days, far inside i32.
    i32::try_from(secs / SECONDS_PER_DAY).expect("jiff date range fits in i32 days")
}

/// Floors a nanosecond count to microseconds (toward negative infinity), which
/// is how the previous chrono implementation truncated sub-microsecond digits.
fn floor_nanos_to_micros(nanos: i128) -> i64 {
    // jiff's range is about +-3.2e17 microseconds, far inside i64.
    i64::try_from(nanos.div_euclid(NANOS_PER_MICRO)).expect("jiff range fits in i64 microseconds")
}

/// UTC timestamp to microseconds since the Unix epoch (sub-microsecond digits floored).
pub(crate) fn timestamp_to_micros(value: Timestamp) -> i64 {
    floor_nanos_to_micros(value.as_nanosecond())
}

/// UTC timestamp to nanoseconds since the Unix epoch.
///
/// # Panics
///
/// If the timestamp is outside the `i64` nanosecond range
/// (`1677-09-21T00:12:43.145224192Z..=2262-04-11T23:47:16.854775807Z`), matching
/// the previous chrono implementation, which also panicked here.
pub(crate) fn timestamp_to_nanos(value: Timestamp) -> i64 {
    let nanos = value.as_nanosecond();
    i64::try_from(nanos).unwrap_or_else(|_| {
        panic!("timestamp {value} ({nanos} ns) does not fit the i64 nanosecond encoding")
    })
}

/// Civil datetime to microseconds since the Unix epoch (sub-microsecond digits floored).
pub(crate) fn datetime_to_micros(value: DateTime) -> i64 {
    floor_nanos_to_micros(value.duration_since(UNIX_EPOCH_DATETIME).as_nanos())
}

/// Civil datetime to nanoseconds since the Unix epoch.
///
/// # Panics
///
/// If the datetime is outside the `i64` nanosecond range, matching the
/// previous chrono implementation, which also panicked here.
pub(crate) fn datetime_to_nanos(value: DateTime) -> i64 {
    let nanos = value.duration_since(UNIX_EPOCH_DATETIME).as_nanos();
    i64::try_from(nanos).unwrap_or_else(|_| {
        panic!("datetime {value} ({nanos} ns) does not fit the i64 nanosecond encoding")
    })
}

/// Civil time to microseconds since midnight (sub-microsecond digits truncated).
pub(crate) fn time_to_micros(value: Time) -> u64 {
    let nanos = value.duration_since(Time::midnight()).as_nanos();
    debug_assert!((0..86_400_000_000_000).contains(&nanos));
    u64::try_from(nanos / NANOS_PER_MICRO).expect("a civil time is within one day")
}

// ---- chrono-compatible Debug text ---------------------------------------------------

/// Formats a date the way `chrono::NaiveDate`'s `Debug` did: `YYYY-MM-DD` for
/// years `0..=9999`, otherwise a signed, at-least-four-digit year (`-0001-01-01`).
pub(crate) struct DateDebug(pub(crate) Date);

/// Formats a time the way `chrono::NaiveTime`'s `Debug` did: `HH:MM:SS`, then
/// the fraction (if non-zero) as 3, 6 or 9 digits, whichever is the shortest exact one.
pub(crate) struct TimeDebug(pub(crate) Time);

/// Formats a datetime the way `chrono::NaiveDateTime`'s `Debug` did: `<date>T<time>`.
pub(crate) struct DateTimeDebug(pub(crate) DateTime);

/// Formats a timestamp the way `chrono::DateTime<Utc>`'s `Debug` did: `<date>T<time>Z`.
pub(crate) struct TimestampDebug(pub(crate) Timestamp);

impl fmt::Debug for DateDebug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let year = self.0.year();
        if (0..=9999).contains(&year) {
            write!(f, "{year:04}")?;
        } else {
            write!(f, "{year:+05}")?;
        }
        write!(f, "-{:02}-{:02}", self.0.month(), self.0.day())
    }
}

impl fmt::Debug for TimeDebug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let t = self.0;
        write!(f, "{:02}:{:02}:{:02}", t.hour(), t.minute(), t.second())?;
        let nanos = t.subsec_nanosecond();
        if nanos == 0 {
            Ok(())
        } else if nanos % 1_000_000 == 0 {
            write!(f, ".{:03}", nanos / 1_000_000)
        } else if nanos % 1_000 == 0 {
            write!(f, ".{:06}", nanos / 1_000)
        } else {
            write!(f, ".{nanos:09}")
        }
    }
}

impl fmt::Debug for DateTimeDebug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?}T{:?}",
            DateDebug(self.0.date()),
            TimeDebug(self.0.time())
        )
    }
}

impl fmt::Debug for TimestampDebug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let utc = jiff::tz::Offset::UTC.to_datetime(self.0);
        write!(f, "{:?}Z", DateTimeDebug(utc))
    }
}

/// Differential tests against chrono, the implementation this module replaced.
/// chrono is a dev-dependency only; it serves as the oracle for the encoding
/// and for the `Debug` text.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Variant;
    use proptest::prelude::*;

    /// Days since the epoch of -9999-01-01 and 9999-12-31, jiff's date range.
    const MIN_DAYS: i32 = -4_371_587;
    const MAX_DAYS: i32 = 2_932_896;

    fn chrono_utc_from_nanos(nanos: i64) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp_nanos(nanos)
    }

    fn chrono_time(micros: u64) -> chrono::NaiveTime {
        let nanos = micros * 1_000;
        chrono::NaiveTime::from_num_seconds_from_midnight_opt(
            (nanos / 1_000_000_000) as u32,
            (nanos % 1_000_000_000) as u32,
        )
        .unwrap()
    }

    #[test]
    fn date_range_edges_match_chrono_text() {
        for days in [
            MIN_DAYS,
            MIN_DAYS + 1,
            -719_528,
            -719_529,
            -1,
            0,
            1,
            MAX_DAYS,
        ] {
            let ours = date_from_days(days).unwrap();
            let chrono = chrono::DateTime::UNIX_EPOCH
                .checked_add_signed(chrono::Duration::days(i64::from(days)))
                .unwrap()
                .date_naive();
            assert_eq!(format!("{:?}", DateDebug(ours)), format!("{chrono:?}"));
            assert_eq!(date_to_days(ours), days);
        }
    }

    /// Values chrono accepted that jiff cannot represent are explicit errors.
    #[test]
    fn values_beyond_jiff_range_are_errors() {
        for days in [MIN_DAYS - 1, MAX_DAYS + 1, 95_000_000, -95_000_000] {
            assert!(date_from_days(days).is_err(), "{days}");
        }
        let year_10000 = 253_402_300_800_000_000i64; // 10000-01-01T00:00:00Z in micros
        let before_min = (i64::from(MIN_DAYS) - 1) * 86_400_000_000; // -10000-12-31T00:00:00
        for micros in [year_10000, before_min] {
            assert!(chrono::DateTime::from_timestamp_micros(micros).is_some());
            assert!(timestamp_from_micros(micros).is_err(), "{micros}");
            assert!(datetime_from_micros(micros).is_err(), "{micros}");
        }
    }

    #[test]
    fn negative_sub_micro_nanos_floor_like_chrono() {
        for nanos in [-1i64, -999, -1_000, -1_001, -1_500, 1_500, 999] {
            let chrono = chrono_utc_from_nanos(nanos);
            let ts = Timestamp::from_nanosecond(i128::from(nanos)).unwrap();
            assert_eq!(
                timestamp_to_micros(ts),
                chrono.timestamp_micros(),
                "{nanos}"
            );
            let dt = jiff::tz::Offset::UTC.to_datetime(ts);
            assert_eq!(
                datetime_to_micros(dt),
                chrono.naive_utc().and_utc().timestamp_micros(),
                "{nanos}"
            );
        }
    }

    #[test]
    #[should_panic(expected = "does not fit the i64 nanosecond encoding")]
    fn timestamp_nanos_out_of_i64_range_panics_like_chrono() {
        timestamp_to_nanos(Timestamp::MAX);
    }

    #[test]
    #[should_panic(expected = "does not fit the i64 nanosecond encoding")]
    fn datetime_nanos_out_of_i64_range_panics_like_chrono() {
        datetime_to_nanos(DateTime::MIN);
    }

    proptest! {
        #[test]
        fn date_matches_chrono(days in MIN_DAYS..=MAX_DAYS) {
            let ours = date_from_days(days).unwrap();
            let chrono = chrono::DateTime::UNIX_EPOCH
                .checked_add_signed(chrono::Duration::days(i64::from(days)))
                .unwrap()
                .date_naive();
            prop_assert_eq!(format!("{:?}", DateDebug(ours)), format!("{chrono:?}"));
            prop_assert_eq!(format!("{:?}", Variant::Date(ours)), format!("Date({chrono:?})"));
            prop_assert_eq!(date_to_days(ours), days);
        }

        #[test]
        fn timestamp_micros_match_chrono(
            micros in Timestamp::MIN.as_microsecond()..=Timestamp::MAX.as_microsecond()
        ) {
            let chrono = chrono::DateTime::from_timestamp_micros(micros).unwrap();
            let ts = timestamp_from_micros(micros).unwrap();
            prop_assert_eq!(format!("{:?}", TimestampDebug(ts)), format!("{chrono:?}"));
            prop_assert_eq!(timestamp_to_micros(ts), micros);

            let dt = datetime_from_micros(micros).unwrap();
            prop_assert_eq!(format!("{:?}", DateTimeDebug(dt)), format!("{:?}", chrono.naive_utc()));
            prop_assert_eq!(datetime_to_micros(dt), micros);
        }

        #[test]
        fn timestamp_nanos_match_chrono(nanos in any::<i64>()) {
            let chrono = chrono_utc_from_nanos(nanos);
            let ts = timestamp_from_nanos(nanos).unwrap();
            prop_assert_eq!(format!("{:?}", TimestampDebug(ts)), format!("{chrono:?}"));
            prop_assert_eq!(timestamp_to_nanos(ts), nanos);
            prop_assert_eq!(timestamp_to_micros(ts), chrono.timestamp_micros());

            let dt = datetime_from_nanos(nanos).unwrap();
            prop_assert_eq!(format!("{:?}", DateTimeDebug(dt)), format!("{:?}", chrono.naive_utc()));
            prop_assert_eq!(datetime_to_nanos(dt), nanos);
            prop_assert_eq!(datetime_to_micros(dt), chrono.timestamp_micros());

            // `From` picks the micro/nano variant exactly as it did with chrono.
            let ours_is_nanos = matches!(Variant::from(ts), Variant::TimestampNanos(_));
            let chrono_is_nanos = !chrono::Timelike::nanosecond(&chrono).is_multiple_of(1000);
            prop_assert_eq!(ours_is_nanos, chrono_is_nanos);
            let ours_is_nanos = matches!(Variant::from(dt), Variant::TimestampNtzNanos(_));
            prop_assert_eq!(ours_is_nanos, chrono_is_nanos);
        }

        #[test]
        fn time_matches_chrono(micros in 0u64..86_400_000_000) {
            let ours = time_from_micros(micros).unwrap();
            let chrono = chrono_time(micros);
            prop_assert_eq!(format!("{:?}", TimeDebug(ours)), format!("{chrono:?}"));
            prop_assert_eq!(time_to_micros(ours), micros);
        }

        #[test]
        fn time_encoding_truncates_nanos_like_chrono(nanos in 0u64..86_400_000_000_000) {
            let chrono = chrono::NaiveTime::from_num_seconds_from_midnight_opt(
                (nanos / 1_000_000_000) as u32,
                (nanos % 1_000_000_000) as u32,
            )
            .unwrap();
            let chrono_micros = chrono::Timelike::num_seconds_from_midnight(&chrono) as u64
                * 1_000_000
                + chrono::Timelike::nanosecond(&chrono) as u64 / 1_000;
            let ours = Time::midnight()
                .checked_add(SignedDuration::from_nanos(nanos as i64))
                .unwrap();
            prop_assert_eq!(time_to_micros(ours), chrono_micros);
            prop_assert_eq!(format!("{:?}", TimeDebug(ours)), format!("{chrono:?}"));
        }
    }
}
