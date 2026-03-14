# Technical Implementation

**Project:** shunt
**Date:** 2026-03-14
**Framework:** SPARC (Situation, Problem, Analysis, Recommendation, Conclusion)

---

## Situation

The Rust ecosystem has mature libraries for sending emails (`lettre`) and for building web servers (`axum`, `actix-web`), but it lacks a dedicated development-time tool for intercepting outbound messages and previewing them locally. Ruby developers have `letter_opener`, Elixir has `Swoosh` with local adapters, and Node.js has `Ethereal` / `MailHog`. Rust developers currently have no equivalent.

During development, teams working on Rust applications that send emails or SMS messages face a recurring friction point: verifying that the right message, with the right content, reaches the right recipient. Without a preview tool, developers resort to one of several suboptimal approaches:

- Running a local SMTP server (MailHog, MailCatcher) as a separate process.
- Logging email content to stdout and visually inspecting it.
- Sending real emails to a test account and checking manually.
- Writing ad-hoc file-based transports that dump raw RFC 5322 bytes.

Each of these approaches introduces operational complexity, requires context-switching between tools, or provides a poor developer experience (raw MIME is not pleasant to read).

---

## Problem

Rust developers need a way to:

1. **Intercept** outbound emails and SMS messages during development without modifying application logic beyond swapping a transport/sender.
2. **Persist** intercepted messages so they survive process restarts and can be reviewed at any time.
3. **Preview** messages in a browser with proper HTML rendering, attachment handling, and live updates.
4. **Test** against intercepted messages in integration tests (e.g., assert that a signup email was sent with the correct subject).

The solution must satisfy several constraints:

- **Library, not a service:** It should be a Cargo dependency, not a separate daemon or Docker container.
- **Zero configuration:** It should work out of the box with sensible defaults.
- **Minimal footprint:** It should not bloat the dependency tree unnecessarily or increase compile times dramatically.
- **Idiomatic Rust:** It should use traits, async/await, and the type system rather than runtime reflection or macros.
- **lettre compatibility:** It must integrate with lettre, the de facto Rust email crate, without requiring changes to lettre itself.

---

## Analysis

### Architecture Decision: Trait-Based Interception

The most Rust-idiomatic approach to interception is implementing the traits that application code already depends on. For email, this means implementing lettre's `AsyncTransport` trait. For SMS, where no dominant crate exists, this means defining a minimal `SmsSender` trait and providing both a shunt implementation and guidance for adapting production providers.

This trait-based approach has several advantages over alternatives:

| Approach                        | Pros                                         | Cons                                              |
|---------------------------------|----------------------------------------------|---------------------------------------------------|
| **Trait implementation (chosen)** | Zero app-level code changes beyond DI; type-safe; compile-time checked | Must track upstream trait changes (lettre) |
| Separate SMTP server            | Works with any language                      | Requires running a daemon; network overhead       |
| Macro-based interception        | Could be transparent                         | Fragile; hard to debug; poor IDE support          |
| Stdout logging                  | Simple                                       | No persistence; no HTML rendering; no UI          |

### Dependency Analysis

Shunt targets approximately 12 direct external dependencies, chosen for stability and minimal transitive dependency footprint:

| Dependency      | Purpose                              | Crate      | Weight   |
|-----------------|--------------------------------------|------------|----------|
| `lettre`        | `AsyncTransport` trait definition    | shunt_email| Medium   |
| `mail-parser`   | Parse RFC 5322 bytes into structured fields | shunt_email | Light |
| `axum`          | HTTP server for preview UI           | shunt_web  | Medium   |
| `tower`         | Middleware (CORS, compression)       | shunt_web  | Light    |
| `tokio`         | Async runtime                        | All        | Heavy (shared) |
| `serde`         | Serialization/deserialization        | shunt_core | Light    |
| `serde_json`    | JSON file format for storage         | shunt_core | Light    |
| `uuid`          | Message identity                     | shunt_core | Light    |
| `chrono`        | Timestamps                           | shunt_core | Light    |
| `rust-embed`    | Embed UI assets into binary          | shunt_web  | Light    |
| `async-trait`   | Async trait support                  | shunt_core | Light    |
| `thiserror`     | Error type derivation                | shunt_core | Light    |

`tokio` is the heaviest dependency, but any application using `lettre` with async support is already paying this cost. Shunt does not introduce a new runtime requirement.

### Workspace Structure

The Cargo workspace splits functionality into five crates to allow selective dependency:

```
shunt/                          # Workspace root
├── Cargo.toml                  # Workspace manifest
├── crates/
│   ├── shunt_core/             # Shared types, MessageStore trait, FileStore
│   ├── shunt_email/            # lettre AsyncTransport implementation
│   ├── shunt_sms/              # SmsSender trait and interceptor
│   └── shunt_web/              # Axum preview server, REST API, embedded UI
└── shunt/                      # Convenience re-export crate
    └── Cargo.toml
```

This structure means a project that only sends emails and wants browser preview adds two dependencies:

```toml
[dev-dependencies]
shunt_email = "0.1"
shunt_web = "0.1"
```

A project that only needs programmatic access to intercepted messages (e.g., for test assertions) adds one:

```toml
[dev-dependencies]
shunt_email = "0.1"
```

### Interception Pipeline

The data flow from application code to browser preview follows a linear pipeline:

```
Application code
       │
       │  calls transport.send(email) or sender.send(from, to, body)
       ▼
  ┌─────────────┐
  │ Interceptor  │  EmailInterceptor or SmsInterceptor
  │              │  Implements AsyncTransport or SmsSender
  └──────┬───────┘
         │
         │  constructs ShuntedMessage
         ▼
  ┌─────────────┐
  │ MessageStore │  FileStore (default)
  │              │  Writes JSON to .shunt/messages/
  └──────┬───────┘
         │
         │  notifies via broadcast channel
         ▼
  ┌─────────────┐
  │ Preview      │  Axum server
  │ Server       │  SSE pushes event to browser
  └──────┬───────┘
         │
         │  HTTP response
         ▼
  ┌─────────────┐
  │ Browser UI   │  Embedded HTML/CSS/JS
  │              │  Renders message with live updates
  └─────────────┘
```

### Email Parsing Strategy

When lettre's `AsyncTransport::send()` is called, the transport receives the email as raw RFC 5322 bytes. Shunt's `EmailInterceptor` takes these bytes and parses them using `mail-parser` to extract structured fields:

1. lettre serializes the `Message` into RFC 5322 format (headers + MIME body).
2. `mail-parser` parses the byte stream into a structured representation.
3. The interceptor maps parsed fields to `EmailContent` (from, to, cc, bcc, subject, text body, HTML body, headers, attachments).
4. The interceptor constructs a `MessageSummary` from the envelope fields.
5. The interceptor builds a `ShuntedMessage` and stores it via `MessageStore`.

This parse-from-bytes approach is deliberate: it tests the exact byte stream that would have been sent to an SMTP server, ensuring fidelity between the preview and what production delivery would have produced.

### UI Architecture

The preview UI is a single-page application (SPA) built with vanilla HTML, CSS, and JavaScript (no framework, no build step). The assets are embedded into the `shunt_web` binary at compile time using `rust-embed`. Key features:

- **Message list:** Left sidebar showing sender, subject, and timestamp for each message.
- **Message detail:** Main panel rendering HTML email (in a sandboxed iframe) or plain text.
- **Channel tabs:** Filter by email, SMS, or all.
- **Live updates:** SSE connection automatically adds new messages to the list.
- **Responsive layout:** Works in any modern browser.

No `npm`, `node_modules`, or frontend build toolchain is required. The UI is authored directly as static files and embedded at `cargo build` time.

---

## Recommendation

Build shunt as a Cargo workspace with the five-crate structure described above, using trait-based adapters as the interception mechanism.

### Key Technical Decisions

1. **Implement `AsyncTransport` for email interception.** This gives lettre users a one-line swap to enable shunting. No changes to application-level email construction code.

2. **Define `SmsSender` as a shunt-owned trait.** The Rust SMS ecosystem is fragmented. Owning the trait gives shunt control over the contract and avoids coupling to any provider SDK.

3. **Use `FileStore` as the default (and initially only) `MessageStore` implementation.** JSON files on disk are human-readable, require no external service, and work on every platform. The trait boundary allows future backends without core changes.

4. **Embed the UI with `rust-embed`.** Zero runtime file dependencies. No build step. The binary is self-contained.

5. **Use SSE for live updates.** Simpler than WebSockets, sufficient for the unidirectional notification pattern, and compatible with HTTP/1.1 proxies.

6. **Async-first with tokio.** All public APIs are async. Blocking file I/O in `FileStore` is wrapped in `spawn_blocking` to avoid starving the runtime.

7. **Workspace-level dependency declarations.** All shared dependencies (serde, tokio, uuid, chrono) are declared in the workspace `Cargo.toml` and inherited by member crates, ensuring version consistency.

---

## Conclusion

Shunt fills a genuine gap in the Rust development tooling ecosystem. By implementing existing traits (`AsyncTransport`) and defining minimal new ones (`SmsSender`, `MessageStore`), it integrates with applications through idiomatic Rust dependency injection rather than runtime magic or external services.

The library is designed to be used as a `[dev-dependency]`, meaning it adds zero weight to production builds. The modular workspace structure ensures that developers pay only for the channels and features they use. The embedded UI eliminates operational overhead, and the file-based storage keeps the tool self-contained and debuggable.

Approximate compile-time impact for a project that already uses lettre and tokio: adding `shunt_email` and `shunt_web` introduces `mail-parser`, `axum`, `rust-embed`, and their transitive dependencies. On a cold build, this adds roughly 15-25 seconds. Incremental builds are minimally affected because the shunt crates are stable dev-dependencies that rarely recompile.
