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

/// Measures the page and sends the window size that fits the board: as wide
/// as the board gets, as tall as the board is, and no taller than the screen.
/// When the screen is too short, the width shrinks with the height, since the
/// board keeps its shape. It never returns, so the channel stays open.
const FIT_WINDOW_JS: &str = r#"
    const frame = () => new Promise(requestAnimationFrame);
    let page, stage;
    while (!(page = document.querySelector(".page")) || !(stage = document.querySelector(".stage"))) {
        await frame();
    }
    // Room for the title bar, and a little air above the Dock.
    const CHROME = 48;
    let sent = "";
    const fit = () => {
        const pageStyle = getComputedStyle(page);
        const padX = parseFloat(pageStyle.paddingLeft) + parseFloat(pageStyle.paddingRight);
        const boardMax = parseFloat(getComputedStyle(stage).maxWidth) || stage.offsetWidth;
        const ratio = page.offsetHeight / (stage.offsetWidth + padX);
        let width = Math.min(boardMax, screen.availWidth - padX) + padX;
        let height = width * ratio;
        const room = screen.availHeight - CHROME;
        if (height > room) {
            height = room;
            width = height / ratio;
        }
        width = Math.round(width);
        height = Math.round(width * ratio);
        const size = `${width}x${height}`;
        if (size !== sent && (Math.abs(width - innerWidth) > 1 || Math.abs(height - innerHeight) > 1)) {
            sent = size;
            dioxus.send([width, height, screen.availTop || 0, screen.availHeight]);
        }
    };
    new ResizeObserver(fit).observe(page);
    fit();
    await new Promise(() => {});
"#;

/// Keeps the window wrapped around the board. The page reports the size it
/// needs whenever that changes: switching between year and month, a goal chip
/// wrapping onto a second line, the stylesheet arriving.
pub fn use_fit_window() {
    use dioxus::desktop::{LogicalPosition, LogicalSize, window};

    use_hook(|| {
        let desktop = window();
        spawn(async move {
            let mut page = document::eval(FIT_WINDOW_JS);
            while let Ok([width, height, avail_top, avail_height]) = page.recv::<[f64; 4]>().await {
                let window = &desktop.window;
                window.set_inner_size(LogicalSize::new(width, height));

                // Growing keeps the top edge where it was, which can push the
                // bottom under the Dock or off the screen. Lift it back up.
                let scale = window.scale_factor();
                let (Ok(position), outer) = (window.outer_position(), window.outer_size()) else {
                    continue;
                };
                let position = position.to_logical::<f64>(scale);
                let outer_height = outer.to_logical::<f64>(scale).height;
                let bottom = avail_top + avail_height;
                if position.y + outer_height > bottom {
                    let y = (bottom - outer_height).max(avail_top);
                    window.set_outer_position(LogicalPosition::new(position.x, y));
                }
            }
        });
    });
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
