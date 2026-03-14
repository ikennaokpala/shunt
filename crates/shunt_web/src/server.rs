use axum::routing::get;
use axum::Router;
use shunt_core::storage::MessageStore;
use shunt_core::ShuntConfig;
use std::sync::Arc;

use crate::handlers;
use crate::sse;

/// Application state shared across all handlers.
#[derive(Clone)]
pub struct AppState {
    pub store: Arc<dyn MessageStore>,
    pub config: ShuntConfig,
}

/// Build the Axum router with all routes.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(handlers::index))
        .route("/messages", get(handlers::list_messages))
        .route("/messages/{id}", get(handlers::get_message))
        .route("/events", get(sse::event_stream))
        .with_state(state)
}

/// Start the web preview server.
pub async fn start_server(
    store: Arc<dyn MessageStore>,
    config: ShuntConfig,
) -> shunt_core::error::Result<()> {
    let state = AppState { store, config: config.clone() };
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(config.web_addr())
        .await
        .map_err(|e| shunt_core::ShuntError::Server(e.to_string()))?;

    axum::serve(listener, app)
        .await
        .map_err(|e| shunt_core::ShuntError::Server(e.to_string()))?;

    Ok(())
}
