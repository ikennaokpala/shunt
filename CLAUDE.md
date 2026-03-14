# Shunt - Project Instructions

## Overview
Shunt is a Rust library that intercepts outbound emails and SMS during development, saving them locally and opening a browser preview. It is the Rust equivalent of Ruby's `letter_opener`.

## Architecture
- **Workspace:** Cargo workspace with 4 library crates + 1 convenience re-export crate
- **shunt_core:** Shared types (`ShuntedMessage`, `EmailContent`, `SmsContent`), storage trait (`MessageStore`), and filesystem implementation (`FileStore`)
- **shunt_email:** lettre `AsyncTransport` implementation that intercepts emails. Uses `mail-parser` to parse RFC 5322 bytes.
- **shunt_sms:** `SmsSender` trait and `SmsInterceptor` implementation
- **shunt_web:** Axum-based preview server with embedded HTML frontend and SSE for live updates
- **shunt:** Convenience crate that re-exports all of the above

## Key Design Decisions
- File-based storage (JSON), no database dependency
- Embedded single-file HTML frontend via rust-embed (no build step)
- SSE for real-time updates (not WebSocket — simpler, one-directional)
- Custom `SmsSender` trait (no standard exists in Rust)
- Integration tests only (no unit tests) per workspace policy

## Testing
- Integration tests only — test full workflows through the public API
- Use `tempdir` for isolated file storage in tests
- Test email interception with real lettre `Message` objects
- Test SMS interception through the `SmsSender` trait
- Test web endpoints with actual HTTP requests

## Dependencies
Key external dependencies:
- `lettre` 0.11 — Email building and transport traits
- `mail-parser` 0.9 — RFC 5322 email parsing
- `axum` 0.8 — Web framework for preview server
- `tokio` 1 — Async runtime
- `serde` / `serde_json` — Serialization
- `rust-embed` 8 — Embed frontend assets
- `open` 5 — Cross-platform browser opening

## Conventions
- All public types are re-exported through the convenience `shunt` crate
- Use builder pattern for configuration (`ShuntConfig`)
- Error types use `thiserror`
- Async-first with `async-trait`
