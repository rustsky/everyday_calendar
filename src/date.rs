//! Minimal proleptic-Gregorian date helpers.
//!
//! The calendar only ever deals in (year, month, day) triples and day-of-year
//! ordinals, so pulling in a full date library would be overkill — and every
//! byte matters in a wasm bundle.

pub const MONTHS_SHORT: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sept", "oct", "nov", "dec",
];

pub const MONTHS_LONG: [&str; 12] = [
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

pub const WEEKDAYS_SHORT: [&str; 7] = ["S", "M", "T", "W", "T", "F", "S"];
pub const WEEKDAYS_LONG: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

/// Largest number of days any year can hold. Also the number of pads on the board.
pub const MAX_DAYS: usize = 366;

pub fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub fn days_in_year(year: i32) -> u32 {
    if is_leap(year) { 366 } else { 365 }
}

/// `month` is zero-based (0 = January).
pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        0 | 2 | 4 | 6 | 7 | 9 | 11 => 31,
        3 | 5 | 8 | 10 => 30,
        1 if is_leap(year) => 29,
        1 => 28,
        _ => 0,
    }
}

/// Zero-based day-of-year ordinal for a zero-based month and one-based day.
pub fn ordinal(year: i32, month: u32, day: u32) -> usize {
    let mut n = 0u32;
    for m in 0..month {
        n += days_in_month(year, m);
    }
    (n + day - 1) as usize
}

/// Inverse of [`ordinal`]: `(zero-based month, one-based day)`.
pub fn from_ordinal(year: i32, ordinal: usize) -> (u32, u32) {
    let mut rest = ordinal as u32;
    for m in 0..12u32 {
        let len = days_in_month(year, m);
        if rest < len {
            return (m, rest + 1);
        }
        rest -= len;
    }
    (11, 31)
}

/// Sakamoto's algorithm. Returns 0 for Sunday through 6 for Saturday.
pub fn weekday(year: i32, month: u32, day: u32) -> u32 {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 2 { year - 1 } else { year };
    let raw = y + y / 4 - y / 100 + y / 400 + T[month as usize] + day as i32;
    raw.rem_euclid(7) as u32
}

/// A calendar date, used for "today" and for walking streaks backwards.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Date {
    pub year: i32,
    /// Zero-based day-of-year ordinal.
    pub ordinal: usize,
}

impl Date {
    pub fn new(year: i32, month: u32, day: u32) -> Self {
        Self {
            year,
            ordinal: ordinal(year, month, day),
        }
    }

    pub fn month_day(&self) -> (u32, u32) {
        from_ordinal(self.year, self.ordinal)
    }

    pub fn prev(self) -> Self {
        if self.ordinal == 0 {
            let year = self.year - 1;
            Self {
                year,
                ordinal: days_in_year(year) as usize - 1,
            }
        } else {
            Self {
                year: self.year,
                ordinal: self.ordinal - 1,
            }
        }
    }

    pub fn next(self) -> Self {
        if self.ordinal + 1 >= days_in_year(self.year) as usize {
            Self {
                year: self.year + 1,
                ordinal: 0,
            }
        } else {
            Self {
                year: self.year,
                ordinal: self.ordinal + 1,
            }
        }
    }

    /// e.g. `"Tuesday, 3 March 2026"`.
    pub fn long_label(&self) -> String {
        let (m, d) = self.month_day();
        let wd = weekday(self.year, m, d);
        format!(
            "{}, {} {} {}",
            WEEKDAYS_LONG[wd as usize], d, MONTHS_LONG[m as usize], self.year
        )
    }
}

/// The browser's local "today".
pub fn today() -> Date {
    let now = js_sys::Date::new_0();
    Date::new(
        now.get_full_year() as i32,
        now.get_month(),
        now.get_date(),
    )
}
