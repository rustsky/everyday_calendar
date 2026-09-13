//! Talking to the sync server, if there is one.
//!
//! The server lives at the same origin as the page. When the page is served by
//! anything else — a plain static file server, or a file:// URL — the probe
//! fails once and the app stays entirely local, which is the original promise.

use edc_core::model::Doc;
use gloo_net::http::Request;

const DOC_URL: &str = "/api/doc";
const HEALTH_URL: &str = "/api/health";

/// What the console shows about syncing.
#[derive(Clone, PartialEq, Debug)]
pub enum Status {
    /// No sync server answered; everything stays in this browser.
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
            Status::Local => "This browser only".to_string(),
            Status::Syncing => "Syncing…".to_string(),
            Status::Synced { .. } => "Synced".to_string(),
            Status::Failed { .. } => "Sync failed".to_string(),
        }
    }

    pub fn detail(&self) -> String {
        match self {
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

/// True when a sync server answers at this origin.
pub async fn available() -> bool {
    match Request::get(HEALTH_URL).send().await {
        Ok(response) => response.ok(),
        Err(_) => false,
    }
}

/// Sends the local document and returns the server's merged result.
///
/// One call covers both directions: the server folds in what it is given, then
/// hands back everything it knows.
pub async fn exchange(doc: &Doc) -> Result<Doc, String> {
    let request = Request::post(DOC_URL)
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
