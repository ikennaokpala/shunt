# Testing Strategy

**Project:** shunt
**Date:** 2026-03-14
**Framework:** SPARC (Situation, Problem, Analysis, Recommendation, Conclusion)

---

## Situation

Shunt is a development tool library that intercepts outbound emails and SMS messages, stores them locally, and serves them via a web preview. The library's value proposition depends entirely on correct behaviour across the full pipeline: a message enters through a trait-based adapter, passes through the storage layer, and becomes visible through the REST API and browser UI.

The workspace follows a strict **integration tests only** policy. No unit tests. This is a deliberate architectural choice grounded in the observation that shunt's bugs will almost always be integration bugs: a message that serializes correctly but fails to parse, a stored file that the web server cannot read, an SSE event that fires but contains malformed JSON. Testing individual functions in isolation would miss these failure modes.

The library must be tested without connecting to real SMTP servers, real SMS providers, or real browsers. All external I/O must be handled through controlled interfaces.

---

## Problem

Testing shunt presents several specific challenges:

1. **No real SMTP server:** The email interceptor must be tested using real `lettre::Message` objects, but the test must not require a running SMTP server.

2. **No real SMS provider:** The SMS interceptor must be tested without calling Twilio, AWS SNS, or any other provider.

3. **File system isolation:** Tests that write to the file store must not interfere with each other when running in parallel. Shared mutable file system state is a common source of flaky tests.

4. **HTTP server lifecycle:** Tests that exercise the REST API must start and stop an Axum server, make HTTP requests against it, and clean up afterwards. Server port conflicts between parallel tests must be avoided.

5. **SSE verification:** Tests must verify that the SSE stream emits events when new messages are stored. This requires coordinating between a message-producing task and an event-consuming task.

6. **Cross-crate workflows:** The most valuable tests span multiple crates (e.g., construct a lettre message in `shunt_email`, store it via `shunt_core`, and retrieve it via `shunt_web`). These tests live in the convenience crate or a dedicated integration test crate.

7. **Multipart email complexity:** Emails can be plain text, HTML, multipart (text + HTML), or multipart with attachments. Each variant exercises different parsing paths in `mail-parser` and different rendering paths in the preview.

---

## Analysis

### Test Environment Architecture

```
┌──────────────────────────────────────────────────────┐
│                  Integration Test                     │
│                                                      │
│  1. Create TempDir (isolated store path)             │
│  2. Build FileStore pointing to TempDir              │
│  3. Build EmailInterceptor / SmsInterceptor          │
│  4. Start preview server on random port              │
│  5. Perform actions (send messages)                  │
│  6. Assert via HTTP API (reqwest) or store queries   │
│  7. TempDir drops → cleanup automatic                │
└──────────────────────────────────────────────────────┘
```

### Isolation Strategy

**File system isolation:** Each test creates its own `TempDir` (via the `tempfile` crate), which provides a unique, empty directory. The `FileStore` is configured to write to this directory. When the `TempDir` guard drops at the end of the test, all files are automatically deleted. This eliminates cross-test interference and ensures tests can run in parallel with `cargo test`.

**Network isolation:** Each test that starts a preview server binds to port `0`, letting the OS assign a random available port. The actual bound address is captured from the server's `TcpListener` and used for subsequent HTTP requests. This prevents port conflicts even when many tests run concurrently.

**Temporal isolation:** Tests do not depend on wall-clock time. Where timestamps are relevant, tests assert on ordering or presence rather than exact values.

### Test Categories

#### Category 1: Email Interception Pipeline

Tests that construct a real `lettre::Message`, send it through the `EmailInterceptor`, and verify the resulting `ShuntedMessage` in the store.

| Test Case                          | What It Verifies                                                      |
|------------------------------------|-----------------------------------------------------------------------|
| Plain text email                   | From, to, subject, text body extracted correctly.                     |
| HTML email                         | HTML body extracted; text body may be absent.                         |
| Multipart email (text + HTML)      | Both bodies extracted; neither is lost.                               |
| Email with CC and BCC              | CC and BCC lists populated correctly.                                 |
| Email with attachments             | `AttachmentInfo` records created with correct filename, type, size.   |
| Email with multiple recipients     | `to` field contains all recipients; summary matches.                  |
| Email with non-ASCII subject       | UTF-8 encoded subjects survive the serialize-parse round trip.        |
| Email with custom headers          | Custom headers (e.g., `X-Mailer`) appear in the stored headers list.  |
| Large email body                   | Bodies exceeding typical buffer sizes are stored without truncation.   |

#### Category 2: SMS Interception Pipeline

Tests that call `SmsSender::send()` on the SMS interceptor and verify the resulting `ShuntedMessage`.

| Test Case                          | What It Verifies                                                      |
|------------------------------------|-----------------------------------------------------------------------|
| Basic SMS                          | From, to, body stored correctly.                                      |
| SMS with metadata                  | Provider-specific metadata (encoding, segments) preserved in HashMap. |
| SMS with Unicode body              | Emoji, CJK characters, RTL text survive storage.                      |
| SMS with empty metadata            | Empty metadata HashMap is handled without error.                      |

#### Category 3: Message Storage

Tests that exercise the `MessageStore` trait implementation directly.

| Test Case                          | What It Verifies                                                      |
|------------------------------------|-----------------------------------------------------------------------|
| Store and retrieve                 | `store()` followed by `get()` returns an identical message.           |
| List ordering                      | `list()` returns messages newest-first.                               |
| Delete single                      | `delete()` removes the specified message; others remain.              |
| Clear all                          | `clear()` removes all messages; `list()` returns empty.               |
| Get nonexistent                    | `get()` with unknown UUID returns `None`, not an error.               |
| Concurrent writes                  | Multiple `store()` calls from different tasks do not corrupt data.    |
| Persistence across instances       | A new `FileStore` pointed at the same directory reads previously stored messages. |

#### Category 4: Web Preview API

Tests that start the Axum server, make HTTP requests via `reqwest`, and verify responses.

| Test Case                          | What It Verifies                                                      |
|------------------------------------|-----------------------------------------------------------------------|
| List empty                         | `GET /api/v1/messages` returns `200` with empty array.                |
| List with messages                 | After storing messages, list returns summaries with correct fields.   |
| Get single message                 | `GET /api/v1/messages/:id` returns full content.                      |
| Get nonexistent message            | `GET /api/v1/messages/:id` with unknown ID returns `404`.             |
| Delete message                     | `DELETE /api/v1/messages/:id` returns `204`; message is gone.         |
| Clear messages                     | `DELETE /api/v1/messages` returns `204`; list is empty.               |
| Serve embedded UI                  | `GET /` returns `200` with `text/html` content type.                  |
| Attachment download                | `GET /api/v1/messages/:id/attachments/:filename` returns file bytes.  |
| CORS headers                       | Responses include appropriate `Access-Control-Allow-Origin` headers.  |

#### Category 5: SSE Live Updates

Tests that verify the Server-Sent Events stream.

| Test Case                          | What It Verifies                                                      |
|------------------------------------|-----------------------------------------------------------------------|
| New message event                  | Storing a message causes an SSE event with the message ID.            |
| Multiple events                    | Storing three messages produces three SSE events in order.            |
| Event format                       | SSE events are valid `text/event-stream` format with JSON data.       |
| Connection resilience              | A new SSE connection receives events for subsequently stored messages.|

#### Category 6: Cross-Crate Workflow

End-to-end tests that span the full pipeline.

| Test Case                          | What It Verifies                                                      |
|------------------------------------|-----------------------------------------------------------------------|
| Email to browser                   | Construct lettre Message → send via interceptor → fetch via REST API → verify HTML body matches. |
| SMS to browser                     | Send via SmsSender → fetch via REST API → verify body matches.        |
| Mixed channel listing              | Store emails and SMS → list via API → verify kind discrimination.     |
| Test assertion workflow            | Store email → query store programmatically → assert subject and recipients. |

### Test Dependencies

| Dependency    | Purpose                                          | Scope          |
|--------------|--------------------------------------------------|----------------|
| `tempfile`   | Create isolated temporary directories per test.  | `[dev-dependencies]` |
| `reqwest`    | Make HTTP requests to the preview server.         | `[dev-dependencies]` |
| `tokio`      | Async test runtime (`#[tokio::test]`).            | `[dev-dependencies]` |
| `lettre`     | Construct real `Message` objects for email tests. | `[dev-dependencies]` of `shunt_email` |

---

## Recommendation

### Test Organisation

```
crates/
├── shunt_core/
│   └── tests/
│       └── store_tests.rs           # Category 3: MessageStore tests
├── shunt_email/
│   └── tests/
│       └── email_interceptor.rs     # Category 1: Email pipeline tests
├── shunt_sms/
│   └── tests/
│       └── sms_interceptor.rs       # Category 2: SMS pipeline tests
├── shunt_web/
│   └── tests/
│       ├── api_tests.rs             # Category 4: REST API tests
│       └── sse_tests.rs             # Category 5: SSE tests
└── shunt/                           # Convenience crate
    └── tests/
        └── workflows.rs             # Category 6: Cross-crate workflows
```

### Test Execution

All tests run with `cargo test` from the workspace root. No special setup, no external services, no Docker containers.

```bash
# Run all tests across all crates
cargo test

# Run tests for a specific crate
cargo test -p shunt_core
cargo test -p shunt_email
cargo test -p shunt_web

# Run a specific test by name
cargo test email_with_attachments
```

### Test Patterns

**Standard test setup:**

```rust
#[tokio::test]
async fn shunted_email_appears_in_api() {
    // 1. Create isolated environment
    let dir = TempDir::new().unwrap();
    let store = Arc::new(FileStore::new(dir.path()));
    let interceptor = EmailInterceptor::new(store.clone());

    // 2. Start preview server on random port
    let addr = start_server(store.clone(), 0).await;

    // 3. Construct and send a real lettre message
    let email = Message::builder()
        .from("sender@example.com".parse().unwrap())
        .to("recipient@example.com".parse().unwrap())
        .subject("Test Subject")
        .body("Hello, world!".to_string())
        .unwrap();

    interceptor.send(email).await.unwrap();

    // 4. Verify via HTTP API
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{}/api/v1/messages", addr))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let messages: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["summary"]["subject"], "Test Subject");
}
```

### What Is NOT Tested

In keeping with the integration-only policy, the following are explicitly out of scope:

- **Individual parsing functions in isolation.** Parsing is tested through the full interception pipeline.
- **Serde round-trip of individual types.** Serialization is tested through store-and-retrieve workflows.
- **Axum handler functions in isolation.** Handlers are tested through HTTP requests.
- **Internal helper functions.** Helpers are tested through the public API they support.
- **UI rendering correctness.** The embedded UI is a development convenience; visual correctness is verified manually.

---

## Conclusion

### Test Matrix Summary

| Channel | Variant              | Interception | Storage | API  | SSE  | Cross-Crate |
|---------|---------------------|:------------:|:-------:|:----:|:----:|:-----------:|
| Email   | Plain text           |      x       |    x    |  x   |  x   |      x      |
| Email   | HTML                 |      x       |    x    |  x   |      |      x      |
| Email   | Multipart            |      x       |    x    |  x   |      |             |
| Email   | With attachments     |      x       |    x    |  x   |      |      x      |
| Email   | Multiple recipients  |      x       |    x    |      |      |             |
| Email   | Non-ASCII subject    |      x       |    x    |      |      |             |
| Email   | Custom headers       |      x       |    x    |      |      |             |
| SMS     | Basic                |      x       |    x    |  x   |  x   |      x      |
| SMS     | With metadata        |      x       |    x    |  x   |      |             |
| SMS     | Unicode body         |      x       |    x    |      |      |             |

The testing strategy ensures that shunt's core value proposition -- intercepting, storing, and previewing messages -- is verified through real, end-to-end workflows. Every test exercises the code the same way a developer's application would use it: by calling trait methods, writing to the store, and reading from the HTTP API. No mocks of internal services. No isolated function tests. The tests are the specification.
