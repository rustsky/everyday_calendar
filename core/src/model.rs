//! The synced document.
//!
//! Every fact is a last-write-wins register: one per goal, and one per
//! (goal, year, day). Two devices merge by keeping the newer stamp for each
//! key, so lighting a day on a phone and a different day on a laptop both
//! survive — which a whole-year bitset could not do.
//!
//! Clearing a day writes `lit: false` rather than removing the entry. Without
//! that tombstone a merge would resurrect the day from the other device.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::bits::YearBits;
use crate::date::{self, Date};

pub type GoalId = String;

pub const DOC_VERSION: u32 = 3;

/// When a write happened and who made it. `by` breaks ties so that two devices
/// writing in the same millisecond still converge on the same answer.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Stamp {
    pub at: u64,
    pub by: String,
}

impl Stamp {
    pub fn new(at: u64, by: impl Into<String>) -> Self {
        Self { at, by: by.into() }
    }

    pub fn wins_over(&self, other: &Stamp) -> bool {
        (self.at, self.by.as_str()) > (other.at, other.by.as_str())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Accent {
    #[default]
    Gold,
    Jade,
    Rose,
    Ice,
    Ember,
}

impl Accent {
    pub const ALL: [Accent; 5] = [
        Accent::Gold,
        Accent::Jade,
        Accent::Rose,
        Accent::Ice,
        Accent::Ember,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            Accent::Gold => "gold",
            Accent::Jade => "jade",
            Accent::Rose => "rose",
            Accent::Ice => "ice",
            Accent::Ember => "ember",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Accent::Gold => "Gold",
            Accent::Jade => "Jade",
            Accent::Rose => "Rose",
            Accent::Ice => "Ice",
            Accent::Ember => "Ember",
        }
    }
}

/// A goal's metadata. Deleting a goal is a flag, not a removal, so the deletion
/// survives a merge with a device that still has the goal.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct GoalRecord {
    pub name: String,
    #[serde(default)]
    pub accent: Accent,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub order: u32,
    #[serde(flatten)]
    pub stamp: Stamp,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct DayOp {
    pub lit: bool,
    #[serde(flatten)]
    pub stamp: Stamp,
}

type YearMap = BTreeMap<u16, DayOp>;
type GoalDays = BTreeMap<i32, YearMap>;

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Doc {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub goals: BTreeMap<GoalId, GoalRecord>,
    /// goal → year → day ordinal → the newest write for that day.
    #[serde(default)]
    pub days: BTreeMap<GoalId, GoalDays>,
}

impl Doc {
    pub fn new() -> Self {
        Self {
            version: DOC_VERSION,
            ..Default::default()
        }
    }

    /// Live goals, in display order.
    pub fn goal_ids(&self) -> Vec<GoalId> {
        let mut ids: Vec<&GoalId> = self
            .goals
            .iter()
            .filter(|(_, record)| !record.deleted)
            .map(|(id, _)| id)
            .collect();
        ids.sort_by_key(|id| (self.goals[*id].order, (*id).clone()));
        ids.into_iter().cloned().collect()
    }

    pub fn goal(&self, id: &str) -> Option<&GoalRecord> {
        self.goals.get(id).filter(|record| !record.deleted)
    }

    pub fn next_order(&self) -> u32 {
        self.goals
            .values()
            .map(|record| record.order)
            .max()
            .map_or(0, |max| max + 1)
    }

    /// Writes goal metadata, keeping whichever version is newer.
    pub fn put_goal(&mut self, id: &str, record: GoalRecord) {
        match self.goals.get(id) {
            Some(existing) if !record.stamp.wins_over(&existing.stamp) => {}
            _ => {
                self.goals.insert(id.to_string(), record);
            }
        }
    }

    /// Convenience for editing one field of a live goal.
    pub fn edit_goal(&mut self, id: &str, stamp: Stamp, edit: impl FnOnce(&mut GoalRecord)) {
        let Some(existing) = self.goals.get(id) else {
            return;
        };
        let mut next = existing.clone();
        edit(&mut next);
        next.stamp = stamp;
        self.put_goal(id, next);
    }

    pub fn set_day(&mut self, goal: &str, year: i32, day: usize, lit: bool, stamp: Stamp) {
        if day >= date::MAX_DAYS {
            return;
        }
        let entry = self
            .days
            .entry(goal.to_string())
            .or_default()
            .entry(year)
            .or_default();
        let day = day as u16;
        match entry.get(&day) {
            Some(existing) if !stamp.wins_over(&existing.stamp) => {}
            _ => {
                entry.insert(day, DayOp { lit, stamp });
            }
        }
    }

    pub fn is_lit(&self, goal: &str, date: Date) -> bool {
        self.days
            .get(goal)
            .and_then(|years| years.get(&date.year))
            .and_then(|days| days.get(&(date.ordinal as u16)))
            .is_some_and(|op| op.lit)
    }

    pub fn year_bits(&self, goal: &str, year: i32) -> YearBits {
        let mut bits = YearBits::default();
        if let Some(days) = self.days.get(goal).and_then(|years| years.get(&year)) {
            for (day, op) in days {
                if op.lit {
                    bits.set(*day as usize, true);
                }
            }
        }
        bits
    }

    pub fn total(&self, goal: &str) -> u32 {
        self.days.get(goal).map_or(0, |years| {
            years
                .values()
                .map(|days| days.values().filter(|op| op.lit).count() as u32)
                .sum()
        })
    }

    /// Inclusive range of years holding any lit day.
    pub fn span(&self, goal: &str) -> Option<(i32, i32)> {
        let years = self.days.get(goal)?;
        let mut lit_years = years
            .iter()
            .filter(|(_, days)| days.values().any(|op| op.lit))
            .map(|(year, _)| *year);
        let first = lit_years.next()?;
        let last = lit_years.last().unwrap_or(first);
        Some((first, last))
    }

    /// Clears every lit day in a year. Returns what was there, for Undo.
    pub fn clear_year(&mut self, goal: &str, year: i32, stamp: Stamp) -> YearBits {
        let before = self.year_bits(goal, year);
        for day in before.days() {
            self.set_day(goal, year, day, false, stamp.clone());
        }
        before
    }

    /// Puts a year back exactly as `bits` describes it, clearing anything else.
    pub fn restore_year(&mut self, goal: &str, year: i32, bits: YearBits, stamp: Stamp) {
        for day in 0..date::days_in_year(year) as usize {
            self.set_day(goal, year, day, bits.get(day), stamp.clone());
        }
    }

    /// Folds another replica in. Both sides converge on the same document
    /// regardless of the order merges happen in.
    pub fn merge(&mut self, other: &Doc) {
        for (id, record) in &other.goals {
            self.put_goal(id, record.clone());
        }
        for (goal, years) in &other.days {
            for (year, days) in years {
                for (day, op) in days {
                    self.set_day(goal, *year, *day as usize, op.lit, op.stamp.clone());
                }
            }
        }
        self.version = DOC_VERSION;
    }

    /// Number of stored day writes — the thing that grows over time.
    pub fn day_entries(&self) -> usize {
        self.days
            .values()
            .flat_map(|years| years.values())
            .map(BTreeMap::len)
            .sum()
    }
}

/// Builds a short, readable id out of caller-supplied entropy. The platform
/// provides the randomness; the core stays dependency-free.
pub fn id_from(seed: u64) -> String {
    format!("{seed:012x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp(at: u64, by: &str) -> Stamp {
        Stamp::new(at, by)
    }

    fn goal(name: &str, at: u64, by: &str) -> GoalRecord {
        GoalRecord {
            name: name.to_string(),
            accent: Accent::Gold,
            deleted: false,
            order: 0,
            stamp: stamp(at, by),
        }
    }

    #[test]
    fn concurrent_edits_on_different_days_both_survive() {
        let mut phone = Doc::new();
        phone.put_goal("g1", goal("Draw", 1, "phone"));
        phone.set_day("g1", 2026, 10, true, stamp(100, "phone"));

        let mut laptop = phone.clone();
        laptop.set_day("g1", 2026, 20, true, stamp(101, "laptop"));
        phone.set_day("g1", 2026, 30, true, stamp(102, "phone"));

        let mut merged = phone.clone();
        merged.merge(&laptop);

        let bits = merged.year_bits("g1", 2026);
        assert!(bits.get(10) && bits.get(20) && bits.get(30));
        assert_eq!(bits.count(), 3);
    }

    #[test]
    fn the_newer_write_wins_on_the_same_day() {
        let mut a = Doc::new();
        a.set_day("g1", 2026, 5, true, stamp(100, "phone"));

        let mut b = Doc::new();
        b.set_day("g1", 2026, 5, false, stamp(200, "laptop"));

        let mut merged = a.clone();
        merged.merge(&b);
        assert!(!merged.year_bits("g1", 2026).get(5));

        // Merging the other way round has to reach the same answer.
        let mut other = b.clone();
        other.merge(&a);
        assert_eq!(merged, other);
    }

    #[test]
    fn clearing_a_year_is_not_undone_by_a_stale_peer() {
        let mut laptop = Doc::new();
        for day in [1usize, 2, 3] {
            laptop.set_day("g1", 2026, day, true, stamp(100, "laptop"));
        }
        let phone = laptop.clone();

        laptop.clear_year("g1", 2026, stamp(200, "laptop"));
        laptop.merge(&phone);

        assert_eq!(laptop.year_bits("g1", 2026).count(), 0);
    }

    #[test]
    fn merging_is_order_independent() {
        let mut a = Doc::new();
        a.put_goal("g1", goal("Draw", 5, "a"));
        a.set_day("g1", 2026, 1, true, stamp(10, "a"));
        a.set_day("g1", 2026, 2, true, stamp(40, "a"));

        let mut b = Doc::new();
        b.put_goal("g1", goal("Sketch", 9, "b"));
        b.set_day("g1", 2026, 2, false, stamp(20, "b"));
        b.set_day("g1", 2026, 3, true, stamp(30, "b"));

        let mut ab = a.clone();
        ab.merge(&b);
        let mut ba = b.clone();
        ba.merge(&a);

        assert_eq!(ab, ba);
        assert_eq!(ab.goal("g1").unwrap().name, "Sketch");
        assert_eq!(ab.year_bits("g1", 2026).count(), 3);
    }

    #[test]
    fn a_deleted_goal_stays_deleted_after_merge() {
        let mut a = Doc::new();
        a.put_goal("g1", goal("Draw", 1, "a"));
        let b = a.clone();

        a.edit_goal("g1", stamp(50, "a"), |record| record.deleted = true);
        a.merge(&b);

        assert!(a.goal("g1").is_none());
        assert!(a.goal_ids().is_empty());
    }

    #[test]
    fn tie_breaks_are_deterministic() {
        let mut a = Doc::new();
        a.set_day("g1", 2026, 7, true, stamp(100, "aaa"));
        let mut b = Doc::new();
        b.set_day("g1", 2026, 7, false, stamp(100, "zzz"));

        let mut ab = a.clone();
        ab.merge(&b);
        let mut ba = b.clone();
        ba.merge(&a);
        assert_eq!(ab, ba);
        assert!(!ab.year_bits("g1", 2026).get(7));
    }

    #[test]
    fn span_ignores_years_that_only_hold_tombstones() {
        let mut doc = Doc::new();
        doc.set_day("g1", 2024, 3, true, stamp(1, "a"));
        doc.set_day("g1", 2026, 3, true, stamp(1, "a"));
        doc.clear_year("g1", 2026, stamp(2, "a"));
        assert_eq!(doc.span("g1"), Some((2024, 2024)));
    }
}
