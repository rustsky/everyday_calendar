//! The client's own copy of the document: `localStorage` in the browser, a
//! few files in the data directory on the desktop.
//!
//! It is the source of truth for rendering, and it works with no server at
//! all. The sync server, when there is one, is a replica this pushes to and
//! pulls from — never a dependency.

use std::collections::BTreeMap;

use edc_core::legacy::{self, Backup};
use edc_core::model::{Accent, Doc, GoalRecord, Stamp};
use edc_core::prefs::Prefs;

use crate::platform;

pub const DOC_KEY: &str = "everydaycalendar.doc";
pub const PREFS_KEY: &str = "everydaycalendar.prefs";
pub const DEVICE_KEY: &str = "everydaycalendar.device";
/// The layout this app used before the day log existed.
const V2_KEY: &str = "everydaycalendar.v2";

/// This device's stable id, minted on first run. It is the tie-break in
/// last-write-wins, and it labels writes in the merged document.
pub fn device_id() -> String {
    if let Some(existing) = platform::store_get(DEVICE_KEY)
        && !existing.is_empty()
    {
        return existing;
    }
    let fresh = platform::random_id();
    platform::store_set(DEVICE_KEY, &fresh);
    fresh
}

/// Reads this device's copy. Deliberately does not invent a starting goal:
/// on a synced setup the goals arrive from the server, and minting one here
/// would give every new device its own duplicate "Every day". See
/// [`starter_goal`], which runs once sync has had its say.
pub fn load_doc(device: &str) -> Doc {
    if let Some(raw) = platform::store_get(DOC_KEY)
        && let Ok(doc) = serde_json::from_str::<Doc>(&raw)
    {
        return doc;
    }

    // Nothing in the current format: pick up whatever an older version of this
    // app, or the original everydaycalendar.app, left behind.
    let mut doc = Doc::new();
    let stamp = Stamp::new(platform::now_ms(), device);
    if let Some(backup) = previous_format().or_else(original_app) {
        legacy::import_into(&mut doc, &backup, stamp, platform::random_id);
    }
    doc
}

/// The goal a brand new, empty calendar starts with.
pub fn starter_goal(device: &str) -> (String, GoalRecord) {
    (
        platform::random_id(),
        GoalRecord {
            name: "Every day".to_string(),
            accent: Accent::Gold,
            deleted: false,
            order: 0,
            stamp: Stamp::new(platform::now_ms(), device),
        },
    )
}

/// This app's own v2 blob. `Backup` reads it directly — the goal list has the
/// same shape, and the settings alongside it are simply ignored.
fn previous_format() -> Option<Backup> {
    let raw = platform::store_get(V2_KEY)?;
    serde_json::from_str::<Backup>(&raw).ok()
}

/// The original app stored one 61-character string per year, under the year
/// itself as the key.
fn original_app() -> Option<Backup> {
    let years: BTreeMap<i32, String> = platform::store_entries()
        .into_iter()
        .filter_map(|(key, value)| {
            let year = key.parse::<i32>().ok()?;
            (1970..=2200).contains(&year).then_some((year, value))
        })
        .collect();
    legacy::backup_from_year_strings(years)
}

pub fn save_doc(doc: &Doc) {
    if let Ok(json) = serde_json::to_string(doc) {
        platform::store_set(DOC_KEY, &json);
    }
}

pub fn load_prefs() -> Prefs {
    let mut prefs = platform::store_get(PREFS_KEY)
        .and_then(|raw| serde_json::from_str::<Prefs>(&raw).ok())
        .unwrap_or_else(|| {
            let mut fresh = Prefs::default();
            // A twelve-by-thirty-one grid is unreadable on a phone, so
            // first-time visitors on a narrow screen start on the month view.
            if platform::narrow_viewport() {
                fresh.view = edc_core::View::Month;
            }
            fresh
        });
    prefs.sanitize();
    prefs
}

pub fn save_prefs(prefs: &Prefs) {
    if let Ok(json) = serde_json::to_string(prefs) {
        platform::store_set(PREFS_KEY, &json);
    }
}

/// Suggested filename for an export, e.g. `every-day-calendar-2026-03-04.json`.
pub fn export_filename(today: edc_core::Date) -> String {
    let (month, day) = today.month_day();
    format!(
        "every-day-calendar-{:04}-{:02}-{:02}.json",
        today.year,
        month + 1,
        day
    )
}
