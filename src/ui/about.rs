//! The back of the board — the panel you see when you flip it over.

use dioxus::prelude::*;

use super::use_ctx;

pub fn About() -> Element {
    let mut ctx = use_ctx();

    rsx! {
        div { class: "backplate",
            div { class: "hangers", aria_hidden: "true",
                span { class: "hanger" }
                span { class: "hanger" }
            }

            div { class: "backplate-body",
                h2 { "The Every Day Calendar" }
                p { class: "lede",
                    "Pick one thing. Do it. Light the day. The only rule is that the "
                    "chain looks better unbroken."
                }

                h3 { "How the board works" }
                dl {
                    dt { "Press and hold" }
                    dd {
                        "A day only changes when you mean it. Hold until the pad fills, "
                        "and it locks in. Clearing a lit day takes a longer hold than "
                        "lighting one — breaking a streak should cost more than making it."
                    }
                    dt { "Hold January 1" }
                    dd {
                        "Ten seconds on the first pad clears the whole year, exactly like "
                        "the hardware. There's an Undo if your thumb was faster than your mind."
                    }
                    dt { "The dimmer" }
                    dd {
                        "The physical calendar has a brightness knob so it can live in a "
                        "bedroom without lighting it up. So does this one."
                    }
                    dt { "More than one goal" }
                    dd {
                        "The original board tracks a single habit. Here you can keep up "
                        "to eight, each with its own glow, and switch between them above "
                        "the frame."
                    }
                }

                h3 { "Where your data lives" }
                p {
                    "In this browser, in local storage, and nowhere else. There is no "
                    "account, no sync, and no analytics. Export a backup from Settings if "
                    "you want a copy you control — saves from the original "
                    "everydaycalendar.app are picked up automatically the first time you "
                    "load this page."
                }

                h3 { "Credit" }
                p {
                    "The Every Day Calendar was designed by "
                    a { href: "https://yetch.studio/", "Simone Giertz" }
                    ", funded on "
                    a { href: "https://www.kickstarter.com/projects/simonegiertz/the-every-day-calendar",
                        "Kickstarter"
                    }
                    " in 2018, and it is a genuinely good object. This is an unaffiliated "
                    "web tribute, rebuilt in Rust and "
                    a { href: "https://dioxuslabs.com/", "Dioxus" }
                    ", after the original HTML edition by "
                    a { href: "https://github.com/zmxv/everydaycalendar", "Zhen Wang" }
                    "."
                }

                button {
                    r#type: "button",
                    class: "button",
                    onclick: move |_| ctx.flipped.set(false),
                    "Back to the board"
                }
            }
        }
    }
}
