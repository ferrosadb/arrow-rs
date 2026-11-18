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

//! Parsing with `chrono` 0.4.39 strftime format strings, adapted from
//! `chrono::format::{parse, parsed}` (MIT / Apache-2.0).
//!
//! Accepts exactly the inputs `chrono`'s `NaiveDate::parse_from_str`,
//! `NaiveTime::parse_from_str`, `NaiveDateTime::parse_from_str` and
//! `DateTime::parse_from_str` accepted, and resolves them identically.

use super::date::{CivilDate, Weekday};
use super::format::{Fixed, Item, Numeric, StrftimeItems};
use super::scan;
use super::time::{fixed_offset_opt, CivilDateTime, CivilTime, ZonedDateTime};
use std::fmt;

/// The reason a string could not be parsed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParseErrorKind {
    /// A field is out of its permitted range
    OutOfRange,
    /// The fields are inconsistent with each other
    Impossible,
    /// Not enough fields to determine a value
    NotEnough,
    /// Unexpected characters
    Invalid,
    /// Input ended early
    TooShort,
    /// Trailing input
    TooLong,
    /// The format string is invalid
    BadFormat,
}

/// An error parsing with a format string
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParseError(ParseErrorKind);

impl ParseError {
    /// The kind of error
    pub const fn kind(&self) -> ParseErrorKind {
        self.0
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self.0 {
            ParseErrorKind::OutOfRange => "input is out of range",
            ParseErrorKind::Impossible => "no possible date and time matching input",
            ParseErrorKind::NotEnough => "input is not enough for unique date and time",
            ParseErrorKind::Invalid => "input contains invalid characters",
            ParseErrorKind::TooShort => "premature end of input",
            ParseErrorKind::TooLong => "trailing input",
            ParseErrorKind::BadFormat => "bad or unsupported format string",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for ParseError {}

/// Result of a parse step
pub type ParseResult<T> = Result<T, ParseError>;

pub(super) const OUT_OF_RANGE: ParseError = ParseError(ParseErrorKind::OutOfRange);
const IMPOSSIBLE: ParseError = ParseError(ParseErrorKind::Impossible);
const NOT_ENOUGH: ParseError = ParseError(ParseErrorKind::NotEnough);
pub(super) const INVALID: ParseError = ParseError(ParseErrorKind::Invalid);
pub(super) const TOO_SHORT: ParseError = ParseError(ParseErrorKind::TooShort);
const TOO_LONG: ParseError = ParseError(ParseErrorKind::TooLong);
const BAD_FORMAT: ParseError = ParseError(ParseErrorKind::BadFormat);

/// Parses `s` as a date with the chrono strftime format `fmt`
pub fn parse_date(s: &str, fmt: &str) -> ParseResult<CivilDate> {
    let mut parsed = Parsed::default();
    parse(&mut parsed, s, StrftimeItems::new(fmt))?;
    parsed.to_naive_date()
}

/// Parses `s` as a time of day with the chrono strftime format `fmt`
pub fn parse_time(s: &str, fmt: &str) -> ParseResult<CivilTime> {
    let mut parsed = Parsed::default();
    parse(&mut parsed, s, StrftimeItems::new(fmt))?;
    parsed.to_naive_time()
}

/// Parses `s` as a naive datetime, ignoring any parsed offset
pub fn parse_datetime(s: &str, fmt: &str) -> ParseResult<CivilDateTime> {
    let mut parsed = Parsed::default();
    parse(&mut parsed, s, StrftimeItems::new(fmt))?;
    parsed.to_naive_datetime_with_offset(0)
}

/// Parses `s` as a datetime with a fixed offset, which must be present
pub fn parse_datetime_with_offset(s: &str, fmt: &str) -> ParseResult<ZonedDateTime> {
    let mut parsed = Parsed::default();
    parse(&mut parsed, s, StrftimeItems::new(fmt))?;
    parsed.to_datetime()
}

/// Parses `s` against `items`, requiring all input to be consumed
fn parse<'a>(
    parsed: &mut Parsed,
    s: &str,
    items: impl Iterator<Item = Item<'a>>,
) -> ParseResult<()> {
    match parse_internal(parsed, s, items)? {
        "" => Ok(()),
        _ => Err(TOO_LONG),
    }
}

fn parse_internal<'a, 'b>(
    parsed: &mut Parsed,
    mut s: &'b str,
    items: impl Iterator<Item = Item<'a>>,
) -> ParseResult<&'b str> {
    for item in items {
        s = match item {
            Item::Literal(prefix) => parse_literal(s, prefix)?,
            Item::Space(_) => s.trim_start(),
            Item::Numeric(spec, _) => parse_numeric(parsed, s, spec)?,
            Item::Fixed(spec) => parse_fixed(parsed, s, spec)?,
            Item::Error => return Err(BAD_FORMAT),
        };
    }
    Ok(s)
}

fn parse_literal<'b>(s: &'b str, prefix: &str) -> ParseResult<&'b str> {
    if s.len() < prefix.len() {
        return Err(TOO_SHORT);
    }
    match s.strip_prefix(prefix) {
        Some(rest) => Ok(rest),
        None => Err(INVALID),
    }
}

type Setter = fn(&mut Parsed, i64) -> ParseResult<()>;

fn numeric_setter(spec: Numeric) -> (usize, bool, Setter) {
    use Numeric::*;
    match spec {
        Year => (4, true, Parsed::set_year),
        YearDiv100 => (2, false, Parsed::set_year_div_100),
        YearMod100 => (2, false, Parsed::set_year_mod_100),
        IsoYear => (4, true, Parsed::set_isoyear),
        IsoYearDiv100 => (2, false, Parsed::set_isoyear_div_100),
        IsoYearMod100 => (2, false, Parsed::set_isoyear_mod_100),
        Month => (2, false, Parsed::set_month),
        Day => (2, false, Parsed::set_day),
        WeekFromSun => (2, false, Parsed::set_week_from_sun),
        WeekFromMon => (2, false, Parsed::set_week_from_mon),
        IsoWeek => (2, false, Parsed::set_isoweek),
        NumDaysFromSun => (1, false, set_weekday_with_num_days_from_sunday),
        WeekdayFromMon => (1, false, set_weekday_with_number_from_monday),
        Ordinal => (3, false, Parsed::set_ordinal),
        Hour => (2, false, Parsed::set_hour),
        Hour12 => (2, false, Parsed::set_hour12),
        Minute => (2, false, Parsed::set_minute),
        Second => (2, false, Parsed::set_second),
        Nanosecond => (9, false, Parsed::set_nanosecond),
        Timestamp => (usize::MAX, false, Parsed::set_timestamp),
    }
}

fn parse_numeric<'b>(parsed: &mut Parsed, s: &'b str, spec: Numeric) -> ParseResult<&'b str> {
    let (width, signed, set) = numeric_setter(spec);
    let s = s.trim_start();
    let (s, v) = match (signed, s.as_bytes().first()) {
        (true, Some(b'-')) => {
            let (s, v) = scan::number(&s[1..], 1, usize::MAX)?;
            (s, 0i64.checked_sub(v).ok_or(OUT_OF_RANGE)?)
        }
        (true, Some(b'+')) => scan::number(&s[1..], 1, usize::MAX)?,
        _ => scan::number(s, 1, width)?,
    };
    set(parsed, v)?;
    Ok(s)
}

fn parse_nanos_no_dot<'b>(parsed: &mut Parsed, s: &'b str, digits: usize) -> ParseResult<&'b str> {
    if s.len() < digits {
        return Err(TOO_SHORT);
    }
    let (s, nano) = scan::nanosecond_fixed(s, digits)?;
    parsed.set_nanosecond(nano)?;
    Ok(s)
}

fn parse_offset<'b>(
    parsed: &mut Parsed,
    s: &'b str,
    allow_zulu: bool,
    allow_missing_minutes: bool,
) -> ParseResult<&'b str> {
    let (s, offset) = scan::timezone_offset(s.trim_start(), allow_zulu, allow_missing_minutes)?;
    parsed.set_offset(i64::from(offset))?;
    Ok(s)
}

fn parse_fixed<'b>(parsed: &mut Parsed, s: &'b str, spec: Fixed) -> ParseResult<&'b str> {
    use Fixed::*;
    match spec {
        ShortMonthName => {
            let (s, month0) = scan::short_month0(s)?;
            parsed.set_month(i64::from(month0) + 1)?;
            Ok(s)
        }
        LongMonthName => {
            let (s, month0) = scan::short_or_long_month0(s)?;
            parsed.set_month(i64::from(month0) + 1)?;
            Ok(s)
        }
        ShortWeekdayName => {
            let (s, weekday) = scan::short_weekday(s)?;
            parsed.set_weekday(weekday)?;
            Ok(s)
        }
        LongWeekdayName => {
            let (s, weekday) = scan::short_or_long_weekday(s)?;
            parsed.set_weekday(weekday)?;
            Ok(s)
        }
        LowerAmPm | UpperAmPm => parse_ampm(parsed, s),
        Nanosecond | Nanosecond3 | Nanosecond6 | Nanosecond9 => match s.strip_prefix('.') {
            Some(rest) => {
                let (s, nano) = scan::nanosecond(rest)?;
                parsed.set_nanosecond(nano)?;
                Ok(s)
            }
            None => Ok(s),
        },
        Nanosecond3NoDot => parse_nanos_no_dot(parsed, s, 3),
        Nanosecond6NoDot => parse_nanos_no_dot(parsed, s, 6),
        Nanosecond9NoDot => parse_nanos_no_dot(parsed, s, 9),
        TimezoneName => Ok(s.trim_start_matches(|c: char| !c.is_whitespace())),
        TimezoneOffsetColon
        | TimezoneOffsetDoubleColon
        | TimezoneOffsetTripleColon
        | TimezoneOffset => parse_offset(parsed, s, false, false),
        TimezoneOffsetPermissive => parse_offset(parsed, s, true, true),
        RFC3339 => parse_rfc3339_relaxed(parsed, s),
    }
}

fn parse_ampm<'b>(parsed: &mut Parsed, s: &'b str) -> ParseResult<&'b str> {
    if s.len() < 2 {
        return Err(TOO_SHORT);
    }
    let pm = match (s.as_bytes()[0] | 32, s.as_bytes()[1] | 32) {
        (b'a', b'm') => false,
        (b'p', b'm') => true,
        _ => return Err(INVALID),
    };
    parsed.set_ampm(pm)?;
    Ok(&s[2..])
}

/// The relaxed RFC 3339 grammar used by `%+`
fn parse_rfc3339_relaxed<'b>(parsed: &mut Parsed, s: &'b str) -> ParseResult<&'b str> {
    use super::format::Pad;
    const DATE_ITEMS: &[Item<'static>] = &[
        Item::Numeric(Numeric::Year, Pad::Zero),
        Item::Space(""),
        Item::Literal("-"),
        Item::Numeric(Numeric::Month, Pad::Zero),
        Item::Space(""),
        Item::Literal("-"),
        Item::Numeric(Numeric::Day, Pad::Zero),
    ];
    const TIME_ITEMS: &[Item<'static>] = &[
        Item::Numeric(Numeric::Hour, Pad::Zero),
        Item::Space(""),
        Item::Literal(":"),
        Item::Numeric(Numeric::Minute, Pad::Zero),
        Item::Space(""),
        Item::Literal(":"),
        Item::Numeric(Numeric::Second, Pad::Zero),
        Item::Fixed(Fixed::Nanosecond),
        Item::Space(""),
    ];
    let s = parse_internal(parsed, s, DATE_ITEMS.iter().copied())?;
    let s = match s.as_bytes().first() {
        Some(b't' | b'T' | b' ') => &s[1..],
        Some(_) => return Err(INVALID),
        None => return Err(TOO_SHORT),
    };
    let s = parse_internal(parsed, s, TIME_ITEMS.iter().copied())?;
    let s = s.trim_start();
    let (s, offset) = match s.len() >= 3 && s.as_bytes()[..3].eq_ignore_ascii_case(b"UTC") {
        true => (&s[3..], 0),
        false => scan::timezone_offset(s, true, false)?,
    };
    parsed.set_offset(i64::from(offset))?;
    Ok(s)
}

fn set_weekday_with_num_days_from_sunday(p: &mut Parsed, v: i64) -> ParseResult<()> {
    match v {
        0..=6 => p.set_weekday(Weekday::from_monday0(v - 1)),
        _ => Err(OUT_OF_RANGE),
    }
}

fn set_weekday_with_number_from_monday(p: &mut Parsed, v: i64) -> ParseResult<()> {
    match v {
        1..=7 => p.set_weekday(Weekday::from_monday0(v - 1)),
        _ => Err(OUT_OF_RANGE),
    }
}

fn set_if_consistent<T: PartialEq>(old: &mut Option<T>, new: T) -> ParseResult<()> {
    match old {
        Some(old) if *old == new => Ok(()),
        Some(_) => Err(IMPOSSIBLE),
        None => {
            *old = Some(new);
            Ok(())
        }
    }
}

fn set_ranged<T: PartialEq + TryFrom<i64>>(
    field: &mut Option<T>,
    value: i64,
    range: std::ops::RangeInclusive<i64>,
) -> ParseResult<()> {
    if !range.contains(&value) {
        return Err(OUT_OF_RANGE);
    }
    let value = T::try_from(value).map_err(|_| OUT_OF_RANGE)?;
    set_if_consistent(field, value)
}

/// Fields collected while parsing, resolved like `chrono::format::Parsed`
#[derive(Clone, PartialEq, Eq, Debug, Default)]
struct Parsed {
    year: Option<i32>,
    year_div_100: Option<i32>,
    year_mod_100: Option<i32>,
    isoyear: Option<i32>,
    isoyear_div_100: Option<i32>,
    isoyear_mod_100: Option<i32>,
    month: Option<u32>,
    week_from_sun: Option<u32>,
    week_from_mon: Option<u32>,
    isoweek: Option<u32>,
    weekday: Option<Weekday>,
    ordinal: Option<u32>,
    day: Option<u32>,
    hour_div_12: Option<u32>,
    hour_mod_12: Option<u32>,
    minute: Option<u32>,
    second: Option<u32>,
    nanosecond: Option<u32>,
    timestamp: Option<i64>,
    offset: Option<i32>,
}

const I32_RANGE: std::ops::RangeInclusive<i64> = i32::MIN as i64..=i32::MAX as i64;
const U31_RANGE: std::ops::RangeInclusive<i64> = 0..=i32::MAX as i64;

impl Parsed {
    fn set_year(&mut self, v: i64) -> ParseResult<()> {
        set_ranged(&mut self.year, v, I32_RANGE)
    }
    fn set_year_div_100(&mut self, v: i64) -> ParseResult<()> {
        set_ranged(&mut self.year_div_100, v, U31_RANGE)
    }
    fn set_year_mod_100(&mut self, v: i64) -> ParseResult<()> {
        set_ranged(&mut self.year_mod_100, v, 0..=99)
    }
    fn set_isoyear(&mut self, v: i64) -> ParseResult<()> {
        set_ranged(&mut self.isoyear, v, I32_RANGE)
    }
    fn set_isoyear_div_100(&mut self, v: i64) -> ParseResult<()> {
        set_ranged(&mut self.isoyear_div_100, v, U31_RANGE)
    }
    fn set_isoyear_mod_100(&mut self, v: i64) -> ParseResult<()> {
        set_ranged(&mut self.isoyear_mod_100, v, 0..=99)
    }
    fn set_month(&mut self, v: i64) -> ParseResult<()> {
        set_ranged(&mut self.month, v, 1..=12)
    }
    fn set_week_from_sun(&mut self, v: i64) -> ParseResult<()> {
        set_ranged(&mut self.week_from_sun, v, 0..=53)
    }
    fn set_week_from_mon(&mut self, v: i64) -> ParseResult<()> {
        set_ranged(&mut self.week_from_mon, v, 0..=53)
    }
    fn set_isoweek(&mut self, v: i64) -> ParseResult<()> {
        set_ranged(&mut self.isoweek, v, 1..=53)
    }
    fn set_weekday(&mut self, v: Weekday) -> ParseResult<()> {
        set_if_consistent(&mut self.weekday, v)
    }
    fn set_ordinal(&mut self, v: i64) -> ParseResult<()> {
        set_ranged(&mut self.ordinal, v, 1..=366)
    }
    fn set_day(&mut self, v: i64) -> ParseResult<()> {
        set_ranged(&mut self.day, v, 1..=31)
    }
    fn set_ampm(&mut self, pm: bool) -> ParseResult<()> {
        set_if_consistent(&mut self.hour_div_12, pm as u32)
    }
    fn set_hour12(&mut self, v: i64) -> ParseResult<()> {
        if !(1..=12).contains(&v) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.hour_mod_12, (v % 12) as u32)
    }
    fn set_hour(&mut self, v: i64) -> ParseResult<()> {
        if !(0..=23).contains(&v) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.hour_div_12, (v / 12) as u32)?;
        set_if_consistent(&mut self.hour_mod_12, (v % 12) as u32)
    }
    fn set_minute(&mut self, v: i64) -> ParseResult<()> {
        set_ranged(&mut self.minute, v, 0..=59)
    }
    fn set_second(&mut self, v: i64) -> ParseResult<()> {
        set_ranged(&mut self.second, v, 0..=60)
    }
    fn set_nanosecond(&mut self, v: i64) -> ParseResult<()> {
        set_ranged(&mut self.nanosecond, v, 0..=999_999_999)
    }
    fn set_timestamp(&mut self, v: i64) -> ParseResult<()> {
        set_if_consistent(&mut self.timestamp, v)
    }
    fn set_offset(&mut self, v: i64) -> ParseResult<()> {
        set_ranged(&mut self.offset, v, I32_RANGE)
    }

    fn verify_ymd(&self, date: CivilDate) -> bool {
        let year = date.year();
        let (div, rem) = match year >= 0 {
            true => (Some(year / 100), Some(year % 100)),
            false => (None, None),
        };
        self.year.unwrap_or(year) == year
            && self.year_div_100.or(div) == div
            && self.year_mod_100.or(rem) == rem
            && self.month.unwrap_or(date.month()) == date.month()
            && self.day.unwrap_or(date.day()) == date.day()
    }

    fn verify_isoweekdate(&self, date: CivilDate) -> bool {
        let (isoyear, isoweek) = date.iso_week();
        let (div, rem) = match isoyear >= 0 {
            true => (Some(isoyear / 100), Some(isoyear % 100)),
            false => (None, None),
        };
        self.isoyear.unwrap_or(isoyear) == isoyear
            && self.isoyear_div_100.or(div) == div
            && self.isoyear_mod_100.or(rem) == rem
            && self.isoweek.unwrap_or(isoweek) == isoweek
            && self.weekday.unwrap_or(date.weekday()) == date.weekday()
    }

    fn verify_ordinal(&self, date: CivilDate) -> bool {
        let week_from_sun = date.weeks_from(Weekday::Sun);
        let week_from_mon = date.weeks_from(Weekday::Mon);
        self.ordinal.unwrap_or(date.ordinal()) == date.ordinal()
            && self.week_from_sun.map_or(week_from_sun, |v| v as i32) == week_from_sun
            && self.week_from_mon.map_or(week_from_mon, |v| v as i32) == week_from_mon
    }

    fn to_naive_date(&self) -> ParseResult<CivilDate> {
        let given_year = resolve_year(self.year, self.year_div_100, self.year_mod_100)?;
        let given_isoyear = resolve_year(self.isoyear, self.isoyear_div_100, self.isoyear_mod_100)?;
        let (verified, date) = match (given_year, given_isoyear, self) {
            (
                Some(year),
                _,
                Parsed {
                    month: Some(month),
                    day: Some(day),
                    ..
                },
            ) => {
                let date = CivilDate::from_ymd_opt(year, *month, *day).ok_or(OUT_OF_RANGE)?;
                (
                    self.verify_isoweekdate(date) && self.verify_ordinal(date),
                    date,
                )
            }
            (
                Some(year),
                _,
                Parsed {
                    ordinal: Some(ordinal),
                    ..
                },
            ) => {
                let date = CivilDate::from_yo_opt(year, *ordinal).ok_or(OUT_OF_RANGE)?;
                (self.verify_all(date), date)
            }
            (
                Some(year),
                _,
                Parsed {
                    week_from_sun: Some(week),
                    weekday: Some(wd),
                    ..
                },
            ) => {
                let date = resolve_week_date(year, *week, *wd, Weekday::Sun)?;
                (self.verify_all(date), date)
            }
            (
                Some(year),
                _,
                Parsed {
                    week_from_mon: Some(week),
                    weekday: Some(wd),
                    ..
                },
            ) => {
                let date = resolve_week_date(year, *week, *wd, Weekday::Mon)?;
                (self.verify_all(date), date)
            }
            (
                _,
                Some(isoyear),
                Parsed {
                    isoweek: Some(week),
                    weekday: Some(wd),
                    ..
                },
            ) => {
                let date = CivilDate::from_isoywd_opt(isoyear, *week, *wd).ok_or(OUT_OF_RANGE)?;
                (self.verify_ymd(date) && self.verify_ordinal(date), date)
            }
            _ => return Err(NOT_ENOUGH),
        };
        match verified {
            true => Ok(date),
            false => Err(IMPOSSIBLE),
        }
    }

    fn verify_all(&self, date: CivilDate) -> bool {
        self.verify_ymd(date) && self.verify_isoweekdate(date) && self.verify_ordinal(date)
    }

    fn to_naive_time(&self) -> ParseResult<CivilTime> {
        let field = |v: Option<u32>, max: u32| match v {
            Some(v) if v <= max => Ok(v),
            Some(_) => Err(OUT_OF_RANGE),
            None => Err(NOT_ENOUGH),
        };
        let hour = field(self.hour_div_12, 1)? * 12 + field(self.hour_mod_12, 11)?;
        let minute = field(self.minute, 59)?;
        let (second, mut nano) = match self.second.unwrap_or(0) {
            v @ 0..=59 => (v, 0),
            60 => (59, 1_000_000_000),
            _ => return Err(OUT_OF_RANGE),
        };
        nano += match self.nanosecond {
            Some(v @ 0..=999_999_999) if self.second.is_some() => v,
            Some(0..=999_999_999) => return Err(NOT_ENOUGH),
            Some(_) => return Err(OUT_OF_RANGE),
            None => 0,
        };
        CivilTime::from_hms_nano_opt(hour, minute, second, nano).ok_or(OUT_OF_RANGE)
    }

    fn to_naive_datetime_with_offset(&self, offset: i32) -> ParseResult<CivilDateTime> {
        let date = self.to_naive_date();
        let time = self.to_naive_time();
        match (date, time, self.timestamp) {
            (Ok(date), Ok(time), given) => {
                let datetime = CivilDateTime::new(date, time);
                let timestamp = datetime.timestamp() - i64::from(offset);
                let leap = datetime.time.nanosecond() >= 1_000_000_000;
                match given {
                    Some(g) if g != timestamp && !(leap && g == timestamp + 1) => Err(IMPOSSIBLE),
                    _ => Ok(datetime),
                }
            }
            (date, time, Some(timestamp)) => {
                self.datetime_from_timestamp(date, time, timestamp, offset)
            }
            (date, time, None) => {
                date?;
                time?;
                unreachable!("either date or time failed to resolve")
            }
        }
    }

    /// Reconstructs date and time fields from a timestamp (`%s`)
    fn datetime_from_timestamp(
        &self,
        date: ParseResult<CivilDate>,
        time: ParseResult<CivilTime>,
        timestamp: i64,
        offset: i32,
    ) -> ParseResult<CivilDateTime> {
        let kinds = [date.err(), time.err()].map(|e| e.map(|e| e.kind()));
        if kinds.contains(&Some(ParseErrorKind::OutOfRange)) {
            return Err(OUT_OF_RANGE);
        }
        if kinds.contains(&Some(ParseErrorKind::Impossible)) {
            return Err(IMPOSSIBLE);
        }
        let ts = timestamp
            .checked_add(i64::from(offset))
            .ok_or(OUT_OF_RANGE)?;
        let mut datetime = CivilDateTime::from_timestamp(ts, 0).ok_or(OUT_OF_RANGE)?;
        let mut parsed = self.clone();
        if parsed.second == Some(60) {
            match datetime.time.second() {
                59 => {}
                0 => {
                    datetime = datetime
                        .checked_add_nanos(-1_000_000_000)
                        .expect("`NaiveDateTime - TimeDelta` overflowed");
                }
                _ => return Err(IMPOSSIBLE),
            }
        } else {
            parsed.set_second(i64::from(datetime.time.second()))?;
        }
        parsed.set_year(i64::from(datetime.date.year()))?;
        parsed.set_ordinal(i64::from(datetime.date.ordinal()))?;
        parsed.set_hour(i64::from(datetime.time.hour()))?;
        parsed.set_minute(i64::from(datetime.time.minute()))?;
        let date = parsed.to_naive_date()?;
        let time = parsed.to_naive_time()?;
        Ok(CivilDateTime::new(date, time))
    }

    fn to_datetime(&self) -> ParseResult<ZonedDateTime> {
        let offset = match (self.offset, self.timestamp) {
            (Some(off), _) => off,
            (None, Some(_)) => 0,
            (None, None) => return Err(NOT_ENOUGH),
        };
        let local = self.to_naive_datetime_with_offset(offset)?;
        let offset = fixed_offset_opt(offset).ok_or(OUT_OF_RANGE)?;
        let utc = local.checked_sub_offset(offset).ok_or(IMPOSSIBLE)?;
        Ok(ZonedDateTime { utc, offset })
    }
}

fn resolve_year(y: Option<i32>, q: Option<i32>, r: Option<i32>) -> ParseResult<Option<i32>> {
    match (y, q, r) {
        (y, None, None) => Ok(y),
        (Some(y), q, r @ (Some(0..=99) | None)) => {
            if y < 0 {
                return Err(IMPOSSIBLE);
            }
            let (q_, r_) = (y / 100, y % 100);
            match q.unwrap_or(q_) == q_ && r.unwrap_or(r_) == r_ {
                true => Ok(Some(y)),
                false => Err(IMPOSSIBLE),
            }
        }
        (None, Some(q), Some(r @ 0..=99)) => {
            if q < 0 {
                return Err(IMPOSSIBLE);
            }
            let y = q.checked_mul(100).and_then(|v| v.checked_add(r));
            Ok(Some(y.ok_or(OUT_OF_RANGE)?))
        }
        (None, None, Some(r @ 0..=99)) => Ok(Some(r + if r < 70 { 2000 } else { 1900 })),
        (None, Some(_), None) => Err(NOT_ENOUGH),
        (_, _, Some(_)) => Err(OUT_OF_RANGE),
    }
}

fn resolve_week_date(
    year: i32,
    week: u32,
    weekday: Weekday,
    week_start_day: Weekday,
) -> ParseResult<CivilDate> {
    if week > 53 {
        return Err(OUT_OF_RANGE);
    }
    let first_day_of_year = CivilDate::from_yo_opt(year, 1).ok_or(OUT_OF_RANGE)?;
    let first_week_start = 1 + week_start_day.days_since(first_day_of_year.weekday()) as i32;
    let weekday = weekday.days_since(week_start_day) as i32;
    let ordinal = first_week_start + (week as i32 - 1) * 7 + weekday;
    if ordinal <= 0 {
        return Err(IMPOSSIBLE);
    }
    CivilDate::from_yo_opt(year, ordinal as u32).ok_or(IMPOSSIBLE)
}

/// Returns true if the format string contains a time zone item, the test
/// arrow uses to decide between naive and offset-aware parsing
pub fn format_has_zone(fmt: &str) -> bool {
    StrftimeItems::new(fmt).any(|item| {
        matches!(
            item,
            Item::Fixed(
                Fixed::RFC3339
                    | Fixed::TimezoneName
                    | Fixed::TimezoneOffsetColon
                    | Fixed::TimezoneOffset
            )
        )
    })
}
