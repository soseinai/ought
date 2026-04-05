use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::{
    Router,
    extract::Query,
    response::{IntoResponse, Json},
    routing::get,
};
use rust_embed::Embed;
use serde::Deserialize;

use ought_spec::{Config, SpecGraph};

use crate::api::build_api_response;
use crate::proofs::ProofIndex;
use crate::search::SearchIndex;

#[derive(Deserialize)]
struct SearchParams {
    q: Option<String>,
    limit: Option<usize>,
}

#[derive(Embed)]
#[folder = "dist/"]
struct Assets;

async fn static_handler(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_text_plain();
            (
                [(axum::http::header::CONTENT_TYPE, mime.as_ref().to_string())],
                content.data.to_vec(),
            )
                .into_response()
        }
        None => {
            // SPA fallback: serve index.html for client-side routing
            match Assets::get("index.html") {
                Some(content) => (
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "text/html".to_string(),
                    )],
                    content.data.to_vec(),
                )
                    .into_response(),
                None => axum::http::StatusCode::NOT_FOUND.into_response(),
            }
        }
    }
}

/// Serve the ought viewer on the given port.
/// Parses specs from the config, starts an HTTP server, and optionally opens the browser.
pub async fn serve(config_path: Option<&Path>, port: u16, open_browser: bool) -> anyhow::Result<()> {
    let (cfg_path, config) = match config_path {
        Some(path) => {
            let config = Config::load(path)?;
            (path.to_path_buf(), config)
        }
        None => Config::discover()?,
    };

    let config_dir = cfg_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    let roots: Vec<PathBuf> = config
        .specs
        .roots
        .iter()
        .map(|r| config_dir.join(r))
        .collect();

    let graph = SpecGraph::from_roots(&roots).map_err(|errors| {
        let messages: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        anyhow::anyhow!("spec parse errors:\n  {}", messages.join("\n  "))
    })?;

    let proof_index = ProofIndex::build(&config, &config_dir);
    eprintln!(
        "Proof index built: {} proofs across {} clauses",
        proof_index.proof_count(),
        proof_index.clause_count()
    );

    let api_json = build_api_response(graph.specs(), &proof_index);
    let index = Arc::new(SearchIndex::build(graph.specs()));

    eprintln!(
        "Search index built: {} clauses indexed",
        index.clause_count()
    );

    let json_data = api_json.clone();
    let search_index = index.clone();
    let app = Router::new()
        .route(
            "/api/specs",
            get(move || {
                let data = json_data.clone();
                async move { Json(data) }
            }),
        )
        .route(
            "/api/search",
            get(move |params: Query<SearchParams>| {
                let idx = search_index.clone();
                async move {
                    let limit = params.limit.unwrap_or(20).min(100);
                    let query = params.q.as_deref().unwrap_or("");
                    Json(idx.search(query, limit))
                }
            }),
        )
        .fallback(static_handler);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    eprintln!("Serving ought viewer at http://localhost:{}", port);

    if open_browser {
        let url = format!("http://localhost:{}", port);
        let _ = std::process::Command::new("open").arg(&url).spawn();
    }

    axum::serve(listener, app).await?;

    Ok(())
}
