//! The app shell: shared context, the interaction rituals, and the root layout.

mod about;
mod board;
mod console;

use dioxus::core::Task;
use dioxus::prelude::*;
use edc_core::model::{Doc, GoalId, Stamp};
use edc_core::prefs::Prefs;
use edc_core::stats::{self, Stats};
use edc_core::{Date, YearBits, date};

use crate::audio::{self, Tone};
use crate::sync::{self, Status};
use crate::{platform, storage};

/// How long a deliberate press has to last before a day changes state.
pub const HOLD_LIGHT_MS: u32 = 550;
/// Turning a day back off is harder than turning it on, on purpose.
pub const HOLD_DIM_MS: u32 = 900;
/// The hardware clears itself when January 1 is held for ten seconds.
pub const HOLD_RESET_MS: u32 = 10_000;
/// How long the weekday label stays up after the press ends, so a quick press
/// reads as a reveal rather than a flash.
pub const PEEK_LINGER_MS: u32 = 150;
/// Length of the power-on light sweep.
const BOOT_MS: u32 = 1_700;
/// How often the sync loop wakes up to look for work.
const SYNC_TICK_MS: u32 = 1_000;
/// How long to go without talking to the server before checking in anyway, so
/// changes made on another device turn up on their own.
const SYNC_POLL_MS: u64 = 20_000;

/// The pad currently naming its weekday. Outlives the hold by [`PEEK_LINGER_MS`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Peek {
    pub ord: usize,
    id: u64,
}

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
    pub undo: Option<(GoalId, i32, YearBits)>,
}

pub struct Burst {
    pub id: u64,
    pub label: String,
}

#[derive(Clone, Copy)]
pub struct Ctx {
    pub doc: Signal<Doc>,
    pub prefs: Signal<Prefs>,
    /// This browser's id. Stamped on every write, and the merge tie-break.
    pub device: Signal<String>,
    pub today: Signal<Date>,
    pub year: Signal<i32>,
    pub month: Signal<u32>,
    pub focus: Signal<usize>,
    pub flipped: Signal<bool>,
    pub booting: Signal<bool>,
    pub hold: Signal<Option<Hold>>,
    pub hold_gen: Signal<u64>,
    /// Which pad is showing its weekday, if any.
    pub peek: Signal<Option<Peek>>,
    pub toast: Signal<Option<Toast>>,
    pub burst: Signal<Option<Burst>>,
    pub announce: Signal<String>,
    pub sync: Signal<Status>,
    /// Set by every local edit, cleared once the server has it.
    pub dirty: Signal<bool>,
    pub last_sync: Signal<u64>,
    counter: Signal<u64>,
}

impl Ctx {
    fn next_id(&mut self) -> u64 {
        let id = *self.counter.peek() + 1;
        self.counter.set(id);
        id
    }

    pub fn stamp(&self) -> Stamp {
        Stamp::new(platform::now_ms(), self.device.peek().clone())
    }

    /// The goal this device is looking at, falling back to the first one when
    /// the remembered goal has been deleted on another device.
    pub fn goal_id(&self) -> GoalId {
        let doc = self.doc.read();
        let ids = doc.goal_ids();
        self.prefs
            .read()
            .active
            .clone()
            .filter(|id| ids.contains(id))
            .or_else(|| ids.first().cloned())
            .unwrap_or_default()
    }

    pub fn stats(&self) -> Stats {
        stats::compute(
            &self.doc.read(),
            &self.goal_id(),
            *self.today.read(),
            *self.year.read(),
            *self.month.read(),
        )
    }

    pub fn is_lit(&self, ord: usize) -> bool {
        self.doc.read().is_lit(
            &self.goal_id(),
            Date {
                year: *self.year.read(),
                ordinal: ord,
            },
        )
    }
}

pub fn use_ctx() -> Ctx {
    use_context::<Ctx>()
}

/// Records that there is something for the sync loop to send.
fn touch(mut ctx: Ctx) {
    ctx.dirty.set(true);
}

/// Starts naming the held day.
fn begin_peek(mut ctx: Ctx, ord: usize) {
    let id = ctx.next_id();
    ctx.peek.set(Some(Peek { ord, id }));
}

/// Stops naming it, after a short linger so the label does not flash.
fn end_peek(mut ctx: Ctx) {
    let Some(current) = *ctx.peek.peek() else {
        return;
    };
    spawn(async move {
        platform::sleep(PEEK_LINGER_MS).await;
        // Another press may have claimed the label in the meantime.
        if ctx.peek.peek().is_some_and(|p| p.id == current.id) {
            ctx.peek.set(None);
        }
    });
}

pub fn announce(mut ctx: Ctx, text: impl Into<String>) {
    ctx.announce.set(text.into());
}

pub fn show_toast(mut ctx: Ctx, text: impl Into<String>, undo: Option<(GoalId, i32, YearBits)>) {
    let id = ctx.next_id();
    ctx.toast.set(Some(Toast {
        id,
        text: text.into(),
        undo,
    }));
    spawn(async move {
        platform::sleep(7_000).await;
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
    if ctx.prefs.peek().sound {
        audio::play(Tone::Milestone);
    }
    spawn(async move {
        platform::sleep(2_600).await;
        let still_showing = ctx.burst.peek().as_ref().is_some_and(|b| b.id == id);
        if still_showing {
            ctx.burst.set(None);
        }
    });
}

/// Flips a single day and reports what happened.
pub fn toggle_day(ctx: Ctx, ord: usize) {
    let year = *ctx.year.peek();
    let goal = ctx.goal_id();
    let lit = !ctx.is_lit(ord);
    let stamp = ctx.stamp();
    {
        let mut doc = ctx.doc;
        doc.write().set_day(&goal, year, ord, lit, stamp);
    }
    touch(ctx);

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

    if ctx.prefs.peek().sound {
        audio::play(if lit { Tone::Light } else { Tone::Dim });
    }
    if lit && let Some(milestone) = stats::milestone_for(after.current) {
        celebrate(ctx, milestone);
    }
}

/// Clears the year on the board, keeping a snapshot for Undo.
pub fn reset_year(ctx: Ctx) {
    let year = *ctx.year.peek();
    let goal = ctx.goal_id();
    let current = ctx.doc.peek().year_bits(&goal, year);
    reset_year_to(ctx, current);
}

/// `restore` is what Undo puts back. For the January 1 hold that is the state
/// from before the press lit the pad, not after — the reset gesture shouldn't
/// leave its own fingerprint behind.
fn reset_year_to(ctx: Ctx, restore: YearBits) {
    let year = *ctx.year.peek();
    let goal = ctx.goal_id();
    let current = ctx.doc.peek().year_bits(&goal, year);
    if current.is_empty() && restore.is_empty() {
        return;
    }

    let stamp = ctx.stamp();
    {
        let mut doc = ctx.doc;
        doc.write().clear_year(&goal, year, stamp);
    }
    touch(ctx);

    announce(ctx, format!("{year} cleared."));
    show_toast(ctx, format!("{year} cleared."), Some((goal, year, restore)));
}

pub fn undo_reset(mut ctx: Ctx, goal: GoalId, year: i32, bits: YearBits) {
    let stamp = ctx.stamp();
    {
        let mut doc = ctx.doc;
        doc.write().restore_year(&goal, year, bits, stamp);
    }
    touch(ctx);
    ctx.toast.set(None);
    announce(ctx, format!("{year} restored."));
}

/// Begins a press. In ritual mode the day only changes once the press has been
/// held long enough; otherwise it flips immediately, like the original app.
pub fn press(mut ctx: Ctx, ord: usize) {
    if !ctx.prefs.peek().ritual {
        toggle_day(ctx, ord);
        return;
    }

    let generation = *ctx.hold_gen.peek() + 1;
    ctx.hold_gen.set(generation);

    // Snapshotted before the press changes anything, so a January 1 reset can
    // undo the whole gesture rather than just the clear.
    let before_gesture = ctx.doc.peek().year_bits(&ctx.goal_id(), *ctx.year.peek());

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
    begin_peek(ctx, ord);

    spawn(async move {
        platform::sleep(ms).await;
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
            platform::sleep(HOLD_RESET_MS).await;
            if *ctx.hold_gen.peek() != generation {
                return;
            }
            reset_year_to(ctx, before_gesture);
        }
        ctx.hold.set(None);
        end_peek(ctx);
    });
}

/// Ends a press, cancelling anything still pending.
pub fn release(mut ctx: Ctx) {
    let generation = *ctx.hold_gen.peek() + 1;
    ctx.hold_gen.set(generation);
    if ctx.hold.peek().is_some() {
        ctx.hold.set(None);
    }
    end_peek(ctx);
}

/// Drag-to-paint, available only when the ritual is switched off — this is how
/// the original app behaved.
pub fn paint(ctx: Ctx, ord: usize) {
    if ctx.prefs.peek().ritual {
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

/// One exchange with the server: send everything, adopt what comes back.
async fn exchange(mut ctx: Ctx, endpoint: &str) {
    ctx.sync.set(Status::Syncing);
    ctx.dirty.set(false);
    let snapshot = ctx.doc.peek().clone();

    match sync::exchange(endpoint, &snapshot).await {
        Ok(merged) => {
            let mut next = ctx.doc.peek().clone();
            next.merge(&merged);
            // Only touch the signal when something actually changed, so a quiet
            // poll doesn't re-render the whole board every twenty seconds.
            if next != *ctx.doc.peek() {
                ctx.doc.set(next);
            }
            let now = platform::now_ms();
            ctx.last_sync.set(now);
            ctx.sync.set(Status::Synced { at: now });
        }
        Err(message) => {
            ctx.sync.set(Status::Failed { message });
            // Keep the change queued so the next tick tries again.
            ctx.dirty.set(true);
        }
    }
}

/// Gives a genuinely empty calendar something to light. Runs only after sync
/// has had its chance to supply the goals, so devices don't each mint one.
fn ensure_a_goal(ctx: Ctx) {
    if !ctx.doc.peek().goal_ids().is_empty() {
        return;
    }
    let (id, record) = storage::starter_goal(&ctx.device.peek());
    let mut doc = ctx.doc;
    doc.write().put_goal(&id, record);
    touch(ctx);
}

pub fn sync_now(ctx: Ctx) {
    if ctx.sync.peek().is_local() {
        return;
    }
    let Some(endpoint) = sync::endpoint(&ctx.prefs.peek()) else {
        return;
    };
    spawn(async move { exchange(ctx, &endpoint).await });
}

/// Finds the server, then keeps the document in step with it.
async fn sync_loop(mut ctx: Ctx, endpoint: Option<String>) {
    let Some(endpoint) = endpoint else {
        if !ctx.sync.peek().is_local() {
            ctx.sync.set(Status::Local);
        }
        ensure_a_goal(ctx);
        return;
    };

    while !sync::available(&endpoint).await {
        if !cfg!(feature = "desktop") {
            // Nothing behind this origin, so the page stays local for good.
            ensure_a_goal(ctx);
            return;
        }
        // The desktop app was pointed at this server on purpose, so it keeps
        // knocking until the server turns up or the address changes.
        ctx.sync.set(Status::Failed {
            message: format!("no answer from {endpoint}"),
        });
        ensure_a_goal(ctx);
        platform::sleep(SYNC_POLL_MS as u32).await;
    }

    // The first exchange runs before any starter goal is minted, so a new
    // device adopts the goals already on the server instead of adding a
    // duplicate of its own.
    exchange(ctx, &endpoint).await;
    ensure_a_goal(ctx);
    loop {
        platform::sleep(SYNC_TICK_MS).await;
        let stale = platform::now_ms().saturating_sub(*ctx.last_sync.peek()) > SYNC_POLL_MS;
        if (*ctx.dirty.peek() || stale) && platform::is_visible() {
            exchange(ctx, &endpoint).await;
        }
    }
}

pub fn App() -> Element {
    let ctx = use_hook(|| {
        let device = storage::device_id();
        let doc = storage::load_doc(&device);
        let prefs = storage::load_prefs();
        let today = platform::today();
        let boot = prefs.boot_sequence && !platform::prefers_reduced_motion();

        Ctx {
            year: Signal::new(today.year),
            month: Signal::new(today.month_day().0),
            focus: Signal::new(today.ordinal),
            doc: Signal::new(doc),
            prefs: Signal::new(prefs),
            device: Signal::new(device),
            today: Signal::new(today),
            flipped: Signal::new(false),
            booting: Signal::new(boot),
            hold: Signal::new(None),
            hold_gen: Signal::new(0),
            peek: Signal::new(None),
            toast: Signal::new(None),
            burst: Signal::new(None),
            announce: Signal::new(String::new()),
            sync: Signal::new(Status::Local),
            dirty: Signal::new(false),
            last_sync: Signal::new(0),
            counter: Signal::new(0),
        }
    });
    use_context_provider(|| ctx);
    platform::use_fit_window();
    platform::use_tray();

    // The browser's copy is written on every change. It is the source of truth
    // for rendering, with or without a server.
    use_effect(move || storage::save_doc(&ctx.doc.read()));
    use_effect(move || storage::save_prefs(&ctx.prefs.read()));

    // The power-on light sweep runs exactly once per load, then gets out of
    // the way.
    use_hook(|| {
        if *ctx.booting.peek() {
            let mut booting = ctx.booting;
            spawn(async move {
                platform::sleep(BOOT_MS).await;
                booting.set(false);
            });
        }
    });

    // Sync, if a server answers. The web client's endpoint never changes, so
    // this runs once; the desktop app starts over when its address is edited.
    let endpoint = use_memo(move || sync::endpoint(&ctx.prefs.read()));
    let mut sync_task = use_signal(|| None::<Task>);
    use_effect(move || {
        let endpoint = endpoint();
        if let Some(previous) = sync_task.write().take() {
            previous.cancel();
        }
        sync_task.set(Some(spawn(sync_loop(ctx, endpoint))));
    });

    let prefs = ctx.prefs.read();
    let theme = prefs.theme.slug();
    let view = prefs.view;
    let brightness = prefs.brightness;
    let ritual = prefs.ritual;
    drop(prefs);

    let goal_id = ctx.goal_id();
    let accent = ctx
        .doc
        .read()
        .goal(&goal_id)
        .map(|record| record.accent.slug())
        .unwrap_or("gold");

    let booting = *ctx.booting.read();
    let flipped = *ctx.flipped.read();

    rsx! {
        div {
            class: "page theme-{theme} accent-{accent}",
            class: if cfg!(feature = "desktop") { "desktop" },
            class: if booting { "booting" },
            class: if ritual { "ritual" },
            style: "--brightness: {brightness};",

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
                            about::About { view }
                        }
                    }
                }
            }

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
    let undo = toast.undo.clone();

    rsx! {
        div { class: "toast", role: "status",
            span { "{text}" }
            if let Some((goal, year, bits)) = undo {
                button {
                    class: "link",
                    onclick: move |_| undo_reset(ctx, goal.clone(), year, bits),
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
