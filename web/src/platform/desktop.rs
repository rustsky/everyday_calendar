//! The desktop edges. Storage is a handful of small files in the user's data
//! directory, and anything that has to happen inside the page is sent to the
//! webview as script.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{Datelike, Local};
use dioxus::prelude::*;
use edc_core::Date;

/// Where this copy of the calendar lives, as the sync status names it.
pub const HERE: &str = "This computer";

/// The computer's local "today".
pub fn today() -> Date {
    let now = Local::now();
    Date::new(now.year(), now.month0(), now.day())
}

/// Milliseconds since the epoch. Every write is stamped with this, so a device
/// whose clock is badly wrong will win or lose merges it shouldn't.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

/// A short random id, used for goals and for this computer's device id.
pub fn random_id() -> String {
    format!(
        "{:06x}{:06x}",
        fastrand::u32(..0x100_0000),
        fastrand::u32(..0x100_0000)
    )
}

pub async fn sleep(ms: u32) {
    tokio::time::sleep(Duration::from_millis(ms.into())).await;
}

/// The stylesheet already shortens every animation when reduced motion is
/// asked for, so the boot sweep needs no separate check here.
pub fn prefers_reduced_motion() -> bool {
    false
}

/// The window opens wide enough for the year board.
pub fn narrow_viewport() -> bool {
    false
}

/// A desktop app keeps syncing while its window is in the background.
pub fn is_visible() -> bool {
    true
}

/// Moves keyboard focus to an element by id, without disturbing anything else.
pub fn focus(id: &str) {
    let id = serde_json::to_string(id).unwrap_or_default();
    document::eval(&format!("document.getElementById({id})?.focus();"));
}

/// The directory holding this computer's calendar and settings.
pub fn data_dir() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("everydaycalendar"))
}

pub fn store_get(key: &str) -> Option<String> {
    std::fs::read_to_string(data_dir()?.join(key)).ok()
}

/// Writes to a sibling temp file and renames, so an interrupted write can never
/// leave a half-written document behind.
pub fn store_set(key: &str, value: &str) {
    let Some(dir) = data_dir() else { return };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let temp = dir.join(format!("{key}.tmp"));
    if std::fs::write(&temp, value).is_ok() {
        let _ = std::fs::rename(&temp, dir.join(key));
    }
}

/// Only the browser has saves from older versions to pick up.
pub fn store_entries() -> Vec<(String, String)> {
    Vec::new()
}

/// Asks where to save, then writes the file there. Nothing leaves the machine.
pub fn download(filename: &str, contents: &str) {
    let filename = filename.to_string();
    let contents = contents.to_string();
    spawn(async move {
        let Some(target) = rfd::AsyncFileDialog::new()
            .set_file_name(&filename)
            .add_filter("JSON", &["json"])
            .save_file()
            .await
        else {
            return;
        };
        let _ = std::fs::write(target.path(), contents);
    });
}
