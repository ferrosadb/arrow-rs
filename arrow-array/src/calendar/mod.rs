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

//! Calendar arithmetic, formatting and parsing used by the arrow kernels.
//!
//! Arrow historically delegated these to `chrono` 0.4 and exposed its
//! behaviour to users: the `±262143` year range, leap-second times such as
//! `23:59:60`, and chrono's strftime format-string syntax in
//! `FormatOptions` and `Parser::parse_formatted`. This module reproduces that
//! behaviour exactly without depending on `chrono`. Public APIs convert to
//! and from [`jiff`] types at the edges (see [`crate::temporal_conversions`]).
//!
//! Formatting and parsing are adapted from chrono 0.4.39
//! (<https://github.com/chronotope/chrono>, MIT OR Apache-2.0).
//!
//! This module is public so the other arrow crates can share it; it is not
//! intended as a general purpose date library.

mod convert;
mod date;
pub mod format;
pub mod parse;
mod scan;
mod time;

pub use date::{is_leap_year, CivilDate, Weekday, MAX_YEAR, MIN_YEAR};
pub use time::{
    fixed_offset_opt, CivilDateTime, CivilTime, Duration, OffsetDisplay, ZonedDateTime,
};

#[cfg(test)]
mod parse_tests;
