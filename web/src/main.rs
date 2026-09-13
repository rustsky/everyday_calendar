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

fn main() {
    dioxus::launch(Root);
}

fn Root() -> Element {
    rsx! {
        document::Stylesheet { href: MAIN_CSS }
        ui::App {}
    }
}
