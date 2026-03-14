use axum::extract::State;
use axum::response::sse::{Event, Sse};
use futures::stream::Stream;
use std::convert::Infallible;
use std::time::Duration;

use crate::server::AppState;

/// SSE endpoint that notifies clients when new messages arrive.
/// Polls the store periodically and emits events when the message count changes.
pub async fn event_stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = async_stream::stream! {
        let mut last_count: usize = 0;

        loop {
            if let Ok(messages) = state.store.list().await {
                let current_count = messages.len();
                if current_count != last_count {
                    last_count = current_count;
                    let data = serde_json::json!({
                        "count": current_count,
                        "latest": messages.first()
                    });
                    yield Ok(Event::default()
                        .event("messages")
                        .data(data.to_string()));
                }
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}
