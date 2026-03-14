use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShuntedMessage {
    pub id: Uuid,
    pub kind: MessageKind,
    pub created_at: DateTime<Utc>,
    pub summary: MessageSummary,
    pub content: MessageContent,
}

impl ShuntedMessage {
    pub fn new_email(content: EmailContent) -> Self {
        let summary = MessageSummary {
            from: content.from.clone(),
            to: content.to.clone(),
            subject: Some(content.subject.clone()),
        };
        Self {
            id: Uuid::new_v4(),
            kind: MessageKind::Email,
            created_at: Utc::now(),
            summary,
            content: MessageContent::Email(content),
        }
    }

    pub fn new_sms(content: SmsContent) -> Self {
        let summary = MessageSummary {
            from: content.from.clone(),
            to: vec![content.to.clone()],
            subject: None,
        };
        Self {
            id: Uuid::new_v4(),
            kind: MessageKind::Sms,
            created_at: Utc::now(),
            summary,
            content: MessageContent::Sms(content),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageKind {
    Email,
    Sms,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSummary {
    pub from: String,
    pub to: Vec<String>,
    pub subject: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum MessageContent {
    Email(EmailContent),
    Sms(SmsContent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailContent {
    pub from: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
    pub headers: HashMap<String, String>,
    pub attachments: Vec<AttachmentInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsContent {
    pub from: String,
    pub to: String,
    pub body: String,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentInfo {
    pub filename: String,
    pub content_type: String,
    pub size_bytes: usize,
}
