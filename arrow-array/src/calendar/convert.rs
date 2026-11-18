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

//! Conversions between the calendar types and `jiff`.
//!
//! `jiff` supports years -9999 to 9999 and has no leap seconds, so
//! conversions to `jiff` are fallible; conversions from `jiff` are not.

use super::{CivilDate, CivilDateTime, CivilTime, Duration};

impl CivilDate {
    /// Converts to `jiff`, `None` outside years -9999 to 9999
    pub fn to_jiff(&self) -> Option<jiff::civil::Date> {
        let year = i16::try_from(self.year()).ok()?;
        jiff::civil::Date::new(year, self.month() as i8, self.day() as i8).ok()
    }
}

impl From<jiff::civil::Date> for CivilDate {
    fn from(d: jiff::civil::Date) -> Self {
        Self::from_ymd_opt(d.year() as i32, d.month() as u32, d.day() as u32)
            .expect("every jiff date is within the calendar range")
    }
}

impl CivilTime {
    /// Converts to `jiff`, `None` for a leap second
    pub fn to_jiff(&self) -> Option<jiff::civil::Time> {
        let nanos = i32::try_from(self.nanosecond()).ok()?;
        let (h, m, s) = (self.hour() as i8, self.minute() as i8, self.second() as i8);
        jiff::civil::Time::new(h, m, s, nanos).ok()
    }
}

impl From<jiff::civil::Time> for CivilTime {
    fn from(t: jiff::civil::Time) -> Self {
        let (h, m, s) = (t.hour() as u32, t.minute() as u32, t.second() as u32);
        Self::from_hms_nano_opt(h, m, s, t.subsec_nanosecond() as u32)
            .expect("every jiff time is a valid time of day")
    }
}

impl CivilDateTime {
    /// Converts to `jiff`, `None` outside years -9999 to 9999 or for a leap second
    pub fn to_jiff(&self) -> Option<jiff::civil::DateTime> {
        Some(jiff::civil::DateTime::from_parts(
            self.date.to_jiff()?,
            self.time.to_jiff()?,
        ))
    }

    /// Converts to a `jiff` instant, treating `self` as UTC
    pub fn to_jiff_timestamp(&self) -> Option<jiff::Timestamp> {
        let nanos = i32::try_from(self.time.nanosecond()).ok()?;
        jiff::Timestamp::new(self.timestamp(), nanos).ok()
    }
}

impl From<jiff::civil::DateTime> for CivilDateTime {
    fn from(dt: jiff::civil::DateTime) -> Self {
        Self::new(dt.date().into(), dt.time().into())
    }
}

impl From<Duration> for jiff::SignedDuration {
    fn from(d: Duration) -> Self {
        let (secs, nanos) = d.parts();
        jiff::SignedDuration::new(secs, nanos as i32)
    }
}
