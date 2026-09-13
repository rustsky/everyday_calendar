//! Import, export, and migration.
//!
//! Backups are written in the flat per-year bitset shape rather than the day
//! log: it is a tenth of the size, it is readable, and it is what the original
//! `everydaycalendar.app` wrote. The cost is that a backup carries no
//! timestamps, so importing one is a single write at the moment of import.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::bits::YearBits;
use crate::model::{Accent, Doc, GoalRecord, Stamp};

/// The portable backup format, also the layout this app stored before the day
/// log existed.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Backup {
    #[serde(default)]
    pub version: u32,
    pub goals: Vec<BackupGoal>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct BackupGoal {
    pub name: String,
    #[serde(default)]
    pub accent: Accent,
    #[serde(default)]
    pub years: BTreeMap<i32, YearBits>,
}

pub const BACKUP_VERSION: u32 = 3;

pub fn export(doc: &Doc) -> Backup {
    let goals = doc
        .goal_ids()
        .into_iter()
        .filter_map(|id| {
            let record = doc.goal(&id)?;
            let mut years = BTreeMap::new();
            if let Some(stored) = doc.days.get(&id) {
                for year in stored.keys() {
                    let bits = doc.year_bits(&id, *year);
                    if !bits.is_empty() {
                        years.insert(*year, bits);
                    }
                }
            }
            Some(BackupGoal {
                name: record.name.clone(),
                accent: record.accent,
                years,
            })
        })
        .collect();

    Backup {
        version: BACKUP_VERSION,
        goals,
    }
}

pub fn export_json(doc: &Doc) -> String {
    serde_json::to_string_pretty(&export(doc)).unwrap_or_default()
}

/// Folds a backup into a document. Every day in the backup is written with
/// `stamp`, so an import beats anything older and loses to anything newer.
pub fn import_into(doc: &mut Doc, backup: &Backup, stamp: Stamp, mut next_id: impl FnMut() -> String) {
    for (index, goal) in backup.goals.iter().enumerate() {
        // Match on name so re-importing a backup updates the goal it came from
        // instead of duplicating it.
        let id = doc
            .goal_ids()
            .into_iter()
            .find(|id| doc.goal(id).is_some_and(|record| record.name == goal.name))
            .unwrap_or_else(&mut next_id);

        doc.put_goal(
            &id,
            GoalRecord {
                name: goal.name.clone(),
                accent: goal.accent,
                deleted: false,
                order: index as u32,
                stamp: stamp.clone(),
            },
        );

        for (year, bits) in &goal.years {
            for day in bits.days() {
                doc.set_day(&id, *year, day, true, stamp.clone());
            }
        }
    }
    doc.version = crate::model::DOC_VERSION;
}

pub fn import_json(
    doc: &mut Doc,
    text: &str,
    stamp: Stamp,
    next_id: impl FnMut() -> String,
) -> Result<usize, String> {
    let backup: Backup = serde_json::from_str(text).map_err(|error| error.to_string())?;
    let count = backup.goals.len();
    import_into(doc, &backup, stamp, next_id);
    Ok(count)
}

/// Builds a backup out of the original app's per-year strings, keyed by year.
pub fn backup_from_year_strings(years: BTreeMap<i32, String>) -> Option<Backup> {
    let years: BTreeMap<i32, YearBits> = years
        .into_iter()
        .map(|(year, text)| (year, YearBits::decode(&text)))
        .filter(|(_, bits)| !bits.is_empty())
        .collect();

    (!years.is_empty()).then(|| Backup {
        version: BACKUP_VERSION,
        goals: vec![BackupGoal {
            name: "Every day".to_string(),
            accent: Accent::Gold,
            years,
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date;

    fn ids() -> impl FnMut() -> String {
        let mut n = 0;
        move || {
            n += 1;
            format!("g{n}")
        }
    }

    #[test]
    fn a_backup_round_trips_through_the_day_log() {
        let mut doc = Doc::new();
        doc.put_goal(
            "g1",
            GoalRecord {
                name: "Draw".into(),
                accent: Accent::Jade,
                deleted: false,
                order: 0,
                stamp: Stamp::new(1, "a"),
            },
        );
        doc.set_day("g1", 2026, date::ordinal(2026, 2, 4), true, Stamp::new(1, "a"));
        doc.set_day("g1", 2025, 0, true, Stamp::new(1, "a"));

        let json = export_json(&doc);
        let mut restored = Doc::new();
        import_json(&mut restored, &json, Stamp::new(2, "b"), ids()).unwrap();

        let id = restored.goal_ids()[0].clone();
        assert_eq!(restored.goal(&id).unwrap().name, "Draw");
        assert_eq!(restored.goal(&id).unwrap().accent, Accent::Jade);
        assert_eq!(restored.total(&id), 2);
    }

    #[test]
    fn importing_twice_does_not_duplicate_a_goal() {
        let mut doc = Doc::new();
        let backup = Backup {
            version: BACKUP_VERSION,
            goals: vec![BackupGoal {
                name: "Draw".into(),
                accent: Accent::Gold,
                years: BTreeMap::new(),
            }],
        };
        import_into(&mut doc, &backup, Stamp::new(1, "a"), ids());
        import_into(&mut doc, &backup, Stamp::new(2, "a"), ids());
        assert_eq!(doc.goal_ids().len(), 1);
    }

    #[test]
    fn the_original_per_year_strings_migrate() {
        let mut bits = YearBits::default();
        bits.set(0, true);
        bits.set(200, true);
        let years = BTreeMap::from([(2019, bits.encode())]);

        let backup = backup_from_year_strings(years).unwrap();
        let mut doc = Doc::new();
        import_into(&mut doc, &backup, Stamp::new(1, "a"), ids());

        let id = doc.goal_ids()[0].clone();
        assert_eq!(doc.total(&id), 2);
        assert!(doc.year_bits(&id, 2019).get(200));
    }
}
