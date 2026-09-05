# The Every Day Calendar

A web recreation of [Simone Giertz's Every Day Calendar][kickstarter], rebuilt
in Rust with [Dioxus][dioxus].

Set one goal. Light one day at a time. Don't break the chain.

![The 2026 board: twelve columns of gold hexagons on a dark circuit board in a
bamboo frame, most of the year lit, a 24-day streak running up to today, with
streak totals and a brightness dimmer
below.](docs/screenshot-year.png)

This is a rewrite of [zmxv/everydaycalendar][original], the original HTML5
edition, with the parts of the hardware — and of the Kickstarter pitch — that
the web version never had.

## What's here that wasn't before

The original web app was a faithful little thing: a 12×31 grid of gold
hexagons, click to toggle, saved to `localStorage`. It left out most of what
makes the object work.

| | Original web app | This |
| --- | --- | --- |
| **The goal** | Never asked | Named, editable, printed on the board's silkscreen |
| **Press and hold** | Instant click toggle | A day only changes after a deliberate hold, and clearing one takes longer than lighting it |
| **Reset** | Click 365 pads | Hold January 1 for ten seconds, exactly like the hardware — with Undo |
| **Brightness** | — | A dimmer, like the knob on the back of the real board |
| **Power-on sequence** | — | The light sweep the hardware runs at boot |
| **Streaks** | — | Current streak, longest streak, trailing year, all-time, and a milestone bar |
| **Today** | Indistinguishable | Ringed and pulsing, and the streak warns you while today is still dark |
| **More than one habit** | Explicitly punted to another app | Up to eight goals, each with its own glow |
| **Phones** | Explicitly punted to another app | Responsive, plus a large-pad month view |
| **Keyboard** | — | Full arrow-key navigation, hold on Space or Enter, live region for screen readers |
| **Your data** | `localStorage`, plus Google Analytics | `localStorage`, plus JSON export/import, and no analytics at all |

On a narrow screen the board switches to a month of large pads, and there's a
dark theme:

<img src="docs/screenshot-month.png" width="390"
     alt="The same calendar on a phone-width screen in the dark theme, showing
     September 2026 as a week grid of large hexagons with the first six days
     lit.">

Two things from the original are kept on purpose:

- **Quick-tap mode.** Settings → *Press and hold to change a day* → off restores
  the original instant-toggle behaviour, drag-to-paint included.
- **The save format.** Each year is still packed into the same 61-character,
  six-bits-per-character string. Saves from `everydaycalendar.app` in this
  browser are picked up automatically the first time you load this app.

## Running it

```sh
rustup target add wasm32-unknown-unknown
cargo install dioxus-cli --version 0.7 --locked

dx serve --platform web        # http://127.0.0.1:8080
dx build --platform web --release
```

The release build lands in `target/dx/everydaycalendar/release/web/public` and
is a plain static directory — copy it anywhere that serves files.

```sh
cargo test                     # date maths, the save codec, streak arithmetic
```

## How it's put together

```
src/
  main.rs      launch, and the stylesheet asset
  date.rs      proleptic-Gregorian date maths, no date crate
  store.rs     goals, the 366-bit year encoding, localStorage, legacy migration
  stats.rs     streaks, totals, milestones
  audio.rs     the optional chime, synthesised from oscillators
  ui/
    mod.rs     shared context, the hold/reset/undo rituals, root layout
    board.rs   the PCB, the pads, keyboard navigation
    console.rs goals, readout, dimmer, settings, export/import
    about.rs   the back of the board
assets/main.css
```

State lives in one `Signal<AppState>` provided through context; every write is
mirrored to `localStorage` by a single effect. Pads take their state as props
so a change re-renders only the pads it touched.

The board is CSS, not images: hexagons are `clip-path` polygons, the bamboo
frame is layered gradients, and pad sizing is driven by container query units
so the whole board scales from a phone to a desktop without a media query.

## Privacy

Nothing leaves your browser. No account, no sync, no analytics, no network
requests after the page loads — the physical calendar is proudly 0%
internet-connected, and so is this. Use **Export backup** in Settings if you
want a copy you control.

## Credit

The Every Day Calendar was designed by [Simone Giertz][yetch] and funded on
[Kickstarter][kickstarter] in 2018. This is an unaffiliated tribute, and it
stands on [Zhen Wang's original web edition][original].

MIT licensed, same as the original.

[kickstarter]: https://www.kickstarter.com/projects/simonegiertz/the-every-day-calendar
[original]: https://github.com/zmxv/everydaycalendar
[dioxus]: https://dioxuslabs.com/
[yetch]: https://yetch.studio/
