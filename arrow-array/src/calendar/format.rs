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

//! `strftime`-style format strings with exactly the syntax and output of
//! `chrono` 0.4.39 (`chrono::format::strftime`, English locale).
//!
//! Adapted from chrono (<https://github.com/chronotope/chrono>), which is
//! dual licensed MIT / Apache-2.0. Arrow format options have always taken
//! chrono format strings, so the syntax is preserved verbatim rather than
//! switched to `jiff::fmt::strtime`, whose specifiers differ (`%f`, `%.f`,
//! `%c`, `%+`, ...).

use super::date::{write_hundreds, write_iso_year, CivilDate, Weekday};
use super::time::{write_auto_si_fraction, CivilDateTime, CivilTime, OffsetDisplay};
use std::fmt::{self, Write};

/// Padding for numeric items
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum Pad {
    /// No padding
    None,
    /// Zero padding
    Zero,
    /// Space padding
    Space,
}

/// Numeric format items
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum Numeric {
    /// `%Y`
    Year,
    /// `%C`
    YearDiv100,
    /// `%y`
    YearMod100,
    /// `%G`
    IsoYear,
    /// `%g` (div 100 form is unused by strftime but kept for parsing)
    IsoYearDiv100,
    /// `%g`
    IsoYearMod100,
    /// `%m`
    Month,
    /// `%d`, `%e`
    Day,
    /// `%U`
    WeekFromSun,
    /// `%W`
    WeekFromMon,
    /// `%V`
    IsoWeek,
    /// `%w`
    NumDaysFromSun,
    /// `%u`
    WeekdayFromMon,
    /// `%j`
    Ordinal,
    /// `%H`, `%k`
    Hour,
    /// `%I`, `%l`
    Hour12,
    /// `%M`
    Minute,
    /// `%S`
    Second,
    /// `%f`
    Nanosecond,
    /// `%s`
    Timestamp,
}

/// Fixed-format items
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum Fixed {
    /// `%b`
    ShortMonthName,
    /// `%B`
    LongMonthName,
    /// `%a`
    ShortWeekdayName,
    /// `%A`
    LongWeekdayName,
    /// `%P`
    LowerAmPm,
    /// `%p`
    UpperAmPm,
    /// `%.f`
    Nanosecond,
    /// `%.3f`
    Nanosecond3,
    /// `%.6f`
    Nanosecond6,
    /// `%.9f`
    Nanosecond9,
    /// `%3f`
    Nanosecond3NoDot,
    /// `%6f`
    Nanosecond6NoDot,
    /// `%9f`
    Nanosecond9NoDot,
    /// `%Z`
    TimezoneName,
    /// `%:z`
    TimezoneOffsetColon,
    /// `%::z`
    TimezoneOffsetDoubleColon,
    /// `%:::z`
    TimezoneOffsetTripleColon,
    /// `%z`
    TimezoneOffset,
    /// `%#z` (parsing only)
    TimezoneOffsetPermissive,
    /// `%+`
    RFC3339,
}

/// A single format item
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum Item<'a> {
    /// Literal text
    Literal(&'a str),
    /// Whitespace
    Space(&'a str),
    /// A numeric value with padding
    Numeric(Numeric, Pad),
    /// A fixed-format value
    Fixed(Fixed),
    /// An invalid specifier
    Error,
}

const fn num(n: Numeric) -> Item<'static> {
    Item::Numeric(n, Pad::None)
}
const fn num0(n: Numeric) -> Item<'static> {
    Item::Numeric(n, Pad::Zero)
}
const fn nums(n: Numeric) -> Item<'static> {
    Item::Numeric(n, Pad::Space)
}
const fn fixed(f: Fixed) -> Item<'static> {
    Item::Fixed(f)
}

use Numeric::*;

static D_FMT: &[Item<'static>] = &[
    num0(Month),
    Item::Literal("/"),
    num0(Day),
    Item::Literal("/"),
    num0(YearMod100),
];
static D_T_FMT: &[Item<'static>] = &[
    fixed(Fixed::ShortWeekdayName),
    Item::Space(" "),
    fixed(Fixed::ShortMonthName),
    Item::Space(" "),
    nums(Day),
    Item::Space(" "),
    num0(Hour),
    Item::Literal(":"),
    num0(Minute),
    Item::Literal(":"),
    num0(Second),
    Item::Space(" "),
    num0(Year),
];
static T_FMT: &[Item<'static>] = &[
    num0(Hour),
    Item::Literal(":"),
    num0(Minute),
    Item::Literal(":"),
    num0(Second),
];
static T_FMT_AMPM: &[Item<'static>] = &[
    num0(Hour12),
    Item::Literal(":"),
    num0(Minute),
    Item::Literal(":"),
    num0(Second),
    Item::Space(" "),
    fixed(Fixed::UpperAmPm),
];
static F_FMT: &[Item<'static>] = &[
    num0(Year),
    Item::Literal("-"),
    num0(Month),
    Item::Literal("-"),
    num0(Day),
];
static R_FMT: &[Item<'static>] = &[num0(Hour), Item::Literal(":"), num0(Minute)];
static V_FMT: &[Item<'static>] = &[
    nums(Day),
    Item::Literal("-"),
    fixed(Fixed::ShortMonthName),
    Item::Literal("-"),
    num0(Year),
];

/// Iterator over the items of a `chrono` strftime format string
#[derive(Clone, Debug)]
pub struct StrftimeItems<'a> {
    remainder: &'a str,
    queue: &'static [Item<'static>],
}

impl<'a> StrftimeItems<'a> {
    /// Creates an iterator over the items in `s`
    pub const fn new(s: &'a str) -> Self {
        Self {
            remainder: s,
            queue: &[],
        }
    }
}

impl<'a> Iterator for StrftimeItems<'a> {
    type Item = Item<'a>;

    fn next(&mut self) -> Option<Item<'a>> {
        if let Some((item, rest)) = self.queue.split_first() {
            self.queue = rest;
            return Some(*item);
        }
        let (remainder, item) = self.parse_next_item(self.remainder)?;
        self.remainder = remainder;
        Some(item)
    }
}

/// Takes the next char of `s`, or `None` at end of input
fn next_char(s: &str) -> Option<(char, &str)> {
    let c = s.chars().next()?;
    Some((c, &s[c.len_utf8()..]))
}

impl<'a> StrftimeItems<'a> {
    fn queue_slice(&mut self, slice: &'static [Item<'static>]) -> Item<'a> {
        self.queue = &slice[1..];
        slice[0]
    }

    fn parse_next_item(&mut self, remainder: &'a str) -> Option<(&'a str, Item<'a>)> {
        match remainder.chars().next() {
            None => None,
            Some('%') => Some(self.parse_specifier(&remainder[1..])),
            Some(c) if c.is_whitespace() => {
                let end = remainder
                    .find(|c: char| !c.is_whitespace())
                    .unwrap_or(remainder.len());
                Some((&remainder[end..], Item::Space(&remainder[..end])))
            }
            Some(_) => {
                let end = remainder
                    .find(|c: char| c.is_whitespace() || c == '%')
                    .unwrap_or(remainder.len());
                Some((&remainder[end..], Item::Literal(&remainder[..end])))
            }
        }
    }

    fn parse_specifier(&mut self, remainder: &'a str) -> (&'a str, Item<'a>) {
        let Some((spec, mut remainder)) = next_char(remainder) else {
            return (remainder, Item::Error);
        };
        let pad_override = match spec {
            '-' => Some(Pad::None),
            '0' => Some(Pad::Zero),
            '_' => Some(Pad::Space),
            _ => None,
        };
        let is_alternate = spec == '#';
        let spec = match pad_override.is_some() || is_alternate {
            true => match next_char(remainder) {
                Some((c, rest)) => {
                    remainder = rest;
                    c
                }
                None => return (remainder, Item::Error),
            },
            false => spec,
        };
        if is_alternate && spec != 'z' {
            return (remainder, Item::Error);
        }
        let (remainder, item) = self.specifier_item(spec, is_alternate, remainder);
        match pad_override {
            None => (remainder, item),
            Some(pad) => match item {
                Item::Numeric(kind, _) if self.queue.is_empty() => {
                    (remainder, Item::Numeric(kind, pad))
                }
                _ => (remainder, Item::Error),
            },
        }
    }

    fn specifier_item(
        &mut self,
        spec: char,
        is_alternate: bool,
        remainder: &'a str,
    ) -> (&'a str, Item<'a>) {
        let item = match spec {
            'A' => fixed(Fixed::LongWeekdayName),
            'B' => fixed(Fixed::LongMonthName),
            'C' => num0(YearDiv100),
            'D' => self.queue_slice(D_FMT),
            'F' => self.queue_slice(F_FMT),
            'G' => num0(IsoYear),
            'H' => num0(Hour),
            'I' => num0(Hour12),
            'M' => num0(Minute),
            'P' => fixed(Fixed::LowerAmPm),
            'R' => self.queue_slice(R_FMT),
            'S' => num0(Second),
            'T' => self.queue_slice(T_FMT),
            'U' => num0(WeekFromSun),
            'V' => num0(IsoWeek),
            'W' => num0(WeekFromMon),
            'X' => self.queue_slice(T_FMT),
            'Y' => num0(Year),
            'Z' => fixed(Fixed::TimezoneName),
            'a' => fixed(Fixed::ShortWeekdayName),
            'b' | 'h' => fixed(Fixed::ShortMonthName),
            'c' => self.queue_slice(D_T_FMT),
            'd' => num0(Day),
            'e' => nums(Day),
            'f' => num0(Nanosecond),
            'g' => num0(IsoYearMod100),
            'j' => num0(Ordinal),
            'k' => nums(Hour),
            'l' => nums(Hour12),
            'm' => num0(Month),
            'n' => Item::Space("\n"),
            'p' => fixed(Fixed::UpperAmPm),
            'r' => self.queue_slice(T_FMT_AMPM),
            's' => num(Timestamp),
            't' => Item::Space("\t"),
            'u' => num(WeekdayFromMon),
            'v' => self.queue_slice(V_FMT),
            'w' => num(NumDaysFromSun),
            'x' => self.queue_slice(D_FMT),
            'y' => num0(YearMod100),
            'z' if is_alternate => fixed(Fixed::TimezoneOffsetPermissive),
            'z' => fixed(Fixed::TimezoneOffset),
            '+' => fixed(Fixed::RFC3339),
            ':' => return colon_specifier(remainder),
            '.' => return dot_specifier(remainder),
            '3' | '6' | '9' => return nodot_specifier(spec, remainder),
            '%' => Item::Literal("%"),
            _ => Item::Error,
        };
        (remainder, item)
    }
}

fn colon_specifier(remainder: &str) -> (&str, Item<'_>) {
    if let Some(rest) = remainder.strip_prefix("::z") {
        (rest, fixed(Fixed::TimezoneOffsetTripleColon))
    } else if let Some(rest) = remainder.strip_prefix(":z") {
        (rest, fixed(Fixed::TimezoneOffsetDoubleColon))
    } else if let Some(rest) = remainder.strip_prefix('z') {
        (rest, fixed(Fixed::TimezoneOffsetColon))
    } else {
        (remainder, Item::Error)
    }
}

fn dot_specifier(remainder: &str) -> (&str, Item<'_>) {
    let Some((c, rest)) = next_char(remainder) else {
        return (remainder, Item::Error);
    };
    let digits = match c {
        'f' => return (rest, fixed(Fixed::Nanosecond)),
        '3' => Fixed::Nanosecond3,
        '6' => Fixed::Nanosecond6,
        '9' => Fixed::Nanosecond9,
        _ => return (rest, Item::Error),
    };
    match next_char(rest) {
        Some(('f', rest)) => (rest, fixed(digits)),
        Some((_, rest)) => (rest, Item::Error),
        None => (rest, Item::Error),
    }
}

fn nodot_specifier(spec: char, remainder: &str) -> (&str, Item<'_>) {
    let item = match spec {
        '3' => Fixed::Nanosecond3NoDot,
        '6' => Fixed::Nanosecond6NoDot,
        _ => Fixed::Nanosecond9NoDot,
    };
    match next_char(remainder) {
        Some(('f', rest)) => (rest, fixed(item)),
        Some((_, rest)) => (rest, Item::Error),
        None => (remainder, Item::Error),
    }
}

const SHORT_MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const LONG_MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const SHORT_WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const LONG_WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

/// The values a format string is rendered from
///
/// Mirrors `chrono::format::DelayedFormat`: specifiers needing a component
/// that is absent fail with [`fmt::Error`]. The offset's `%Z` name is its
/// fixed-offset rendering, as with arrow's previous `TzOffset`.
#[derive(Copy, Clone, Debug)]
pub struct FormatInput {
    date: Option<CivilDate>,
    time: Option<CivilTime>,
    offset: Option<i32>,
}

impl FormatInput {
    /// Only a date
    pub const fn date(date: CivilDate) -> Self {
        Self {
            date: Some(date),
            time: None,
            offset: None,
        }
    }

    /// Only a time
    pub const fn time(time: CivilTime) -> Self {
        Self {
            date: None,
            time: Some(time),
            offset: None,
        }
    }

    /// A naive datetime
    pub const fn datetime(dt: CivilDateTime) -> Self {
        Self {
            date: Some(dt.date),
            time: Some(dt.time),
            offset: None,
        }
    }

    /// A local datetime with its offset from UTC in seconds
    pub const fn zoned(local: CivilDateTime, offset: i32) -> Self {
        Self {
            date: Some(local.date),
            time: Some(local.time),
            offset: Some(offset),
        }
    }
}

/// A value paired with a format string, rendered on `Display`
#[derive(Clone, Debug)]
pub struct DelayedFormat<'a> {
    input: FormatInput,
    format: &'a str,
}

impl<'a> DelayedFormat<'a> {
    /// Renders `input` with the chrono strftime `format`
    pub const fn new(input: FormatInput, format: &'a str) -> Self {
        Self { input, format }
    }
}

impl fmt::Display for DelayedFormat<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut result = String::new();
        format_items(&mut result, &self.input, StrftimeItems::new(self.format))?;
        f.pad(&result)
    }
}

/// Renders `items` into `w`
pub fn format_items<'a>(
    w: &mut impl Write,
    input: &FormatInput,
    items: impl Iterator<Item = Item<'a>>,
) -> fmt::Result {
    for item in items {
        match item {
            Item::Literal(s) | Item::Space(s) => w.write_str(s),
            Item::Numeric(spec, pad) => format_numeric(w, input, spec, pad),
            Item::Fixed(spec) => format_fixed(w, input, spec),
            Item::Error => Err(fmt::Error),
        }?;
    }
    Ok(())
}

fn write_two(w: &mut impl Write, v: u8, pad: Pad) -> fmt::Result {
    let ones = b'0' + v % 10;
    match (v / 10, pad) {
        (0, Pad::None) => {}
        (0, Pad::Space) => w.write_char(' ')?,
        (tens, _) => w.write_char((b'0' + tens) as char)?,
    }
    w.write_char(ones as char)
}

fn write_n(w: &mut impl Write, n: usize, v: i64, pad: Pad, always_sign: bool) -> fmt::Result {
    match (always_sign, pad) {
        (true, Pad::None) => write!(w, "{:+}", v),
        (true, Pad::Zero) => write!(w, "{:+01$}", v, n + 1),
        (true, Pad::Space) => write!(w, "{:+1$}", v, n + 1),
        (false, Pad::None) => write!(w, "{}", v),
        (false, Pad::Zero) => write!(w, "{:01$}", v, n),
        (false, Pad::Space) => write!(w, "{:1$}", v, n),
    }
}

fn write_year(w: &mut impl Write, year: i32, pad: Pad) -> fmt::Result {
    if (1000..=9999).contains(&year) {
        write_hundreds(w, (year / 100) as u8)?;
        write_hundreds(w, (year % 100) as u8)
    } else {
        write_n(w, 4, year as i64, pad, !(0..10_000).contains(&year))
    }
}

fn format_date_numeric(w: &mut impl Write, d: CivilDate, spec: Numeric, pad: Pad) -> fmt::Result {
    match spec {
        Year => write_year(w, d.year(), pad),
        YearDiv100 => write_two(w, d.year().div_euclid(100) as u8, pad),
        YearMod100 => write_two(w, d.year().rem_euclid(100) as u8, pad),
        IsoYear => write_year(w, d.iso_week().0, pad),
        IsoYearDiv100 => write_two(w, d.iso_week().0.div_euclid(100) as u8, pad),
        IsoYearMod100 => write_two(w, d.iso_week().0.rem_euclid(100) as u8, pad),
        Month => write_two(w, d.month() as u8, pad),
        Day => write_two(w, d.day() as u8, pad),
        WeekFromSun => write_two(w, d.weeks_from(Weekday::Sun) as u8, pad),
        WeekFromMon => write_two(w, d.weeks_from(Weekday::Mon) as u8, pad),
        IsoWeek => write_two(w, d.iso_week().1 as u8, pad),
        NumDaysFromSun => write_two(w, d.weekday().num_days_from_sunday() as u8, Pad::None),
        WeekdayFromMon => write_two(w, d.weekday().number_from_monday() as u8, Pad::None),
        Ordinal => write_n(w, 3, d.ordinal() as i64, pad, false),
        _ => Err(fmt::Error),
    }
}

fn format_time_numeric(w: &mut impl Write, t: CivilTime, spec: Numeric, pad: Pad) -> fmt::Result {
    match spec {
        Hour => write_two(w, t.hour() as u8, pad),
        Hour12 => write_two(w, t.hour12().1 as u8, pad),
        Minute => write_two(w, t.minute() as u8, pad),
        Second => write_two(w, (t.second() + t.nanosecond() / 1_000_000_000) as u8, pad),
        Nanosecond => write_n(w, 9, (t.nanosecond() % 1_000_000_000) as i64, pad, false),
        _ => Err(fmt::Error),
    }
}

fn format_numeric(w: &mut impl Write, input: &FormatInput, spec: Numeric, pad: Pad) -> fmt::Result {
    match (spec, input.date, input.time) {
        (Hour | Hour12 | Minute | Second | Nanosecond, _, Some(t)) => {
            format_time_numeric(w, t, spec, pad)
        }
        (Timestamp, Some(d), Some(t)) => {
            let offset = input.offset.unwrap_or(0) as i64;
            let timestamp = CivilDateTime::new(d, t).timestamp() - offset;
            write_n(w, 9, timestamp, pad, false)
        }
        (Hour | Hour12 | Minute | Second | Nanosecond | Timestamp, _, _) => Err(fmt::Error),
        (_, Some(d), _) => format_date_numeric(w, d, spec, pad),
        (_, None, _) => Err(fmt::Error),
    }
}

fn format_nanos(w: &mut impl Write, t: CivilTime, spec: Fixed) -> fmt::Result {
    let nano = t.nanosecond();
    match spec {
        Fixed::Nanosecond => write_auto_si_fraction(w, nano % 1_000_000_000),
        Fixed::Nanosecond3 => write!(w, ".{:03}", nano / 1_000_000 % 1000),
        Fixed::Nanosecond6 => write!(w, ".{:06}", nano / 1_000 % 1_000_000),
        Fixed::Nanosecond9 => write!(w, ".{:09}", nano % 1_000_000_000),
        Fixed::Nanosecond3NoDot => write!(w, "{:03}", nano / 1_000_000 % 1_000),
        Fixed::Nanosecond6NoDot => write!(w, "{:06}", nano / 1_000 % 1_000_000),
        Fixed::Nanosecond9NoDot => write!(w, "{:09}", nano % 1_000_000_000),
        Fixed::LowerAmPm => w.write_str(if t.hour12().0 { "pm" } else { "am" }),
        Fixed::UpperAmPm => w.write_str(if t.hour12().0 { "PM" } else { "AM" }),
        _ => Err(fmt::Error),
    }
}

fn format_date_fixed(w: &mut impl Write, d: CivilDate, spec: Fixed) -> fmt::Result {
    let weekday = d.weekday().num_days_from_sunday() as usize;
    match spec {
        Fixed::ShortMonthName => w.write_str(SHORT_MONTHS[d.month() as usize - 1]),
        Fixed::LongMonthName => w.write_str(LONG_MONTHS[d.month() as usize - 1]),
        Fixed::ShortWeekdayName => w.write_str(SHORT_WEEKDAYS[weekday]),
        Fixed::LongWeekdayName => w.write_str(LONG_WEEKDAYS[weekday]),
        _ => Err(fmt::Error),
    }
}

fn format_offset_fixed(w: &mut impl Write, off: i32, spec: Fixed) -> fmt::Result {
    let (precision, colons) = match spec {
        Fixed::TimezoneName => return write!(w, "{}", OffsetDisplay(off)),
        Fixed::TimezoneOffset => (OffsetPrecision::Minutes, false),
        Fixed::TimezoneOffsetColon => (OffsetPrecision::Minutes, true),
        Fixed::TimezoneOffsetDoubleColon => (OffsetPrecision::Seconds, true),
        Fixed::TimezoneOffsetTripleColon => (OffsetPrecision::Hours, false),
        _ => return Err(fmt::Error),
    };
    write_offset(w, off, precision, colons, false)
}

fn format_fixed(w: &mut impl Write, input: &FormatInput, spec: Fixed) -> fmt::Result {
    use Fixed::*;
    match (spec, input.date, input.time, input.offset) {
        (ShortMonthName | LongMonthName | ShortWeekdayName | LongWeekdayName, Some(d), _, _) => {
            format_date_fixed(w, d, spec)
        }
        (
            LowerAmPm | UpperAmPm | Nanosecond | Nanosecond3 | Nanosecond6 | Nanosecond9
            | Nanosecond3NoDot | Nanosecond6NoDot | Nanosecond9NoDot,
            _,
            Some(t),
            _,
        ) => format_nanos(w, t, spec),
        (
            TimezoneName
            | TimezoneOffset
            | TimezoneOffsetColon
            | TimezoneOffsetDoubleColon
            | TimezoneOffsetTripleColon,
            _,
            _,
            Some(off),
        ) => format_offset_fixed(w, off, spec),
        (RFC3339, Some(d), Some(t), Some(off)) => {
            write_rfc3339(w, CivilDateTime::new(d, t), off, false)
        }
        _ => Err(fmt::Error),
    }
}

/// Precision of a rendered offset
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum OffsetPrecision {
    Hours,
    Minutes,
    Seconds,
}

/// `chrono::format::OffsetFormat::format` with zero padding
fn write_offset(
    w: &mut (impl Write + ?Sized),
    off: i32,
    precision: OffsetPrecision,
    colons: bool,
    allow_zulu: bool,
) -> fmt::Result {
    if allow_zulu && off == 0 {
        return w.write_char('Z');
    }
    let (sign, off) = if off < 0 { ('-', -off) } else { ('+', off) };
    let (hours, mins, secs) = match precision {
        // Minutes and seconds are truncated
        OffsetPrecision::Hours => ((off / 3600) as u8, 0, 0),
        // Seconds are rounded to the nearest minute
        OffsetPrecision::Minutes => {
            let minutes = (off + 30) / 60;
            ((minutes / 60) as u8, (minutes % 60) as u8, 0)
        }
        OffsetPrecision::Seconds => {
            let minutes = off / 60;
            ((minutes / 60) as u8, (minutes % 60) as u8, (off % 60) as u8)
        }
    };
    w.write_char(sign)?;
    if hours < 10 {
        w.write_char('0')?;
        w.write_char((b'0' + hours) as char)?;
    } else {
        write_hundreds(w, hours)?;
    }
    if precision != OffsetPrecision::Hours {
        if colons {
            w.write_char(':')?;
        }
        write_hundreds(w, mins)?;
    }
    if precision == OffsetPrecision::Seconds {
        if colons {
            w.write_char(':')?;
        }
        write_hundreds(w, secs)?;
    }
    Ok(())
}

/// RFC 3339 with automatic sub-second precision (`chrono`'s
/// `SecondsFormat::AutoSi`), using `Z` for a zero offset when `use_z`
pub fn write_rfc3339(
    w: &mut (impl Write + ?Sized),
    local: CivilDateTime,
    off: i32,
    use_z: bool,
) -> fmt::Result {
    let date = local.date;
    write_iso_year(w, date.year())?;
    w.write_char('-')?;
    write_hundreds(w, date.month() as u8)?;
    w.write_char('-')?;
    write_hundreds(w, date.day() as u8)?;
    w.write_char('T')?;
    let time = local.time;
    let (sec, nano) = match time.nanosecond() >= 1_000_000_000 {
        true => (time.second() + 1, time.nanosecond() - 1_000_000_000),
        false => (time.second(), time.nanosecond()),
    };
    write_hundreds(w, time.hour() as u8)?;
    w.write_char(':')?;
    write_hundreds(w, time.minute() as u8)?;
    w.write_char(':')?;
    write_hundreds(w, sec as u8)?;
    write_auto_si_fraction(w, nano)?;
    write_offset(w, off, OffsetPrecision::Minutes, true, use_z)
}

/// Displays a datetime as RFC 3339 (see [`write_rfc3339`])
#[derive(Copy, Clone, Debug)]
pub struct Rfc3339 {
    /// Local datetime
    pub local: CivilDateTime,
    /// Offset in seconds
    pub offset: i32,
    /// Render a zero offset as `Z`
    pub use_z: bool,
}

impl fmt::Display for Rfc3339 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_rfc3339(f, self.local, self.offset, self.use_z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    /// Specifier strings, including invalid and edge-case ones
    pub(crate) const FORMATS: &[&str] = &[
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y %C %y %G %g %m %b %B %h %d %e %a %A %w %u %U %W %V %j",
        "%D %x %F %v %H %k %I %l %P %p %M %S %f %.f %.3f %.6f %.9f %3f %6f %9f",
        "%R %T %X %r %c %s %t %n %%",
        "%Z %z %:z %::z %:::z %+",
        "%-d %-m %_d %_m %0e %-H %_H %-j %_j %0j %-Y %_Y %0Y %-f %_f",
        "%-D",
        "%_F",
        "%-T",
        "%-a",
        "%#z",
        "%#Y",
        "%",
        "%Q",
        "%.",
        "%.4f",
        "%.3",
        "%3",
        "%3g",
        "%:",
        "%:x",
        "%::",
        "%-",
        "%_",
        "%0",
        "%#",
        "%-%",
        "%E",
        "%O",
        "naïve %Y ünïcödé %m",
        "  \t%Y  ",
        "",
        "%%%%Y",
        "%-s",
        "%_u",
        "%0w",
    ];

    fn sample_datetimes() -> Vec<chrono::NaiveDateTime> {
        let mut out = vec![];
        let dates = [
            (-262_143, 1, 1),
            (-10_000, 7, 4),
            (-1, 12, 31),
            (0, 1, 1),
            (1, 1, 1),
            (99, 3, 5),
            (999, 12, 31),
            (1969, 12, 31),
            (1970, 1, 1),
            (2000, 2, 29),
            (2004, 1, 1),
            (2020, 12, 31),
            (2021, 1, 3),
            (9999, 12, 31),
            (10_000, 1, 1),
            (262_142, 12, 31),
        ];
        let times = [
            (0, 0, 0, 0),
            (0, 1, 2, 3),
            (11, 59, 59, 999_999_999),
            (12, 0, 0, 120_000_000),
            (13, 4, 5, 123_456_000),
            (23, 59, 59, 1_000_000_000),
            (23, 59, 59, 1_123_456_789),
        ];
        for (y, m, d) in dates {
            for (h, mi, s, n) in times {
                let date = chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap();
                let time = chrono::NaiveTime::from_hms_nano_opt(h, mi, s, n).unwrap();
                out.push(date.and_time(time));
            }
        }
        out
    }

    fn ours(dt: &chrono::NaiveDateTime) -> CivilDateTime {
        use chrono::{Datelike, Timelike};
        let date = CivilDate::from_num_days_from_ce_opt(dt.num_days_from_ce()).unwrap();
        let time = CivilTime::from_num_seconds_from_midnight_opt(
            dt.num_seconds_from_midnight(),
            dt.nanosecond(),
        )
        .unwrap();
        CivilDateTime::new(date, time)
    }

    fn render(input: FormatInput, fmt: &str) -> Result<String, fmt::Error> {
        let mut s = String::new();
        format_items(&mut s, &input, StrftimeItems::new(fmt)).map(|_| s)
    }

    fn chrono_render(d: impl fmt::Display) -> Result<String, fmt::Error> {
        let mut s = String::new();
        write!(s, "{d}").map(|_| s)
    }

    #[test]
    fn test_items_match_chrono() {
        for fmt in FORMATS {
            let ours: Vec<String> = StrftimeItems::new(fmt).map(|i| format!("{i:?}")).collect();
            let theirs: Vec<String> = chrono::format::StrftimeItems::new(fmt)
                .map(|i| format!("{i:?}"))
                .collect();
            assert_eq!(ours.len(), theirs.len(), "{fmt}: {ours:?} vs {theirs:?}");
            let errs = |v: &[String]| v.iter().map(|i| i == "Error").collect::<Vec<_>>();
            assert_eq!(errs(&ours), errs(&theirs), "{fmt}");
        }
    }

    #[test]
    fn test_format_matches_chrono() {
        let offsets = [
            0, 1, 29, 30, 59, 3600, -3600, 19_800, -34_200, 86_399, -86_399, 45_296,
        ];
        for dt in sample_datetimes() {
            let civil = ours(&dt);
            for fmt in FORMATS {
                let a = render(FormatInput::datetime(civil), fmt);
                let b = chrono_render(dt.format(fmt));
                assert_eq!(a, b, "{dt:?} {fmt}");
                let a = render(FormatInput::date(civil.date), fmt);
                let b = chrono_render(dt.date().format(fmt));
                assert_eq!(a, b, "{dt:?} {fmt}");
                let a = render(FormatInput::time(civil.time), fmt);
                let b = chrono_render(dt.time().format(fmt));
                assert_eq!(a, b, "{dt:?} {fmt}");
                for off in offsets {
                    let tz = chrono::FixedOffset::east_opt(off).unwrap();
                    let Some(zoned) = tz.from_local_datetime(&dt).single() else {
                        continue;
                    };
                    let a = render(FormatInput::zoned(civil, off), fmt);
                    let b = chrono_render(zoned.format(fmt));
                    assert_eq!(a, b, "{dt:?} {off} {fmt}");
                    let a = Rfc3339 {
                        local: civil,
                        offset: off,
                        use_z: true,
                    }
                    .to_string();
                    let b = zoned.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true);
                    assert_eq!(a, b);
                    let a = Rfc3339 {
                        local: civil,
                        offset: off,
                        use_z: false,
                    }
                    .to_string();
                    assert_eq!(a, zoned.to_rfc3339());
                }
            }
        }
    }
}
