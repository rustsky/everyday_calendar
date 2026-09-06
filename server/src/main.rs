//! A sync server for the Every Day Calendar.
//!
//! It holds one document in a JSON file and merges whatever clients send it.
//! There are no accounts and no authentication: this is meant to run on your
//! own machine, reachable over your own network — a LAN, or a Tailscale
//! network. Do not expose it to the open internet.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use clap::Parser;
use edc_core::model::{DOC_VERSION, Doc};
use serde::Serialize;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

#[derive(Parser, Debug)]
#[command(name = "edc-sync", about = "Sync server for the Every Day Calendar")]
struct Args {
    /// Address to listen on. Defaults to every interface so phones on the same
    /// network can reach it.
    #[arg(long, default_value = "0.0.0.0:8080")]
    addr: SocketAddr,

    /// Where the merged document is kept.
    #[arg(long, default_value = "edc-data/doc.json")]
    data: PathBuf,

    /// Directory of built web assets to serve alongside the API. Point this at
    /// `target/dx/everydaycalendar/release/web/public`.
    #[arg(long)]
    dir: Option<PathBuf>,
}

struct AppState {
    doc: Mutex<Doc>,
    path: PathBuf,
}

#[derive(Serialize)]
struct Health {
    ok: bool,
    version: u32,
    goals: usize,
    day_entries: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "edc_sync=info,tower_http=warn".into()),
        )
        .init();

    let args = Args::parse();
    let doc = load(&args.data).await?;
    tracing::info!(
        path = %args.data.display(),
        goals = doc.goals.len(),
        day_entries = doc.day_entries(),
        "loaded document"
    );

    let state = Arc::new(AppState {
        doc: Mutex::new(doc),
        path: args.data.clone(),
    });

    let mut app = Router::new()
        .route("/api/doc", get(read_doc).post(merge_doc))
        .route("/api/health", get(health))
        // The client is served from the same origin in the normal setup, but
        // allow cross-origin calls so a separately served build still syncs.
        .layer(CorsLayer::permissive())
        .with_state(state);

    if let Some(dir) = &args.dir {
        let index = dir.join("index.html");
        app = app.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(index)));
        tracing::info!(dir = %dir.display(), "serving web assets");
    }

    let listener = tokio::net::TcpListener::bind(args.addr).await?;
    tracing::info!(addr = %args.addr, "listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutting down");
}

async fn load(path: &Path) -> Result<Doc, Box<dyn std::error::Error>> {
    match tokio::fs::read_to_string(path).await {
        Ok(text) => Ok(serde_json::from_str(&text)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Doc::new()),
        Err(error) => Err(error.into()),
    }
}

/// Writes to a sibling temp file and renames, so an interrupted write can
/// never leave a half-written document behind.
async fn save(path: &Path, doc: &Doc) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent).await?;
        }
    }
    let json = serde_json::to_string(doc).unwrap_or_default();
    let temp = path.with_extension("json.tmp");
    tokio::fs::write(&temp, json).await?;
    tokio::fs::rename(&temp, path).await
}

async fn health(State(state): State<Arc<AppState>>) -> Json<Health> {
    let doc = state.doc.lock().await;
    Json(Health {
        ok: true,
        version: DOC_VERSION,
        goals: doc.goals.len(),
        day_entries: doc.day_entries(),
    })
}

async fn read_doc(State(state): State<Arc<AppState>>) -> Json<Doc> {
    Json(state.doc.lock().await.clone())
}

/// Folds the client's document in and hands back the merged result, which the
/// client then adopts. Both directions of sync happen in this one call.
async fn merge_doc(
    State(state): State<Arc<AppState>>,
    Json(incoming): Json<Doc>,
) -> Result<Json<Doc>, (StatusCode, String)> {
    let mut doc = state.doc.lock().await;
    doc.merge(&incoming);

    if let Err(error) = save(&state.path, &doc).await {
        tracing::error!(%error, "could not write document");
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("could not write document: {error}"),
        ));
    }

    Ok(Json(doc.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use edc_core::model::Stamp;

    #[tokio::test]
    async fn a_document_survives_a_save_and_load() {
        let dir = std::env::temp_dir().join(format!("edc-test-{}", std::process::id()));
        let path = dir.join("doc.json");

        let mut doc = Doc::new();
        doc.set_day("g1", 2026, 5, true, Stamp::new(1, "test"));
        save(&path, &doc).await.unwrap();

        let loaded = load(&path).await.unwrap();
        assert_eq!(loaded, doc);

        tokio::fs::remove_dir_all(&dir).await.unwrap();
    }

    #[tokio::test]
    async fn a_missing_file_starts_an_empty_document() {
        let path = std::env::temp_dir().join("edc-does-not-exist-4f2a/doc.json");
        assert_eq!(load(&path).await.unwrap(), Doc::new());
    }
}
