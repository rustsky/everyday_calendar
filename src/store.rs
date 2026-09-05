//! Persistent state: goals, the lit-day bitsets, and device settings.
//!
//! Everything lives in `localStorage`. Nothing is ever sent anywhere — the
//! physical Every Day Calendar is proudly 0% internet-connected and so is this.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::date::{Date, MAX_DAYS};

pub const STORAGE_KEY: &str = "everydaycalendar.v2";
pub const SCHEMA_VERSION: u32 = 2;

/// One year of progress: 366 bits, one per day.
///
/// Serialized in the same 61-character, six-bits-per-character encoding the
/// original web app used, so existing saves survive the port untouched.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct YearBits([u64; 6]);

const ENCODED_LEN: usize = 61;
const ENCODE_BASE: u8 = b'*';

impl YearBits {
    pub fn get(&self, day: usize) -> bool {
        day < MAX_DAYS && self.0[day / 64] & (1 << (day % 64)) != 0
    }

    pub fn set(&mut self, day: usize, on: bool) {
        if day >= MAX_DAYS {
            return;
        }
        let mask = 1u64 << (day % 64);
        if on {
            self.0[day / 64] |= mask;
        } else {
            self.0[day / 64] &= !mask;
        }
    }

    pub fn toggle(&mut self, day: usize) -> bool {
        let next = !self.get(day);
        self.set(day, next);
        next
    }

    pub fn count(&self) -> u32 {
        self.0.iter().map(|w| w.count_ones()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|w| *w == 0)
    }

    pub fn encode(&self) -> String {
        let mut out = String::with_capacity(ENCODED_LEN);
        for chunk in 0..ENCODED_LEN {
            let mut value = 0u8;
            for bit in 0..6 {
                if self.get(chunk * 6 + bit) {
                    value |= 1 << bit;
                }
            }
            out.push((ENCODE_BASE + value) as char);
        }
        out
    }

    pub fn decode(text: &str) -> Self {
        let mut bits = Self::default();
        for (chunk, ch) in text.bytes().take(ENCODED_LEN).enumerate() {
            if !(ENCODE_BASE..ENCODE_BASE + 64).contains(&ch) {
                continue;
            }
            let value = ch - ENCODE_BASE;
            for bit in 0..6 {
                if value & (1 << bit) != 0 {
                    bits.set(chunk * 6 + bit, true);
                }
            }
        }
        bits
    }
}

impl Serialize for YearBits {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.encode())
    }
}

impl<'de> Deserialize<'de> for YearBits {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Ok(Self::decode(&text))
    }
}

/// The accent colour a goal's LEDs glow in. `Gold` is the original hardware.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Accent {
    Gold,
    Jade,
    Rose,
    Ice,
    Ember,
}

impl Accent {
    pub const ALL: [Accent; 5] = [
        Accent::Gold,
        Accent::Jade,
        Accent::Rose,
        Accent::Ice,
        Accent::Ember,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            Accent::Gold => "gold",
            Accent::Jade => "jade",
            Accent::Rose => "rose",
            Accent::Ice => "ice",
            Accent::Ember => "ember",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Accent::Gold => "Gold",
            Accent::Jade => "Jade",
            Accent::Rose => "Rose",
            Accent::Ice => "Ice",
            Accent::Ember => "Ember",
        }
    }
}

impl Default for Accent {
    fn default() -> Self {
        Accent::Gold
    }
}

/// A single habit being tracked, with one bitset per year.
#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct Goal {
    pub name: String,
    #[serde(default)]
    pub accent: Accent,
    #[serde(default)]
    pub years: BTreeMap<i32, YearBits>,
}

impl Goal {
    pub fn new(name: impl Into<String>, accent: Accent) -> Self {
        Self {
            name: name.into(),
            accent,
            years: BTreeMap::new(),
        }
    }

    pub fn year(&self, year: i32) -> YearBits {
        self.years.get(&year).copied().unwrap_or_default()
    }

    pub fn is_lit(&self, date: Date) -> bool {
        self.years
            .get(&date.year)
            .is_some_and(|bits| bits.get(date.ordinal))
    }

    pub fn set_year(&mut self, year: i32, bits: YearBits) {
        if bits.is_empty() {
            self.years.remove(&year);
        } else {
            self.years.insert(year, bits);
        }
    }

    /// Toggles a day and returns its new state.
    pub fn toggle(&mut self, year: i32, day: usize) -> bool {
        let mut bits = self.year(year);
        let lit = bits.toggle(day);
        self.set_year(year, bits);
        lit
    }

    pub fn total(&self) -> u32 {
        self.years.values().map(|b| b.count()).sum()
    }

    /// Inclusive range of years holding any data, if any.
    pub fn span(&self) -> Option<(i32, i32)> {
        let first = *self.years.keys().next()?;
        let last = *self.years.keys().next_back()?;
        Some((first, last))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum View {
    Year,
    Month,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Studio,
    Midnight,
}

impl Theme {
    pub fn slug(self) -> &'static str {
        match self {
            Theme::Studio => "studio",
            Theme::Midnight => "midnight",
        }
    }
}

fn default_brightness() -> u8 {
    88
}

fn default_true() -> bool {
    true
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct AppState {
    #[serde(default)]
    pub version: u32,
    pub goals: Vec<Goal>,
    #[serde(default)]
    pub active: usize,
    /// LED brightness, 0–100, exactly like the hardware's dimmer.
    #[serde(default = "default_brightness")]
    pub brightness: u8,
    /// Require a deliberate press-and-hold to change a day.
    #[serde(default = "default_true")]
    pub ritual: bool,
    #[serde(default)]
    pub sound: bool,
    #[serde(default = "default_true")]
    pub boot_sequence: bool,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub view: View,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::Studio
    }
}

impl Default for View {
    fn default() -> Self {
        View::Year
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION,
            goals: vec![Goal::new("Every day", Accent::Gold)],
            active: 0,
            brightness: default_brightness(),
            ritual: true,
            sound: false,
            boot_sequence: true,
            theme: Theme::Studio,
            view: View::Year,
        }
    }
}

impl AppState {
    pub fn active_goal(&self) -> &Goal {
        let index = self.active.min(self.goals.len().saturating_sub(1));
        &self.goals[index]
    }

    pub fn active_goal_mut(&mut self) -> &mut Goal {
        let index = self.active.min(self.goals.len().saturating_sub(1));
        &mut self.goals[index]
    }

    /// Repairs anything a hand-edited or truncated import might have broken.
    pub fn sanitize(&mut self) {
        if self.goals.is_empty() {
            self.goals.push(Goal::new("Every day", Accent::Gold));
        }
        if self.active >= self.goals.len() {
            self.active = 0;
        }
        self.brightness = self.brightness.min(100);
        self.version = SCHEMA_VERSION;
    }
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

/// Reads state from `localStorage`, falling back to a one-time migration of the
/// original app's per-year keys, then to a fresh default.
pub fn load() -> AppState {
    let Some(store) = storage() else {
        return AppState::default();
    };

    if let Ok(Some(raw)) = store.get_item(STORAGE_KEY) {
        if let Ok(mut state) = serde_json::from_str::<AppState>(&raw) {
            state.sanitize();
            return state;
        }
    }

    let mut state = AppState::default();
    if let Some(years) = migrate_legacy(&store) {
        state.goals[0].years = years;
    }
    // A twelve-by-thirty-one grid is unreadable on a phone, so first-time
    // visitors on a narrow screen start on the month view.
    if narrow_viewport() {
        state.view = View::Month;
    }
    state
}

fn narrow_viewport() -> bool {
    web_sys::window()
        .and_then(|w| w.inner_width().ok())
        .and_then(|v| v.as_f64())
        .is_some_and(|width| width < 700.0)
}

/// The original app stored one 61-character string per year under the year
/// itself as the key. Pick those up so nobody loses a streak in the port.
fn migrate_legacy(store: &web_sys::Storage) -> Option<BTreeMap<i32, YearBits>> {
    let len = store.length().ok()?;
    let mut years = BTreeMap::new();
    for i in 0..len {
        let Ok(Some(key)) = store.key(i) else { continue };
        let Ok(year) = key.parse::<i32>() else {
            continue;
        };
        if !(1970..=2200).contains(&year) {
            continue;
        }
        let Ok(Some(value)) = store.get_item(&key) else {
            continue;
        };
        let bits = YearBits::decode(&value);
        if !bits.is_empty() {
            years.insert(year, bits);
        }
    }
    (!years.is_empty()).then_some(years)
}

pub fn save(state: &AppState) {
    let Some(store) = storage() else { return };
    if let Ok(json) = serde_json::to_string(state) {
        let _ = store.set_item(STORAGE_KEY, &json);
    }
}

/// A portable backup file. Same shape as the stored blob, pretty-printed.
pub fn export_json(state: &AppState) -> String {
    serde_json::to_string_pretty(state).unwrap_or_default()
}

pub fn import_json(text: &str) -> Result<AppState, String> {
    let mut state: AppState = serde_json::from_str(text).map_err(|e| e.to_string())?;
    state.sanitize();
    Ok(state)
}

/// Suggested filename for an export, e.g. `every-day-calendar-2026-03-04.json`.
pub fn export_filename(today: Date) -> String {
    let (month, day) = today.month_day();
    format!(
        "every-day-calendar-{:04}-{:02}-{:02}.json",
        today.year,
        month + 1,
        day
    )
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_the_legacy_encoding() {
        let mut bits = YearBits::default();
        for day in [0usize, 1, 5, 6, 59, 200, 364, 365] {
            bits.set(day, true);
        }
        let encoded = bits.encode();
        assert_eq!(encoded.len(), ENCODED_LEN);
        assert_eq!(YearBits::decode(&encoded), bits);
        assert_eq!(bits.count(), 8);
    }

    #[test]
    fn matches_the_original_bit_layout() {
        // The original packed bit `i` as `state[i / 6] & (1 << (i % 6))` and
        // wrote each six-bit group as `char(42 + value)`.
        let mut bits = YearBits::default();
        bits.set(0, true);
        assert!(bits.encode().starts_with('+'));
        let mut bits = YearBits::default();
        bits.set(5, true);
        assert!(bits.encode().starts_with('J'));
    }

    #[test]
    fn dates_step_across_year_boundaries() {
        let jan1 = Date::new(2026, 0, 1);
        let dec31 = jan1.prev();
        assert_eq!(dec31.year, 2025);
        assert_eq!(dec31.ordinal, 364);
        assert_eq!(dec31.next(), jan1);

        let leap_dec31 = Date::new(2024, 11, 31);
        assert_eq!(leap_dec31.ordinal, 365);
        assert_eq!(leap_dec31.next(), Date::new(2025, 0, 1));
    }

    #[test]
    fn empty_years_are_not_stored() {
        let mut goal = Goal::new("Test", Accent::Gold);
        goal.toggle(2026, 10);
        assert_eq!(goal.years.len(), 1);
        goal.toggle(2026, 10);
        assert!(goal.years.is_empty());
    }
}
