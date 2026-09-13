//! The board itself: the PCB face, its silkscreen, and the 366 pads.

use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

use super::{Ctx, console, go_to_month, paint, press, release, use_ctx};
use crate::platform;
use edc_core::prefs::View;
use edc_core::{Date, date};

#[derive(Props, Clone, PartialEq)]
pub struct BoardProps {
    pub view: View,
}

pub fn Board(props: BoardProps) -> Element {
    let mut ctx = use_ctx();
    let year = *ctx.year.read();
    let today = *ctx.today.read();
    let streak = ctx.stats().current;
    let unit = if streak == 1 { "day" } else { "days" };

    let grid = match props.view {
        View::Year => rsx! { YearGrid { year, today } },
        View::Month => rsx! { MonthGrid { year, today } },
    };

    rsx! {
        div { class: "board",
            div { class: "silkscreen",
                console::GoalBar {}
                span { class: "silk-streak",
                    span { class: "silk-streak-value", "{streak}" }
                    span { class: "silk-streak-unit", "{unit}" }
                }
            }
            {grid}
            div { class: "board-foot",
                console::BoardNav {}
                button {
                    r#type: "button",
                    class: "button",
                    onclick: move |_| ctx.flipped.set(true),
                    "Flip it over"
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct GridProps {
    year: i32,
    today: Date,
}

fn YearGrid(props: GridProps) -> Element {
    let ctx = use_ctx();
    let year = props.year;
    let today = props.today;
    let bits = ctx.doc.read().year_bits(&ctx.goal_id(), year);
    let hold = *ctx.hold.read();
    let peek = *ctx.peek.read();
    let last_ordinal = date::days_in_year(year) as usize - 1;
    let focus = (*ctx.focus.read()).min(last_ordinal);

    rsx! {
        div { class: "grid year-grid", role: "group", aria_label: "{year} calendar",
            div { class: "months", aria_hidden: "true",
                for (index , name) in date::MONTHS_SHORT.iter().enumerate() {
                    span { key: "{index}", class: "month-head", "{name}" }
                }
            }
            for day_index in 0..31u32 {
                div { key: "row-{day_index}", class: "grid-row",
                    for month in 0..12u32 {
                        {
                            let length = date::days_in_month(year, month);
                            if day_index >= length {
                                rsx! {
                                    span {
                                        key: "blank-{month}-{day_index}",
                                        class: "pad-blank",
                                        aria_hidden: "true",
                                    }
                                }
                            } else {
                                let day = day_index + 1;
                                let ord = date::ordinal(year, month, day);
                                let date = Date { year, ordinal: ord };
                                rsx! {
                                    Pad {
                                        key: "{ord}",
                                        ord,
                                        day,
                                        stagger: day_index as usize * 12 + month as usize,
                                        lit: bits.get(ord),
                                        is_today: date == today,
                                        future: is_future(date, today),
                                        focused: focus == ord,
                                        hold_ms: hold.filter(|h| h.ord == ord).map(|h| h.ms),
                                        arming: hold.is_some_and(|h| h.ord == ord && h.arming_reset),
                                        peeking: peek.is_some_and(|p| p.ord == ord),
                                        weekday_short: date.weekday_short(),
                                        trace: day < length,
                                        label: date.long_label(),
                                        view: View::Year,
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn MonthGrid(props: GridProps) -> Element {
    let ctx = use_ctx();
    let year = props.year;
    let today = props.today;
    let month = *ctx.month.read();
    let bits = ctx.doc.read().year_bits(&ctx.goal_id(), year);
    let hold = *ctx.hold.read();
    let peek = *ctx.peek.read();

    let month_name = date::MONTHS_LONG[month as usize];
    let lead = date::weekday(year, month, 1) as usize;
    let length = date::days_in_month(year, month) as usize;
    let month_start = date::ordinal(year, month, 1);

    // Keep exactly one pad in the tab order even when focus sits in another
    // month.
    let raw_focus = *ctx.focus.read();
    let focus = if (month_start..month_start + length).contains(&raw_focus) {
        raw_focus
    } else {
        month_start
    };

    rsx! {
        div { class: "grid month-grid", role: "group", aria_label: "{month_name} {year}",
            div { class: "month-title",
                button {
                    class: "step",
                    r#type: "button",
                    aria_label: "Previous month",
                    onclick: move |_| go_to_month(ctx, -1),
                    "❮"
                }
                span { class: "month-title-label", "{month_name} {year}" }
                button {
                    class: "step",
                    r#type: "button",
                    aria_label: "Next month",
                    onclick: move |_| go_to_month(ctx, 1),
                    "❯"
                }
            }
            div { class: "weekdays", aria_hidden: "true",
                for (index , name) in date::WEEKDAYS_SHORT.iter().enumerate() {
                    span { key: "{index}", class: "weekday", "{name}" }
                }
            }
            for week in 0..week_count(lead, length) {
                div { key: "week-{week}", class: "month-week",
                    for column in 0..7usize {
                        {
                            let slot = week * 7 + column;
                            if slot < lead || slot >= lead + length {
                                rsx! {
                                    span {
                                        key: "blank-{slot}",
                                        class: "pad-blank",
                                        aria_hidden: "true",
                                    }
                                }
                            } else {
                                let day_index = slot - lead;
                                let day = day_index as u32 + 1;
                                let ord = month_start + day_index;
                                let date = Date { year, ordinal: ord };
                                rsx! {
                                    Pad {
                                        key: "{ord}",
                                        ord,
                                        day,
                                        stagger: day_index,
                                        lit: bits.get(ord),
                                        is_today: date == today,
                                        future: is_future(date, today),
                                        focused: focus == ord,
                                        hold_ms: hold.filter(|h| h.ord == ord).map(|h| h.ms),
                                        arming: hold.is_some_and(|h| h.ord == ord && h.arming_reset),
                                        peeking: peek.is_some_and(|p| p.ord == ord),
                                        weekday_short: date.weekday_short(),
                                        trace: false,
                                        label: date.long_label(),
                                        view: View::Month,
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn week_count(lead: usize, length: usize) -> usize {
    (lead + length).div_ceil(7)
}

fn is_future(date: Date, today: Date) -> bool {
    (date.year, date.ordinal) > (today.year, today.ordinal)
}

#[derive(Props, Clone, PartialEq)]
struct PadProps {
    ord: usize,
    day: u32,
    /// Position in the power-on sweep.
    stagger: usize,
    lit: bool,
    is_today: bool,
    future: bool,
    focused: bool,
    /// `Some(duration)` while this pad is the one being held.
    hold_ms: Option<u32>,
    /// The press has crossed into "keep holding to clear the year".
    arming: bool,
    /// Show this day's weekday instead of its number, while the pad is held.
    peeking: bool,
    weekday_short: &'static str,
    /// Draw the trace running down to the next day in the month.
    trace: bool,
    label: String,
    view: View,
}

fn Pad(props: PadProps) -> Element {
    let ctx = use_ctx();
    let mut focus = ctx.focus;
    let ord = props.ord;
    let view = props.view;
    let held = props.hold_ms.is_some();
    let hold_ms = props.hold_ms.unwrap_or(super::HOLD_LIGHT_MS);
    let lit_word = if props.lit { "Lit" } else { "Not lit" };
    // Holding a pad names its day. Two characters, exactly as wide as the
    // two-digit numbers the pad already prints.
    let glyph = if props.peeking {
        props.weekday_short.to_string()
    } else {
        props.day.to_string()
    };

    rsx! {
        button {
            id: "pad-{ord}",
            r#type: "button",
            class: "pad",
            class: if props.lit { "is-lit" },
            class: if props.is_today { "is-today" },
            class: if props.future { "is-future" },
            class: if held { "is-holding" },
            class: if props.arming { "is-arming" },
            class: if props.trace { "has-trace" },
            style: "--i: {props.stagger}; --hold-ms: {hold_ms}ms;",
            tabindex: if props.focused { 0 } else { -1 },
            aria_pressed: props.lit,
            aria_label: "{props.label}. {lit_word}.",

            onpointerdown: move |event| {
                event.prevent_default();
                focus.set(ord);
                press(ctx, ord);
            },
            onpointerup: move |_| release(ctx),
            onpointerleave: move |_| release(ctx),
            onpointercancel: move |_| release(ctx),
            onpointerenter: move |event| {
                if !event.held_buttons().is_empty() {
                    paint(ctx, ord);
                }
            },
            oncontextmenu: move |event| event.prevent_default(),
            onblur: move |_| release(ctx),
            onfocus: move |_| focus.set(ord),
            onkeydown: move |event| {
                let key = event.key();
                if is_activate(&key) {
                    event.prevent_default();
                    if !event.is_auto_repeating() {
                        press(ctx, ord);
                    }
                } else if navigate(ctx, view, ord, &key) {
                    event.prevent_default();
                }
            },
            onkeyup: move |event| {
                if is_activate(&event.key()) {
                    release(ctx);
                }
            },

            span { class: "glow", aria_hidden: "true" }
            span { class: "cap",
                span { class: "lamp", aria_hidden: "true" }
                span { class: "fill", aria_hidden: "true" }
                span { class: "num", "{glyph}" }
            }
            if props.arming {
                span { class: "arm-label", "Keep holding to clear the year" }
            }
        }
    }
}

fn is_activate(key: &Key) -> bool {
    match key {
        Key::Enter => true,
        Key::Character(c) => c.as_str() == " ",
        _ => false,
    }
}

/// Arrow-key movement across the board. Returns true when the key was consumed.
fn navigate(mut ctx: Ctx, view: View, ord: usize, key: &Key) -> bool {
    let year = *ctx.year.peek();
    let (month, day) = date::from_ordinal(year, ord);
    let length = date::days_in_month(year, month);

    let (day_delta, month_delta, absolute): (i32, i32, Option<u32>) = match (view, key) {
        (_, Key::Home) => (0, 0, Some(1)),
        (_, Key::End) => (0, 0, Some(length)),
        (_, Key::PageUp) => (0, -1, None),
        (_, Key::PageDown) => (0, 1, None),
        (View::Year, Key::ArrowUp) => (-1, 0, None),
        (View::Year, Key::ArrowDown) => (1, 0, None),
        (View::Year, Key::ArrowLeft) => (0, -1, None),
        (View::Year, Key::ArrowRight) => (0, 1, None),
        (View::Month, Key::ArrowLeft) => (-1, 0, None),
        (View::Month, Key::ArrowRight) => (1, 0, None),
        (View::Month, Key::ArrowUp) => (-7, 0, None),
        (View::Month, Key::ArrowDown) => (7, 0, None),
        _ => return false,
    };

    if month_delta != 0 {
        let (next_year, next_month) = match month as i32 + month_delta {
            m if m < 0 => (year - 1, 11),
            m if m > 11 => (year + 1, 0),
            m => (year, m as u32),
        };
        let next_day = day.min(date::days_in_month(next_year, next_month));
        ctx.year.set(next_year);
        ctx.month.set(next_month);
        move_focus(ctx, date::ordinal(next_year, next_month, next_day));
        return true;
    }

    if let Some(day) = absolute {
        move_focus(ctx, date::ordinal(year, month, day));
        return true;
    }

    // Day movement spills into neighbouring months, and past the turn of the
    // year.
    let mut cursor = Date { year, ordinal: ord };
    for _ in 0..day_delta.abs() {
        cursor = if day_delta < 0 {
            cursor.prev()
        } else {
            cursor.next()
        };
    }
    if cursor.year != year {
        ctx.year.set(cursor.year);
    }
    ctx.month.set(cursor.month_day().0);
    move_focus(ctx, cursor.ordinal);
    true
}

fn move_focus(mut ctx: Ctx, ord: usize) {
    ctx.focus.set(ord);
    // The target pad may not exist until the grid has re-rendered.
    spawn(async move {
        TimeoutFuture::new(0).await;
        platform::focus(&format!("pad-{ord}"));
    });
}
