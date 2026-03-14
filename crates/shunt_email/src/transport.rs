use async_trait::async_trait;
use lettre::AsyncTransport;
use shunt_core::storage::MessageStore;
use shunt_core::types::ShuntedMessage;
use shunt_core::ShuntConfig;
use std::sync::Arc;
use uuid::Uuid;

use crate::parser::parse_email;

/// A lettre-compatible transport that shunts emails to local storage
/// instead of sending them.
pub struct ShuntEmailTransport {
    store: Arc<dyn MessageStore>,
    config: ShuntConfig,
}

impl ShuntEmailTransport {
    pub fn new(store: Arc<dyn MessageStore>, config: ShuntConfig) -> Self {
        Self { store, config }
    }
}

/// Response returned when an email is successfully shunted.
#[derive(Debug)]
pub struct ShuntEmailResponse {
    pub message_id: Uuid,
    pub preview_url: String,
}

impl std::fmt::Display for ShuntEmailResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Shunted email {} -> {}", self.message_id, self.preview_url)
    }
}

/// Error type for the shunt email transport.
#[derive(Debug, thiserror::Error)]
pub enum ShuntTransportError {
    #[error("Email parse error: {0}")]
    Parse(#[from] shunt_core::ShuntError),

    #[error("Lettre error: {0}")]
    Lettre(#[from] lettre::error::Error),
}

#[async_trait]
impl AsyncTransport for ShuntEmailTransport {
    type Ok = ShuntEmailResponse;
    type Error = ShuntTransportError;

    async fn send_raw(&self, _envelope: &lettre::address::Envelope, email: &[u8]) -> Result<Self::Ok, Self::Error> {
        let email_content = parse_email(email)?;

        let message = ShuntedMessage::new_email(email_content);
        let id = message.id;

        self.store.store(&message).await?;

        let preview_url = format!("{}/messages/{}", self.config.web_url(), id);

        if self.config.open_browser {
            let _ = open::that(&preview_url);
        }

        Ok(ShuntEmailResponse {
            message_id: id,
            preview_url,
        })
    }
}
