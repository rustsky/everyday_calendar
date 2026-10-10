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
    /// The sync server the desktop app talks to. The web client ignores this
    /// and asks the origin that served it.
    #[serde(default)]
    pub server: Option<String>,
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
            server: None,
        }
    }
}

impl Prefs {
    pub fn sanitize(&mut self) {
        self.brightness = self.brightness.min(100);
        self.server = self.server.as_deref().and_then(normalize_server);
    }
}

/// Tidies a typed server address: trims it, drops trailing slashes, and
/// assumes `http://` when no scheme is given. Blank means no server.
pub fn normalize_server(raw: &str) -> Option<String> {
    let address = raw.trim().trim_end_matches('/');
    if address.is_empty() {
        None
    } else if address.contains("://") {
        Some(address.to_string())
    } else {
        Some(format!("http://{address}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_addresses_are_tidied() {
        assert_eq!(normalize_server(""), None);
        assert_eq!(normalize_server("   "), None);
        assert_eq!(
            normalize_server(" my-mac:8080/ "),
            Some("http://my-mac:8080".to_string())
        );
        assert_eq!(
            normalize_server("https://calendar.example.ts.net"),
            Some("https://calendar.example.ts.net".to_string())
        );
    }

    #[test]
    fn prefs_without_a_server_still_load() {
        let prefs: Prefs = serde_json::from_str(r#"{"brightness":40}"#).unwrap();
        assert_eq!(prefs.server, None);
        assert_eq!(prefs.brightness, 40);
    }
}
