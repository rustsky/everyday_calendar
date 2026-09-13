//! The back of the board: the log. Every year tracked, one row each, plus the
//! totals and the weekday breakdown the front of the board cannot show.

use dioxus::prelude::*;
use edc_core::date;
use edc_core::stats::{self, Stats};

use super::use_ctx;

pub fn About() -> Element {
    let mut ctx = use_ctx();
    let goal = ctx.goal_id();
    let today = *ctx.today.read();
    let stats = ctx.stats();
    let doc = ctx.doc.read();

    let years: Vec<YearRow> = doc
        .years(&goal)
        .into_iter()
        .map(|year| {
            let days = date::days_in_year(year);
            let bits = doc.year_bits(&goal, year);
            YearRow {
                year,
                lit: bits.count(),
                days,
                runs: stats::lit_runs(&bits, days),
            }
        })
        .collect();

    let rates = stats::weekday_rates(&doc, &goal, *ctx.year.read(), today);
    let total: u32 = years.iter().map(|row| row.lit).sum();

    rsx! {
        div { class: "backplate",
            div { class: "hangers", aria_hidden: "true",
                span { class: "hanger" }
                span { class: "hanger" }
            }

            div { class: "backplate-body log",
                div { class: "log-head",
                    h2 { "All time" }
                    span { class: "log-total", "{total} days lit" }
                }

                if years.is_empty() {
                    p { class: "log-empty",
                        "Nothing logged yet. Light a day on the front and it shows up here."
                    }
                } else {
                    div { class: "log-years",
                        for row in years.iter() {
                            LogYear { key: "{row.year}", row: row.clone() }
                        }
                    }
                }

                div { class: "log-figures",
                    Figure { label: "Current", value: stats.current }
                    Figure { label: "Longest", value: stats.longest }
                    Figure { label: "Last 365", value: stats.last_year }
                }

                MilestoneBar { stats }

                section { class: "log-weekdays", aria_label: "Completion by weekday",
                    h3 { "By weekday" }
                    div { class: "weekday-bars",
                        for (index , name) in date::WEEKDAYS_SHORT.iter().enumerate() {
                            div { key: "{index}", class: "weekday-bar",
                                div {
                                    class: "weekday-track",
                                    role: "progressbar",
                                    aria_valuemin: 0,
                                    aria_valuemax: 100,
                                    aria_valuenow: rates[index].percent() as i64,
                                    aria_label: "{date::WEEKDAYS_LONG[index]}",
                                    span {
                                        class: "weekday-fill",
                                        style: "width: {rates[index].percent()}%;",
                                    }
                                }
                                span { class: "weekday-name", "{name}" }
                                span { class: "weekday-pct", "{rates[index].percent()}%" }
                            }
                        }
                    }
                }

                div { class: "log-credit",
                    p {
                        "After Simone Giertz's "
                        a { href: "https://www.kickstarter.com/projects/simonegiertz/the-every-day-calendar",
                            "Every Day Calendar"
                        }
                        ", 2018. Original web edition by "
                        a { href: "https://github.com/zmxv/everydaycalendar", "Zhen Wang" }
                        "."
                    }
                    p { "Your days live in this browser. No account, no analytics." }
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

#[derive(Clone, PartialEq)]
struct YearRow {
    year: i32,
    lit: u32,
    days: u32,
    /// Lit days as `(first ordinal, length)`, so a row is a few dozen spans.
    runs: Vec<(u32, u32)>,
}

#[derive(Props, Clone, PartialEq)]
struct LogYearProps {
    row: YearRow,
}

fn LogYear(props: LogYearProps) -> Element {
    let row = props.row;
    let percent = (row.lit * 100).div_ceil(row.days).min(100);
    let span = row.days as f32;

    rsx! {
        div { class: "log-year",
            div { class: "log-year-head",
                span { class: "log-year-label", "{row.year}" }
                span { class: "log-year-count", "{row.lit} · {percent}%" }
            }
            div {
                class: "log-track",
                role: "img",
                aria_label: "{row.year}: {row.lit} of {row.days} days lit",
                for (start , length) in row.runs.iter().copied() {
                    span {
                        key: "{start}",
                        class: "log-run",
                        style: "left: {start as f32 / span * 100.0}%; width: {length as f32 / span * 100.0}%;",
                    }
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct FigureProps {
    label: &'static str,
    value: u32,
}

fn Figure(props: FigureProps) -> Element {
    rsx! {
        div { class: "log-figure",
            span { class: "log-figure-label", "{props.label}" }
            span { class: "log-figure-value", "{props.value}" }
            span { class: "log-figure-unit", if props.value == 1 { "day" } else { "days" } }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct MilestoneProps {
    stats: Stats,
}

/// The milestone track, moved here from the console along with the rest of the
/// numbers.
fn MilestoneBar(props: MilestoneProps) -> Element {
    let stats = props.stats;
    let target = stats.next_milestone();
    let progress = target
        .map(|t| (stats.current as f32 / t as f32 * 100.0).clamp(0.0, 100.0))
        .unwrap_or(100.0);

    let caption = match target {
        Some(target) if stats.current == 0 => {
            format!("Light today to start a streak. First milestone: {target} days.")
        }
        Some(target) => {
            let left = target - stats.current;
            format!(
                "{left} more {} to {target}.",
                if left == 1 { "day" } else { "days" }
            )
        }
        None => "Past every milestone. Keep going.".to_string(),
    };

    rsx! {
        div { class: "milestone",
            div {
                class: "milestone-track",
                role: "progressbar",
                aria_valuemin: 0,
                aria_valuemax: 100,
                aria_valuenow: progress as i64,
                aria_label: "Progress to next milestone",
                span { class: "milestone-fill", style: "width: {progress}%;" }
            }
            p {
                class: "milestone-caption",
                class: if stats.at_risk { "is-warning" },
                if stats.at_risk {
                    "Today is still dark. "
                }
                "{caption}"
            }
        }
    }
}
