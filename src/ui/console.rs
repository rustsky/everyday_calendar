//! Everything around the board: goals, the readout, the dimmer, and settings.

use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use wasm_bindgen::{JsCast, JsValue};

use super::{Ctx, go_to_year, reset_year, show_toast, use_ctx};
use crate::date::Date;
use crate::stats::Stats;
use crate::store::{self, Accent, Goal, Theme, View};

pub fn GoalBar() -> Element {
    let mut ctx = use_ctx();
    let state = ctx.state.read();
    let goals: Vec<(usize, String, &'static str, bool)> = state
        .goals
        .iter()
        .enumerate()
        .map(|(index, goal)| {
            (
                index,
                goal.name.clone(),
                goal.accent.slug(),
                index == state.active,
            )
        })
        .collect();
    let can_add = state.goals.len() < 8;
    drop(state);

    rsx! {
        nav { class: "goalbar", aria_label: "Goals",
            ul { class: "goal-list",
                for (index , name , accent , active) in goals {
                    li { key: "{index}",
                        button {
                            r#type: "button",
                            class: "goal-chip accent-{accent}",
                            class: if active { "is-active" },
                            aria_pressed: active,
                            onclick: move |_| ctx.state.write().active = index,
                            span { class: "goal-dot", aria_hidden: "true" }
                            "{name}"
                        }
                    }
                }
            }
            if can_add {
                button {
                    r#type: "button",
                    class: "goal-add",
                    onclick: move |_| {
                        let mut state = ctx.state.write();
                        let accent = Accent::ALL[state.goals.len() % Accent::ALL.len()];
                        state.goals.push(Goal::new("New goal", accent));
                        state.active = state.goals.len() - 1;
                    },
                    "+ Add a goal"
                }
            }
        }
    }
}

pub fn Readout() -> Element {
    let ctx = use_ctx();
    let stats = ctx.stats();
    let year = *ctx.year.read();
    let target = stats.next_milestone();
    let progress = target
        .map(|t| (stats.current as f32 / t as f32 * 100.0).clamp(0.0, 100.0))
        .unwrap_or(100.0);

    rsx! {
        section { class: "readout", aria_label: "Progress",
            Tile {
                label: "Current streak",
                value: "{stats.current}",
                unit: if stats.current == 1 { "day" } else { "days" },
                emphasis: true,
            }
            Tile { label: "Longest streak", value: "{stats.longest}", unit: "days" }
            Tile {
                label: "{year}",
                value: "{stats.year_lit}",
                unit: "of {stats.year_days} · {stats.year_percent()}%",
            }
            Tile { label: "Last 365 days", value: "{stats.last_year}", unit: "days" }
            Tile { label: "All time", value: "{stats.total}", unit: "days" }
        }
        MilestoneBar { stats, target, progress }
    }
}

#[derive(Props, Clone, PartialEq)]
struct TileProps {
    label: String,
    value: String,
    unit: String,
    #[props(default = false)]
    emphasis: bool,
}

fn Tile(props: TileProps) -> Element {
    rsx! {
        div {
            class: "tile",
            class: if props.emphasis { "is-primary" },
            span { class: "tile-value", "{props.value}" }
            span { class: "tile-unit", "{props.unit}" }
            span { class: "tile-label", "{props.label}" }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct MilestoneProps {
    stats: Stats,
    target: Option<u32>,
    progress: f32,
}

fn MilestoneBar(props: MilestoneProps) -> Element {
    let stats = props.stats;
    let caption = match props.target {
        Some(target) if stats.current == 0 => {
            format!("Light today to start a streak. First milestone: {target} days.")
        }
        Some(target) => format!(
            "{} more {} to {target}.",
            target - stats.current,
            if target - stats.current == 1 {
                "day"
            } else {
                "days"
            }
        ),
        None => "Past every milestone. Keep going.".to_string(),
    };

    rsx! {
        div { class: "milestone",
            div {
                class: "milestone-track",
                role: "progressbar",
                aria_valuemin: 0,
                aria_valuemax: 100,
                aria_valuenow: props.progress as i64,
                aria_label: "Progress to next milestone",
                span { class: "milestone-fill", style: "width: {props.progress}%;" }
            }
            p { class: "milestone-caption",
                class: if stats.at_risk { "is-warning" },
                if stats.at_risk {
                    "Today is still dark. "
                }
                "{caption}"
            }
        }
    }
}

pub fn Console() -> Element {
    let mut ctx = use_ctx();
    let state = ctx.state.read();
    let view = state.view;
    let brightness = state.brightness;
    let sound = state.sound;
    let theme = state.theme;
    drop(state);

    let year = *ctx.year.read();
    let today = *ctx.today.read();
    let settings_open = *ctx.settings_open.read();
    let flipped = *ctx.flipped.read();
    let on_today = year == today.year;

    rsx! {
        section { class: "console", aria_label: "Controls",
            div { class: "console-group",
                button {
                    r#type: "button",
                    class: "step",
                    aria_label: "Previous year",
                    onclick: move |_| go_to_year(ctx, year - 1),
                    "❮"
                }
                span { class: "year-readout", "{year}" }
                button {
                    r#type: "button",
                    class: "step",
                    aria_label: "Next year",
                    onclick: move |_| go_to_year(ctx, year + 1),
                    "❯"
                }
                button {
                    r#type: "button",
                    class: "button",
                    disabled: on_today,
                    onclick: move |_| go_to_year(ctx, today.year),
                    "Today"
                }
            }

            div { class: "console-group segmented", role: "group", aria_label: "Board view",
                button {
                    r#type: "button",
                    class: "segment",
                    class: if view == View::Year { "is-on" },
                    aria_pressed: view == View::Year,
                    onclick: move |_| ctx.state.write().view = View::Year,
                    "Year"
                }
                button {
                    r#type: "button",
                    class: "segment",
                    class: if view == View::Month { "is-on" },
                    aria_pressed: view == View::Month,
                    onclick: move |_| ctx.state.write().view = View::Month,
                    "Month"
                }
            }

            div { class: "console-group dimmer",
                label { r#for: "brightness", "Brightness" }
                input {
                    id: "brightness",
                    r#type: "range",
                    min: "0",
                    max: "100",
                    step: "1",
                    value: "{brightness}",
                    oninput: move |event| {
                        if let Ok(value) = event.value().parse::<u8>() {
                            ctx.state.write().brightness = value.min(100);
                        }
                    },
                }
                span { class: "dimmer-value", "{brightness}%" }
            }

            div { class: "console-group",
                button {
                    r#type: "button",
                    class: "button",
                    class: if sound { "is-on" },
                    aria_pressed: sound,
                    onclick: move |_| {
                        let mut state = ctx.state.write();
                        state.sound = !state.sound;
                    },
                    if sound { "Sound on" } else { "Sound off" }
                }
                button {
                    r#type: "button",
                    class: "button",
                    aria_pressed: theme == Theme::Midnight,
                    onclick: move |_| {
                        let mut state = ctx.state.write();
                        state.theme = if state.theme == Theme::Studio {
                            Theme::Midnight
                        } else {
                            Theme::Studio
                        };
                    },
                    if theme == Theme::Studio { "Midnight" } else { "Studio" }
                }
                button {
                    r#type: "button",
                    class: "button",
                    class: if flipped { "is-on" },
                    aria_pressed: flipped,
                    onclick: move |_| {
                        let next = !*ctx.flipped.peek();
                        ctx.flipped.set(next);
                    },
                    if flipped { "Back to the board" } else { "Flip it over" }
                }
                button {
                    r#type: "button",
                    class: "button",
                    class: if settings_open { "is-on" },
                    aria_expanded: settings_open,
                    onclick: move |_| {
                        let next = !*ctx.settings_open.peek();
                        ctx.settings_open.set(next);
                    },
                    "Settings"
                }
            }
        }
        p { class: "hint",
            if ctx.state.read().ritual {
                "Press and hold a day until it fills. Hold January 1 for ten seconds to clear the year."
            } else {
                "Tap a day to light it, or drag across several. Quick-tap mode is on."
            }
        }
    }
}

pub fn Settings() -> Element {
    let mut ctx = use_ctx();
    let state = ctx.state.read();
    let name = state.active_goal().name.clone();
    let accent = state.active_goal().accent;
    let ritual = state.ritual;
    let boot = state.boot_sequence;
    let can_delete = state.goals.len() > 1;
    drop(state);

    let year = *ctx.year.read();
    let today = *ctx.today.read();

    rsx! {
        section { class: "settings", aria_label: "Settings",
            div { class: "settings-row",
                label { r#for: "goal-name", "Goal" }
                input {
                    id: "goal-name",
                    r#type: "text",
                    maxlength: "48",
                    value: "{name}",
                    placeholder: "What are you doing every day?",
                    oninput: move |event| {
                        ctx.state.write().active_goal_mut().name = event.value();
                    },
                }
            }

            div { class: "settings-row",
                span { class: "settings-label", "Glow" }
                div { class: "swatches", role: "radiogroup", aria_label: "LED colour",
                    for option in Accent::ALL {
                        button {
                            key: "{option.slug()}",
                            r#type: "button",
                            role: "radio",
                            class: "swatch accent-{option.slug()}",
                            class: if option == accent { "is-on" },
                            aria_checked: option == accent,
                            aria_label: "{option.label()}",
                            title: "{option.label()}",
                            onclick: move |_| {
                                ctx.state.write().active_goal_mut().accent = option;
                            },
                        }
                    }
                }
            }

            Switch {
                id: "ritual",
                label: "Press and hold to change a day",
                hint: "Off turns the board back into a quick-tap toggle, like the original web app.",
                checked: ritual,
                on_toggle: move |value| ctx.state.write().ritual = value,
            }
            Switch {
                id: "boot",
                label: "Power-on light sequence",
                hint: "The sweep across the board when the page loads.",
                checked: boot,
                on_toggle: move |value| ctx.state.write().boot_sequence = value,
            }

            div { class: "settings-row settings-actions",
                button {
                    r#type: "button",
                    class: "button",
                    onclick: move |_| {
                        let json = store::export_json(&ctx.state.peek());
                        download_backup(&json, today);
                    },
                    "Export backup"
                }
                label { class: "button", r#for: "import-file", "Import backup" }
                input {
                    id: "import-file",
                    class: "sr-only",
                    r#type: "file",
                    accept: ".json,application/json",
                    onchange: move |event| {
                        let Some(file) = event.files().into_iter().next() else {
                            return;
                        };
                        spawn(async move {
                            match file.read_string().await {
                                Ok(text) => match store::import_json(&text) {
                                    Ok(imported) => {
                                        let goals = imported.goals.len();
                                        ctx.state.set(imported);
                                        show_toast(
                                            ctx,
                                            format!(
                                                "Imported {goals} {}.",
                                                if goals == 1 { "goal" } else { "goals" },
                                            ),
                                            None,
                                        );
                                    }
                                    Err(error) => {
                                        show_toast(ctx, format!("That file didn't parse: {error}"), None)
                                    }
                                },
                                Err(_) => show_toast(ctx, "Couldn't read that file.", None),
                            }
                        });
                    },
                }
                button {
                    r#type: "button",
                    class: "button danger",
                    onclick: move |_| reset_year(ctx),
                    "Clear {year}"
                }
                if can_delete {
                    button {
                        r#type: "button",
                        class: "button danger",
                        onclick: move |_| delete_active_goal(ctx),
                        "Delete this goal"
                    }
                }
            }

            details { class: "shortcuts",
                summary { "Keyboard" }
                dl {
                    dt { "Space or Enter" }
                    dd { "Hold to light or clear the focused day." }
                    dt { "Arrow keys" }
                    dd { "Move across the board. Up and down walk days, left and right walk months." }
                    dt { "Home and End" }
                    dd { "First and last day of the month." }
                    dt { "Page Up and Page Down" }
                    dd { "Previous and next month." }
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct SwitchProps {
    id: String,
    label: String,
    hint: String,
    checked: bool,
    on_toggle: EventHandler<bool>,
}

fn Switch(props: SwitchProps) -> Element {
    let checked = props.checked;
    rsx! {
        div { class: "settings-row switch-row",
            div { class: "switch-text",
                label { r#for: "{props.id}", "{props.label}" }
                p { class: "switch-hint", "{props.hint}" }
            }
            button {
                id: "{props.id}",
                r#type: "button",
                role: "switch",
                class: "switch",
                class: if checked { "is-on" },
                aria_checked: checked,
                onclick: move |_| props.on_toggle.call(!checked),
                span { class: "switch-knob", aria_hidden: "true" }
            }
        }
    }
}

fn delete_active_goal(mut ctx: Ctx) {
    let removed = {
        let mut state = ctx.state.write();
        if state.goals.len() <= 1 {
            return;
        }
        let index = state.active;
        let goal = state.goals.remove(index);
        state.active = index.saturating_sub(1);
        goal.name
    };
    show_toast(ctx, format!("Deleted \"{removed}\"."), None);
}

/// Hands the browser a JSON file. Nothing leaves the machine.
fn download_backup(json: &str, today: Date) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let parts = js_sys::Array::new();
    parts.push(&JsValue::from_str(json));

    let Ok(blob) = web_sys::Blob::new_with_str_sequence(&parts) else {
        return;
    };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else {
        return;
    };
    if let Ok(anchor) = document
        .create_element("a")
        .and_then(|el| el.dyn_into::<web_sys::HtmlAnchorElement>().map_err(Into::into))
    {
        anchor.set_href(&url);
        anchor.set_download(&store::export_filename(today));
        anchor.click();
    }

    // Firefox cancels the download if the object URL is revoked too eagerly.
    spawn(async move {
        TimeoutFuture::new(20_000).await;
        let _ = web_sys::Url::revoke_object_url(&url);
    });
}
