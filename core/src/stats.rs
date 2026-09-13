//! Streak arithmetic — the part the original web app left out.
//!
//! "Don't break the chain" is the whole premise of the Every Day Calendar, so
//! the chain is worth measuring.

use crate::date::{self, Date};
use crate::model::Doc;

/// Streak lengths worth making a fuss about.
pub const MILESTONES: [u32; 8] = [7, 14, 30, 50, 100, 180, 365, 500];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Stats {
    /// Days in the run ending today (or yesterday, if today isn't lit yet).
    pub current: u32,
    /// Longest run anywhere in the record.
    pub longest: u32,
    /// Every lit day, all years.
    pub total: u32,
    /// Lit days in the year currently on the board.
    pub year_lit: u32,
    pub year_days: u32,
    /// Lit days in the month currently in view.
    pub month_lit: u32,
    pub month_days: u32,
    /// Lit days in the trailing 365 days ending today.
    pub last_year: u32,
    pub today_done: bool,
    /// Set while the current run is still alive only because today is pending.
    pub at_risk: bool,
}

impl Stats {
    pub fn year_percent(&self) -> u32 {
        if self.year_days == 0 {
            0
        } else {
            (self.year_lit * 100).div_ceil(self.year_days).min(100)
        }
    }

    /// The next milestone the current streak is climbing towards.
    pub fn next_milestone(&self) -> Option<u32> {
        MILESTONES.iter().copied().find(|m| *m > self.current)
    }
}

/// How often one weekday gets kept.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct WeekdayRate {
    pub done: u32,
    /// Days of this weekday that have actually happened.
    pub total: u32,
}

impl WeekdayRate {
    pub fn percent(&self) -> u32 {
        if self.total == 0 {
            0
        } else {
            (self.done * 100).div_ceil(self.total).min(100)
        }
    }
}

/// Completion per weekday across one year, indexed from Sunday. Days still in
/// the future are left out, so a year in progress is not scored against days
/// that have not happened.
pub fn weekday_rates(doc: &Doc, goal: &str, year: i32, today: Date) -> [WeekdayRate; 7] {
    let mut rates = [WeekdayRate::default(); 7];
    if today.year < year {
        return rates;
    }
    let last = if today.year == year {
        today.ordinal
    } else {
        date::days_in_year(year) as usize - 1
    };
    for ordinal in 0..=last {
        let (month, day) = date::from_ordinal(year, ordinal);
        let slot = &mut rates[date::weekday(year, month, day) as usize];
        slot.total += 1;
        if doc.is_lit(goal, Date { year, ordinal }) {
            slot.done += 1;
        }
    }
    rates
}

/// Lit days collapsed into `(first ordinal, length)` runs. A year of ticks is
/// a few dozen runs, so the log draws spans rather than 366 elements a row.
pub fn lit_runs(bits: &crate::bits::YearBits, days: u32) -> Vec<(u32, u32)> {
    let mut runs: Vec<(u32, u32)> = Vec::new();
    for day in 0..days {
        if !bits.get(day as usize) {
            continue;
        }
        match runs.last_mut() {
            Some((start, len)) if *start + *len == day => *len += 1,
            _ => runs.push((day, 1)),
        }
    }
    runs
}

/// Returns the milestone this streak length just hit, if any.
pub fn milestone_for(streak: u32) -> Option<u32> {
    MILESTONES.contains(&streak).then_some(streak)
}

/// Counts back from `from` while days stay lit, capped so a corrupt import
/// can't spin forever.
fn run_ending_at(doc: &Doc, goal: &str, from: Date) -> u32 {
    let mut count = 0;
    let mut cursor = from;
    while doc.is_lit(goal, cursor) && count < 200_000 {
        count += 1;
        cursor = cursor.prev();
    }
    count
}

pub fn compute(doc: &Doc, goal: &str, today: Date, board_year: i32, board_month: u32) -> Stats {
    let today_done = doc.is_lit(goal, today);

    // A day that hasn't happened yet shouldn't read as a broken chain, so when
    // today is still pending the run is measured from yesterday.
    let current = if today_done {
        run_ending_at(doc, goal, today)
    } else {
        run_ending_at(doc, goal, today.prev())
    };

    let mut longest = 0;
    let mut run = 0;
    if let Some((first, last)) = doc.span(goal) {
        let mut cursor = Date {
            year: first,
            ordinal: 0,
        };
        let end = Date {
            year: last,
            ordinal: date::days_in_year(last) as usize - 1,
        };
        loop {
            if doc.is_lit(goal, cursor) {
                run += 1;
                longest = longest.max(run);
            } else {
                run = 0;
            }
            if cursor == end {
                break;
            }
            cursor = cursor.next();
        }
    }

    let year_bits = doc.year_bits(goal, board_year);
    let year_days = date::days_in_year(board_year);
    let month_days = date::days_in_month(board_year, board_month);
    let month_start = date::ordinal(board_year, board_month, 1);
    let month_lit = (0..month_days as usize)
        .filter(|offset| year_bits.get(month_start + offset))
        .count() as u32;

    let mut last_year = 0;
    let mut cursor = today;
    for _ in 0..365 {
        if doc.is_lit(goal, cursor) {
            last_year += 1;
        }
        cursor = cursor.prev();
    }

    Stats {
        current,
        longest: longest.max(current),
        total: doc.total(goal),
        year_lit: year_bits.count(),
        year_days,
        month_lit,
        month_days,
        last_year,
        today_done,
        at_risk: current > 0 && !today_done,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Stamp;

    fn doc_with(days: &[(i32, u32, u32)]) -> Doc {
        let mut doc = Doc::new();
        for (year, month, day) in days {
            doc.set_day(
                "g1",
                *year,
                date::ordinal(*year, *month, *day),
                true,
                Stamp::new(1, "test"),
            );
        }
        doc
    }

    #[test]
    fn counts_a_streak_ending_today() {
        let doc = doc_with(&[(2026, 2, 1), (2026, 2, 2), (2026, 2, 3)]);
        let stats = compute(&doc, "g1", Date::new(2026, 2, 3), 2026, 2);
        assert_eq!(stats.current, 3);
        assert!(stats.today_done);
        assert!(!stats.at_risk);
    }

    #[test]
    fn today_pending_does_not_break_the_chain() {
        let doc = doc_with(&[(2026, 2, 1), (2026, 2, 2)]);
        let stats = compute(&doc, "g1", Date::new(2026, 2, 3), 2026, 2);
        assert_eq!(stats.current, 2);
        assert!(stats.at_risk);
    }

    #[test]
    fn a_gap_yesterday_ends_the_chain() {
        let doc = doc_with(&[(2026, 2, 1)]);
        let stats = compute(&doc, "g1", Date::new(2026, 2, 3), 2026, 2);
        assert_eq!(stats.current, 0);
        assert_eq!(stats.longest, 1);
    }

    #[test]
    fn streaks_cross_the_new_year() {
        let doc = doc_with(&[(2025, 11, 30), (2025, 11, 31), (2026, 0, 1)]);
        let stats = compute(&doc, "g1", Date::new(2026, 0, 1), 2026, 0);
        assert_eq!(stats.current, 3);
        assert_eq!(stats.longest, 3);
        assert_eq!(stats.total, 3);
    }

    #[test]
    fn month_totals_track_the_board() {
        let doc = doc_with(&[(2026, 1, 1), (2026, 1, 2), (2026, 3, 9)]);
        let stats = compute(&doc, "g1", Date::new(2026, 1, 2), 2026, 1);
        assert_eq!(stats.month_lit, 2);
        assert_eq!(stats.month_days, 28);
        assert_eq!(stats.year_lit, 3);
    }

    #[test]
    fn a_cleared_day_stops_counting() {
        let mut doc = doc_with(&[(2026, 2, 1), (2026, 2, 2), (2026, 2, 3)]);
        doc.set_day(
            "g1",
            2026,
            date::ordinal(2026, 2, 2),
            false,
            Stamp::new(9, "test"),
        );
        let stats = compute(&doc, "g1", Date::new(2026, 2, 3), 2026, 2);
        assert_eq!(stats.current, 1);
        assert_eq!(stats.total, 2);
    }

    #[test]
    fn the_log_lists_only_years_holding_a_lit_day() {
        let mut doc = doc_with(&[(2024, 0, 1), (2026, 5, 4)]);
        // A year that was written to and then cleared is not part of the log.
        doc.set_day("g1", 2025, 0, true, Stamp::new(1, "test"));
        doc.set_day("g1", 2025, 0, false, Stamp::new(2, "test"));
        assert_eq!(doc.years("g1"), vec![2024, 2026]);
        assert!(doc.years("nobody").is_empty());
    }

    #[test]
    fn runs_collapse_neighbouring_days() {
        let doc = doc_with(&[(2026, 0, 1), (2026, 0, 2), (2026, 0, 3), (2026, 0, 9)]);
        let bits = doc.year_bits("g1", 2026);
        // Three consecutive days plus a lone one, not four separate ticks.
        assert_eq!(lit_runs(&bits, 365), vec![(0, 3), (8, 1)]);
        assert_eq!(lit_runs(&crate::bits::YearBits::default(), 365), vec![]);
    }

    #[test]
    fn weekdays_only_count_days_that_have_happened() {
        // 1 January 2026 is a Thursday, so the 1st and the 8th are Thursdays.
        let doc = doc_with(&[(2026, 0, 1)]);
        let rates = weekday_rates(&doc, "g1", 2026, Date::new(2026, 0, 8));
        let thursday = rates[4];
        assert_eq!((thursday.done, thursday.total), (1, 2));
        assert_eq!(thursday.percent(), 50);

        // Sunday the 4th has happened and was missed; Saturday has not.
        assert_eq!((rates[0].done, rates[0].total), (0, 1));
        assert_eq!(rates[6].total, 1);

        // A year that has not started scores nothing rather than zero percent.
        let future = weekday_rates(&doc, "g1", 2027, Date::new(2026, 0, 8));
        assert!(future.iter().all(|rate| rate.total == 0));
    }
}
