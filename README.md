# Shunt

[![Crates.io](https://img.shields.io/crates/v/shunt_core.svg)](https://crates.io/crates/shunt_core)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**Intercept and preview outbound emails and SMS during development.**

Shunt redirects your outbound messages to a local browser preview instead of sending them. Think of it as Ruby's [`letter_opener`](https://github.com/ryanb/letter_opener), but for Rust — and it handles SMS too.

## The Problem

During development, you don't want to send real emails or SMS messages. The alternatives are clunky:

- **SMTP servers** like MailHog or MailCrab require running a separate process
- **lettre's `FileTransport`** dumps raw RFC 5322 bytes to disk — not human-readable
- **lettre's `StubTransport`** swallows messages silently — you can't verify content
- **SMS** has no interception story at all in the Rust ecosystem

Shunt solves all of this at the library level. No external processes, no infrastructure. Add it as a dev-dependency, swap your transport, and intercepted messages open right in your browser.

## Features

- **Email interception** — Drop-in lettre `AsyncTransport` that captures emails instead of sending them
- **SMS interception** — Trait-based SMS capture with `SmsSender` / `SmsInterceptor`
- **Browser preview** — Messages open automatically in your default browser
- **Web UI** — Browse all shunted messages at `http://localhost:9876` with a dark-themed dashboard
- **Live updates** — New messages appear instantly via Server-Sent Events (SSE)
- **File storage** — Messages stored as human-readable JSON files in `tmp/shunt/`
- **HTML & plain text** — Preview HTML emails rendered in an iframe, with tabs for plain text and headers
- **SMS bubble view** — SMS messages displayed in a chat-bubble style preview
- **Zero config** — Works out of the box with sensible defaults
- **Modular** — Use the full `shunt-rs` crate or pick individual crates (`shunt_email`, `shunt_sms`, etc.)

## Installation

Add `shunt-rs` as a dev-dependency to your `Cargo.toml`:

```toml
[dev-dependencies]
shunt-rs = "0.1"
```

Or use individual crates for fine-grained dependency control:

```toml
[dev-dependencies]
shunt_email = "0.1"  # Only email interception, no SMS or web
shunt_sms = "0.1"    # Only SMS interception
shunt_web = "0.1"    # Only the preview web server
shunt_core = "0.1"   # Only core types and storage
```

## Quick Start

### Email (lettre integration)

Swap your lettre transport for `ShuntEmailTransport` during development:

```rust
use shunt_rs::prelude::*;
use shunt_rs::lettre::{AsyncTransport, Message};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ShuntConfig::default();
    let store = Arc::new(FileStore::new(&config));
    let transport = ShuntEmailTransport::new(store, config);

    let email = Message::builder()
        .from("sender@example.com".parse()?)
        .to("recipient@example.com".parse()?)
        .subject("Hello from Shunt!")
        .body("This email was shunted to your browser.".to_string())?;

    // Email is saved locally and opened in your browser
    transport.send(email).await?;

    Ok(())
}
```

#### Real-World Pattern: Swap Transport by Environment

```rust
use shunt_rs::prelude::*;
use shunt_rs::lettre::{AsyncTransport, Message, AsyncSmtpTransport, Tokio1Executor};
use std::sync::Arc;

enum AppTransport {
    Smtp(AsyncSmtpTransport<Tokio1Executor>),
    Shunt(ShuntEmailTransport),
}

fn build_transport() -> AppTransport {
    if cfg!(debug_assertions) {
        let config = ShuntConfig::default();
        let store = Arc::new(FileStore::new(&config));
        AppTransport::Shunt(ShuntEmailTransport::new(store, config))
    } else {
        AppTransport::Smtp(
            AsyncSmtpTransport::<Tokio1Executor>::relay("smtp.example.com")
                .unwrap()
                .build()
        )
    }
}
```

### SMS

Define your SMS sending behind the `SmsSender` trait, then swap in `SmsInterceptor` for development:

```rust
use shunt_rs::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ShuntConfig::default();
    let store = Arc::new(FileStore::new(&config));
    let sms = SmsInterceptor::new(store, config);

    // SMS is saved locally and opened in your browser
    sms.send_sms(
        "+1234567890",
        "+0987654321",
        "Your verification code is 123456",
        HashMap::from([
            ("campaign".to_string(), "signup-verification".to_string()),
        ]),
    ).await?;

    Ok(())
}
```

#### Implementing SmsSender for Your Provider

```rust
use shunt_rs::prelude::*;
use std::collections::HashMap;
use async_trait::async_trait;

struct TwilioSender {
    account_sid: String,
    auth_token: String,
}

#[async_trait]
impl SmsSender for TwilioSender {
    type Error = reqwest::Error;

    async fn send_sms(
        &self,
        from: &str,
        to: &str,
        body: &str,
        _metadata: HashMap<String, String>,
    ) -> Result<(), Self::Error> {
        // Your real Twilio API call here
        todo!()
    }
}

// In development, use SmsInterceptor instead of TwilioSender
```

### Web Preview Server

Start the preview server to browse all shunted messages in a web dashboard:

```rust
use shunt_rs::prelude::*;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ShuntConfig::default();
    let store = Arc::new(FileStore::new(&config));

    println!("Shunt preview server running at {}", config.web_url());

    // Starts web UI at http://localhost:9876
    start_server(store, config).await?;

    Ok(())
}
```

The web UI provides:
- A sidebar listing all shunted messages (newest first)
- Email preview with tabs for HTML, plain text, and raw headers
- SMS preview with a chat-bubble style display
- Live updates via SSE — new messages appear without refreshing

## Configuration

All configuration is optional. Defaults work out of the box:

```rust
let config = ShuntConfig::new()
    .storage_dir("tmp/my_messages")  // Default: "tmp/shunt"
    .open_browser(false)              // Default: true (auto-open browser on shunt)
    .web_port(3000)                   // Default: 9876
    .web_host("0.0.0.0");            // Default: "127.0.0.1"
```

| Option | Default | Description |
|--------|---------|-------------|
| `storage_dir` | `tmp/shunt` | Directory where shunted message JSON files are stored |
| `open_browser` | `true` | Automatically open the browser when a message is shunted |
| `web_port` | `9876` | Port for the web preview server |
| `web_host` | `127.0.0.1` | Host for the web preview server |

## Architecture

Shunt is organized as a Cargo workspace with modular crates:

```
shunt-rs (convenience re-export crate)
│
├── shunt_core      Core types, storage trait, and file store
│   ├── ShuntedMessage, EmailContent, SmsContent (domain types)
│   ├── MessageStore trait (async storage abstraction)
│   ├── FileStore (JSON file-based implementation)
│   ├── ShuntConfig (builder-pattern configuration)
│   └── ShuntError (unified error type)
│
├── shunt_email     lettre AsyncTransport adapter
│   ├── ShuntEmailTransport (implements lettre::AsyncTransport)
│   └── parse_email() (RFC 5322 parser via mail-parser)
│
├── shunt_sms       SMS interception
│   ├── SmsSender trait (implement for your SMS provider)
│   └── SmsInterceptor (dev replacement that stores messages)
│
└── shunt_web       Web preview server
    ├── Axum-based HTTP server with REST API
    ├── SSE endpoint for live updates
    └── Embedded single-file HTML/CSS/JS frontend (via rust-embed)
```

### API Endpoints (shunt_web)

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/` | Web preview UI (embedded HTML) |
| `GET` | `/messages` | List all shunted messages (JSON) |
| `GET` | `/messages/{id}` | Get a single message by UUID (JSON) |
| `GET` | `/events` | SSE stream for live message notifications |

### Storage Format

Each shunted message is stored as a JSON file in the configured `storage_dir`:

```
tmp/shunt/
├── 550e8400-e29b-41d4-a716-446655440000.json
├── 6ba7b810-9dad-11d1-80b4-00c04fd430c8.json
└── ...
```

Files are human-readable and contain the full message content, metadata, and timestamps.

## How It Works

```
Your Application                    Shunt                         Browser
      │                               │                              │
      │  send(email)                  │                              │
      ├──────────────────────────────►│                              │
      │                               │  parse RFC 5322 bytes        │
      │                               │  extract HTML/text/headers   │
      │                               │  save as JSON file           │
      │                               │                              │
      │                               │  open::that(preview_url)     │
      │                               ├─────────────────────────────►│
      │                               │                              │  Preview!
      │  Ok(ShuntEmailResponse)       │                              │
      │◄──────────────────────────────┤                              │
```

1. You send an email or SMS through Shunt's transport/interceptor
2. Shunt parses the message (RFC 5322 for emails) and extracts content
3. The message is saved as a human-readable JSON file
4. Your default browser opens to preview the message
5. The web UI shows all shunted messages with live SSE updates

## Crates

| Crate | crates.io | Description |
|-------|-----------|-------------|
| [`shunt_core`](crates/shunt_core) | [![](https://img.shields.io/crates/v/shunt_core.svg)](https://crates.io/crates/shunt_core) | Core types, storage trait, file store |
| [`shunt_email`](crates/shunt_email) | [![](https://img.shields.io/crates/v/shunt_email.svg)](https://crates.io/crates/shunt_email) | lettre `AsyncTransport` adapter |
| [`shunt_sms`](crates/shunt_sms) | [![](https://img.shields.io/crates/v/shunt_sms.svg)](https://crates.io/crates/shunt_sms) | `SmsSender` trait + interceptor |
| [`shunt_web`](crates/shunt_web) | [![](https://img.shields.io/crates/v/shunt_web.svg)](https://crates.io/crates/shunt_web) | Axum preview server + embedded UI |
| [`shunt-rs`](shunt) | [![](https://img.shields.io/crates/v/shunt-rs.svg)](https://crates.io/crates/shunt-rs) | Convenience re-export of all crates |

## Minimum Supported Rust Version (MSRV)

Rust **1.75** or later.

## Inspiration

- [letter_opener](https://github.com/ryanb/letter_opener) — Preview email in the browser (Ruby)
- [notifications_opener](https://github.com/faucet-pipeline/notifications_opener) — Preview SMS/notifications (Ruby)
- [MailCrab](https://github.com/tweedegolf/mailcrab) — Email test server (Rust, but requires separate process)

## Contributing

Contributions are welcome! Please open an issue or submit a pull request.

## License

[MIT](LICENSE)
