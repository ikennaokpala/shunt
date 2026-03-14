# Bounded Contexts

**Project:** shunt
**Date:** 2026-03-14
**Status:** Living document

---

## Overview

Shunt's architecture is organised around four bounded contexts, each encapsulating a distinct area of responsibility. The contexts are aligned with the Cargo workspace crate boundaries, making the logical architecture directly visible in the physical code structure.

```
┌──────────────────────────────────────────────────────────────────────┐
│                        Developer's Application                       │
│                                                                      │
│   uses lettre::AsyncTransport       uses SmsSender trait             │
│          │                                  │                        │
└──────────┼──────────────────────────────────┼────────────────────────┘
           │                                  │
           ▼                                  ▼
┌─────────────────────┐          ┌─────────────────────┐
│  1. MESSAGE         │          │  4. PROVIDER        │
│     INTERCEPTION    │          │     ADAPTERS        │
│                     │          │                     │
│  (Core Domain)      │          │  (Anti-Corruption   │
│                     │          │   Layer)            │
│  shunt_core types   │          │                     │
│  + shunt_email      │          │  shunt_email:       │
│  + shunt_sms        │          │    AsyncTransport   │
│                     │          │  shunt_sms:         │
│                     │          │    SmsSender trait   │
└────────┬────────────┘          └──────────┬──────────┘
         │                                  │
         │        stores messages           │
         ▼                                  │
┌─────────────────────┐                     │
│  2. MESSAGE         │◄────────────────────┘
│     STORAGE         │
│                     │
│  (Persistence)      │
│                     │
│  MessageStore trait  │
│  FileStore impl     │
│  JSON serialization │
└────────┬────────────┘
         │
         │  reads messages
         ▼
┌─────────────────────┐
│  3. MESSAGE         │
│     PREVIEW         │
│                     │
│  (Read Path)        │
│                     │
│  Axum web server    │
│  REST API           │
│  SSE live updates   │
│  Embedded UI        │
└─────────────────────┘
```

---

## Bounded Context 1: Message Interception

**Crate:** `shunt_core` (types), `shunt_email` (email interception), `shunt_sms` (SMS interception)

### Responsibility

This is the core domain of shunt. Its job is to capture outbound messages that the developer's application would normally send to real recipients and redirect ("shunt") them into local storage instead. The interception must be transparent to the calling application: the developer writes the same code they would use to send a real email or SMS, but the message never leaves the machine.

### Key Concepts

| Concept              | Description                                                                                       |
|---------------------|---------------------------------------------------------------------------------------------------|
| `ShuntedMessage`    | The aggregate root representing a captured message.                                               |
| `MessageKind`       | Discriminator between email and SMS channels.                                                     |
| `EmailInterceptor`  | Implements lettre's `AsyncTransport` trait, converting outbound emails into `ShuntedMessage` records. |
| `SmsInterceptor`    | Implements the `SmsSender` trait, converting outbound SMS calls into `ShuntedMessage` records.      |
| Interception        | The act of capturing a message at the transport boundary.                                         |

### Boundaries and Interfaces

- **Inbound:** Receives messages from the developer's application through standard Rust trait interfaces (`AsyncTransport` for email, `SmsSender` for SMS).
- **Outbound:** Passes constructed `ShuntedMessage` instances to the Message Storage context via the `MessageStore` trait.
- **Invariant:** Every message that enters this context must be assigned a unique `Uuid`, timestamped, and classified by `MessageKind` before being forwarded to storage.

### Design Decisions

- Email interception reuses lettre's `AsyncTransport` trait rather than inventing a custom interface. This means any application already using lettre can swap in shunt's transport with a single line change.
- SMS interception uses a shunt-defined `SmsSender` trait because the Rust ecosystem has no dominant SMS crate analogous to lettre. Defining our own trait keeps shunt decoupled from any specific provider SDK.
- The core types live in `shunt_core` so that both `shunt_email` and `shunt_sms` depend on shared definitions without depending on each other.

---

## Bounded Context 2: Message Storage

**Crate:** `shunt_core` (trait and default implementation)

### Responsibility

Persists intercepted messages durably so they survive process restarts and can be queried by the preview UI. Provides a trait-based abstraction so the storage backend can be replaced without affecting other contexts.

### Key Concepts

| Concept        | Description                                                                                      |
|---------------|--------------------------------------------------------------------------------------------------|
| `MessageStore`| Async trait defining the persistence contract: store, get, list, delete, clear.                   |
| `FileStore`   | Default implementation that writes each message as a JSON file in a configurable directory.       |
| Store path    | The filesystem directory where messages are persisted (defaults to `.shunt/` in the project root).|

### Boundaries and Interfaces

- **Inbound:** Receives `ShuntedMessage` instances from the Message Interception context.
- **Outbound:** Provides query access to stored messages for the Message Preview context.
- **Invariant:** A stored message must be retrievable by its `id` immediately after a successful `store()` call. The store must be safe for concurrent access from multiple threads (the trait requires `Send + Sync`).

### Storage Format

```
.shunt/
├── messages/
│   ├── 550e8400-e29b-41d4-a716-446655440000.json
│   ├── 6ba7b810-9dad-11d1-80b4-00c04fd430c8.json
│   └── ...
└── attachments/
    ├── 550e8400-e29b-41d4-a716-446655440000/
    │   ├── report.pdf
    │   └── image.png
    └── ...
```

### Design Decisions

- JSON was chosen over binary formats (bincode, MessagePack) because developer-time tooling benefits from human-readable storage. Developers can inspect stored messages directly if needed.
- The `MessageStore` trait is async even though the default `FileStore` performs blocking file I/O (wrapped in `tokio::task::spawn_blocking`). This ensures the trait is forward-compatible with genuinely async backends like SQLite via `sqlx`.
- Attachment content is stored as separate files rather than inline Base64 in the JSON. This avoids inflating JSON file sizes and allows the web server to stream attachments directly from disk.

---

## Bounded Context 3: Message Preview

**Crate:** `shunt_web`

### Responsibility

Provides a browser-based interface for developers to view intercepted messages. This is a read-only context: it queries the Message Storage context and renders messages for human consumption. It never modifies message content; the only write operation it exposes is deletion (clearing the inbox).

### Key Concepts

| Concept           | Description                                                                                 |
|------------------|---------------------------------------------------------------------------------------------|
| Axum web server  | Lightweight HTTP server that starts on a local port and serves the preview UI.              |
| REST API         | JSON endpoints for listing messages, fetching individual messages, and deleting messages.    |
| SSE endpoint     | Server-Sent Events stream that notifies the browser of new messages in real time.           |
| Embedded UI      | Static HTML, CSS, and JavaScript bundled into the binary via `rust-embed`. Zero build step. |
| Preview          | The rendered view of a shunted message in the browser.                                      |

### Boundaries and Interfaces

- **Inbound:** HTTP requests from the developer's browser.
- **Outbound:** Reads from the `MessageStore` trait to fetch stored messages.
- **Invariant:** The web server must not block the developer's application. It runs in a background Tokio task and binds to a configurable port (default `9099`).

### API Surface

| Method   | Path                      | Description                              |
|----------|---------------------------|------------------------------------------|
| `GET`    | `/`                       | Serve the embedded UI (index.html).      |
| `GET`    | `/api/v1/messages`        | List all shunted messages (summary view).|
| `GET`    | `/api/v1/messages/:id`    | Fetch a single message with full content. |
| `DELETE` | `/api/v1/messages/:id`    | Delete a single message.                 |
| `DELETE` | `/api/v1/messages`        | Clear all messages.                      |
| `GET`    | `/api/v1/messages/stream` | SSE stream of new message events.        |
| `GET`    | `/api/v1/messages/:id/attachments/:filename` | Download an attachment. |

### Design Decisions

- The UI is embedded into the binary using `rust-embed` so that `shunt_web` has zero runtime file dependencies. There is no separate build step for frontend assets.
- SSE was chosen over WebSockets for live updates because it is simpler, unidirectional (server-to-client is all that is needed), and works through HTTP proxies without special configuration.
- The web server is designed to be optional. Applications that only need programmatic access to shunted messages (e.g., in integration tests) can use `shunt_core` and `shunt_email`/`shunt_sms` without pulling in `shunt_web` at all.

---

## Bounded Context 4: Provider Adapters

**Crates:** `shunt_email`, `shunt_sms`

### Responsibility

This context acts as an anti-corruption layer between the external APIs that the developer's application uses (lettre for email, various SDKs for SMS) and shunt's internal domain model. It translates external message representations into `ShuntedMessage` instances without leaking external types into the core domain.

### Key Concepts

| Concept                  | Description                                                                                     |
|-------------------------|-------------------------------------------------------------------------------------------------|
| `AsyncTransport` impl  | lettre's transport trait, implemented by `EmailInterceptor` to capture emails.                  |
| `SmsSender` trait       | Shunt's own trait for SMS sending, allowing application code to swap between real and shunt.    |
| Anti-corruption layer   | Parsing and conversion logic that transforms lettre `Message` objects into `EmailContent`.      |
| `mail-parser`           | Used to parse raw RFC 5322 email bytes into structured fields.                                  |

### Boundaries and Interfaces

- **Inbound:** Receives messages in external formats (lettre `Message`, raw SMS parameters).
- **Outbound:** Produces `ShuntedMessage` instances and forwards them to the Message Storage context.
- **Invariant:** External types (e.g., `lettre::Message`, `lettre::address::Mailbox`) must not appear in `shunt_core`. All translation happens within the adapter crates.

### Translation Flow: Email

```
Developer code                    shunt_email                     shunt_core
─────────────                    ───────────                     ──────────
lettre::Message ──► AsyncTransport::send()
                         │
                         ├─► Serialize to RFC 5322 bytes
                         ├─► Parse with mail-parser
                         ├─► Extract fields → EmailContent
                         ├─► Build MessageSummary
                         ├─► Construct ShuntedMessage
                         │
                         └─► MessageStore::store() ──────────────► FileStore
```

### Translation Flow: SMS

```
Developer code                    shunt_sms                      shunt_core
─────────────                    ─────────                      ──────────
SmsSender::send(from, to, body)
                         │
                         ├─► Build SmsContent { from, to, body, metadata }
                         ├─► Build MessageSummary
                         ├─► Construct ShuntedMessage
                         │
                         └─► MessageStore::store() ──────────────► FileStore
```

### Design Decisions

- The email adapter depends on both `lettre` (for the `AsyncTransport` trait) and `mail-parser` (for parsing the RFC 5322 byte stream that lettre produces). This two-step process (serialize then parse) is deliberate: it tests the exact byte stream the application would have sent to an SMTP server.
- The SMS adapter is intentionally minimal. Because there is no standard SMS trait in the Rust ecosystem, shunt defines `SmsSender` with the simplest possible signature. Provider-specific parameters (encoding, priority, callback URLs) are captured in the `metadata` HashMap rather than as typed fields.
- Both adapters hold an `Arc<dyn MessageStore>` so they can be shared across threads and Tokio tasks without lifetime complications.

---

## Context Map

The relationships between bounded contexts follow these patterns:

```
┌─────────────────┐     Conformist      ┌─────────────────┐
│  Provider        │ ◄─────────────────  │  Message         │
│  Adapters        │                     │  Interception    │
│  (ACL)           │ ────────────────►   │  (Core)          │
└────────┬─────────┘     Shared Kernel   └────────┬─────────┘
         │               (shunt_core)             │
         │                                        │
         │  Published Language                    │  Published Language
         │  (MessageStore trait)                  │  (MessageStore trait)
         ▼                                        ▼
┌─────────────────┐                     ┌─────────────────┐
│  Message         │  ◄──────────────── │  Message         │
│  Storage         │   Published Lang.  │  Preview         │
│  (Persistence)   │   (MessageStore)   │  (Read Path)     │
└─────────────────┘                     └─────────────────┘
```

| Relationship                          | Pattern            | Description                                                          |
|--------------------------------------|--------------------|----------------------------------------------------------------------|
| Provider Adapters → Core Domain      | Anti-Corruption Layer | Adapters translate external types into domain types.               |
| Core Domain ↔ Storage                | Shared Kernel       | Both contexts share `shunt_core` types and the `MessageStore` trait.|
| Storage ↔ Preview                    | Published Language  | Preview reads from storage via the `MessageStore` trait contract.  |
| Provider Adapters → Storage          | Published Language  | Adapters write to storage via the `MessageStore` trait contract.   |

---

## Context Boundaries and Crate Mapping

| Bounded Context       | Primary Crate   | Supporting Crate | External Dependencies              |
|-----------------------|----------------|-------------------|-------------------------------------|
| Message Interception  | `shunt_core`   | `shunt_email`, `shunt_sms` | `uuid`, `chrono`, `serde`  |
| Message Storage       | `shunt_core`   | —                 | `tokio`, `serde_json`               |
| Message Preview       | `shunt_web`    | —                 | `axum`, `tower`, `rust-embed`, `tokio` |
| Provider Adapters     | `shunt_email`  | `shunt_sms`       | `lettre`, `mail-parser`             |

---

## Evolution Strategy

Adding a new communication channel (e.g., push notifications, webhook capture) follows a consistent pattern:

1. Define the channel-specific content type (e.g., `PushContent`) in `shunt_core` or a new crate.
2. Add a variant to `MessageKind` and `MessageContent`.
3. Create a new crate (e.g., `shunt_push`) containing the adapter that implements the appropriate trait.
4. The new adapter writes `ShuntedMessage` instances through `MessageStore` -- no changes needed in the storage or preview contexts.
5. The `shunt_web` UI renders the new channel type automatically via its generic message display logic (or with a channel-specific template if needed).

This pattern ensures that existing bounded contexts remain stable when the system is extended with new channels.
