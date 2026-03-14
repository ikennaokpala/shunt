use lettre::{AsyncTransport, Message, message::header::ContentType, message::MultiPart, message::SinglePart};
use shunt_core::storage::MessageStore;
use shunt_core::ShuntConfig;
use shunt_email::ShuntEmailTransport;
use shunt_sms::{SmsInterceptor, SmsSender};
use std::collections::HashMap;
use std::sync::Arc;

/// A notification service that sends emails and SMS.
/// Uses shunt transports during development to intercept all messages.
pub struct NotificationService {
    email_transport: ShuntEmailTransport,
    sms_sender: SmsInterceptor,
}

impl NotificationService {
    pub fn new(store: Arc<dyn MessageStore>, config: ShuntConfig) -> Self {
        Self {
            email_transport: ShuntEmailTransport::new(store.clone(), config.clone()),
            sms_sender: SmsInterceptor::new(store, config),
        }
    }

    /// Send a welcome email to a new user.
    pub async fn send_welcome_email(
        &self,
        to_email: &str,
        user_name: &str,
    ) -> Result<uuid::Uuid, Box<dyn std::error::Error + Send + Sync>> {
        let email = Message::builder()
            .from("noreply@myapp.com".parse()?)
            .to(to_email.parse()?)
            .subject(format!("Welcome, {}!", user_name))
            .header(ContentType::TEXT_PLAIN)
            .body(format!(
                "Hi {},\n\nWelcome to MyApp! We're glad you're here.\n\nBest,\nThe MyApp Team",
                user_name
            ))?;

        let resp = self.email_transport.send(email).await?;
        Ok(resp.message_id)
    }

    /// Send an HTML email with both text and HTML parts.
    pub async fn send_html_email(
        &self,
        to_email: &str,
        subject: &str,
        text_body: &str,
        html_body: &str,
    ) -> Result<uuid::Uuid, Box<dyn std::error::Error + Send + Sync>> {
        let email = Message::builder()
            .from("noreply@myapp.com".parse()?)
            .to(to_email.parse()?)
            .subject(subject)
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(text_body.to_string()),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html_body.to_string()),
                    ),
            )?;

        let resp = self.email_transport.send(email).await?;
        Ok(resp.message_id)
    }

    /// Send an email to multiple recipients with CC.
    pub async fn send_team_email(
        &self,
        to_emails: &[&str],
        cc_emails: &[&str],
        subject: &str,
        body: &str,
    ) -> Result<uuid::Uuid, Box<dyn std::error::Error + Send + Sync>> {
        let mut builder = Message::builder()
            .from("admin@myapp.com".parse()?)
            .subject(subject);

        for to in to_emails {
            builder = builder.to(to.parse()?);
        }

        for cc in cc_emails {
            builder = builder.cc(cc.parse()?);
        }

        let email = builder
            .header(ContentType::TEXT_PLAIN)
            .body(body.to_string())?;

        let resp = self.email_transport.send(email).await?;
        Ok(resp.message_id)
    }

    /// Send a verification SMS.
    pub async fn send_verification_sms(
        &self,
        phone: &str,
        code: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let metadata = HashMap::from([
            ("type".to_string(), "verification".to_string()),
            ("code".to_string(), code.to_string()),
        ]);

        self.sms_sender
            .send_sms(
                "+18005551000",
                phone,
                &format!("Your verification code is: {}", code),
                metadata,
            )
            .await?;

        Ok(())
    }

    /// Send a ride notification SMS.
    pub async fn send_ride_sms(
        &self,
        phone: &str,
        driver_name: &str,
        eta_minutes: u32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let metadata = HashMap::from([
            ("type".to_string(), "ride_update".to_string()),
            ("driver".to_string(), driver_name.to_string()),
            ("eta".to_string(), eta_minutes.to_string()),
        ]);

        self.sms_sender
            .send_sms(
                "+18005551000",
                phone,
                &format!(
                    "{} is on the way! ETA: {} minutes.",
                    driver_name, eta_minutes
                ),
                metadata,
            )
            .await?;

        Ok(())
    }
}
