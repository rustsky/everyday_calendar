//! The app shell: shared context, the interaction rituals, and the root layout.

mod about;
mod board;
mod console;

use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use wasm_bindgen::JsCast;

use crate::audio::{self, Tone};
use crate::date::{self, Date};
use crate::stats::{self, Stats};
use crate::store::{self, AppState, YearBits};

/// How long a deliberate press has to last before a day changes state.
pub const HOLD_LIGHT_MS: u32 = 550;
/// Turning a day back off is harder than turning it on, on purpose.
pub const HOLD_DIM_MS: u32 = 900;
/// The hardware clears itself when January 1 is held for ten seconds.
pub const HOLD_RESET_MS: u32 = 10_000;
/// Length of the power-on light sweep.
const BOOT_MS: u32 = 1_700;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Hold {
    pub ord: usize,
    pub ms: u32,
    /// True once the press has crossed into "keep holding to clear the year".
    pub arming_reset: bool,
}

pub struct Toast {
    pub id: u64,
    pub text: String,
    /// A year snapshot that the toast's Undo button can restore.
    pub undo: Option<(usize, i32, YearBits)>,
}

pub struct Burst {
    pub id: u64,
    pub label: String,
}

#[derive(Clone, Copy)]
pub struct Ctx {
    pub state: Signal<AppState>,
    pub today: Signal<Date>,
    pub year: Signal<i32>,
    pub month: Signal<u32>,
    pub focus: Signal<usize>,
    pub flipped: Signal<bool>,
    pub booting: Signal<bool>,
    pub settings_open: Signal<bool>,
    pub hold: Signal<Option<Hold>>,
    pub hold_gen: Signal<u64>,
    pub toast: Signal<Option<Toast>>,
    pub burst: Signal<Option<Burst>>,
    pub announce: Signal<String>,
    counter: Signal<u64>,
}

impl Ctx {
    fn next_id(&mut self) -> u64 {
        let id = *self.counter.peek() + 1;
        self.counter.set(id);
        id
    }

    pub fn stats(&self) -> Stats {
        let state = self.state.read();
        stats::compute(
            state.active_goal(),
            *self.today.read(),
            *self.year.read(),
            *self.month.read(),
        )
    }

    pub fn is_lit(&self, ord: usize) -> bool {
        self.state.read().active_goal().year(*self.year.read()).get(ord)
    }
}

pub fn use_ctx() -> Ctx {
    use_context::<Ctx>()
}

fn prefers_reduced_motion() -> bool {
    web_sys::window()
        .and_then(|w| w.match_media("(prefers-reduced-motion: reduce)").ok().flatten())
        .map(|m| m.matches())
        .unwrap_or(false)
}

/// Moves keyboard focus to a pad without disturbing anything else.
pub fn focus_pad(ord: usize) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    if let Some(element) = document.get_element_by_id(&format!("pad-{ord}")) {
        if let Ok(element) = element.dyn_into::<web_sys::HtmlElement>() {
            let _ = element.focus();
        }
    }
}

pub fn announce(mut ctx: Ctx, text: impl Into<String>) {
    ctx.announce.set(text.into());
}

pub fn show_toast(mut ctx: Ctx, text: impl Into<String>, undo: Option<(usize, i32, YearBits)>) {
    let id = ctx.next_id();
    ctx.toast.set(Some(Toast {
        id,
        text: text.into(),
        undo,
    }));
    spawn(async move {
        TimeoutFuture::new(7_000).await;
        let still_showing = ctx.toast.peek().as_ref().is_some_and(|t| t.id == id);
        if still_showing {
            ctx.toast.set(None);
        }
    });
}

fn celebrate(mut ctx: Ctx, streak: u32) {
    let id = ctx.next_id();
    let label = match streak {
        365 => "A full year.".to_string(),
        n => format!("{n} days in a row."),
    };
    ctx.burst.set(Some(Burst { id, label }));
    if ctx.state.peek().sound {
        audio::play(Tone::Milestone);
    }
    spawn(async move {
        TimeoutFuture::new(2_600).await;
        let still_showing = ctx.burst.peek().as_ref().is_some_and(|b| b.id == id);
        if still_showing {
            ctx.burst.set(None);
        }
    });
}

/// Flips a single day and reports what happened.
pub fn toggle_day(mut ctx: Ctx, ord: usize) {
    let year = *ctx.year.peek();
    let lit = {
        let mut state = ctx.state.write();
        state.active_goal_mut().toggle(year, ord)
    };

    let after = ctx.stats();
    let date = Date { year, ordinal: ord };
    let verb = if lit { "lit" } else { "cleared" };
    announce(
        ctx,
        format!(
            "{} {}. Current streak {} {}.",
            date.long_label(),
            verb,
            after.current,
            if after.current == 1 { "day" } else { "days" }
        ),
    );

    if ctx.state.peek().sound {
        audio::play(if lit { Tone::Light } else { Tone::Dim });
    }
    if lit {
        if let Some(milestone) = stats::milestone_for(after.current) {
            celebrate(ctx, milestone);
        }
    }
}

/// Clears the year on the board, keeping a snapshot for Undo.
pub fn reset_year(ctx: Ctx) {
    let year = *ctx.year.peek();
    let current = ctx.state.peek().active_goal().year(year);
    reset_year_to(ctx, current);
}

/// `restore` is what Undo puts back. For the January 1 hold that is the state
/// from before the press lit the pad, not after — the reset gesture shouldn't
/// leave its own fingerprint behind.
fn reset_year_to(mut ctx: Ctx, restore: YearBits) {
    let year = *ctx.year.peek();
    let (index, current) = {
        let state = ctx.state.peek();
        (state.active, state.active_goal().year(year))
    };
    if current.is_empty() && restore.is_empty() {
        return;
    }
    {
        let mut state = ctx.state.write();
        state.active_goal_mut().set_year(year, YearBits::default());
    }
    announce(ctx, format!("{year} cleared."));
    show_toast(ctx, format!("{year} cleared."), Some((index, year, restore)));
}

pub fn undo_reset(mut ctx: Ctx, index: usize, year: i32, bits: YearBits) {
    {
        let mut state = ctx.state.write();
        if let Some(goal) = state.goals.get_mut(index) {
            goal.set_year(year, bits);
        }
    }
    ctx.toast.set(None);
    announce(ctx, format!("{year} restored."));
}

/// Begins a press. In ritual mode the day only changes once the press has been
/// held long enough; otherwise it flips immediately, like the original app.
pub fn press(mut ctx: Ctx, ord: usize) {
    if !ctx.state.peek().ritual {
        toggle_day(ctx, ord);
        return;
    }

    let generation = *ctx.hold_gen.peek() + 1;
    ctx.hold_gen.set(generation);

    // Snapshotted before the press changes anything, so a January 1 reset can
    // undo the whole gesture rather than just the clear.
    let before_gesture = ctx.state.peek().active_goal().year(*ctx.year.peek());

    let ms = if ctx.is_lit(ord) {
        HOLD_DIM_MS
    } else {
        HOLD_LIGHT_MS
    };
    ctx.hold.set(Some(Hold {
        ord,
        ms,
        arming_reset: false,
    }));

    spawn(async move {
        TimeoutFuture::new(ms).await;
        if *ctx.hold_gen.peek() != generation {
            return;
        }
        toggle_day(ctx, ord);

        // January 1 doubles as the hardware's reset pad: keep holding it and
        // the whole year goes dark.
        if ord == 0 {
            ctx.hold.set(Some(Hold {
                ord,
                ms: HOLD_RESET_MS,
                arming_reset: true,
            }));
            TimeoutFuture::new(HOLD_RESET_MS).await;
            if *ctx.hold_gen.peek() != generation {
                return;
            }
            reset_year_to(ctx, before_gesture);
        }
        ctx.hold.set(None);
    });
}

/// Ends a press, cancelling anything still pending.
pub fn release(mut ctx: Ctx) {
    let generation = *ctx.hold_gen.peek() + 1;
    ctx.hold_gen.set(generation);
    if ctx.hold.peek().is_some() {
        ctx.hold.set(None);
    }
}

/// Drag-to-paint, available only when the ritual is switched off — this is how
/// the original app behaved.
pub fn paint(ctx: Ctx, ord: usize) {
    if ctx.state.peek().ritual {
        return;
    }
    toggle_day(ctx, ord);
}

pub fn go_to_year(mut ctx: Ctx, year: i32) {
    ctx.year.set(year);
    let today = *ctx.today.peek();
    let ord = if today.year == year { today.ordinal } else { 0 };
    ctx.focus.set(ord);
    let (month, _) = date::from_ordinal(year, ord);
    ctx.month.set(month);
}

pub fn go_to_month(mut ctx: Ctx, delta: i32) {
    let year = *ctx.year.peek();
    let raw = *ctx.month.peek() as i32 + delta;
    let (year, month) = match raw {
        m if m < 0 => (year - 1, 11),
        m if m > 11 => (year + 1, 0),
        m => (year, m as u32),
    };
    ctx.year.set(year);
    ctx.month.set(month);

    // Land on today when stepping into the current month, otherwise the 1st.
    let today = *ctx.today.peek();
    let ord = if today.year == year && today.month_day().0 == month {
        today.ordinal
    } else {
        date::ordinal(year, month, 1)
    };
    ctx.focus.set(ord);
}

pub fn App() -> Element {
    let ctx = use_hook(|| {
        let state = store::load();
        let today = date::today();
        let reduced_motion = prefers_reduced_motion();
        let boot = state.boot_sequence && !reduced_motion;
        Ctx {
            year: Signal::new(today.year),
            month: Signal::new(today.month_day().0),
            focus: Signal::new(today.ordinal),
            state: Signal::new(state),
            today: Signal::new(today),
            flipped: Signal::new(false),
            booting: Signal::new(boot),
            settings_open: Signal::new(false),
            hold: Signal::new(None),
            hold_gen: Signal::new(0),
            toast: Signal::new(None),
            burst: Signal::new(None),
            announce: Signal::new(String::new()),
            counter: Signal::new(0),
        }
    });
    use_context_provider(|| ctx);

    // Every change is written straight back to localStorage; there is nowhere
    // else for it to go.
    use_effect(move || {
        let state = ctx.state.read();
        store::save(&state);
    });

    // The power-on light sweep runs exactly once per load, then gets out of
    // the way.
    use_hook(|| {
        if *ctx.booting.peek() {
            let mut booting = ctx.booting;
            spawn(async move {
                TimeoutFuture::new(BOOT_MS).await;
                booting.set(false);
            });
        }
    });

    let state = ctx.state.read();
    let theme = state.theme.slug();
    let accent = state.active_goal().accent.slug();
    let view = state.view;
    let brightness = state.brightness;
    let ritual = state.ritual;
    drop(state);

    let booting = *ctx.booting.read();
    let flipped = *ctx.flipped.read();

    rsx! {
        div {
            class: "page theme-{theme} accent-{accent}",
            class: if booting { "booting" },
            class: if ritual { "ritual" },
            style: "--brightness: {brightness};",

            Masthead {}
            console::GoalBar {}

            main { class: "stage",
                div {
                    class: "device",
                    class: if flipped { "flipped" },
                    section {
                        class: "face front",
                        aria_hidden: flipped,
                        div { class: "frame",
                            Bolts {}
                            board::Board { view }
                        }
                    }
                    section {
                        class: "face back",
                        aria_hidden: !flipped,
                        div { class: "frame",
                            Bolts {}
                            about::About {}
                        }
                    }
                }
            }

            console::Readout {}
            console::Console {}
            if *ctx.settings_open.read() {
                console::Settings {}
            }
            Fineprint {}
            ToastBar {}
            Celebration {}

            div {
                class: "sr-only",
                role: "status",
                aria_live: "polite",
                "{ctx.announce}"
            }
        }
    }
}

#[component]
fn Masthead() -> Element {
    rsx! {
        header { class: "masthead",
            h1 { "The Every Day Calendar" }
            p { "One goal. One day at a time. Don't break the chain." }
        }
    }
}

/// The eight frame screws from the original board.
#[component]
fn Bolts() -> Element {
    rsx! {
        div { class: "bolts", aria_hidden: "true",
            for i in 0..8 {
                span { key: "{i}", class: "bolt bolt-{i}" }
            }
        }
    }
}

#[component]
fn ToastBar() -> Element {
    let mut ctx = use_ctx();
    let toast = ctx.toast.read();
    let Some(toast) = toast.as_ref() else {
        return rsx! {};
    };
    let text = toast.text.clone();
    let undo = toast.undo;

    rsx! {
        div { class: "toast", role: "status",
            span { "{text}" }
            if let Some((index, year, bits)) = undo {
                button {
                    class: "link",
                    onclick: move |_| undo_reset(ctx, index, year, bits),
                    "Undo"
                }
            }
            button {
                class: "toast-close",
                aria_label: "Dismiss",
                onclick: move |_| ctx.toast.set(None),
                "×"
            }
        }
    }
}

#[component]
fn Celebration() -> Element {
    let ctx = use_ctx();
    let burst = ctx.burst.read();
    let Some(burst) = burst.as_ref() else {
        return rsx! {};
    };

    rsx! {
        div { class: "celebration", key: "{burst.id}", aria_hidden: "true",
            div { class: "sparks",
                for i in 0..14 {
                    span { key: "{i}", class: "spark", style: "--n: {i};" }
                }
            }
            p { class: "celebration-label", "{burst.label}" }
        }
    }
}

#[component]
fn Fineprint() -> Element {
    rsx! {
        footer { class: "fineprint",
            p {
                "Everything you tap is stored in this browser and nowhere else. "
                "0% internet-connected, same as the real thing."
            }
            p {
                "After "
                a { href: "https://www.kickstarter.com/projects/simonegiertz/the-every-day-calendar",
                    "Simone Giertz's Every Day Calendar"
                }
                ". Not affiliated with Simone Giertz or Yetch."
            }
        }
    }
}
