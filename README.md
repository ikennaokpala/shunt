# Shunt

**Intercept and preview outbound emails and SMS during development.**

Shunt redirects your outbound messages to a local browser preview instead of sending them. Think of it as Ruby's `letter_opener`, but for Rust — and it handles SMS too.

## Features

- **Email interception** — Drop-in lettre `AsyncTransport` that captures emails
- **SMS interception** — Trait-based SMS capture with `SmsSender` / `SmsInterceptor`
- **Browser preview** — Messages open automatically in your default browser
- **Web UI** — Browse all shunted messages at `http://localhost:9876`
- **Live updates** — New messages appear instantly via Server-Sent Events
- **File storage** — Messages stored as readable JSON files in `tmp/shunt/`
- **Zero config** — Works out of the box with sensible defaults

## Quick Start

### Email (lettre integration)

Add `shunt` as a dev-dependency:

```toml
[dev-dependencies]
shunt = "0.1"
```

Swap your lettre transport for `ShuntEmailTransport` during development:

```rust
use shunt::prelude::*;
use lettre::{AsyncTransport, Message};
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
    transport.send(&email).await?;

    Ok(())
}
```

### SMS

Define your SMS sending behind the `SmsSender` trait, then swap in `SmsInterceptor` for development:

```rust
use shunt::prelude::*;
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
        "Hello from Shunt!",
        HashMap::new(),
    ).await?;

    Ok(())
}
```

### Web Preview Server

Start the preview server to browse all shunted messages:

```rust
use shunt::prelude::*;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ShuntConfig::default();
    let store = Arc::new(FileStore::new(&config));

    // Starts web UI at http://localhost:9876
    start_server(store, config).await?;

    Ok(())
}
```

## Configuration

```rust
let config = ShuntConfig::new()
    .storage_dir("tmp/my_messages")  // Default: "tmp/shunt"
    .open_browser(false)              // Default: true
    .web_port(3000)                   // Default: 9876
    .web_host("0.0.0.0");            // Default: "127.0.0.1"
```

## Architecture

```
shunt (convenience re-export)
├── shunt_core     — Shared types, storage trait, file store
├── shunt_email    — lettre AsyncTransport adapter
├── shunt_sms      — SmsSender trait + interceptor
└── shunt_web      — Axum preview server + embedded UI
```

Use individual crates for fine-grained dependency control:

```toml
[dev-dependencies]
shunt_email = "0.1"  # Only email, no SMS or web
```

## How It Works

1. You send an email/SMS through Shunt's transport/interceptor
2. Shunt parses the message and saves it as a JSON file
3. Your browser opens to preview the message
4. The web UI shows all shunted messages with live updates

## License

MIT
