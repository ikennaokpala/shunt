use fabricate::FactoryBuilder;
use lettre::message::header::ContentType;
use lettre::message::{MultiPart, SinglePart};
use lettre::{AsyncTransport, Message};
use reqwest::Client;
use serde_json::Value;
use shunt_core::storage::{FileStore, MessageStore};
use shunt_core::types::*;
use shunt_core::ShuntConfig;
use shunt_email::ShuntEmailTransport;
use shunt_sms::{SmsInterceptor, SmsSender};
use shunt_web::server::{build_router, AppState};
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;

use shunt_sample_app::factories::*;
use shunt_sample_app::notifications::NotificationService;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn setup() -> (TempDir, Arc<FileStore>, ShuntConfig) {
    let tmp = TempDir::new().unwrap();
    let config = ShuntConfig::new()
        .storage_dir(tmp.path().join("messages"))
        .open_browser(false);
    let store = Arc::new(FileStore::new(&config));
    (tmp, store, config)
}

/// Start the shunt web server on a random port, return (base_url, join_handle).
async fn start_test_server(
    store: Arc<dyn MessageStore>,
    config: ShuntConfig,
) -> (String, tokio::task::JoinHandle<()>) {
    let state = AppState {
        store,
        config: config.clone(),
    };
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
    (base_url, handle)
}

// ===========================================================================
// 1. FILESTORE TESTS — exhaustive CRUD edge cases
// ===========================================================================

#[tokio::test]
async fn test_store_and_retrieve_email() {
    let (_tmp, store, _cfg) = setup();

    let email = EmailContent {
        from: "alice@test.com".into(),
        to: vec!["bob@test.com".into()],
        cc: vec![],
        bcc: vec![],
        subject: "Hello".into(),
        text_body: Some("Hello Bob".into()),
        html_body: None,
        headers: HashMap::new(),
        attachments: vec![],
    };
    let msg = ShuntedMessage::new_email(email);
    let id = msg.id;

    store.store(&msg).await.unwrap();
    let retrieved = store.get(id).await.unwrap();

    assert_eq!(retrieved.id, id);
    assert_eq!(retrieved.kind, MessageKind::Email);
    assert_eq!(retrieved.summary.from, "alice@test.com");
    assert_eq!(retrieved.summary.to, vec!["bob@test.com"]);
    assert_eq!(retrieved.summary.subject.as_deref(), Some("Hello"));
}

#[tokio::test]
async fn test_store_and_retrieve_sms() {
    let (_tmp, store, _cfg) = setup();

    let sms = SmsContent {
        from: "+1111".into(),
        to: "+2222".into(),
        body: "Hey".into(),
        metadata: HashMap::from([("key".into(), "val".into())]),
    };
    let msg = ShuntedMessage::new_sms(sms);
    let id = msg.id;

    store.store(&msg).await.unwrap();
    let retrieved = store.get(id).await.unwrap();

    assert_eq!(retrieved.kind, MessageKind::Sms);
    match &retrieved.content {
        MessageContent::Sms(c) => {
            assert_eq!(c.from, "+1111");
            assert_eq!(c.to, "+2222");
            assert_eq!(c.body, "Hey");
            assert_eq!(c.metadata.get("key").unwrap(), "val");
        }
        _ => panic!("Expected SMS content"),
    }
}

#[tokio::test]
async fn test_get_nonexistent_message_returns_not_found() {
    let (_tmp, store, _cfg) = setup();
    let fake_id = uuid::Uuid::new_v4();
    let result = store.get(fake_id).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("not found"),
        "Error should mention not found: {}",
        err
    );
}

#[tokio::test]
async fn test_delete_message() {
    let (_tmp, store, _cfg) = setup();

    let msg = ShuntedMessage::new_sms(SmsContent {
        from: "a".into(),
        to: "b".into(),
        body: "c".into(),
        metadata: HashMap::new(),
    });
    let id = msg.id;
    store.store(&msg).await.unwrap();

    store.delete(id).await.unwrap();
    assert!(store.get(id).await.is_err());
}

#[tokio::test]
async fn test_delete_nonexistent_returns_not_found() {
    let (_tmp, store, _cfg) = setup();
    let result = store.delete(uuid::Uuid::new_v4()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_clear_removes_all_messages() {
    let (_tmp, store, _cfg) = setup();

    for i in 0..5 {
        let msg = ShuntedMessage::new_sms(SmsContent {
            from: format!("from{}", i),
            to: format!("to{}", i),
            body: format!("body{}", i),
            metadata: HashMap::new(),
        });
        store.store(&msg).await.unwrap();
    }

    assert_eq!(store.list().await.unwrap().len(), 5);
    store.clear().await.unwrap();
    assert_eq!(store.list().await.unwrap().len(), 0);
}

#[tokio::test]
async fn test_list_returns_newest_first() {
    let (_tmp, store, _cfg) = setup();

    let mut ids = Vec::new();
    for i in 0..3 {
        let msg = ShuntedMessage::new_sms(SmsContent {
            from: "f".into(),
            to: "t".into(),
            body: format!("msg{}", i),
            metadata: HashMap::new(),
        });
        ids.push(msg.id);
        store.store(&msg).await.unwrap();
        // Tiny sleep to ensure distinct timestamps
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    let listed = store.list().await.unwrap();
    assert_eq!(listed.len(), 3);
    // Newest first
    assert_eq!(listed[0].id, ids[2]);
    assert_eq!(listed[2].id, ids[0]);
}

#[tokio::test]
async fn test_list_on_empty_store() {
    let (_tmp, store, _cfg) = setup();
    let msgs = store.list().await.unwrap();
    assert!(msgs.is_empty());
}

#[tokio::test]
async fn test_store_message_with_unicode_content() {
    let (_tmp, store, _cfg) = setup();

    let email = EmailContent {
        from: "田中@example.jp".into(),
        to: vec!["محمد@example.sa".into()],
        cc: vec![],
        bcc: vec![],
        subject: "こんにちは 🌍".into(),
        text_body: Some("مرحبا 你好 한국어 Ñoño".into()),
        html_body: Some("<p>こんにちは 🌍</p>".into()),
        headers: HashMap::new(),
        attachments: vec![],
    };
    let msg = ShuntedMessage::new_email(email);
    let id = msg.id;

    store.store(&msg).await.unwrap();
    let retrieved = store.get(id).await.unwrap();

    match &retrieved.content {
        MessageContent::Email(c) => {
            assert_eq!(c.subject, "こんにちは 🌍");
            assert_eq!(c.text_body.as_deref(), Some("مرحبا 你好 한국어 Ñoño"));
            assert_eq!(c.from, "田中@example.jp");
        }
        _ => panic!("Expected email"),
    }
}

#[tokio::test]
async fn test_store_message_with_empty_fields() {
    let (_tmp, store, _cfg) = setup();

    let email = EmailContent {
        from: "".into(),
        to: vec![],
        cc: vec![],
        bcc: vec![],
        subject: "".into(),
        text_body: None,
        html_body: None,
        headers: HashMap::new(),
        attachments: vec![],
    };
    let msg = ShuntedMessage::new_email(email);
    let id = msg.id;

    store.store(&msg).await.unwrap();
    let retrieved = store.get(id).await.unwrap();
    match &retrieved.content {
        MessageContent::Email(c) => {
            assert_eq!(c.from, "");
            assert!(c.to.is_empty());
            assert!(c.text_body.is_none());
        }
        _ => panic!("Expected email"),
    }
}

#[tokio::test]
async fn test_store_message_with_large_body() {
    let (_tmp, store, _cfg) = setup();

    let large_body = "X".repeat(1_000_000); // 1MB body
    let email = EmailContent {
        from: "big@test.com".into(),
        to: vec!["dest@test.com".into()],
        cc: vec![],
        bcc: vec![],
        subject: "Large email".into(),
        text_body: Some(large_body.clone()),
        html_body: None,
        headers: HashMap::new(),
        attachments: vec![],
    };
    let msg = ShuntedMessage::new_email(email);
    let id = msg.id;

    store.store(&msg).await.unwrap();
    let retrieved = store.get(id).await.unwrap();
    match &retrieved.content {
        MessageContent::Email(c) => {
            assert_eq!(c.text_body.as_ref().unwrap().len(), 1_000_000);
        }
        _ => panic!("Expected email"),
    }
}

#[tokio::test]
async fn test_store_many_messages_concurrently() {
    let (_tmp, store, _cfg) = setup();
    let store = store.clone();

    let mut handles = Vec::new();
    for i in 0..50 {
        let s = store.clone();
        handles.push(tokio::spawn(async move {
            let msg = ShuntedMessage::new_sms(SmsContent {
                from: format!("from{}", i),
                to: format!("to{}", i),
                body: format!("concurrent msg {}", i),
                metadata: HashMap::new(),
            });
            s.store(&msg).await.unwrap();
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let all = store.list().await.unwrap();
    assert_eq!(all.len(), 50);
}

// ===========================================================================
// 2. EMAIL TRANSPORT TESTS — lettre integration
// ===========================================================================

#[tokio::test]
async fn test_shunt_plain_text_email() {
    let (_tmp, store, config) = setup();
    let transport = ShuntEmailTransport::new(store.clone(), config);

    let email = Message::builder()
        .from("sender@test.com".parse().unwrap())
        .to("recipient@test.com".parse().unwrap())
        .subject("Plain text test")
        .header(ContentType::TEXT_PLAIN)
        .body("Hello, this is plain text.".to_string())
        .unwrap();

    let resp = transport.send(email).await.unwrap();
    let stored = store.get(resp.message_id).await.unwrap();

    assert_eq!(stored.kind, MessageKind::Email);
    match &stored.content {
        MessageContent::Email(c) => {
            assert_eq!(c.subject, "Plain text test");
            assert!(c.text_body.as_ref().unwrap().contains("Hello, this is plain text."));
            assert_eq!(c.to, vec!["recipient@test.com"]);
            assert_eq!(c.from, "sender@test.com");
        }
        _ => panic!("Expected email"),
    }
}

#[tokio::test]
async fn test_shunt_html_email() {
    let (_tmp, store, config) = setup();
    let transport = ShuntEmailTransport::new(store.clone(), config);

    let email = Message::builder()
        .from("sender@test.com".parse().unwrap())
        .to("recipient@test.com".parse().unwrap())
        .subject("HTML test")
        .multipart(
            MultiPart::alternative()
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_PLAIN)
                        .body("Plain version".to_string()),
                )
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_HTML)
                        .body("<h1>HTML version</h1>".to_string()),
                ),
        )
        .unwrap();

    let resp = transport.send(email).await.unwrap();
    let stored = store.get(resp.message_id).await.unwrap();

    match &stored.content {
        MessageContent::Email(c) => {
            assert!(c.text_body.as_ref().unwrap().contains("Plain version"));
            assert!(c.html_body.as_ref().unwrap().contains("<h1>HTML version</h1>"));
        }
        _ => panic!("Expected email"),
    }
}

#[tokio::test]
async fn test_shunt_email_with_multiple_recipients() {
    let (_tmp, store, config) = setup();
    let transport = ShuntEmailTransport::new(store.clone(), config);

    let email = Message::builder()
        .from("sender@test.com".parse().unwrap())
        .to("a@test.com".parse().unwrap())
        .to("b@test.com".parse().unwrap())
        .cc("cc1@test.com".parse().unwrap())
        .cc("cc2@test.com".parse().unwrap())
        .subject("Multi-recipient")
        .header(ContentType::TEXT_PLAIN)
        .body("Hello team".to_string())
        .unwrap();

    let resp = transport.send(email).await.unwrap();
    let stored = store.get(resp.message_id).await.unwrap();

    match &stored.content {
        MessageContent::Email(c) => {
            assert!(c.to.len() >= 2, "Should have at least 2 To recipients: {:?}", c.to);
            assert!(c.cc.len() >= 2, "Should have at least 2 CC recipients: {:?}", c.cc);
        }
        _ => panic!("Expected email"),
    }
}

#[tokio::test]
async fn test_shunt_email_with_unicode_subject() {
    let (_tmp, store, config) = setup();
    let transport = ShuntEmailTransport::new(store.clone(), config);

    let email = Message::builder()
        .from("sender@test.com".parse().unwrap())
        .to("recipient@test.com".parse().unwrap())
        .subject("日本語テスト 🎉")
        .header(ContentType::TEXT_PLAIN)
        .body("Unicode subject test".to_string())
        .unwrap();

    let resp = transport.send(email).await.unwrap();
    let stored = store.get(resp.message_id).await.unwrap();

    match &stored.content {
        MessageContent::Email(c) => {
            assert!(
                c.subject.contains("日本語テスト") || c.subject.contains("=?"),
                "Subject should contain Japanese text or be encoded: {}",
                c.subject
            );
        }
        _ => panic!("Expected email"),
    }
}

#[tokio::test]
async fn test_shunt_email_preview_url_contains_message_id() {
    let (_tmp, store, config) = setup();
    let transport = ShuntEmailTransport::new(store.clone(), config);

    let email = Message::builder()
        .from("a@test.com".parse().unwrap())
        .to("b@test.com".parse().unwrap())
        .subject("URL test")
        .header(ContentType::TEXT_PLAIN)
        .body("test".to_string())
        .unwrap();

    let resp = transport.send(email).await.unwrap();
    assert!(
        resp.preview_url.contains(&resp.message_id.to_string()),
        "Preview URL should contain message ID: {}",
        resp.preview_url
    );
}

#[tokio::test]
async fn test_send_multiple_emails_sequentially() {
    let (_tmp, store, config) = setup();
    let transport = ShuntEmailTransport::new(store.clone(), config);

    let mut ids = Vec::new();
    for i in 0..10 {
        let email = Message::builder()
            .from("sender@test.com".parse().unwrap())
            .to("recipient@test.com".parse().unwrap())
            .subject(format!("Email #{}", i))
            .header(ContentType::TEXT_PLAIN)
            .body(format!("Body #{}", i))
            .unwrap();

        let resp = transport.send(email).await.unwrap();
        ids.push(resp.message_id);
    }

    let all = store.list().await.unwrap();
    assert_eq!(all.len(), 10);

    // All IDs should be unique
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), 10);
}

// ===========================================================================
// 3. SMS INTERCEPTOR TESTS
// ===========================================================================

#[tokio::test]
async fn test_sms_interceptor_stores_message() {
    let (_tmp, store, config) = setup();
    let interceptor = SmsInterceptor::new(store.clone(), config);

    interceptor
        .send_sms("+1111", "+2222", "Hello SMS", HashMap::new())
        .await
        .unwrap();

    let msgs = store.list().await.unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].kind, MessageKind::Sms);

    match &msgs[0].content {
        MessageContent::Sms(c) => {
            assert_eq!(c.from, "+1111");
            assert_eq!(c.to, "+2222");
            assert_eq!(c.body, "Hello SMS");
        }
        _ => panic!("Expected SMS"),
    }
}

#[tokio::test]
async fn test_sms_with_metadata() {
    let (_tmp, store, config) = setup();
    let interceptor = SmsInterceptor::new(store.clone(), config);

    let meta = HashMap::from([
        ("campaign_id".to_string(), "abc123".to_string()),
        ("priority".to_string(), "high".to_string()),
        ("empty_value".to_string(), "".to_string()),
    ]);

    interceptor
        .send_sms("+1111", "+2222", "With metadata", meta)
        .await
        .unwrap();

    let msgs = store.list().await.unwrap();
    match &msgs[0].content {
        MessageContent::Sms(c) => {
            assert_eq!(c.metadata.get("campaign_id").unwrap(), "abc123");
            assert_eq!(c.metadata.get("priority").unwrap(), "high");
            assert_eq!(c.metadata.get("empty_value").unwrap(), "");
            assert_eq!(c.metadata.len(), 3);
        }
        _ => panic!("Expected SMS"),
    }
}

#[tokio::test]
async fn test_sms_with_unicode_body() {
    let (_tmp, store, config) = setup();
    let interceptor = SmsInterceptor::new(store.clone(), config);

    let body = "🚗 Your driver محمد is arriving! ETA: 5分 한국어テスト";
    interceptor
        .send_sms("+1111", "+2222", body, HashMap::new())
        .await
        .unwrap();

    let msgs = store.list().await.unwrap();
    match &msgs[0].content {
        MessageContent::Sms(c) => {
            assert_eq!(c.body, body);
        }
        _ => panic!("Expected SMS"),
    }
}

#[tokio::test]
async fn test_sms_with_empty_body() {
    let (_tmp, store, config) = setup();
    let interceptor = SmsInterceptor::new(store.clone(), config);

    interceptor
        .send_sms("+1111", "+2222", "", HashMap::new())
        .await
        .unwrap();

    let msgs = store.list().await.unwrap();
    match &msgs[0].content {
        MessageContent::Sms(c) => assert_eq!(c.body, ""),
        _ => panic!("Expected SMS"),
    }
}

#[tokio::test]
async fn test_sms_with_very_long_body() {
    let (_tmp, store, config) = setup();
    let interceptor = SmsInterceptor::new(store.clone(), config);

    let body = "A".repeat(10_000);
    interceptor
        .send_sms("+1111", "+2222", &body, HashMap::new())
        .await
        .unwrap();

    let msgs = store.list().await.unwrap();
    match &msgs[0].content {
        MessageContent::Sms(c) => assert_eq!(c.body.len(), 10_000),
        _ => panic!("Expected SMS"),
    }
}

#[tokio::test]
async fn test_concurrent_sms_sends() {
    let (_tmp, store, config) = setup();

    let mut handles = Vec::new();
    for i in 0..30 {
        let interceptor = SmsInterceptor::new(store.clone(), config.clone());
        handles.push(tokio::spawn(async move {
            interceptor
                .send_sms(
                    &format!("+1{:010}", i),
                    &format!("+2{:010}", i),
                    &format!("Concurrent SMS {}", i),
                    HashMap::new(),
                )
                .await
                .unwrap();
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    assert_eq!(store.list().await.unwrap().len(), 30);
}

// ===========================================================================
// 4. WEB API TESTS — HTTP endpoint integration
// ===========================================================================

#[tokio::test]
async fn test_web_index_returns_html() {
    let (_tmp, store, config) = setup();
    let (base_url, _handle) = start_test_server(store, config).await;

    let client = Client::new();
    let resp = client.get(&base_url).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("Shunt"), "Index should contain 'Shunt': {}", &body[..100]);
    assert!(body.contains("</html>"));
}

#[tokio::test]
async fn test_web_list_messages_empty() {
    let (_tmp, store, config) = setup();
    let (base_url, _handle) = start_test_server(store, config).await;

    let client = Client::new();
    let resp = client
        .get(format!("{}/messages", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let json: Value = resp.json().await.unwrap();
    assert_eq!(json["meta"]["total"], 0);
    assert!(json["data"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_web_list_after_storing_messages() {
    let (_tmp, store, config) = setup();

    // Store some messages first
    for i in 0..3 {
        let msg = ShuntedMessage::new_sms(SmsContent {
            from: format!("from{}", i),
            to: format!("to{}", i),
            body: format!("body{}", i),
            metadata: HashMap::new(),
        });
        store.store(&msg).await.unwrap();
    }

    let (base_url, _handle) = start_test_server(store, config).await;

    let client = Client::new();
    let resp = client
        .get(format!("{}/messages", base_url))
        .send()
        .await
        .unwrap();
    let json: Value = resp.json().await.unwrap();

    assert_eq!(json["meta"]["total"], 3);
    assert_eq!(json["data"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn test_web_get_single_message() {
    let (_tmp, store, config) = setup();

    let msg = ShuntedMessage::new_email(EmailContent {
        from: "web@test.com".into(),
        to: vec!["dest@test.com".into()],
        cc: vec![],
        bcc: vec![],
        subject: "Web test".into(),
        text_body: Some("Body".into()),
        html_body: None,
        headers: HashMap::new(),
        attachments: vec![],
    });
    let id = msg.id;
    store.store(&msg).await.unwrap();

    let (base_url, _handle) = start_test_server(store, config).await;

    let client = Client::new();
    let resp = client
        .get(format!("{}/messages/{}", base_url, id))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let json: Value = resp.json().await.unwrap();
    assert_eq!(json["data"]["id"], id.to_string());
    assert_eq!(json["data"]["kind"], "email");
}

#[tokio::test]
async fn test_web_get_nonexistent_message_returns_404() {
    let (_tmp, store, config) = setup();
    let (base_url, _handle) = start_test_server(store, config).await;

    let client = Client::new();
    let fake_id = uuid::Uuid::new_v4();
    let resp = client
        .get(format!("{}/messages/{}", base_url, fake_id))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    let json: Value = resp.json().await.unwrap();
    assert_eq!(json["error"]["code"], "RESOURCE_NOT_FOUND");
}

#[tokio::test]
async fn test_web_get_with_invalid_uuid_returns_error() {
    let (_tmp, store, config) = setup();
    let (base_url, _handle) = start_test_server(store, config).await;

    let client = Client::new();
    let resp = client
        .get(format!("{}/messages/not-a-uuid", base_url))
        .send()
        .await
        .unwrap();
    // Axum returns 400 or 422 for path parse errors
    assert!(
        resp.status().is_client_error(),
        "Should be client error, got {}",
        resp.status()
    );
}

// ===========================================================================
// 5. NOTIFICATION SERVICE TESTS (email + SMS through shunt)
// ===========================================================================

#[tokio::test]
async fn test_notification_service_welcome_email() {
    let (_tmp, store, config) = setup();
    let svc = NotificationService::new(store.clone(), config);

    let id = svc
        .send_welcome_email("user@test.com", "Alice")
        .await
        .unwrap();

    let msg = store.get(id).await.unwrap();
    assert_eq!(msg.kind, MessageKind::Email);
    match &msg.content {
        MessageContent::Email(c) => {
            assert!(c.subject.contains("Alice"));
            assert!(c.text_body.as_ref().unwrap().contains("Alice"));
            assert_eq!(c.to, vec!["user@test.com"]);
        }
        _ => panic!("Expected email"),
    }
}

#[tokio::test]
async fn test_notification_service_html_email() {
    let (_tmp, store, config) = setup();
    let svc = NotificationService::new(store.clone(), config);

    let id = svc
        .send_html_email(
            "user@test.com",
            "Report Ready",
            "Your report is ready",
            "<h1>Report</h1><p>Your report is ready</p>",
        )
        .await
        .unwrap();

    let msg = store.get(id).await.unwrap();
    match &msg.content {
        MessageContent::Email(c) => {
            assert!(c.text_body.as_ref().unwrap().contains("report is ready"));
            assert!(c.html_body.as_ref().unwrap().contains("<h1>Report</h1>"));
        }
        _ => panic!("Expected email"),
    }
}

#[tokio::test]
async fn test_notification_service_team_email() {
    let (_tmp, store, config) = setup();
    let svc = NotificationService::new(store.clone(), config);

    let id = svc
        .send_team_email(
            &["alice@test.com", "bob@test.com"],
            &["manager@test.com"],
            "Sprint Review",
            "Team meeting at 3pm",
        )
        .await
        .unwrap();

    let msg = store.get(id).await.unwrap();
    match &msg.content {
        MessageContent::Email(c) => {
            assert!(c.to.contains(&"alice@test.com".to_string()));
            assert!(c.to.contains(&"bob@test.com".to_string()));
            assert!(c.cc.contains(&"manager@test.com".to_string()));
        }
        _ => panic!("Expected email"),
    }
}

#[tokio::test]
async fn test_notification_service_verification_sms() {
    let (_tmp, store, config) = setup();
    let svc = NotificationService::new(store.clone(), config);

    svc.send_verification_sms("+15551234567", "847291")
        .await
        .unwrap();

    let msgs = store.list().await.unwrap();
    assert_eq!(msgs.len(), 1);
    match &msgs[0].content {
        MessageContent::Sms(c) => {
            assert!(c.body.contains("847291"));
            assert_eq!(c.to, "+15551234567");
            assert_eq!(c.metadata.get("type").unwrap(), "verification");
            assert_eq!(c.metadata.get("code").unwrap(), "847291");
        }
        _ => panic!("Expected SMS"),
    }
}

#[tokio::test]
async fn test_notification_service_ride_sms() {
    let (_tmp, store, config) = setup();
    let svc = NotificationService::new(store.clone(), config);

    svc.send_ride_sms("+15551234567", "Dave", 5).await.unwrap();

    let msgs = store.list().await.unwrap();
    match &msgs[0].content {
        MessageContent::Sms(c) => {
            assert!(c.body.contains("Dave"));
            assert!(c.body.contains("5 minutes"));
            assert_eq!(c.metadata.get("type").unwrap(), "ride_update");
        }
        _ => panic!("Expected SMS"),
    }
}

// ===========================================================================
// 6. FABRICATE + SHUNT INTEGRATION — factory-generated data through shunt
// ===========================================================================

#[tokio::test]
async fn test_fabricate_user_generates_welcome_email() {
    let (_tmp, store, config) = setup();
    let svc = NotificationService::new(store.clone(), config);
    let mut ctx = test_context();

    let user = FactoryBuilder::new(UserFactory::new())
        .with_trait("verified")
        .build(&mut ctx)
        .unwrap();

    let id = svc
        .send_welcome_email(&user.email, &user.full_name)
        .await
        .unwrap();

    let msg = store.get(id).await.unwrap();
    match &msg.content {
        MessageContent::Email(c) => {
            assert!(c.subject.contains(&user.full_name));
            assert_eq!(c.to, vec![user.email.clone()]);
        }
        _ => panic!("Expected email"),
    }
}

#[tokio::test]
async fn test_fabricate_user_with_override_email() {
    let (_tmp, store, config) = setup();
    let svc = NotificationService::new(store.clone(), config);
    let mut ctx = test_context();

    let user = FactoryBuilder::new(UserFactory::new())
        .set("email", serde_json::json!("custom+tag@example.com"))
        .set("full_name", serde_json::json!("Custom User"))
        .build(&mut ctx)
        .unwrap();

    assert_eq!(user.email, "custom+tag@example.com");
    assert_eq!(user.full_name, "Custom User");

    let id = svc
        .send_welcome_email(&user.email, &user.full_name)
        .await
        .unwrap();

    let msg = store.get(id).await.unwrap();
    match &msg.content {
        MessageContent::Email(c) => {
            assert_eq!(c.to, vec!["custom+tag@example.com"]);
            assert!(c.subject.contains("Custom User"));
        }
        _ => panic!("Expected email"),
    }
}

#[tokio::test]
async fn test_fabricate_japanese_user_receives_email() {
    let (_tmp, store, config) = setup();
    let svc = NotificationService::new(store.clone(), config);
    let mut ctx = test_context();

    let user = FactoryBuilder::new(UserFactory::new())
        .with_trait("japanese")
        .build(&mut ctx)
        .unwrap();

    assert_eq!(user.full_name, "田中太郎");
    assert_eq!(user.locale, "ja");

    let id = svc
        .send_welcome_email(&user.email, &user.full_name)
        .await
        .unwrap();

    let msg = store.get(id).await.unwrap();
    match &msg.content {
        MessageContent::Email(c) => {
            // Subject may be MIME-encoded for non-ASCII
            assert!(
                c.subject.contains("田中太郎") || c.subject.contains("=?"),
                "Subject should contain Japanese name or be encoded: {}",
                c.subject
            );
        }
        _ => panic!("Expected email"),
    }
}

#[tokio::test]
async fn test_fabricate_arabic_user_receives_sms() {
    let (_tmp, store, config) = setup();
    let svc = NotificationService::new(store.clone(), config);
    let mut ctx = test_context();

    let user = FactoryBuilder::new(UserFactory::new())
        .with_trait("arabic")
        .build(&mut ctx)
        .unwrap();

    assert_eq!(user.full_name, "محمد أحمد");

    svc.send_ride_sms(&user.phone, &user.full_name, 3)
        .await
        .unwrap();

    let msgs = store.list().await.unwrap();
    match &msgs[0].content {
        MessageContent::Sms(c) => {
            assert!(c.body.contains("محمد أحمد"));
            assert_eq!(c.to, user.phone);
        }
        _ => panic!("Expected SMS"),
    }
}

#[tokio::test]
async fn test_fabricate_emoji_name_user_email() {
    let (_tmp, store, config) = setup();
    let svc = NotificationService::new(store.clone(), config);
    let mut ctx = test_context();

    let user = FactoryBuilder::new(UserFactory::new())
        .with_trait("emoji_name")
        .build(&mut ctx)
        .unwrap();

    assert!(user.full_name.contains("😀"));

    let id = svc
        .send_welcome_email(&user.email, &user.full_name)
        .await
        .unwrap();

    let msg = store.get(id).await.unwrap();
    match &msg.content {
        MessageContent::Email(c) => {
            assert!(c.text_body.as_ref().unwrap().contains(&user.full_name));
        }
        _ => panic!("Expected email"),
    }
}

#[tokio::test]
async fn test_fabricate_long_name_user_email() {
    let (_tmp, store, config) = setup();
    let svc = NotificationService::new(store.clone(), config);
    let mut ctx = test_context();

    let user = FactoryBuilder::new(UserFactory::new())
        .with_trait("long_name")
        .build(&mut ctx)
        .unwrap();

    assert_eq!(user.full_name.len(), 500);

    let id = svc
        .send_welcome_email(&user.email, &user.full_name)
        .await
        .unwrap();

    let msg = store.get(id).await.unwrap();
    assert_eq!(msg.kind, MessageKind::Email);
}

#[tokio::test]
async fn test_fabricate_special_email_user() {
    let (_tmp, store, config) = setup();
    let svc = NotificationService::new(store.clone(), config);
    let mut ctx = test_context();

    let user = FactoryBuilder::new(UserFactory::new())
        .with_trait("special_email")
        .build(&mut ctx)
        .unwrap();

    assert_eq!(user.email, "user+tag@sub.domain.example.com");

    let id = svc
        .send_welcome_email(&user.email, &user.full_name)
        .await
        .unwrap();

    let msg = store.get(id).await.unwrap();
    match &msg.content {
        MessageContent::Email(c) => {
            assert_eq!(c.to, vec!["user+tag@sub.domain.example.com"]);
        }
        _ => panic!("Expected email"),
    }
}

#[tokio::test]
async fn test_fabricate_multiple_users_bulk_notifications() {
    let (_tmp, store, config) = setup();
    let svc = NotificationService::new(store.clone(), config);
    let mut ctx = test_context();

    // Generate 20 users with various traits
    let trait_combos: Vec<Vec<&str>> = vec![
        vec!["verified"],
        vec!["driver"],
        vec!["admin"],
        vec!["japanese"],
        vec!["arabic"],
        vec!["emoji_name"],
        vec!["long_name"],
        vec!["special_email"],
        vec!["verified", "driver"],
        vec!["unverified"],
    ];

    let mut users = Vec::new();
    for combo in &trait_combos {
        let mut builder = FactoryBuilder::new(UserFactory::new());
        for t in combo {
            builder = builder.with_trait(t);
        }
        users.push(builder.build(&mut ctx).unwrap());
    }
    // Generate 10 more with defaults
    for _ in 0..10 {
        users.push(FactoryBuilder::new(UserFactory::new()).build(&mut ctx).unwrap());
    }

    assert_eq!(users.len(), 20);

    // Send email to each user
    for user in &users {
        svc.send_welcome_email(&user.email, &user.full_name)
            .await
            .unwrap();
    }
    // Send SMS to each user
    for user in &users {
        svc.send_verification_sms(&user.phone, "123456")
            .await
            .unwrap();
    }

    let all = store.list().await.unwrap();
    assert_eq!(all.len(), 40); // 20 emails + 20 SMS

    let email_count = all.iter().filter(|m| m.kind == MessageKind::Email).count();
    let sms_count = all.iter().filter(|m| m.kind == MessageKind::Sms).count();
    assert_eq!(email_count, 20);
    assert_eq!(sms_count, 20);
}

#[tokio::test]
async fn test_fabricate_notification_with_xss_body_through_shunt() {
    let (_tmp, store, config) = setup();
    let svc = NotificationService::new(store.clone(), config);
    let mut ctx = test_context();

    let notif = FactoryBuilder::new(NotificationFactory::new())
        .with_trait("xss")
        .build(&mut ctx)
        .unwrap();

    // The XSS payload should be stored faithfully (no sanitization at storage level)
    let id = svc
        .send_html_email(
            &notif.to,
            notif.subject.as_deref().unwrap_or("XSS Test"),
            &notif.body,
            notif.html_body.as_deref().unwrap_or(""),
        )
        .await
        .unwrap();

    let msg = store.get(id).await.unwrap();
    match &msg.content {
        MessageContent::Email(c) => {
            // Verify the content is stored as-is (shunt is a dev tool, not a sanitizer)
            assert!(c.text_body.as_ref().unwrap().contains("<script>"));
        }
        _ => panic!("Expected email"),
    }
}

#[tokio::test]
async fn test_fabricate_notification_with_unicode_through_web_api() {
    let (_tmp, store, config) = setup();
    let svc = NotificationService::new(store.clone(), config.clone());
    let mut ctx = test_context();

    let notif = FactoryBuilder::new(NotificationFactory::new())
        .with_trait("unicode")
        .build(&mut ctx)
        .unwrap();

    svc.send_verification_sms(&ctx.phone(), &notif.body)
        .await
        .unwrap();

    let (base_url, _handle) = start_test_server(store, config).await;

    let client = Client::new();
    let resp = client
        .get(format!("{}/messages", base_url))
        .send()
        .await
        .unwrap();
    let json: Value = resp.json().await.unwrap();

    assert_eq!(json["meta"]["total"], 1);
    let data = &json["data"][0];
    assert_eq!(data["kind"], "sms");
}

#[tokio::test]
async fn test_fabricate_user_nonexistent_trait_returns_error() {
    let mut ctx = test_context();

    let result = FactoryBuilder::new(UserFactory::new())
        .with_trait("nonexistent_trait_that_does_not_exist")
        .build(&mut ctx);

    assert!(result.is_err());
}

// ===========================================================================
// 7. FULL PIPELINE: fabricate → notification → shunt → web API
// ===========================================================================

#[tokio::test]
async fn test_full_pipeline_email_through_web_api() {
    let (_tmp, store, config) = setup();
    let svc = NotificationService::new(store.clone(), config.clone());
    let mut ctx = test_context();

    // 1. Generate user with fabricate
    let user = FactoryBuilder::new(UserFactory::new())
        .with_trait("verified")
        .with_trait("driver")
        .set("full_name", serde_json::json!("Pipeline Test Driver"))
        .build(&mut ctx)
        .unwrap();

    // 2. Send email through notification service (shunted)
    let email_id = svc
        .send_welcome_email(&user.email, &user.full_name)
        .await
        .unwrap();

    // 3. Send SMS through notification service (shunted)
    svc.send_verification_sms(&user.phone, "999888")
        .await
        .unwrap();

    // 4. Start web server and verify via HTTP API
    let (base_url, _handle) = start_test_server(store, config).await;
    let client = Client::new();

    // 4a. List all messages
    let resp = client
        .get(format!("{}/messages", base_url))
        .send()
        .await
        .unwrap();
    let json: Value = resp.json().await.unwrap();
    assert_eq!(json["meta"]["total"], 2);

    // 4b. Get the specific email
    let resp = client
        .get(format!("{}/messages/{}", base_url, email_id))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: Value = resp.json().await.unwrap();
    assert_eq!(json["data"]["kind"], "email");

    let content = &json["data"]["content"];
    assert_eq!(content["type"], "email");
    assert!(content["to"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v.as_str().unwrap() == user.email));
}

#[tokio::test]
async fn test_full_pipeline_mixed_messages_ordering() {
    let (_tmp, store, config) = setup();
    let svc = NotificationService::new(store.clone(), config.clone());
    let mut ctx = test_context();

    // Send alternating emails and SMS
    for i in 0..5 {
        let user = FactoryBuilder::new(UserFactory::new()).build(&mut ctx).unwrap();

        svc.send_welcome_email(&user.email, &user.full_name)
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;

        svc.send_verification_sms(&user.phone, &format!("{:06}", i))
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }

    // Verify via web API
    let (base_url, _handle) = start_test_server(store, config).await;
    let client = Client::new();
    let resp = client
        .get(format!("{}/messages", base_url))
        .send()
        .await
        .unwrap();
    let json: Value = resp.json().await.unwrap();
    assert_eq!(json["meta"]["total"], 10);

    // Verify ordering: newest first
    let data = json["data"].as_array().unwrap();
    for window in data.windows(2) {
        let t1 = window[0]["created_at"].as_str().unwrap();
        let t2 = window[1]["created_at"].as_str().unwrap();
        assert!(t1 >= t2, "Messages should be newest first: {} >= {}", t1, t2);
    }
}

// ===========================================================================
// 8. EDGE CASES & STRESS
// ===========================================================================

#[tokio::test]
async fn test_email_with_attachment_info() {
    let (_tmp, store, _cfg) = setup();

    let email = EmailContent {
        from: "a@test.com".into(),
        to: vec!["b@test.com".into()],
        cc: vec![],
        bcc: vec![],
        subject: "With attachments".into(),
        text_body: Some("See attached".into()),
        html_body: None,
        headers: HashMap::new(),
        attachments: vec![
            AttachmentInfo {
                filename: "report.pdf".into(),
                content_type: "application/pdf".into(),
                size_bytes: 1_048_576,
            },
            AttachmentInfo {
                filename: "image.png".into(),
                content_type: "image/png".into(),
                size_bytes: 256_000,
            },
            AttachmentInfo {
                filename: "日本語ファイル.txt".into(),
                content_type: "text/plain".into(),
                size_bytes: 42,
            },
        ],
    };
    let msg = ShuntedMessage::new_email(email);
    let id = msg.id;

    store.store(&msg).await.unwrap();
    let retrieved = store.get(id).await.unwrap();
    match &retrieved.content {
        MessageContent::Email(c) => {
            assert_eq!(c.attachments.len(), 3);
            assert_eq!(c.attachments[0].filename, "report.pdf");
            assert_eq!(c.attachments[1].size_bytes, 256_000);
            assert_eq!(c.attachments[2].filename, "日本語ファイル.txt");
        }
        _ => panic!("Expected email"),
    }
}

#[tokio::test]
async fn test_store_survives_corrupted_json_file() {
    let tmp = TempDir::new().unwrap();
    let msg_dir = tmp.path().join("messages");
    std::fs::create_dir_all(&msg_dir).unwrap();

    // Write a corrupted JSON file
    std::fs::write(msg_dir.join("bad.json"), "{ not valid json !!!").unwrap();

    let store = FileStore::from_dir(&msg_dir);
    // list() should skip the corrupted file gracefully
    let msgs = store.list().await.unwrap();
    assert!(msgs.is_empty(), "Corrupted file should be skipped");

    // Store a valid message alongside the corrupted one
    let msg = ShuntedMessage::new_sms(SmsContent {
        from: "a".into(),
        to: "b".into(),
        body: "ok".into(),
        metadata: HashMap::new(),
    });
    store.store(&msg).await.unwrap();

    let msgs = store.list().await.unwrap();
    assert_eq!(msgs.len(), 1, "Should list only the valid message");
}

#[tokio::test]
async fn test_serialization_roundtrip_preserves_all_fields() {
    let email = EmailContent {
        from: "full@test.com".into(),
        to: vec!["a@t.com".into(), "b@t.com".into()],
        cc: vec!["cc@t.com".into()],
        bcc: vec!["bcc@t.com".into()],
        subject: "Roundtrip".into(),
        text_body: Some("text".into()),
        html_body: Some("<p>html</p>".into()),
        headers: HashMap::from([
            ("X-Custom".into(), "value".into()),
            ("X-Another".into(), "val2".into()),
        ]),
        attachments: vec![AttachmentInfo {
            filename: "f.txt".into(),
            content_type: "text/plain".into(),
            size_bytes: 100,
        }],
    };

    let msg = ShuntedMessage::new_email(email);
    let json = serde_json::to_string(&msg).unwrap();
    let deserialized: ShuntedMessage = serde_json::from_str(&json).unwrap();

    assert_eq!(msg.id, deserialized.id);
    assert_eq!(msg.kind, deserialized.kind);
    match (&msg.content, &deserialized.content) {
        (MessageContent::Email(a), MessageContent::Email(b)) => {
            assert_eq!(a.from, b.from);
            assert_eq!(a.to, b.to);
            assert_eq!(a.cc, b.cc);
            assert_eq!(a.bcc, b.bcc);
            assert_eq!(a.subject, b.subject);
            assert_eq!(a.text_body, b.text_body);
            assert_eq!(a.html_body, b.html_body);
            assert_eq!(a.headers, b.headers);
            assert_eq!(a.attachments.len(), b.attachments.len());
        }
        _ => panic!("Content type mismatch"),
    }
}

#[tokio::test]
async fn test_sms_serialization_roundtrip() {
    let sms = SmsContent {
        from: "+1".into(),
        to: "+2".into(),
        body: "hello 🌍".into(),
        metadata: HashMap::from([
            ("k1".into(), "v1".into()),
            ("k2".into(), "v2".into()),
        ]),
    };

    let msg = ShuntedMessage::new_sms(sms);
    let json = serde_json::to_string(&msg).unwrap();
    let deserialized: ShuntedMessage = serde_json::from_str(&json).unwrap();

    match (&msg.content, &deserialized.content) {
        (MessageContent::Sms(a), MessageContent::Sms(b)) => {
            assert_eq!(a.from, b.from);
            assert_eq!(a.to, b.to);
            assert_eq!(a.body, b.body);
            assert_eq!(a.metadata, b.metadata);
        }
        _ => panic!("Content type mismatch"),
    }
}

#[tokio::test]
async fn test_config_builder_pattern() {
    let config = ShuntConfig::new()
        .storage_dir("/custom/path")
        .open_browser(false)
        .web_port(3456)
        .web_host("0.0.0.0");

    assert_eq!(config.storage_dir.to_str().unwrap(), "/custom/path");
    assert!(!config.open_browser);
    assert_eq!(config.web_port, 3456);
    assert_eq!(config.web_host, "0.0.0.0");
    assert_eq!(config.web_addr(), "0.0.0.0:3456");
    assert_eq!(config.web_url(), "http://0.0.0.0:3456");
}

#[tokio::test]
async fn test_config_defaults() {
    let config = ShuntConfig::default();
    assert_eq!(config.storage_dir.to_str().unwrap(), "tmp/shunt");
    assert!(config.open_browser);
    assert_eq!(config.web_port, 9876);
    assert_eq!(config.web_host, "127.0.0.1");
}

#[tokio::test]
async fn test_shared_store_between_email_and_sms() {
    let (_tmp, store, config) = setup();

    let email_transport = ShuntEmailTransport::new(store.clone(), config.clone());
    let sms_interceptor = SmsInterceptor::new(store.clone(), config);

    // Send an email
    let email = Message::builder()
        .from("a@test.com".parse().unwrap())
        .to("b@test.com".parse().unwrap())
        .subject("Test")
        .header(ContentType::TEXT_PLAIN)
        .body("test".to_string())
        .unwrap();
    email_transport.send(email).await.unwrap();

    // Send an SMS
    sms_interceptor
        .send_sms("+1", "+2", "test", HashMap::new())
        .await
        .unwrap();

    // Both should be in the same store
    let all = store.list().await.unwrap();
    assert_eq!(all.len(), 2);

    let kinds: Vec<_> = all.iter().map(|m| &m.kind).collect();
    assert!(kinds.contains(&&MessageKind::Email));
    assert!(kinds.contains(&&MessageKind::Sms));
}
