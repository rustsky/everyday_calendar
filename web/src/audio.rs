//! A tiny synthesised chime for the moment a day lights up.
//!
//! The hardware is silent; this is opt-in and off by default. Tones are built
//! from oscillators so the bundle carries no audio files.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// A day was lit.
    Light,
    /// A day was turned back off.
    Dim,
    /// A streak milestone was reached.
    Milestone,
}

impl Tone {
    /// Frequency in hertz, and start offset in seconds, of each note.
    fn notes(self) -> &'static [(f32, f64)] {
        match self {
            // A rising perfect fifth: the "locked in" sound.
            Tone::Light => &[(784.0, 0.0), (1174.7, 0.07)],
            // A short fall, so undoing sounds like undoing.
            Tone::Dim => &[(392.0, 0.0)],
            // A little major arpeggio for milestones.
            Tone::Milestone => &[(523.3, 0.0), (659.3, 0.09), (784.0, 0.18), (1046.5, 0.27)],
        }
    }
}

/// Plays `tone`. Any failure (no audio device, autoplay policy, unsupported
/// browser) is silently ignored — sound is decoration, never a dependency.
#[cfg(not(feature = "desktop"))]
pub fn play(tone: Tone) {
    let _ = try_play(tone);
}

#[cfg(not(feature = "desktop"))]
fn try_play(tone: Tone) -> Result<(), wasm_bindgen::JsValue> {
    use web_sys::{AudioContext, GainNode, OscillatorType};

    let ctx = AudioContext::new()?;
    let now = ctx.current_time();

    let master: GainNode = ctx.create_gain()?;
    master.gain().set_value(0.16);
    master.connect_with_audio_node(&ctx.destination())?;

    for (freq, offset) in tone.notes() {
        let start = now + offset;
        let osc = ctx.create_oscillator()?;
        osc.set_type(OscillatorType::Triangle);
        osc.frequency().set_value(*freq);

        let env = ctx.create_gain()?;
        env.gain().set_value(0.0);
        env.gain().set_value_at_time(0.0, start)?;
        env.gain()
            .linear_ramp_to_value_at_time(1.0, start + 0.012)?;
        env.gain()
            .exponential_ramp_to_value_at_time(0.0001, start + 0.32)?;

        osc.connect_with_audio_node(&env)?;
        env.connect_with_audio_node(&master)?;
        osc.start_with_when(start)?;
        osc.stop_with_when(start + 0.34)?;
    }

    Ok(())
}

/// Plays `tone` through the webview's own Web Audio, the same synthesis as the
/// browser build. One context is kept for the life of the window.
#[cfg(feature = "desktop")]
pub fn play(tone: Tone) {
    use dioxus::prelude::document;

    let notes = serde_json::to_string(tone.notes()).unwrap_or_else(|_| "[]".to_string());
    document::eval(&format!(
        r#"
        try {{
            const ctx = window.__edcAudio = window.__edcAudio || new AudioContext();
            ctx.resume();
            const now = ctx.currentTime;
            const master = ctx.createGain();
            master.gain.value = 0.16;
            master.connect(ctx.destination);
            for (const [freq, offset] of {notes}) {{
                const start = now + offset;
                const osc = ctx.createOscillator();
                osc.type = "triangle";
                osc.frequency.value = freq;
                const env = ctx.createGain();
                env.gain.setValueAtTime(0, start);
                env.gain.linearRampToValueAtTime(1, start + 0.012);
                env.gain.exponentialRampToValueAtTime(0.0001, start + 0.32);
                osc.connect(env);
                env.connect(master);
                osc.start(start);
                osc.stop(start + 0.34);
            }}
        }} catch (_) {{}}
        "#
    ));
}
