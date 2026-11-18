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

//! Oracle tests: every parse result must equal chrono 0.4's, including the
//! error message.

use super::parse::*;
use super::{CivilDate, CivilDateTime, CivilTime};
use chrono::{Datelike, Timelike};

const FORMATS: &[&str] = &[
    "%Y-%m-%d",
    "%Y-%m-%d %H:%M:%S",
    "%Y-%m-%d %H:%M:%S%.f",
    "%Y-%m-%dT%H:%M:%S%.3f",
    "%Y-%m-%d %H:%M:%S%z",
    "%Y-%m-%d %H:%M:%S %:z",
    "%Y-%m-%d %H:%M:%S %::z",
    "%Y-%m-%d %H:%M:%S %:::z",
    "%Y-%m-%d %H:%M:%S %#z",
    "%Y-%m-%d %H:%M:%S %Z",
    "%+",
    "%s",
    "%s%.f",
    "%Y %j",
    "%Y %U %w",
    "%Y %W %u",
    "%G %V %u",
    "%G-W%V-%u %H",
    "%y %m %d",
    "%C%y %m %d",
    "%C %m %d",
    "%d/%m/%Y",
    "%D",
    "%F %T",
    "%v",
    "%c",
    "%x %X",
    "%a %b %e %H:%M:%S %Y",
    "%A, %B %d, %Y",
    "%I:%M %p",
    "%I:%M:%S %P",
    "%l:%M %p",
    "%H%M%S",
    "%H:%M:%S%3f",
    "%H:%M:%S%6f",
    "%H:%M:%S%9f",
    "%H:%M:%S.%f",
    "%H - %M - %S - %.f",
    "%r",
    "%R",
    "%T",
    "%H:%M",
    "%Y%m%d%H%M%S",
    "%-d %-m %Y",
    "%Q",
    "%Y-%m-%d %",
    "%%Y %Y",
];

const INPUTS: &[&str] = &[
    "2020-09-08",
    "2020-9-8",
    "2020-02-30",
    "2020-13-01",
    "-0001-01-01",
    "+12345-01-01",
    "262143-01-01",
    "2020-09-08 13:42:29",
    "2020-09-08 13:42:29.190855",
    "2020-09-08T13:42:29.190",
    "2020-09-08 23:59:60",
    "2020-09-08 23:59:60.5",
    "2020-09-08 13:42:29+0530",
    "2020-09-08 13:42:29-05:30",
    "2020-09-08 13:42:29 +05:30",
    "2020-09-08 13:42:29 -05",
    "2020-09-08 13:42:29 +05:30:00",
    "2020-09-08 13:42:29 +2400",
    "2020-09-08 13:42:29 +2360",
    "2020-09-08 13:42:29 Z",
    "2020-09-08 13:42:29 UTC",
    "2020-09-08 13:42:29 PST",
    "2020-09-08T13:42:29Z",
    "2020-09-08T13:42:29.123456789+01:00",
    "2020-09-08 13:42:29 UTC",
    "2020 - 09 - 08T13:42:29 utc",
    "1599572549",
    "1599572549.5",
    "-1",
    "99999999999999999999",
    "2020 252",
    "2020 366",
    "2021 366",
    "2020 36 2",
    "2020 0 0",
    "2020 53 6",
    "2020 36 7",
    "2020 37 1",
    "2020-W37-2 13",
    "2020 53 5",
    "2021 53 5",
    "20 09 08",
    "70 01 01",
    "69 12 31",
    "2020 09 08",
    "20 09 08",
    "08/09/2020",
    "09/08/20",
    "2020-09-08 13:42:29",
    " 8-Sep-2020",
    "Tue Sep  8 13:42:29 2020",
    "Tue Sep 8 13:42:29 2020",
    "Wed Sep  8 13:42:29 2020",
    "Tuesday, September 08, 2020",
    "tue, sept 08, 2020",
    "09/08/20 13:42:29",
    "1:42 PM",
    "12:42 am",
    "13:42 PM",
    "01:42:29 pm",
    " 1:42 PM",
    "134229",
    "13:42:29123",
    "13:42:29123456",
    "13:42:29123456789",
    "13:42:2912",
    "13:42:29.123",
    "02 - 10 - 01 - .1234567",
    "02 - 10 - 01 - ",
    "01:42:29 PM",
    "13:42",
    "13:42:29",
    "24:00:00",
    "20200908134229",
    "8 9 2020",
    "%2020",
    "2020-09-08 %",
    "",
    " ",
    "2020-09-08 trailing",
    "2020-09-08\u{2212}05:00",
    "2020-09-08 13:42:29 \u{2212}05:00",
];

fn date_key(d: CivilDate) -> i32 {
    d.num_days_from_ce()
}

fn time_key(t: CivilTime) -> (u32, u32) {
    (t.num_seconds_from_midnight(), t.nanosecond())
}

fn dt_key(dt: CivilDateTime) -> (i32, (u32, u32)) {
    (date_key(dt.date), time_key(dt.time))
}

fn chrono_time_key(t: chrono::NaiveTime) -> (u32, u32) {
    (t.num_seconds_from_midnight(), t.nanosecond())
}

fn chrono_dt_key(dt: chrono::NaiveDateTime) -> (i32, (u32, u32)) {
    (dt.date().num_days_from_ce(), chrono_time_key(dt.time()))
}

/// Asserts equal results, returning true if both succeeded
fn check<A: PartialEq + std::fmt::Debug>(
    ours: Result<A, ParseError>,
    theirs: Result<A, chrono::ParseError>,
    context: &str,
) -> bool {
    let ours = ours.map_err(|e| e.to_string());
    let theirs = theirs.map_err(|e| e.to_string());
    assert_eq!(ours, theirs, "{context}");
    ours.is_ok()
}

#[test]
fn test_parse_matches_chrono() {
    let mut successes = [0usize; 4];
    for fmt in FORMATS {
        for s in INPUTS {
            let ctx = format!("{s:?} with {fmt:?}");
            successes[0] += check(
                parse_date(s, fmt).map(date_key),
                chrono::NaiveDate::parse_from_str(s, fmt).map(|d| d.num_days_from_ce()),
                &ctx,
            ) as usize;
            successes[1] += check(
                parse_time(s, fmt).map(time_key),
                chrono::NaiveTime::parse_from_str(s, fmt).map(chrono_time_key),
                &ctx,
            ) as usize;
            successes[2] += check(
                parse_datetime(s, fmt).map(dt_key),
                chrono::NaiveDateTime::parse_from_str(s, fmt).map(chrono_dt_key),
                &ctx,
            ) as usize;
            successes[3] += check(
                parse_datetime_with_offset(s, fmt).map(|z| (dt_key(z.utc), z.offset)),
                chrono::DateTime::parse_from_str(s, fmt)
                    .map(|d| (chrono_dt_key(d.naive_utc()), d.offset().local_minus_utc())),
                &ctx,
            ) as usize;
        }
    }
    // Guard against the corpus degenerating into errors only
    assert!(successes.iter().all(|n| *n >= 10), "{successes:?}");
}

#[test]
fn test_format_has_zone_matches_chrono() {
    use chrono::format::{Fixed, Item, StrftimeItems};
    for fmt in FORMATS.iter().chain(["%::z", "%:::z", "%#z", "%Z"].iter()) {
        let theirs = StrftimeItems::new(fmt).any(|item| match item {
            Item::Fixed(f) => matches!(
                f,
                Fixed::RFC2822
                    | Fixed::RFC3339
                    | Fixed::TimezoneName
                    | Fixed::TimezoneOffsetColon
                    | Fixed::TimezoneOffsetColonZ
                    | Fixed::TimezoneOffset
                    | Fixed::TimezoneOffsetZ
            ),
            _ => false,
        });
        assert_eq!(format_has_zone(fmt), theirs, "{fmt}");
    }
}
