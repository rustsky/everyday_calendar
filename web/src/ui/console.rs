//! Everything around the board: goals, the dimmer, and settings.

use dioxus::prelude::*;
use edc_core::legacy;
use edc_core::model::{Accent, GoalId, GoalRecord};
use edc_core::prefs::{Theme, View, normalize_server};

use super::{Ctx, go_to_year, reset_year, show_toast, sync_now, use_ctx};
use crate::{platform, storage};

pub fn GoalBar() -> Element {
    let mut ctx = use_ctx();
    let active = ctx.goal_id();
    let doc = ctx.doc.read();
    let goals: Vec<(GoalId, String, &'static str, bool)> = doc
        .goal_ids()
        .into_iter()
        .filter_map(|id| {
            let record = doc.goal(&id)?;
            let is_active = id == active;
            Some((id, record.name.clone(), record.accent.slug(), is_active))
        })
        .collect();
    let can_add = goals.len() < 8;
    drop(doc);

    rsx! {
        nav { class: "goalbar", aria_label: "Goals",
            ul { class: "goal-list",
                for (id , name , accent , is_active) in goals {
                    li { key: "{id}",
                        button {
                            r#type: "button",
                            class: "goal-chip accent-{accent}",
                            class: if is_active { "is-active" },
                            aria_pressed: is_active,
                            onclick: move |_| ctx.prefs.write().active = Some(id.clone()),
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
                    onclick: move |_| add_goal(ctx),
                    "+ Add a goal"
                }
            }
        }
    }
}

fn add_goal(mut ctx: Ctx) {
    let id = platform::random_id();
    let stamp = ctx.stamp();
    {
        let mut doc = ctx.doc;
        let order = doc.peek().next_order();
        let accent = Accent::ALL[(order as usize) % Accent::ALL.len()];
        doc.write().put_goal(
            &id,
            GoalRecord {
                name: "New goal".to_string(),
                accent,
                deleted: false,
                order,
                stamp,
            },
        );
    }
    ctx.prefs.write().active = Some(id);
    ctx.dirty.set(true);
}

/// Year and view, carried on the front of the board. These two get reached for
/// often enough that putting them behind a flip would be in the way.
pub fn BoardNav() -> Element {
    let mut ctx = use_ctx();
    let view = ctx.prefs.read().view;
    let year = *ctx.year.read();
    let today = *ctx.today.read();
    let on_today = year == today.year;

    rsx! {
        div { class: "board-nav",
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
                    onclick: move |_| ctx.prefs.write().view = View::Year,
                    "Year"
                }
                button {
                    r#type: "button",
                    class: "segment",
                    class: if view == View::Month { "is-on" },
                    aria_pressed: view == View::Month,
                    onclick: move |_| ctx.prefs.write().view = View::Month,
                    "Month"
                }
            }
        }
    }
}

/// The controls, which live on the back of the board now that nothing sits
/// outside the frame. Flipping is triggered from the board itself.
pub fn Controls() -> Element {
    let mut ctx = use_ctx();
    let prefs = ctx.prefs.read();
    let brightness = prefs.brightness;
    let sound = prefs.sound;
    let theme = prefs.theme;
    drop(prefs);

    rsx! {
        section { class: "controls", aria_label: "Controls",
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
                            ctx.prefs.write().brightness = value.min(100);
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
                        let mut prefs = ctx.prefs.write();
                        prefs.sound = !prefs.sound;
                    },
                    if sound { "Sound on" } else { "Sound off" }
                }
                button {
                    r#type: "button",
                    class: "button",
                    aria_pressed: theme == Theme::Midnight,
                    onclick: move |_| {
                        let mut prefs = ctx.prefs.write();
                        prefs
                            .theme = if prefs.theme == Theme::Studio {
                            Theme::Midnight
                        } else {
                            Theme::Studio
                        };
                    },
                    if theme == Theme::Studio { "Midnight" } else { "Studio" }
                }
            }

            SyncPill {}
        }
    }
}

fn SyncPill() -> Element {
    let ctx = use_ctx();
    let status = ctx.sync.read().clone();
    let local = status.is_local();

    rsx! {
        div { class: "console-group sync",
            span {
                class: "sync-pill sync-{status.slug()}",
                title: "{status.detail()}",
                span { class: "sync-dot", aria_hidden: "true" }
                "{status.label()}"
            }
            if !local {
                button {
                    r#type: "button",
                    class: "link",
                    onclick: move |_| sync_now(ctx),
                    "Sync now"
                }
            }
        }
    }
}

pub fn Settings() -> Element {
    let mut ctx = use_ctx();
    let goal_id = ctx.goal_id();
    let doc = ctx.doc.read();
    let record = doc.goal(&goal_id).cloned();
    let can_delete = doc.goal_ids().len() > 1;
    let day_entries = doc.day_entries();
    drop(doc);

    let Some(record) = record else {
        return rsx! {};
    };
    let name = record.name.clone();
    let accent = record.accent;

    let prefs = ctx.prefs.read();
    let ritual = prefs.ritual;
    let boot = prefs.boot_sequence;
    let server = prefs.server.clone().unwrap_or_default();
    drop(prefs);

    let year = *ctx.year.read();
    let today = *ctx.today.read();
    let device = ctx.device.read().clone();

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
                        let value = event.value();
                        let stamp = ctx.stamp();
                        let id = ctx.goal_id();
                        ctx.doc.write().edit_goal(&id, stamp, |record| record.name = value);
                        ctx.dirty.set(true);
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
                                let stamp = ctx.stamp();
                                let id = ctx.goal_id();
                                ctx.doc.write().edit_goal(&id, stamp, |record| record.accent = option);
                                ctx.dirty.set(true);
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
                on_toggle: move |value| ctx.prefs.write().ritual = value,
            }
            Switch {
                id: "boot",
                label: "Power-on light sequence",
                hint: "The sweep across the board when it first lights up.",
                checked: boot,
                on_toggle: move |value| ctx.prefs.write().boot_sequence = value,
            }

            // The web client syncs with whatever served it; only the desktop
            // app needs telling where the server is.
            if cfg!(feature = "desktop") {
                div { class: "settings-row",
                    label { r#for: "sync-server", "Sync server" }
                    input {
                        id: "sync-server",
                        r#type: "text",
                        inputmode: "url",
                        spellcheck: "false",
                        autocomplete: "off",
                        value: "{server}",
                        placeholder: "http://my-computer:8080",
                        // On commit rather than every keystroke, so a half-typed
                        // address is never probed.
                        onchange: move |event| {
                            ctx.prefs.write().server = normalize_server(&event.value());
                        },
                    }
                }
            }

            div { class: "settings-row settings-actions",
                button {
                    r#type: "button",
                    class: "button",
                    onclick: move |_| {
                        let json = legacy::export_json(&ctx.doc.peek());
                        platform::download(&storage::export_filename(today), &json);
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
                            let Ok(text) = file.read_string().await else {
                                show_toast(ctx, "Couldn't read that file.", None);
                                return;
                            };
                            let stamp = ctx.stamp();
                            let outcome = {
                                let mut doc = ctx.doc.write();
                                legacy::import_json(&mut doc, &text, stamp, platform::random_id)
                            };
                            match outcome {
                                Ok(count) => {
                                    ctx.dirty.set(true);
                                    show_toast(
                                        ctx,
                                        format!(
                                            "Imported {count} {}.",
                                            if count == 1 { "goal" } else { "goals" },
                                        ),
                                        None,
                                    );
                                }
                                Err(error) => {
                                    show_toast(ctx, format!("That file didn't parse: {error}"), None)
                                }
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

            p { class: "settings-note",
                "This device is "
                code { "{device}" }
                ". It stamps every change, and breaks ties when two devices "
                "edit the same day. {day_entries} day writes stored."
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

/// Deletion is a flag rather than a removal, so it survives a merge with a
/// device that still has the goal.
fn delete_active_goal(mut ctx: Ctx) {
    let id = ctx.goal_id();
    let stamp = ctx.stamp();
    let name = {
        let mut doc = ctx.doc;
        let name = doc
            .peek()
            .goal(&id)
            .map(|record| record.name.clone())
            .unwrap_or_default();
        if doc.peek().goal_ids().len() <= 1 {
            return;
        }
        doc.write()
            .edit_goal(&id, stamp, |record| record.deleted = true);
        name
    };

    let next = ctx.doc.peek().goal_ids().first().cloned();
    ctx.prefs.write().active = next;
    ctx.dirty.set(true);
    show_toast(ctx, format!("Deleted \"{name}\"."), None);
}
