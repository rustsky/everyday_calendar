//! The Every Day Calendar — a Dioxus recreation of Simone Giertz's habit board.

// Dioxus components are types, and are named like types.
#![allow(non_snake_case)]

mod audio;
mod platform;
mod storage;
mod sync;
mod ui;

use dioxus::prelude::*;

const MAIN_CSS: Asset = asset!("/assets/main.css");

#[cfg(not(feature = "desktop"))]
fn main() {
    dioxus::launch(Root);
}

/// Painted before the stylesheet lands, so the window never flashes white.
/// The web build gets the same from `index.html`.
#[cfg(feature = "desktop")]
const DESKTOP_HEAD: &str = r#"<meta name="color-scheme" content="light dark" />
<style>
  html, body { margin: 0; background: #7ccbc4; }
  @media (prefers-color-scheme: dark) { html, body { background: #10151a; } }
</style>"#;

#[cfg(feature = "desktop")]
fn main() {
    use dioxus::desktop::tao::window::Icon;
    use dioxus::desktop::{Config, LogicalSize, WindowBuilder, icon_from_memory};

    // Opens at the board's width. Once the page has rendered it sizes the
    // window to the board (`platform::use_fit_window`), which is also why the
    // window isn't resizable: a dragged edge would only be snapped back.
    let window = WindowBuilder::new()
        .with_title("The Every Day Calendar")
        .with_inner_size(LogicalSize::new(700.0, 860.0))
        .with_resizable(false);

    let mut config = Config::new()
        .with_window(window)
        .with_custom_head(DESKTOP_HEAD.to_string());
    if let Ok(icon) = icon_from_memory::<Icon>(include_bytes!("../icons/icon.png")) {
        config = config.with_icon(icon);
    }
    if let Some(dir) = platform::data_dir() {
        // WebView2 otherwise keeps its profile beside the executable, where an
        // installed app may not be allowed to write.
        config = config.with_data_directory(dir.join("webview"));
    }

    dioxus::LaunchBuilder::desktop()
        .with_cfg(config)
        .launch(Root);
}

fn Root() -> Element {
    rsx! {
        document::Stylesheet { href: MAIN_CSS }
        ui::App {}
    }
}
