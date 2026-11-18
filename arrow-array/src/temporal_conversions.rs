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

//! Conversion methods for dates and times.

//! Conversion methods for dates and times.
//!
//! The `*_to_datetime`, `*_to_time`, `as_*` and `duration_*` functions return
//! [`jiff`] types. `jiff` supports years -9999 to 9999 and no leap seconds,
//! so values outside that range convert to `None`. The `*_civil_*` variants
//! return [`crate::calendar`] types covering arrow's full historical range
//! (`±262143` years), and are what the arrow kernels use internally.

use crate::calendar::{CivilDate, CivilDateTime, CivilTime, Duration, ZonedDateTime};
use crate::timezone::Tz;
use crate::ArrowPrimitiveType;
use arrow_schema::{DataType, TimeUnit};

/// Number of seconds in a day
pub const SECONDS_IN_DAY: i64 = 86_400;
/// Number of milliseconds in a second
pub const MILLISECONDS: i64 = 1_000;
/// Number of microseconds in a second
pub const MICROSECONDS: i64 = 1_000_000;
/// Number of nanoseconds in a second
pub const NANOSECONDS: i64 = 1_000_000_000;

/// Number of milliseconds in a day
pub const MILLISECONDS_IN_DAY: i64 = SECONDS_IN_DAY * MILLISECONDS;
/// Number of microseconds in a day
pub const MICROSECONDS_IN_DAY: i64 = SECONDS_IN_DAY * MICROSECONDS;
/// Number of nanoseconds in a day
pub const NANOSECONDS_IN_DAY: i64 = SECONDS_IN_DAY * NANOSECONDS;
/// Number of days between 0001-01-01 and 1970-01-01
pub const EPOCH_DAYS_FROM_CE: i32 = 719_163;

/// converts a `i32` representing a `date32` to a [`CivilDateTime`]
#[inline]
pub fn date32_to_civil_datetime(v: i32) -> Option<CivilDateTime> {
    CivilDateTime::from_timestamp(v as i64 * SECONDS_IN_DAY, 0)
}

/// converts a `i64` representing a `date64` to a [`CivilDateTime`]
#[inline]
pub fn date64_to_civil_datetime(v: i64) -> Option<CivilDateTime> {
    CivilDateTime::from_timestamp_scaled(v, MILLISECONDS)
}

/// converts a `i32` representing a `time32(s)` to a [`CivilTime`]
#[inline]
pub fn time32s_to_civil_time(v: i32) -> Option<CivilTime> {
    CivilTime::from_num_seconds_from_midnight_opt(v as u32, 0)
}

/// converts a `i32` representing a `time32(ms)` to a [`CivilTime`]
#[inline]
pub fn time32ms_to_civil_time(v: i32) -> Option<CivilTime> {
    let v = v as i64;
    CivilTime::from_num_seconds_from_midnight_opt(
        // extract seconds from milliseconds
        (v / MILLISECONDS) as u32,
        // discard extracted seconds and convert milliseconds to nanoseconds
        (v % MILLISECONDS * MICROSECONDS) as u32,
    )
}

/// converts a `i64` representing a `time64(us)` to a [`CivilTime`]
#[inline]
pub fn time64us_to_civil_time(v: i64) -> Option<CivilTime> {
    CivilTime::from_num_seconds_from_midnight_opt(
        // extract seconds from microseconds
        (v / MICROSECONDS) as u32,
        // discard extracted seconds and convert microseconds to nanoseconds
        (v % MICROSECONDS * MILLISECONDS) as u32,
    )
}

/// converts a `i64` representing a `time64(ns)` to a [`CivilTime`]
#[inline]
pub fn time64ns_to_civil_time(v: i64) -> Option<CivilTime> {
    CivilTime::from_num_seconds_from_midnight_opt(
        // extract seconds from nanoseconds
        (v / NANOSECONDS) as u32,
        // discard extracted seconds
        (v % NANOSECONDS) as u32,
    )
}

/// converts a `i64` representing a `timestamp(s)` to a [`CivilDateTime`]
#[inline]
pub fn timestamp_s_to_civil_datetime(v: i64) -> Option<CivilDateTime> {
    CivilDateTime::from_timestamp(v, 0)
}

/// converts a `i64` representing a `timestamp(ms)` to a [`CivilDateTime`]
#[inline]
pub fn timestamp_ms_to_civil_datetime(v: i64) -> Option<CivilDateTime> {
    CivilDateTime::from_timestamp_scaled(v, MILLISECONDS)
}

/// converts a `i64` representing a `timestamp(us)` to a [`CivilDateTime`]
#[inline]
pub fn timestamp_us_to_civil_datetime(v: i64) -> Option<CivilDateTime> {
    CivilDateTime::from_timestamp_scaled(v, MICROSECONDS)
}

/// converts a `i64` representing a `timestamp(ns)` to a [`CivilDateTime`]
#[inline]
pub fn timestamp_ns_to_civil_datetime(v: i64) -> Option<CivilDateTime> {
    CivilDateTime::from_timestamp_scaled(v, NANOSECONDS)
}

/// converts a `i32` representing a `date32` to [`jiff::civil::DateTime`]
#[inline]
pub fn date32_to_datetime(v: i32) -> Option<jiff::civil::DateTime> {
    date32_to_civil_datetime(v)?.to_jiff()
}

/// converts a `i64` representing a `date64` to [`jiff::civil::DateTime`]
#[inline]
pub fn date64_to_datetime(v: i64) -> Option<jiff::civil::DateTime> {
    date64_to_civil_datetime(v)?.to_jiff()
}

/// converts a `i32` representing a `time32(s)` to [`jiff::civil::Time`]
#[inline]
pub fn time32s_to_time(v: i32) -> Option<jiff::civil::Time> {
    time32s_to_civil_time(v)?.to_jiff()
}

/// converts a `i32` representing a `time32(ms)` to [`jiff::civil::Time`]
#[inline]
pub fn time32ms_to_time(v: i32) -> Option<jiff::civil::Time> {
    time32ms_to_civil_time(v)?.to_jiff()
}

/// converts a `i64` representing a `time64(us)` to [`jiff::civil::Time`]
#[inline]
pub fn time64us_to_time(v: i64) -> Option<jiff::civil::Time> {
    time64us_to_civil_time(v)?.to_jiff()
}

/// converts a `i64` representing a `time64(ns)` to [`jiff::civil::Time`]
#[inline]
pub fn time64ns_to_time(v: i64) -> Option<jiff::civil::Time> {
    time64ns_to_civil_time(v)?.to_jiff()
}

/// converts a [`CivilTime`] to a `i32` representing a `time32(s)`
#[inline]
pub fn civil_time_to_time32s(v: CivilTime) -> i32 {
    v.num_seconds_from_midnight() as i32
}

/// converts a [`CivilTime`] to a `i32` representing a `time32(ms)`
#[inline]
pub fn civil_time_to_time32ms(v: CivilTime) -> i32 {
    (v.num_seconds_from_midnight() as i64 * MILLISECONDS
        + v.nanosecond() as i64 * MILLISECONDS / NANOSECONDS) as i32
}

/// converts a [`CivilTime`] to a `i64` representing a `time64(us)`
#[inline]
pub fn civil_time_to_time64us(v: CivilTime) -> i64 {
    v.num_seconds_from_midnight() as i64 * MICROSECONDS
        + v.nanosecond() as i64 * MICROSECONDS / NANOSECONDS
}

/// converts a [`CivilTime`] to a `i64` representing a `time64(ns)`
#[inline]
pub fn civil_time_to_time64ns(v: CivilTime) -> i64 {
    v.num_seconds_from_midnight() as i64 * NANOSECONDS + v.nanosecond() as i64
}

/// converts [`jiff::civil::Time`] to a `i32` representing a `time32(s)`
#[inline]
pub fn time_to_time32s(v: jiff::civil::Time) -> i32 {
    civil_time_to_time32s(v.into())
}

/// converts [`jiff::civil::Time`] to a `i32` representing a `time32(ms)`
#[inline]
pub fn time_to_time32ms(v: jiff::civil::Time) -> i32 {
    civil_time_to_time32ms(v.into())
}

/// converts [`jiff::civil::Time`] to a `i64` representing a `time64(us)`
#[inline]
pub fn time_to_time64us(v: jiff::civil::Time) -> i64 {
    civil_time_to_time64us(v.into())
}

/// converts [`jiff::civil::Time`] to a `i64` representing a `time64(ns)`
#[inline]
pub fn time_to_time64ns(v: jiff::civil::Time) -> i64 {
    civil_time_to_time64ns(v.into())
}

/// converts a `i64` representing a `timestamp(s)` to [`jiff::civil::DateTime`]
#[inline]
pub fn timestamp_s_to_datetime(v: i64) -> Option<jiff::civil::DateTime> {
    timestamp_s_to_civil_datetime(v)?.to_jiff()
}

/// converts a `i64` representing a `timestamp(ms)` to [`jiff::civil::DateTime`]
#[inline]
pub fn timestamp_ms_to_datetime(v: i64) -> Option<jiff::civil::DateTime> {
    timestamp_ms_to_civil_datetime(v)?.to_jiff()
}

/// converts a `i64` representing a `timestamp(us)` to [`jiff::civil::DateTime`]
#[inline]
pub fn timestamp_us_to_datetime(v: i64) -> Option<jiff::civil::DateTime> {
    timestamp_us_to_civil_datetime(v)?.to_jiff()
}

/// converts a `i64` representing a `timestamp(ns)` to [`jiff::civil::DateTime`]
#[inline]
pub fn timestamp_ns_to_datetime(v: i64) -> Option<jiff::civil::DateTime> {
    timestamp_ns_to_civil_datetime(v)?.to_jiff()
}

#[cfg(test)]
fn split_second(v: i64, base: i64) -> (i64, u32) {
    (v.div_euclid(base), v.rem_euclid(base) as u32)
}

/// converts a `i64` representing a `duration(s)` to a [`Duration`]
///
/// # Panics
///
/// If `|v|` exceeds `i64::MAX / 1000` (the range of the previous
/// `chrono::Duration` representation, which panicked identically)
#[inline]
pub fn duration_s_to_calendar_duration(v: i64) -> Duration {
    Duration::try_seconds(v)
        .unwrap_or_else(|| panic!("duration of {v} seconds exceeds i64::MAX milliseconds"))
}

/// converts a `i64` representing a `duration(ms)` to a [`Duration`]
///
/// # Panics
///
/// If `v == i64::MIN`, as with the previous `chrono::Duration` representation
#[inline]
pub fn duration_ms_to_calendar_duration(v: i64) -> Duration {
    Duration::try_milliseconds(v)
        .unwrap_or_else(|| panic!("duration of {v} milliseconds exceeds i64::MAX milliseconds"))
}

/// converts a `i64` representing a `duration(us)` to a [`Duration`]
#[inline]
pub fn duration_us_to_calendar_duration(v: i64) -> Duration {
    Duration::microseconds(v)
}

/// converts a `i64` representing a `duration(ns)` to a [`Duration`]
#[inline]
pub fn duration_ns_to_calendar_duration(v: i64) -> Duration {
    Duration::nanoseconds(v)
}

/// converts a `i64` representing a `duration(s)` to [`jiff::SignedDuration`]
///
/// # Panics
///
/// See [`duration_s_to_calendar_duration`]
#[inline]
pub fn duration_s_to_duration(v: i64) -> jiff::SignedDuration {
    duration_s_to_calendar_duration(v).into()
}

/// converts a `i64` representing a `duration(ms)` to [`jiff::SignedDuration`]
///
/// # Panics
///
/// See [`duration_ms_to_calendar_duration`]
#[inline]
pub fn duration_ms_to_duration(v: i64) -> jiff::SignedDuration {
    duration_ms_to_calendar_duration(v).into()
}

/// converts a `i64` representing a `duration(us)` to [`jiff::SignedDuration`]
#[inline]
pub fn duration_us_to_duration(v: i64) -> jiff::SignedDuration {
    duration_us_to_calendar_duration(v).into()
}

/// converts a `i64` representing a `duration(ns)` to [`jiff::SignedDuration`]
#[inline]
pub fn duration_ns_to_duration(v: i64) -> jiff::SignedDuration {
    duration_ns_to_calendar_duration(v).into()
}

/// Converts an [`ArrowPrimitiveType`] to a [`CivilDateTime`]
pub fn as_civil_datetime<T: ArrowPrimitiveType>(v: i64) -> Option<CivilDateTime> {
    match T::DATA_TYPE {
        DataType::Date32 => date32_to_civil_datetime(v as i32),
        DataType::Date64 => date64_to_civil_datetime(v),
        DataType::Timestamp(unit, _) => match unit {
            TimeUnit::Second => timestamp_s_to_civil_datetime(v),
            TimeUnit::Millisecond => timestamp_ms_to_civil_datetime(v),
            TimeUnit::Microsecond => timestamp_us_to_civil_datetime(v),
            TimeUnit::Nanosecond => timestamp_ns_to_civil_datetime(v),
        },
        _ => None,
    }
}

/// Converts an [`ArrowPrimitiveType`] to a [`ZonedDateTime`] in `tz`
pub fn as_zoned_datetime<T: ArrowPrimitiveType>(v: i64, tz: &Tz) -> Option<ZonedDateTime> {
    tz.from_utc(as_civil_datetime::<T>(v)?)
}

/// Converts an [`ArrowPrimitiveType`] to a [`CivilDate`]
pub fn as_civil_date<T: ArrowPrimitiveType>(v: i64) -> Option<CivilDate> {
    as_civil_datetime::<T>(v).map(|datetime| datetime.date)
}

/// Converts an [`ArrowPrimitiveType`] to a [`CivilTime`]
pub fn as_civil_time<T: ArrowPrimitiveType>(v: i64) -> Option<CivilTime> {
    match T::DATA_TYPE {
        DataType::Time32(unit) => {
            // safe to immediately cast to u32 as `self.value(i)` is positive i32
            let v = v as u32;
            match unit {
                TimeUnit::Second => time32s_to_civil_time(v as i32),
                TimeUnit::Millisecond => time32ms_to_civil_time(v as i32),
                _ => None,
            }
        }
        DataType::Time64(unit) => match unit {
            TimeUnit::Microsecond => time64us_to_civil_time(v),
            TimeUnit::Nanosecond => time64ns_to_civil_time(v),
            _ => None,
        },
        DataType::Timestamp(_, _) => as_civil_datetime::<T>(v).map(|datetime| datetime.time),
        DataType::Date32 | DataType::Date64 => Some(CivilTime::MIDNIGHT),
        _ => None,
    }
}

/// Converts an [`ArrowPrimitiveType`] to a [`Duration`]
///
/// # Panics
///
/// See [`duration_s_to_calendar_duration`] and [`duration_ms_to_calendar_duration`]
pub fn as_calendar_duration<T: ArrowPrimitiveType>(v: i64) -> Option<Duration> {
    match T::DATA_TYPE {
        DataType::Duration(unit) => match unit {
            TimeUnit::Second => Some(duration_s_to_calendar_duration(v)),
            TimeUnit::Millisecond => Some(duration_ms_to_calendar_duration(v)),
            TimeUnit::Microsecond => Some(duration_us_to_calendar_duration(v)),
            TimeUnit::Nanosecond => Some(duration_ns_to_calendar_duration(v)),
        },
        _ => None,
    }
}

/// Converts an [`ArrowPrimitiveType`] to [`jiff::civil::DateTime`]
pub fn as_datetime<T: ArrowPrimitiveType>(v: i64) -> Option<jiff::civil::DateTime> {
    as_civil_datetime::<T>(v)?.to_jiff()
}

/// Converts an [`ArrowPrimitiveType`] to a [`jiff::Zoned`] in `tz`
pub fn as_datetime_with_timezone<T: ArrowPrimitiveType>(v: i64, tz: Tz) -> Option<jiff::Zoned> {
    let timestamp = as_civil_datetime::<T>(v)?.to_jiff_timestamp()?;
    Some(jiff::Zoned::new(timestamp, tz.to_jiff()))
}

/// Converts an [`ArrowPrimitiveType`] to [`jiff::civil::Date`]
pub fn as_date<T: ArrowPrimitiveType>(v: i64) -> Option<jiff::civil::Date> {
    as_civil_date::<T>(v)?.to_jiff()
}

/// Converts an [`ArrowPrimitiveType`] to [`jiff::civil::Time`]
pub fn as_time<T: ArrowPrimitiveType>(v: i64) -> Option<jiff::civil::Time> {
    as_civil_time::<T>(v)?.to_jiff()
}

/// Converts an [`ArrowPrimitiveType`] to [`jiff::SignedDuration`]
///
/// # Panics
///
/// See [`as_calendar_duration`]
pub fn as_duration<T: ArrowPrimitiveType>(v: i64) -> Option<jiff::SignedDuration> {
    as_calendar_duration::<T>(v).map(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn chrono_utc(secs: i64, nanos: u32) -> Option<jiff::civil::DateTime> {
        let dt = chrono::DateTime::from_timestamp(secs, nanos)?.naive_utc();
        let s = dt.format("%Y-%m-%dT%H:%M:%S%.f").to_string();
        Some(s.parse().unwrap())
    }

    #[test]
    fn negative_input_timestamp_ns_to_datetime() {
        assert_eq!(timestamp_ns_to_datetime(-1), chrono_utc(-1, 999_999_999));
        assert_eq!(
            timestamp_ns_to_datetime(-1_000_000_001),
            chrono_utc(-2, 999_999_999)
        );
    }

    #[test]
    fn negative_input_timestamp_us_to_datetime() {
        assert_eq!(timestamp_us_to_datetime(-1), chrono_utc(-1, 999_999_000));
        assert_eq!(
            timestamp_us_to_datetime(-1_000_001),
            chrono_utc(-2, 999_999_000)
        );
    }

    #[test]
    fn negative_input_timestamp_ms_to_datetime() {
        assert_eq!(timestamp_ms_to_datetime(-1), chrono_utc(-1, 999_000_000));
        assert_eq!(
            timestamp_ms_to_datetime(-1_001),
            chrono_utc(-2, 999_000_000)
        );
    }

    #[test]
    fn negative_input_date64_to_datetime() {
        assert_eq!(date64_to_datetime(-1), chrono_utc(-1, 999_000_000));
        assert_eq!(date64_to_datetime(-1_001), chrono_utc(-2, 999_000_000));
    }

    #[test]
    fn test_split_seconds() {
        let (sec, nano_sec) = split_second(100, NANOSECONDS);
        assert_eq!(sec, 0);
        assert_eq!(nano_sec, 100);

        let (sec, nano_sec) = split_second(123_000_000_456, NANOSECONDS);
        assert_eq!(sec, 123);
        assert_eq!(nano_sec, 456);

        let (sec, nano_sec) = split_second(-1, NANOSECONDS);
        assert_eq!(sec, -1);
        assert_eq!(nano_sec, 999_999_999);

        let (sec, nano_sec) = split_second(-123_000_000_001, NANOSECONDS);
        assert_eq!(sec, -124);
        assert_eq!(nano_sec, 999_999_999);
    }

    #[test]
    fn test_jiff_range_is_narrower() {
        // 10000-01-01 is representable in arrow's calendar but not in jiff
        let days = 2_932_897;
        assert!(date32_to_civil_datetime(days).is_some());
        assert_eq!(date32_to_datetime(days), None);
        assert_eq!(
            date32_to_datetime(days - 1).unwrap().to_string(),
            "9999-12-31T00:00:00"
        );
    }

    /// The civil conversions reproduce the chrono functions they replace,
    /// including the `as u32` wrapping of the time conversions
    #[test]
    fn test_conversions_match_chrono() {
        let values = [
            i64::MIN,
            -86_400_000_000_001,
            -4_294_967_296_000_000_000,
            -1,
            0,
            1,
            86_399_999,
            86_400_000,
            4_294_967_296_000_000_005,
            i64::MAX,
        ];
        let ct = |t: Option<chrono::NaiveTime>| t.map(|t| format!("{t:?}"));
        let ot = |t: Option<CivilTime>| t.map(|t| format!("{t:?}"));
        let cd = |t: Option<chrono::NaiveDateTime>| t.map(|t| format!("{t:?}"));
        let od = |t: Option<CivilDateTime>| t.map(|t| format!("{t:?}"));
        for v in values {
            let v32 = v as i32;
            let chrono_time = |secs: i64, nanos: i64| {
                chrono::NaiveTime::from_num_seconds_from_midnight_opt(secs as u32, nanos as u32)
            };
            assert_eq!(
                ot(time32s_to_civil_time(v32)),
                ct(chrono_time(v32 as i64, 0))
            );
            let v32l = v32 as i64;
            let expected = chrono_time(v32l / 1000, v32l % 1000 * 1_000_000);
            assert_eq!(ot(time32ms_to_civil_time(v32)), ct(expected));
            let expected = chrono_time(v / 1_000_000, v % 1_000_000 * 1000);
            assert_eq!(ot(time64us_to_civil_time(v)), ct(expected));
            let expected = chrono_time(v / 1_000_000_000, v % 1_000_000_000);
            assert_eq!(ot(time64ns_to_civil_time(v)), ct(expected));
            let from_ts =
                |s: i64, n: u32| chrono::DateTime::from_timestamp(s, n).map(|d| d.naive_utc());
            let split = |base: i64| (v.div_euclid(base), v.rem_euclid(base) as u32);
            assert_eq!(od(timestamp_s_to_civil_datetime(v)), cd(from_ts(v, 0)));
            let (s, n) = split(1000);
            assert_eq!(
                od(timestamp_ms_to_civil_datetime(v)),
                cd(from_ts(s, n * 1_000_000))
            );
            assert_eq!(
                od(date64_to_civil_datetime(v)),
                cd(from_ts(s, n * 1_000_000))
            );
            let (s, n) = split(1_000_000);
            assert_eq!(
                od(timestamp_us_to_civil_datetime(v)),
                cd(from_ts(s, n * 1000))
            );
            let (s, n) = split(1_000_000_000);
            assert_eq!(od(timestamp_ns_to_civil_datetime(v)), cd(from_ts(s, n)));
            let expected = from_ts(v32 as i64 * 86_400, 0);
            assert_eq!(od(date32_to_civil_datetime(v32)), cd(expected));
            assert_eq!(
                as_civil_time::<Time32SecondType>(v).map(|t| format!("{t:?}")),
                ct(chrono_time((v as u32 as i32) as i64, 0))
            );
        }
    }
}
