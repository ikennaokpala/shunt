use async_trait::async_trait;
use std::collections::HashMap;

/// Trait for sending SMS messages.
///
/// Implement this trait for your SMS provider (e.g., Twilio, Vonage).
/// During development, swap in `SmsInterceptor` to shunt messages
/// to local preview instead of sending them.
#[async_trait]
pub trait SmsSender: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Send an SMS message.
    ///
    /// # Arguments
    /// * `from` - Sender phone number or identifier
    /// * `to` - Recipient phone number
    /// * `body` - Message body text
    /// * `metadata` - Optional key-value metadata (e.g., campaign ID, message type)
    async fn send_sms(
        &self,
        from: &str,
        to: &str,
        body: &str,
        metadata: HashMap<String, String>,
    ) -> Result<(), Self::Error>;
}
