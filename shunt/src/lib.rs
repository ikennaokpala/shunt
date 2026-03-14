//! # Shunt
//!
//! Intercept and preview outbound emails and SMS during development.
//!
//! Shunt redirects your outbound messages to a local browser preview
//! instead of sending them. Think of it as Ruby's `letter_opener`,
//! but for Rust — and it handles SMS too.
//!
//! ## Quick Start (Email)
//!
//! ```rust,no_run
//! use shunt::prelude::*;
//! use shunt::lettre::{AsyncTransport, Message};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = ShuntConfig::default();
//! let store = std::sync::Arc::new(FileStore::new(&config));
//! let transport = ShuntEmailTransport::new(store, config);
//!
//! let email = Message::builder()
//!     .from("sender@example.com".parse()?)
//!     .to("recipient@example.com".parse()?)
//!     .subject("Hello from Shunt!")
//!     .body("This email was shunted to your browser.".to_string())?;
//!
//! // Email is saved locally and opened in your browser
//! transport.send(email).await?;
//! # Ok(())
//! # }
//! ```

/// Re-export core types and traits.
pub use shunt_core::*;

/// Re-export email transport.
pub use shunt_email::*;

/// Re-export SMS types and interceptor.
pub use shunt_sms::*;

/// Re-export web server.
pub use shunt_web::*;

/// Re-export lettre for convenience.
pub use lettre;

/// Prelude module for convenient imports.
pub mod prelude {
    pub use shunt_core::{
        config::ShuntConfig,
        storage::{FileStore, MessageStore},
        types::*,
    };
    pub use shunt_email::ShuntEmailTransport;
    pub use shunt_sms::{SmsInterceptor, SmsSender};
    pub use shunt_web::start_server;
}
