//! Device-local settings.
//!
//! These deliberately do not sync. Brightness, theme and year-versus-month are
//! properties of the screen you are looking at, not of the habit — a phone
//! wants the month view and a dimmer board than a desk monitor does.

use serde::{Deserialize, Serialize};

use crate::model::GoalId;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum View {
    #[default]
    Year,
    Month,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
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

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Prefs {
    /// LED brightness, 0–100, like the hardware's dimmer.
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
    /// Which goal this device is looking at.
    #[serde(default)]
    pub active: Option<GoalId>,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            brightness: default_brightness(),
            ritual: true,
            sound: false,
            boot_sequence: true,
            theme: Theme::default(),
            view: View::default(),
            active: None,
        }
    }
}

impl Prefs {
    pub fn sanitize(&mut self) {
        self.brightness = self.brightness.min(100);
    }
}
