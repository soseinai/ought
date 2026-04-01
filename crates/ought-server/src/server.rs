use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use axum::{
    Router,
    response::{Html, Json},
    routing::get,
};
use ought_spec::{Config, SpecGraph};

use crate::api::build_api_response;
use crate::html::VIEWER_HTML;

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

    let api_json = build_api_response(graph.specs());

    let json_data = api_json.clone();
    let app = Router::new()
        .route("/", get(|| async { Html(VIEWER_HTML) }))
        .route(
            "/api/specs",
            get(move || {
                let data = json_data.clone();
                async move { Json(data) }
            }),
        );

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
