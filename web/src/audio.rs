//! A tiny synthesised chime for the moment a day lights up.
//!
//! The hardware is silent; this is opt-in and off by default. Tones are built
//! from oscillators so the bundle carries no audio files.

use wasm_bindgen::JsValue;
use web_sys::{AudioContext, GainNode, OscillatorType};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// A day was lit.
    Light,
    /// A day was turned back off.
    Dim,
    /// A streak milestone was reached.
    Milestone,
}

/// Plays `tone`. Any failure (no audio device, autoplay policy, unsupported
/// browser) is silently ignored — sound is decoration, never a dependency.
pub fn play(tone: Tone) {
    let _ = try_play(tone);
}

fn try_play(tone: Tone) -> Result<(), JsValue> {
    let ctx = AudioContext::new()?;
    let now = ctx.current_time();

    let notes: &[(f32, f64)] = match tone {
        // A rising perfect fifth: the "locked in" sound.
        Tone::Light => &[(784.0, 0.0), (1174.7, 0.07)],
        // A short fall, so undoing sounds like undoing.
        Tone::Dim => &[(392.0, 0.0)],
        // A little major arpeggio for milestones.
        Tone::Milestone => &[(523.3, 0.0), (659.3, 0.09), (784.0, 0.18), (1046.5, 0.27)],
    };

    let master: GainNode = ctx.create_gain()?;
    master.gain().set_value(0.16);
    master.connect_with_audio_node(&ctx.destination())?;

    for (freq, offset) in notes {
        let start = now + offset;
        let osc = ctx.create_oscillator()?;
        osc.set_type(OscillatorType::Triangle);
        osc.frequency().set_value(*freq);

        let env = ctx.create_gain()?;
        env.gain().set_value(0.0);
        env.gain().set_value_at_time(0.0, start)?;
        env.gain().linear_ramp_to_value_at_time(1.0, start + 0.012)?;
        env.gain()
            .exponential_ramp_to_value_at_time(0.0001, start + 0.32)?;

        osc.connect_with_audio_node(&env)?;
        env.connect_with_audio_node(&master)?;
        osc.start_with_when(start)?;
        osc.stop_with_when(start + 0.34)?;
    }

    Ok(())
}
