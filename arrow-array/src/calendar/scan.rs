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

//! Scanning routines for the format-string parser, adapted from
//! `chrono::format::scan` 0.4.39 (MIT / Apache-2.0).

use super::date::Weekday;
use super::parse::{ParseResult, INVALID, OUT_OF_RANGE, TOO_SHORT};

/// Scans a number with at least `min` and at most `max` ASCII digits
pub(super) fn number(s: &str, min: usize, max: usize) -> ParseResult<(&str, i64)> {
    assert!(min <= max);
    let bytes = s.as_bytes();
    if bytes.len() < min {
        return Err(TOO_SHORT);
    }
    let mut n = 0i64;
    for (i, c) in bytes.iter().take(max).copied().enumerate() {
        if !c.is_ascii_digit() {
            return match i < min {
                true => Err(INVALID),
                false => Ok((&s[i..], n)),
            };
        }
        n = n
            .checked_mul(10)
            .and_then(|n| n.checked_add((c - b'0') as i64))
            .ok_or(OUT_OF_RANGE)?;
    }
    Ok((&s[std::cmp::min(max, bytes.len())..], n))
}

const SCALE: [i64; 10] = [
    0,
    100_000_000,
    10_000_000,
    1_000_000,
    100_000,
    10_000,
    1_000,
    100,
    10,
    1,
];

/// Scans 1-9 fractional digits as nanoseconds, skipping any further digits
pub(super) fn nanosecond(s: &str) -> ParseResult<(&str, i64)> {
    let origlen = s.len();
    let (s, v) = number(s, 1, 9)?;
    let consumed = origlen - s.len();
    let v = v.checked_mul(SCALE[consumed]).ok_or(OUT_OF_RANGE)?;
    Ok((s.trim_start_matches(|c: char| c.is_ascii_digit()), v))
}

/// Scans exactly `digits` fractional digits as nanoseconds
pub(super) fn nanosecond_fixed(s: &str, digits: usize) -> ParseResult<(&str, i64)> {
    let (s, v) = number(s, digits, digits)?;
    Ok((s, v.checked_mul(SCALE[digits]).ok_or(OUT_OF_RANGE)?))
}

/// Scans a three letter, case-insensitive month name, returning `0..12`
pub(super) fn short_month0(s: &str) -> ParseResult<(&str, u8)> {
    if s.len() < 3 {
        return Err(TOO_SHORT);
    }
    let buf = s.as_bytes();
    let month0 = match (buf[0] | 32, buf[1] | 32, buf[2] | 32) {
        (b'j', b'a', b'n') => 0,
        (b'f', b'e', b'b') => 1,
        (b'm', b'a', b'r') => 2,
        (b'a', b'p', b'r') => 3,
        (b'm', b'a', b'y') => 4,
        (b'j', b'u', b'n') => 5,
        (b'j', b'u', b'l') => 6,
        (b'a', b'u', b'g') => 7,
        (b's', b'e', b'p') => 8,
        (b'o', b'c', b't') => 9,
        (b'n', b'o', b'v') => 10,
        (b'd', b'e', b'c') => 11,
        _ => return Err(INVALID),
    };
    Ok((&s[3..], month0))
}

/// Scans a three letter, case-insensitive weekday name
pub(super) fn short_weekday(s: &str) -> ParseResult<(&str, Weekday)> {
    if s.len() < 3 {
        return Err(TOO_SHORT);
    }
    let buf = s.as_bytes();
    let weekday = match (buf[0] | 32, buf[1] | 32, buf[2] | 32) {
        (b'm', b'o', b'n') => Weekday::Mon,
        (b't', b'u', b'e') => Weekday::Tue,
        (b'w', b'e', b'd') => Weekday::Wed,
        (b't', b'h', b'u') => Weekday::Thu,
        (b'f', b'r', b'i') => Weekday::Fri,
        (b's', b'a', b't') => Weekday::Sat,
        (b's', b'u', b'n') => Weekday::Sun,
        _ => return Err(INVALID),
    };
    Ok((&s[3..], weekday))
}

/// Consumes `suffix` case-insensitively if present
fn consume_suffix<'a>(s: &'a str, suffix: &[u8]) -> &'a str {
    match s.len() >= suffix.len() && s.as_bytes()[..suffix.len()].eq_ignore_ascii_case(suffix) {
        true => &s[suffix.len()..],
        false => s,
    }
}

/// Scans a short or long month name
pub(super) fn short_or_long_month0(s: &str) -> ParseResult<(&str, u8)> {
    const SUFFIXES: [&[u8]; 12] = [
        b"uary", b"ruary", b"ch", b"il", b"", b"e", b"y", b"ust", b"tember", b"ober", b"ember",
        b"ember",
    ];
    let (s, month0) = short_month0(s)?;
    Ok((consume_suffix(s, SUFFIXES[month0 as usize]), month0))
}

/// Scans a short or long weekday name
pub(super) fn short_or_long_weekday(s: &str) -> ParseResult<(&str, Weekday)> {
    const SUFFIXES: [&[u8]; 7] = [
        b"day", b"sday", b"nesday", b"rsday", b"day", b"urday", b"day",
    ];
    let (s, weekday) = short_weekday(s)?;
    let suffix = SUFFIXES[weekday.num_days_from_monday() as usize];
    Ok((consume_suffix(s, suffix), weekday))
}

/// Consumes colons and whitespace
pub(super) fn colon_or_space(s: &str) -> ParseResult<&str> {
    Ok(s.trim_start_matches(|c: char| c == ':' || c.is_whitespace()))
}

/// Scans an offset such as `+09:00`, `-0930`, `Z` (if `allow_zulu`)
pub(super) fn timezone_offset(
    s: &str,
    allow_zulu: bool,
    allow_missing_minutes: bool,
) -> ParseResult<(&str, i32)> {
    if allow_zulu {
        if let Some(b'Z' | b'z') = s.as_bytes().first() {
            return Ok((&s[1..], 0));
        }
    }
    let (negative, s) = match s.chars().next() {
        Some('+') => (false, &s[1..]),
        Some('-') => (true, &s[1..]),
        // U+2212 MINUS SIGN
        Some('\u{2212}') => (true, &s['\u{2212}'.len_utf8()..]),
        Some(_) => return Err(INVALID),
        None => return Err(TOO_SHORT),
    };
    let hours = match two_bytes(s)? {
        (h1 @ b'0'..=b'9', h2 @ b'0'..=b'9') => i32::from((h1 - b'0') * 10 + (h2 - b'0')),
        _ => return Err(INVALID),
    };
    let s = colon_or_space(&s[2..])?;
    let minutes = match two_bytes(s) {
        Ok((m1 @ b'0'..=b'5', m2 @ b'0'..=b'9')) => i32::from((m1 - b'0') * 10 + (m2 - b'0')),
        Ok((b'6'..=b'9', b'0'..=b'9')) => return Err(OUT_OF_RANGE),
        Ok(_) => return Err(INVALID),
        Err(_) if allow_missing_minutes => 0,
        Err(_) => return Err(TOO_SHORT),
    };
    let s = match s.len() {
        len if len >= 2 => &s[2..],
        0 => s,
        _ => return Err(TOO_SHORT),
    };
    let seconds = hours * 3600 + minutes * 60;
    Ok((s, if negative { -seconds } else { seconds }))
}

fn two_bytes(s: &str) -> ParseResult<(u8, u8)> {
    match s.as_bytes() {
        [a, b, ..] => Ok((*a, *b)),
        _ => Err(TOO_SHORT),
    }
}
