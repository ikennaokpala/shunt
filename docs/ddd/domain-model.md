# Domain Model

**Project:** shunt
**Date:** 2026-03-14
**Status:** Living document

---

## Overview

Shunt is a Rust library that intercepts outbound emails and SMS messages during development, stores them locally, and presents them in a browser-based preview. The domain model captures the essential concepts required to represent, persist, and display intercepted messages regardless of their underlying channel (email or SMS).

---

## Entity Relationship Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        ShuntedMessage                           │
│  (Aggregate Root)                                               │
├─────────────────────────────────────────────────────────────────┤
│  id: Uuid                                                       │
│  kind: MessageKind                                              │
│  created_at: DateTime<Utc>                                      │
│  summary: MessageSummary                                        │
│  content: MessageContent                                        │
└────────────┬──────────────────┬──────────────────┬──────────────┘
             │                  │                  │
             ▼                  ▼                  ▼
   ┌─────────────────┐ ┌───────────────┐ ┌─────────────────────┐
   │  MessageKind    │ │MessageSummary │ │  MessageContent     │
   │  (Value Object) │ │(Value Object) │ │  (Value Object)     │
   ├─────────────────┤ ├───────────────┤ ├─────────────────────┤
   │  Email          │ │ from: String  │ │  Email(EmailContent)│
   │  Sms            │ │ to: Vec<Str>  │ │  Sms(SmsContent)    │
   └─────────────────┘ │ subject: Opt  │ └───────┬─────────────┘
                        └───────────────┘         │
                                          ┌───────┴───────┐
                                          │               │
                                          ▼               ▼
                                ┌──────────────┐ ┌──────────────┐
                                │ EmailContent │ │  SmsContent  │
                                │(Value Object)│ │(Value Object)│
                                ├──────────────┤ ├──────────────┤
                                │ from         │ │ from         │
                                │ to           │ │ to           │
                                │ cc           │ │ body         │
                                │ bcc          │ │ metadata     │
                                │ subject      │ └──────────────┘
                                │ text_body    │
                                │ html_body    │
                                │ headers      │
                                │ attachments  │
                                └──────┬───────┘
                                       │
                                       ▼
                              ┌─────────────────┐
                              │ AttachmentInfo   │
                              │ (Value Object)   │
                              ├─────────────────┤
                              │ filename         │
                              │ content_type     │
                              │ size_bytes       │
                              └─────────────────┘
```

---

## Entities

### ShuntedMessage (Aggregate Root)

The central entity in the domain. A `ShuntedMessage` represents a single outbound email or SMS that has been intercepted ("shunted") away from its real delivery path and stored locally for developer inspection.

| Field        | Type              | Description                                                        |
|-------------|-------------------|--------------------------------------------------------------------|
| `id`        | `Uuid`            | Globally unique identifier assigned at interception time.          |
| `kind`      | `MessageKind`     | Discriminator indicating whether this is an email or SMS.          |
| `created_at`| `DateTime<Utc>`   | Timestamp of when the message was intercepted.                     |
| `summary`   | `MessageSummary`  | Lightweight envelope data used for list views and search.          |
| `content`   | `MessageContent`  | Full message payload, specific to the channel.                     |

**Identity:** Each `ShuntedMessage` is uniquely identified by its `id` field. Two messages with identical content but different `id` values are distinct entities.

**Invariants:**
- `id` must be a valid v4 UUID.
- `kind` must match the variant held in `content` (i.e., `MessageKind::Email` corresponds to `MessageContent::Email(_)`).
- `created_at` must be set at the moment of interception, never backdated.
- `summary.from` must be a non-empty string.
- `summary.to` must contain at least one recipient.

---

## Value Objects

All value objects are immutable once constructed. They carry no identity of their own and are equal when all their fields are equal.

### MessageKind

Discriminates the communication channel.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageKind {
    Email,
    Sms,
}
```

This enum is used for filtering, display labels, and ensuring type-safe branching throughout the codebase. It is intentionally kept separate from `MessageContent` so that code paths that need only the discriminator (e.g., listing messages) do not require the full payload.

### MessageSummary

A lightweight projection of the message envelope, designed to support listing and searching without loading the full content.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSummary {
    /// Sender address or phone number.
    pub from: String,
    /// One or more recipient addresses or phone numbers.
    pub to: Vec<String>,
    /// Subject line (present for emails, absent for SMS).
    pub subject: Option<String>,
}
```

**Design rationale:** The summary exists so that the web preview can render a message list without deserializing potentially large HTML bodies or attachment metadata. It normalises the concept of "sender" and "recipients" across both email and SMS channels.

### MessageContent

Channel-specific payload, modelled as a tagged enum.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum MessageContent {
    Email(EmailContent),
    Sms(SmsContent),
}
```

### EmailContent

Full representation of an intercepted email.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailContent {
    /// RFC 5322 sender (e.g., "Alice <alice@example.com>").
    pub from: String,
    /// Primary recipients.
    pub to: Vec<String>,
    /// Carbon-copy recipients.
    pub cc: Vec<String>,
    /// Blind carbon-copy recipients.
    pub bcc: Vec<String>,
    /// Subject line.
    pub subject: String,
    /// Plain-text body, if present.
    pub text_body: Option<String>,
    /// HTML body, if present.
    pub html_body: Option<String>,
    /// Raw headers as key-value pairs, preserving order.
    pub headers: Vec<(String, String)>,
    /// Metadata about attached files (file content is stored separately).
    pub attachments: Vec<AttachmentInfo>,
}
```

**Design notes:**
- Both `text_body` and `html_body` are optional because a valid email may contain only one of the two (or, in degenerate cases, neither).
- Headers are stored as an ordered `Vec` of tuples rather than a `HashMap` because email headers can repeat (e.g., multiple `Received` headers) and order can matter.
- Attachment file content is stored as separate files on disk. `AttachmentInfo` holds only metadata to keep the serialized message lightweight.

### SmsContent

Full representation of an intercepted SMS.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsContent {
    /// Sender phone number or short code.
    pub from: String,
    /// Recipient phone number.
    pub to: String,
    /// Message body text.
    pub body: String,
    /// Provider-specific metadata (e.g., encoding, segment count).
    pub metadata: HashMap<String, String>,
}
```

**Design notes:**
- SMS is inherently point-to-point, so `to` is a single `String` rather than a `Vec`.
- The `metadata` map accommodates provider-specific fields without polluting the core type with provider concerns.

### AttachmentInfo

Metadata about an email attachment. The actual file bytes are stored outside the serialized message structure.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentInfo {
    /// Original filename as declared in the email.
    pub filename: String,
    /// MIME content type (e.g., "application/pdf").
    pub content_type: String,
    /// Size of the attachment in bytes.
    pub size_bytes: u64,
}
```

---

## Traits (Domain Interfaces)

### MessageStore

The persistence boundary for shunted messages. All storage implementations must satisfy this trait.

```rust
#[async_trait]
pub trait MessageStore: Send + Sync {
    /// Persist a shunted message. Returns the assigned UUID.
    async fn store(&self, message: &ShuntedMessage) -> Result<Uuid>;

    /// Retrieve a single message by ID.
    async fn get(&self, id: Uuid) -> Result<Option<ShuntedMessage>>;

    /// List all stored messages, ordered by creation time (newest first).
    async fn list(&self) -> Result<Vec<ShuntedMessage>>;

    /// Remove a single message by ID.
    async fn delete(&self, id: Uuid) -> Result<()>;

    /// Remove all stored messages.
    async fn clear(&self) -> Result<()>;
}
```

**Design rationale:** The trait is async to support future backends (e.g., SQLite, Redis) without forcing synchronous I/O. The default implementation is `FileStore`, which writes JSON files to a configurable directory.

### SmsSender

The interception boundary for SMS providers. Application code programs against this trait; shunt provides an implementation that captures rather than sends.

```rust
#[async_trait]
pub trait SmsSender: Send + Sync {
    /// Send an SMS message. In shunt's implementation, this stores
    /// the message locally instead of delivering it.
    async fn send(&self, from: &str, to: &str, body: &str) -> Result<()>;
}
```

**Design rationale:** Unlike email, there is no dominant Rust crate for SMS (analogous to lettre). Rather than coupling to a specific provider SDK, shunt defines its own minimal trait. Application code injects either a real provider or the shunt interceptor via this trait boundary.

---

## Aggregate Boundaries

`ShuntedMessage` is the sole aggregate root. All mutations to a message's content must go through the aggregate. In practice, shunted messages are write-once: they are created at interception time and never modified thereafter. The only state transitions are creation and deletion.

```
[Created] ──── store() ────► [Persisted] ──── delete() ────► [Removed]
```

There is no update path because intercepted messages are historical records. Modifying them would undermine their value as a faithful capture of what the application attempted to send.

---

## Domain Rules

1. **Channel consistency:** The `kind` field must always agree with the `content` variant. A `ShuntedMessage` with `kind: Email` must contain `MessageContent::Email(_)`.
2. **Immutability after interception:** Once a message is stored, its content is never modified. The store supports only create, read, list, and delete operations.
3. **Summary derivability:** The `MessageSummary` is always derivable from the `MessageContent`. It exists as a denormalized convenience, not as an independent source of truth.
4. **Attachment separation:** Attachment file bytes are stored outside the serialized `ShuntedMessage` JSON. `AttachmentInfo` serves as a reference, not a container.
5. **Timestamp accuracy:** `created_at` reflects the wall-clock time at which the message was intercepted, using UTC to avoid timezone ambiguity.
