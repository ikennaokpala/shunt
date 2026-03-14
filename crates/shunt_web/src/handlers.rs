use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::Json;
use rust_embed::Embed;
use uuid::Uuid;

use crate::server::AppState;

#[derive(Embed)]
#[folder = "frontend/"]
struct FrontendAssets;

/// Serve the main preview UI page.
pub async fn index() -> impl IntoResponse {
    match FrontendAssets::get("index.html") {
        Some(content) => {
            let body = std::str::from_utf8(content.data.as_ref())
                .unwrap_or("")
                .to_string();
            Html(body).into_response()
        }
        None => (StatusCode::INTERNAL_SERVER_ERROR, "Frontend not found").into_response(),
    }
}

/// List all shunted messages.
pub async fn list_messages(State(state): State<AppState>) -> Response {
    match state.store.list().await {
        Ok(messages) => Json(serde_json::json!({
            "data": messages,
            "meta": {
                "total": messages.len()
            }
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": {
                    "code": "INTERNAL_ERROR",
                    "message": e.to_string()
                }
            })),
        )
            .into_response(),
    }
}

/// Get a single shunted message by ID.
pub async fn get_message(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Response {
    match state.store.get(id).await {
        Ok(message) => Json(serde_json::json!({
            "data": message
        }))
        .into_response(),
        Err(shunt_core::ShuntError::NotFound(_)) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": {
                    "code": "RESOURCE_NOT_FOUND",
                    "message": format!("Message {} not found", id)
                }
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": {
                    "code": "INTERNAL_ERROR",
                    "message": e.to_string()
                }
            })),
        )
            .into_response(),
    }
}
