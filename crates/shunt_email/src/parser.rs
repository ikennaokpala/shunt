use mail_parser::MimeHeaders;
use shunt_core::types::{AttachmentInfo, EmailContent};
use shunt_core::ShuntError;
use std::collections::HashMap;

/// Parse raw RFC 5322 email bytes into an `EmailContent`.
pub fn parse_email(raw: &[u8]) -> shunt_core::error::Result<EmailContent> {
    let message = mail_parser::MessageParser::default()
        .parse(raw)
        .ok_or_else(|| ShuntError::EmailParse("Failed to parse email".to_string()))?;

    let from = message
        .from()
        .and_then(|addrs| addrs.first())
        .and_then(|addr| addr.address())
        .unwrap_or_default()
        .to_string();

    let to = extract_addresses(message.to());
    let cc = extract_addresses(message.cc());
    let bcc = extract_addresses(message.bcc());

    let subject = message.subject().unwrap_or("(no subject)").to_string();

    let text_body = message.body_text(0).map(|t| t.to_string());
    let html_body = message.body_html(0).map(|h| h.to_string());

    let mut headers = HashMap::new();
    for header in message.headers() {
        let name = header.name().to_string();
        let value = header.value().as_text().unwrap_or_default().to_string();
        if !value.is_empty() {
            headers.insert(name, value);
        }
    }

    let attachments = message
        .attachments()
        .map(|part| {
            let filename = part
                .attachment_name()
                .unwrap_or("unnamed")
                .to_string();
            let content_type = part
                .content_type()
                .map(|ct| {
                    let main = ct.ctype();
                    ct.subtype()
                        .map(|sub| format!("{}/{}", main, sub))
                        .unwrap_or_else(|| main.to_string())
                })
                .unwrap_or_else(|| "application/octet-stream".to_string());
            let size_bytes = part.contents().len();
            AttachmentInfo {
                filename,
                content_type,
                size_bytes,
            }
        })
        .collect();

    Ok(EmailContent {
        from,
        to,
        cc,
        bcc,
        subject,
        text_body,
        html_body,
        headers,
        attachments,
    })
}

fn extract_addresses(addrs: Option<&mail_parser::Address>) -> Vec<String> {
    match addrs {
        Some(addr) => addr
            .iter()
            .filter_map(|a| a.address().map(|s| s.to_string()))
            .collect(),
        None => Vec::new(),
    }
}
