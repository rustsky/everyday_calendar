//! The browser-shaped edges: the clock, randomness, and file downloads.
//!
//! Keeping these in one place is what lets `edc-core` stay platform-free and
//! compile into both the wasm client and the sync server.

use edc_core::Date;
use wasm_bindgen::{JsCast, JsValue};

/// The browser's local "today".
pub fn today() -> Date {
    let now = js_sys::Date::new_0();
    Date::new(now.get_full_year() as i32, now.get_month(), now.get_date())
}

/// Milliseconds since the epoch. Every write is stamped with this, so a device
/// whose clock is badly wrong will win or lose merges it shouldn't.
pub fn now_ms() -> u64 {
    js_sys::Date::now() as u64
}

/// A short random id, used for goals and for this browser's device id.
pub fn random_id() -> String {
    let high = (js_sys::Math::random() * (1u64 << 24) as f64) as u64;
    let low = (js_sys::Math::random() * (1u64 << 24) as f64) as u64;
    format!("{:06x}{:06x}", high & 0xff_ffff, low & 0xff_ffff)
}

pub fn prefers_reduced_motion() -> bool {
    web_sys::window()
        .and_then(|window| {
            window
                .match_media("(prefers-reduced-motion: reduce)")
                .ok()
                .flatten()
        })
        .map(|query| query.matches())
        .unwrap_or(false)
}

pub fn narrow_viewport() -> bool {
    web_sys::window()
        .and_then(|window| window.inner_width().ok())
        .and_then(|value| value.as_f64())
        .is_some_and(|width| width < 700.0)
}

pub fn is_visible() -> bool {
    web_sys::window()
        .and_then(|window| window.document())
        .map(|document| document.visibility_state() == web_sys::VisibilityState::Visible)
        .unwrap_or(true)
}

/// Moves keyboard focus to an element by id, without disturbing anything else.
pub fn focus(id: &str) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    if let Some(element) = document.get_element_by_id(id) {
        if let Ok(element) = element.dyn_into::<web_sys::HtmlElement>() {
            let _ = element.focus();
        }
    }
}

/// Hands the browser a file to save. Nothing leaves the machine.
pub fn download(filename: &str, contents: &str) {
    let _ = try_download(filename, contents);
}

fn try_download(filename: &str, contents: &str) -> Result<(), JsValue> {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| JsValue::from_str("no document"))?;

    let parts = js_sys::Array::new();
    parts.push(&JsValue::from_str(contents));
    let blob = web_sys::Blob::new_with_str_sequence(&parts)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)?;

    let anchor = document
        .create_element("a")?
        .dyn_into::<web_sys::HtmlAnchorElement>()?;
    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.click();

    // Firefox cancels the download if the object URL is revoked too eagerly,
    // so this one is left for the page to clean up on unload.
    Ok(())
}
