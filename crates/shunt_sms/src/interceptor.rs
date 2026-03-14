use async_trait::async_trait;
use shunt_core::storage::MessageStore;
use shunt_core::types::{ShuntedMessage, SmsContent};
use shunt_core::ShuntConfig;
use std::collections::HashMap;
use std::sync::Arc;

use crate::traits::SmsSender;

/// An SMS sender that shunts messages to local storage instead of
/// sending them. Drop-in replacement for any `SmsSender` implementation
/// during development.
pub struct SmsInterceptor {
    store: Arc<dyn MessageStore>,
    config: ShuntConfig,
}

impl SmsInterceptor {
    pub fn new(store: Arc<dyn MessageStore>, config: ShuntConfig) -> Self {
        Self { store, config }
    }
}

#[async_trait]
impl SmsSender for SmsInterceptor {
    type Error = shunt_core::ShuntError;

    async fn send_sms(
        &self,
        from: &str,
        to: &str,
        body: &str,
        metadata: HashMap<String, String>,
    ) -> Result<(), Self::Error> {
        let content = SmsContent {
            from: from.to_string(),
            to: to.to_string(),
            body: body.to_string(),
            metadata,
        };

        let message = ShuntedMessage::new_sms(content);
        let id = message.id;

        self.store.store(&message).await?;

        let preview_url = format!("{}/messages/{}", self.config.web_url(), id);

        if self.config.open_browser {
            let _ = open::that(&preview_url);
        }

        Ok(())
    }
}
