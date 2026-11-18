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

//! Timezone for timestamp arrays
//!
//! A [`Tz`] is either a fixed offset such as `+09:00`, or, with the
//! `chrono-tz` feature, an IANA time zone such as `America/New_York` backed
//! by `jiff`'s bundled time zone database. The feature keeps its historical
//! name so dependents continue to resolve it.

use crate::calendar::{fixed_offset_opt, CivilDateTime, ZonedDateTime};
use arrow_schema::ArrowError;
use std::str::FromStr;

/// Parses a fixed offset of the form "+09:00", "-09" or "+0930", in seconds
fn parse_fixed_offset(tz: &str) -> Option<i32> {
    let bytes = tz.as_bytes();

    let mut values = match bytes.len() {
        // [+-]XX:XX
        6 if bytes[3] == b':' => [bytes[1], bytes[2], bytes[4], bytes[5]],
        // [+-]XXXX
        5 => [bytes[1], bytes[2], bytes[3], bytes[4]],
        // [+-]XX
        3 => [bytes[1], bytes[2], b'0', b'0'],
        _ => return None,
    };
    values.iter_mut().for_each(|x| *x = x.wrapping_sub(b'0'));
    if values.iter().any(|x| *x > 9) {
        return None;
    }
    let secs =
        (values[0] * 10 + values[1]) as i32 * 60 * 60 + (values[2] * 10 + values[3]) as i32 * 60;

    match bytes[0] {
        b'+' => fixed_offset_opt(secs),
        b'-' => fixed_offset_opt(-secs),
        _ => None,
    }
}

/// An Arrow timezone: a fixed offset or (with `chrono-tz`) an IANA zone
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tz(TzInner);

#[derive(Debug, Clone, PartialEq, Eq)]
enum TzInner {
    /// Offset from UTC in seconds, strictly within ±24h
    Offset(i32),
    #[cfg(feature = "chrono-tz")]
    Named(jiff::tz::TimeZone),
}

impl FromStr for Tz {
    type Err = ArrowError;

    fn from_str(tz: &str) -> Result<Self, Self::Err> {
        match parse_fixed_offset(tz) {
            Some(offset) => Ok(Self(TzInner::Offset(offset))),
            None => parse_named(tz),
        }
    }
}

#[cfg(feature = "chrono-tz")]
fn parse_named(tz: &str) -> Result<Tz, ArrowError> {
    // Same message as the chrono-tz based implementation this replaces
    let err = || {
        ArrowError::ParseError(format!(
            "Invalid timezone \"{tz}\": failed to parse timezone"
        ))
    };
    let zone = jiff::tz::TimeZone::get(tz).map_err(|_| err())?;
    // jiff resolves names case-insensitively; chrono-tz required the exact
    // IANA spelling, so anything else is rejected.
    match zone.iana_name() == Some(tz) {
        true => Ok(Tz(TzInner::Named(zone))),
        false => Err(err()),
    }
}

#[cfg(not(feature = "chrono-tz"))]
fn parse_named(tz: &str) -> Result<Tz, ArrowError> {
    Err(ArrowError::ParseError(format!(
        "Invalid timezone \"{tz}\": only offset based timezones supported without chrono-tz feature"
    )))
}

/// Converts a local datetime to `jiff`, ignoring any leap second fraction
/// (time zone rules only depend on whole seconds)
#[cfg(feature = "chrono-tz")]
fn local_to_jiff(local: &CivilDateTime) -> Option<jiff::civil::DateTime> {
    let (date, time) = (local.date, local.time);
    let year = i16::try_from(date.year()).ok()?;
    let (month, day) = (date.month() as i8, date.day() as i8);
    let (hour, minute, second) = (time.hour() as i8, time.minute() as i8, time.second() as i8);
    jiff::civil::DateTime::new(year, month, day, hour, minute, second, 0).ok()
}

impl Tz {
    /// UTC as a zero fixed offset
    pub const UTC: Tz = Tz(TzInner::Offset(0));

    /// Returns the fixed offset in seconds, if this is not a named zone
    pub fn fixed_offset(&self) -> Option<i32> {
        match &self.0 {
            TzInner::Offset(o) => Some(*o),
            #[cfg(feature = "chrono-tz")]
            TzInner::Named(_) => None,
        }
    }

    /// The offset in seconds in effect at the UTC datetime `utc`
    ///
    /// Always `Some` for fixed offsets. For named zones, `None` when `utc`
    /// lies outside the range `jiff` supports (years -9999 to 9999).
    #[cfg_attr(not(feature = "chrono-tz"), allow(unused_variables))]
    pub fn offset_from_utc(&self, utc: &CivilDateTime) -> Option<i32> {
        match &self.0 {
            TzInner::Offset(o) => Some(*o),
            #[cfg(feature = "chrono-tz")]
            TzInner::Named(zone) => {
                let ts = jiff::Timestamp::new(utc.timestamp(), 0).ok()?;
                Some(zone.to_offset(ts).seconds())
            }
        }
    }

    /// The offset in seconds for the local datetime `local`, if it is unique
    ///
    /// `None` if `local` falls in a gap or fold of a named zone, or outside
    /// the range `jiff` supports.
    #[cfg_attr(not(feature = "chrono-tz"), allow(unused_variables))]
    pub fn single_offset_from_local(&self, local: &CivilDateTime) -> Option<i32> {
        match &self.0 {
            TzInner::Offset(o) => Some(*o),
            #[cfg(feature = "chrono-tz")]
            TzInner::Named(zone) => {
                use jiff::tz::AmbiguousOffset;
                match zone.to_ambiguous_timestamp(local_to_jiff(local)?).offset() {
                    AmbiguousOffset::Unambiguous { offset } => Some(offset.seconds()),
                    AmbiguousOffset::Gap { .. } | AmbiguousOffset::Fold { .. } => None,
                }
            }
        }
    }

    /// Views the UTC datetime `utc` in this zone
    pub fn from_utc(&self, utc: CivilDateTime) -> Option<ZonedDateTime> {
        let offset = self.offset_from_utc(&utc)?;
        Some(ZonedDateTime { utc, offset })
    }

    /// Resolves the local datetime `local` in this zone, if it is unique and
    /// the corresponding UTC datetime is in range
    pub fn from_local(&self, local: CivilDateTime) -> Option<ZonedDateTime> {
        let offset = self.single_offset_from_local(&local)?;
        let utc = local.checked_sub_offset(offset)?;
        Some(ZonedDateTime { utc, offset })
    }

    /// This zone as a [`jiff::tz::TimeZone`]
    pub fn to_jiff(&self) -> jiff::tz::TimeZone {
        match &self.0 {
            TzInner::Offset(o) => {
                // jiff offsets span ±25:59:59, a superset of the ±24h accepted here
                let offset = jiff::tz::Offset::from_seconds(*o)
                    .expect("validated fixed offset is within jiff's offset range");
                jiff::tz::TimeZone::fixed(offset)
            }
            #[cfg(feature = "chrono-tz")]
            TzInner::Named(zone) => zone.clone(),
        }
    }
}

impl From<Tz> for jiff::tz::TimeZone {
    fn from(tz: Tz) -> Self {
        tz.to_jiff()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::CivilDateTime;

    fn utc(secs: i64) -> CivilDateTime {
        CivilDateTime::from_timestamp(secs, 0).unwrap()
    }

    #[test]
    fn test_with_offset() {
        let t = utc(946_684_800);

        let tz: Tz = "-00:00".parse().unwrap();
        assert_eq!(tz.offset_from_utc(&t), Some(0));
        let tz: Tz = "+00:00".parse().unwrap();
        assert_eq!(tz.offset_from_utc(&t), Some(0));

        let tz: Tz = "-10:00".parse().unwrap();
        assert_eq!(tz.offset_from_utc(&t), Some(-10 * 60 * 60));
        let tz: Tz = "+09:00".parse().unwrap();
        assert_eq!(tz.offset_from_utc(&t), Some(9 * 60 * 60));

        let tz = "+09".parse::<Tz>().unwrap();
        assert_eq!(tz.offset_from_utc(&t), Some(9 * 60 * 60));

        let tz = "+0900".parse::<Tz>().unwrap();
        assert_eq!(tz.offset_from_utc(&t), Some(9 * 60 * 60));

        let err = "+9:00".parse::<Tz>().unwrap_err().to_string();
        assert!(err.contains("Invalid timezone"), "{}", err);
    }

    #[test]
    fn test_fixed_offset_parse_matches_chrono() {
        // The previous implementation validated with chrono's FixedOffset
        for s in [
            "+23:59", "-23:59", "+24:00", "-24:00", "+99", "+0000", "-0059", "+00:60",
        ] {
            let ours = parse_fixed_offset(s);
            let digits: Vec<i32> = s.as_bytes()[1..]
                .iter()
                .filter(|b| b.is_ascii_digit())
                .map(|b| (b - b'0') as i32)
                .collect();
            let minutes = match digits.len() {
                4 => digits[2] * 10 + digits[3],
                _ => 0,
            };
            let secs = (digits[0] * 10 + digits[1]) * 3600 + minutes * 60;
            let theirs = match s.as_bytes()[0] {
                b'+' => chrono::FixedOffset::east_opt(secs),
                _ => chrono::FixedOffset::west_opt(secs),
            };
            assert_eq!(ours, theirs.map(|o| o.local_minus_utc()), "{s}");
        }
    }

    #[test]
    #[cfg(not(feature = "chrono-tz"))]
    fn test_named_requires_feature() {
        let err = "America/New_York".parse::<Tz>().unwrap_err().to_string();
        assert!(err.contains("without chrono-tz feature"), "{err}");
    }

    #[cfg(feature = "chrono-tz")]
    mod named {
        use super::*;
        use chrono::{Offset, TimeZone};

        #[test]
        fn test_with_timezone() {
            let tz: Tz = "America/Los_Angeles".parse().unwrap();
            let zoned = tz.from_utc(utc(37_800)).unwrap();
            assert_eq!(zoned.local().time.hour(), 2);
            let zoned = tz.from_utc(utc(86_339)).unwrap();
            assert_eq!(zoned.local().time.hour(), 15);
        }

        #[test]
        fn test_case_sensitive() {
            let err = "america/new_york".parse::<Tz>().unwrap_err().to_string();
            assert!(err.contains("Invalid timezone"), "{err}");
            assert!("America/New_York".parse::<Tz>().is_ok());
            assert!("xxx".parse::<Tz>().is_err());
        }

        /// Every name chrono-tz accepted still parses, and vice versa, except
        /// for identifiers added or removed between tzdata releases
        #[test]
        fn test_names_match_chrono_tz() {
            let theirs: std::collections::BTreeSet<&str> =
                chrono_tz::TZ_VARIANTS.iter().map(|t| t.name()).collect();
            let missing: Vec<_> = theirs.iter().filter(|n| n.parse::<Tz>().is_err()).collect();
            assert!(missing.is_empty(), "{missing:?}");
            let err = "Europe/Nowhere".parse::<Tz>().unwrap_err().to_string();
            let expected = "Invalid timezone \"Europe/Nowhere\": failed to parse timezone";
            assert_eq!(err, format!("Parser error: {expected}"));
        }

        #[test]
        fn test_sydney_dst() {
            let tz: Tz = "Australia/Sydney".parse().unwrap();
            // Sydney 2021-04-04T02:30:00+11:00 is 2021-04-03T15:30:00Z
            assert_eq!(tz.offset_from_utc(&utc(1_617_463_800)), Some(11 * 3600));
            // Sydney 2021-04-04T02:30:00+10:00 is 2021-04-03T16:30:00Z
            assert_eq!(tz.offset_from_utc(&utc(1_617_467_400)), Some(10 * 3600));
            // 2021-10-02T15:30:00Z
            assert_eq!(tz.offset_from_utc(&utc(1_633_188_600)), Some(10 * 3600));
            // 2022-10-02T16:30:00Z
            assert_eq!(tz.offset_from_utc(&utc(1_664_728_200)), Some(11 * 3600));
        }

        const ZONES: &[&str] = &[
            "America/New_York",
            "America/Los_Angeles",
            "America/Denver",
            "America/Sao_Paulo",
            "Europe/London",
            "Europe/Berlin",
            "Europe/Moscow",
            "Asia/Kolkata",
            "Asia/Taipei",
            "Asia/Tokyo",
            "Australia/Sydney",
            "Australia/Lord_Howe",
            "Pacific/Chatham",
            "UTC",
            "Etc/GMT+5",
        ];

        /// jiff's bundled tzdb against chrono-tz between 1900 and 2037
        ///
        /// Returns `(zone, utc, ours, theirs)` for every sampled instant
        /// where the UTC offset or local-time resolution differs.
        fn chrono_tz_mismatches() -> Vec<(&'static str, String, Option<i32>, Option<i32>)> {
            let mut out = vec![];
            for name in ZONES {
                let ours: Tz = name.parse().unwrap();
                let theirs: chrono_tz::Tz = name.parse().unwrap();
                // 1900-01-01 to 2038-01-01
                let mut secs = -2_208_988_800;
                while secs < 2_145_916_800 {
                    let naive = chrono::DateTime::from_timestamp(secs, 0)
                        .unwrap()
                        .naive_utc();
                    let expected = theirs.offset_from_utc_datetime(&naive).fix();
                    let actual = ours.offset_from_utc(&utc(secs));
                    if actual != Some(expected.local_minus_utc()) {
                        let theirs = Some(expected.local_minus_utc());
                        out.push((*name, naive.to_string(), actual, theirs));
                    } else {
                        let local = ours.from_utc(utc(secs)).unwrap().local();
                        let expected_local = theirs
                            .offset_from_local_datetime(&(naive + expected))
                            .single()
                            .map(|o| o.fix().local_minus_utc());
                        let actual_local = ours.single_offset_from_local(&local);
                        if actual_local != expected_local {
                            out.push((
                                *name,
                                format!("local {local}"),
                                actual_local,
                                expected_local,
                            ));
                        }
                    }
                    secs += 3_599 * 7;
                }
            }
            out
        }

        #[test]
        fn test_offsets_match_chrono_tz() {
            // The only differences are tzdata revisions: chrono-tz 0.10.4
            // bundles tzdata 2025b, jiff-tzdb a newer release whose Brazil
            // 1965-1966 history differs. Anything else is a regression.
            let unexplained: Vec<_> = chrono_tz_mismatches()
                .into_iter()
                .filter(|(zone, when, _, _)| {
                    let year = when.trim_start_matches("local ");
                    let brazil = year.starts_with("1965") || year.starts_with("1966");
                    !(*zone == "America/Sao_Paulo" && brazil)
                })
                .collect();
            assert!(unexplained.is_empty(), "{unexplained:?}");
        }
    }
}
