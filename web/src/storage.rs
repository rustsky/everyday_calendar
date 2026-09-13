//! `localStorage` is the client's own copy of the document.
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

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

/// This browser's stable id, minted on first run. It is the tie-break in
/// last-write-wins, and it labels writes in the merged document.
pub fn device_id() -> String {
    let Some(store) = storage() else {
        return platform::random_id();
    };
    if let Ok(Some(existing)) = store.get_item(DEVICE_KEY) {
        if !existing.is_empty() {
            return existing;
        }
    }
    let fresh = platform::random_id();
    let _ = store.set_item(DEVICE_KEY, &fresh);
    fresh
}

/// Reads this browser's copy. Deliberately does not invent a starting goal:
/// on a synced setup the goals arrive from the server, and minting one here
/// would give every new device its own duplicate "Every day". See
/// [`starter_goal`], which runs once sync has had its say.
pub fn load_doc(device: &str) -> Doc {
    let Some(store) = storage() else {
        return Doc::new();
    };

    if let Ok(Some(raw)) = store.get_item(DOC_KEY) {
        if let Ok(doc) = serde_json::from_str::<Doc>(&raw) {
            return doc;
        }
    }

    // Nothing in the current format: pick up whatever an older version of this
    // app, or the original everydaycalendar.app, left behind.
    let mut doc = Doc::new();
    let stamp = Stamp::new(platform::now_ms(), device);
    if let Some(backup) = previous_format(&store).or_else(|| original_app(&store)) {
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
fn previous_format(store: &web_sys::Storage) -> Option<Backup> {
    let raw = store.get_item(V2_KEY).ok()??;
    serde_json::from_str::<Backup>(&raw).ok()
}

/// The original app stored one 61-character string per year, under the year
/// itself as the key.
fn original_app(store: &web_sys::Storage) -> Option<Backup> {
    let length = store.length().ok()?;
    let mut years = BTreeMap::new();
    for index in 0..length {
        let Ok(Some(key)) = store.key(index) else {
            continue;
        };
        let Ok(year) = key.parse::<i32>() else { continue };
        if !(1970..=2200).contains(&year) {
            continue;
        }
        if let Ok(Some(value)) = store.get_item(&key) {
            years.insert(year, value);
        }
    }
    legacy::backup_from_year_strings(years)
}

pub fn save_doc(doc: &Doc) {
    let Some(store) = storage() else { return };
    if let Ok(json) = serde_json::to_string(doc) {
        let _ = store.set_item(DOC_KEY, &json);
    }
}

pub fn load_prefs() -> Prefs {
    let mut prefs = storage()
        .and_then(|store| store.get_item(PREFS_KEY).ok().flatten())
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
    let Some(store) = storage() else { return };
    if let Ok(json) = serde_json::to_string(prefs) {
        let _ = store.set_item(PREFS_KEY, &json);
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
