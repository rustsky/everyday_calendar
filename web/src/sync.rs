//! Talking to the sync server, if there is one.
//!
//! The web client asks the origin that served the page. When the page is
//! served by anything else — a plain static file server, or a file:// URL —
//! the probe fails once and the app stays entirely local, which is the
//! original promise. The desktop app has no origin, so it talks to whichever
//! server address is set on the back of the board, and to none when it is
//! blank.

use edc_core::model::Doc;
use edc_core::prefs::Prefs;

use crate::platform;

const DOC_PATH: &str = "/api/doc";
const HEALTH_PATH: &str = "/api/health";

/// What the console shows about syncing.
#[derive(Clone, PartialEq, Debug)]
pub enum Status {
    /// No sync server answered; everything stays on this device.
    Local,
    /// A server is there and the last exchange succeeded.
    Synced {
        at: u64,
    },
    Syncing,
    Failed {
        message: String,
    },
}

impl Status {
    pub fn is_local(&self) -> bool {
        matches!(self, Status::Local)
    }

    pub fn label(&self) -> String {
        match self {
            Status::Local => format!("{} only", platform::HERE),
            Status::Syncing => "Syncing…".to_string(),
            Status::Synced { .. } => "Synced".to_string(),
            Status::Failed { .. } => "Sync failed".to_string(),
        }
    }

    pub fn detail(&self) -> String {
        match self {
            Status::Local if cfg!(feature = "desktop") => {
                "No sync server set. Everything is stored here and nowhere else.".to_string()
            }
            Status::Local => {
                "No sync server on this address. Everything is stored here and nowhere else."
                    .to_string()
            }
            Status::Syncing => "Sending changes to the server.".to_string(),
            Status::Synced { .. } => "Every device on this server sees the same days.".to_string(),
            Status::Failed { message } => format!("Last attempt failed: {message}"),
        }
    }

    pub fn slug(&self) -> &'static str {
        match self {
            Status::Local => "local",
            Status::Syncing => "syncing",
            Status::Synced { .. } => "ok",
            Status::Failed { .. } => "failed",
        }
    }
}

/// The server to try, as a URL prefix for the API paths, or `None` when there
/// is nothing to try.
pub fn endpoint(prefs: &Prefs) -> Option<String> {
    if cfg!(feature = "desktop") {
        prefs.server.clone()
    } else {
        // Same origin: the paths alone are enough.
        Some(String::new())
    }
}

#[cfg(not(feature = "desktop"))]
mod transport {
    use edc_core::model::Doc;
    use gloo_net::http::Request;

    pub async fn available(url: &str) -> bool {
        match Request::get(url).send().await {
            Ok(response) => response.ok(),
            Err(_) => false,
        }
    }

    pub async fn exchange(url: &str, doc: &Doc) -> Result<Doc, String> {
        let request = Request::post(url)
            .json(doc)
            .map_err(|error| error.to_string())?;

        let response = request.send().await.map_err(|error| error.to_string())?;
        if !response.ok() {
            return Err(format!("server said {}", response.status()));
        }
        response
            .json::<Doc>()
            .await
            .map_err(|error| error.to_string())
    }
}

#[cfg(feature = "desktop")]
mod transport {
    use std::sync::OnceLock;
    use std::time::Duration;

    use edc_core::model::Doc;

    fn client() -> &'static reqwest::Client {
        static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
        CLIENT.get_or_init(|| {
            reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap_or_default()
        })
    }

    pub async fn available(url: &str) -> bool {
        match client().get(url).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    pub async fn exchange(url: &str, doc: &Doc) -> Result<Doc, String> {
        let response = client()
            .post(url)
            .json(doc)
            .send()
            .await
            .map_err(|error| error.to_string())?;
        if !response.status().is_success() {
            return Err(format!("server said {}", response.status().as_u16()));
        }
        response
            .json::<Doc>()
            .await
            .map_err(|error| error.to_string())
    }
}

/// True when a sync server answers at `endpoint`.
pub async fn available(endpoint: &str) -> bool {
    transport::available(&format!("{endpoint}{HEALTH_PATH}")).await
}

/// Sends the local document and returns the server's merged result.
///
/// One call covers both directions: the server folds in what it is given, then
/// hands back everything it knows.
pub async fn exchange(endpoint: &str, doc: &Doc) -> Result<Doc, String> {
    transport::exchange(&format!("{endpoint}{DOC_PATH}"), doc).await
}
