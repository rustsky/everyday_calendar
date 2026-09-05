//! Streak arithmetic — the part the original web app left out.
//!
//! "Don't break the chain" is the whole premise of the Every Day Calendar, so
//! the chain is worth measuring.

use crate::date::{self, Date};
use crate::store::Goal;

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

/// Returns the milestone this streak length just hit, if any.
pub fn milestone_for(streak: u32) -> Option<u32> {
    MILESTONES.contains(&streak).then_some(streak)
}

/// Counts back from `from` while days stay lit, capped so a corrupt import
/// can't spin forever.
fn run_ending_at(goal: &Goal, from: Date) -> u32 {
    let mut count = 0;
    let mut cursor = from;
    while goal.is_lit(cursor) && count < 200_000 {
        count += 1;
        cursor = cursor.prev();
    }
    count
}

pub fn compute(goal: &Goal, today: Date, board_year: i32, board_month: u32) -> Stats {
    let today_done = goal.is_lit(today);

    // A day that hasn't happened yet shouldn't read as a broken chain, so when
    // today is still pending the run is measured from yesterday.
    let current = if today_done {
        run_ending_at(goal, today)
    } else {
        run_ending_at(goal, today.prev())
    };

    let mut longest = 0;
    let mut run = 0;
    if let Some((first, last)) = goal.span() {
        let mut cursor = Date {
            year: first,
            ordinal: 0,
        };
        let end = Date {
            year: last,
            ordinal: date::days_in_year(last) as usize - 1,
        };
        loop {
            if goal.is_lit(cursor) {
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

    let year_bits = goal.year(board_year);
    let year_days = date::days_in_year(board_year);
    let month_days = date::days_in_month(board_year, board_month);
    let month_start = date::ordinal(board_year, board_month, 1);
    let month_lit = (0..month_days as usize)
        .filter(|offset| year_bits.get(month_start + offset))
        .count() as u32;

    let mut last_year = 0;
    let mut cursor = today;
    for _ in 0..365 {
        if goal.is_lit(cursor) {
            last_year += 1;
        }
        cursor = cursor.prev();
    }

    Stats {
        current,
        longest: longest.max(current),
        total: goal.total(),
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
    use crate::store::Accent;

    fn goal_with(days: &[(i32, u32, u32)]) -> Goal {
        let mut goal = Goal::new("Test", Accent::Gold);
        for (y, m, d) in days {
            goal.toggle(*y, date::ordinal(*y, *m, *d));
        }
        goal
    }

    #[test]
    fn counts_a_streak_ending_today() {
        let goal = goal_with(&[(2026, 2, 1), (2026, 2, 2), (2026, 2, 3)]);
        let stats = compute(&goal, Date::new(2026, 2, 3), 2026, 2);
        assert_eq!(stats.current, 3);
        assert!(stats.today_done);
        assert!(!stats.at_risk);
    }

    #[test]
    fn today_pending_does_not_break_the_chain() {
        let goal = goal_with(&[(2026, 2, 1), (2026, 2, 2)]);
        let stats = compute(&goal, Date::new(2026, 2, 3), 2026, 2);
        assert_eq!(stats.current, 2);
        assert!(stats.at_risk);
    }

    #[test]
    fn a_gap_yesterday_ends_the_chain() {
        let goal = goal_with(&[(2026, 2, 1)]);
        let stats = compute(&goal, Date::new(2026, 2, 3), 2026, 2);
        assert_eq!(stats.current, 0);
        assert_eq!(stats.longest, 1);
    }

    #[test]
    fn streaks_cross_the_new_year() {
        let goal = goal_with(&[(2025, 11, 30), (2025, 11, 31), (2026, 0, 1)]);
        let stats = compute(&goal, Date::new(2026, 0, 1), 2026, 0);
        assert_eq!(stats.current, 3);
        assert_eq!(stats.longest, 3);
        assert_eq!(stats.total, 3);
    }

    #[test]
    fn month_totals_track_the_board() {
        let goal = goal_with(&[(2026, 1, 1), (2026, 1, 2), (2026, 3, 9)]);
        let stats = compute(&goal, Date::new(2026, 1, 2), 2026, 1);
        assert_eq!(stats.month_lit, 2);
        assert_eq!(stats.month_days, 28);
        assert_eq!(stats.year_lit, 3);
    }
}
