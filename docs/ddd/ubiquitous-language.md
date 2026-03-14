# Ubiquitous Language

**Project:** shunt
**Date:** 2026-03-14
**Status:** Living document

---

## Purpose

This document defines the shared vocabulary used across all shunt crates, documentation, commit messages, API endpoints, and developer-facing communication. Every contributor, whether writing code, documentation, or issue descriptions, should use these terms consistently to eliminate ambiguity.

The terms below are organised into categories. Each entry includes the term, its precise meaning within the shunt project, and usage guidance.

---

## Core Concepts

### Shunt (verb)

**Definition:** To redirect an outbound message away from its intended external recipient and into local storage for developer inspection.

**Usage:** "When the email interceptor is active, all outbound emails are *shunted* to the local file store." The word "shunt" is borrowed from railroad terminology, where a shunt diverts a train onto a side track. In this project, messages are diverted from the delivery track onto the preview track.

**Not:** "Block", "drop", or "suppress". Shunting preserves the message; it does not discard it.

### Preview (noun, verb)

**Definition:** (noun) The browser-based view of a shunted message. (verb) The act of viewing a shunted message in the browser.

**Usage:** "The developer can *preview* the email in the browser at localhost:9099." / "The *preview* shows both the HTML and plain-text renderings."

**Not:** "Dashboard" or "inbox". The preview is a development tool, not a production email client.

### Message

**Definition:** A shunted email or SMS, represented in the domain model as a `ShuntedMessage` entity. The term "message" is channel-agnostic and refers to any intercepted communication regardless of its underlying medium.

**Usage:** "Three *messages* are currently stored in the file store."

**Not:** "Email" or "SMS" when speaking generically. Use "message" for channel-agnostic contexts. Use "email" or "SMS" only when the channel is relevant to the statement.

### Channel

**Definition:** The communication medium through which a message would have been delivered. Shunt currently supports two channels: **email** and **SMS**.

**Usage:** "Shunt supports the email and SMS *channels*." / "A new *channel* can be added by creating a new adapter crate."

**Not:** "Protocol" or "transport type" in general usage. "Transport" has a specific meaning in this project (see below).

---

## Interception and Transport

### Interceptor

**Definition:** A component that captures outgoing messages at the transport boundary. An interceptor implements a sending interface (e.g., lettre's `AsyncTransport` or shunt's `SmsSender`) but stores the message locally instead of delivering it.

**Usage:** "The `EmailInterceptor` implements `AsyncTransport` and shunts all emails to the file store."

**Not:** "Proxy" or "middleware". The interceptor is a replacement transport, not a pass-through layer.

### Transport

**Definition:** The mechanism by which emails are delivered. This term comes from the lettre crate, where `Transport` and `AsyncTransport` are the traits that define how an email is sent (e.g., via SMTP, via file, via shunt).

**Usage:** "Swap the SMTP *transport* for the shunt *transport* in your dev configuration."

**Scope:** This term is specific to the email channel. SMS uses "sender" rather than "transport" because lettre terminology does not apply to SMS.

### Sender (SMS context)

**Definition:** The component responsible for dispatching SMS messages. In shunt, the `SmsSender` trait defines this contract. The shunt implementation stores the message locally; a production implementation would call a provider API (Twilio, AWS SNS, etc.).

**Usage:** "Inject the shunt *sender* as your `SmsSender` implementation during development."

---

## Storage

### Store

**Definition:** The persistence layer where shunted messages are saved. Defined by the `MessageStore` trait. The default implementation is `FileStore`, which writes JSON files to a directory on disk.

**Usage:** "Messages are written to the *store* immediately upon interception." / "The file *store* persists messages as JSON files in `.shunt/messages/`."

**Not:** "Database" (unless a database-backed store is specifically being discussed). The default store is file-based.

### File Store

**Definition:** The default `MessageStore` implementation that persists each shunted message as an individual JSON file in a configurable directory. This is the only storage backend included with shunt.

**Usage:** "The *file store* writes to `.shunt/messages/` by default."

### Store Path

**Definition:** The filesystem directory where the file store persists messages. Configurable at initialisation time. Defaults to `.shunt/` relative to the current working directory.

**Usage:** "Set the *store path* to a temporary directory in integration tests."

---

## Domain Model Types

### Shunted Message

**Definition:** The aggregate root entity representing a single intercepted message. Contains an `id` (UUID), `kind` (email or SMS), `created_at` timestamp, a `MessageSummary`, and the full `MessageContent`.

**Usage:** "Each intercepted email produces exactly one *shunted message* in the store."

### Message Kind

**Definition:** An enum (`Email` or `Sms`) that discriminates the communication channel of a shunted message. Used for filtering and display purposes.

**Usage:** "Filter the message list by *message kind* to show only emails."

### Message Summary

**Definition:** A lightweight projection of a message's envelope data (sender, recipients, subject). Used for rendering message lists without loading full content.

**Usage:** "The list API returns *message summaries* for efficient rendering."

### Message Content

**Definition:** The channel-specific payload of a shunted message. Either `EmailContent` (full email fields including headers and attachments) or `SmsContent` (body, metadata).

**Usage:** "The detail API returns the full *message content* including HTML body and attachment metadata."

### Attachment

**Definition:** A file attached to an intercepted email. The attachment's metadata (filename, content type, size) is stored in `AttachmentInfo` as part of the shunted message. The file bytes are stored separately on disk.

**Usage:** "The email has two *attachments*: a PDF and a PNG image."

---

## Web Preview

### Preview Server

**Definition:** The Axum-based HTTP server that serves the message preview UI and REST API. It runs in a background Tokio task and binds to a configurable local port (default `9099`).

**Usage:** "Start the *preview server* to view shunted messages in the browser."

### Live Update

**Definition:** The mechanism by which the preview UI is notified of new messages in real time, implemented via Server-Sent Events (SSE). When a new message is stored, an event is pushed to all connected browsers.

**Usage:** "The browser receives a *live update* within milliseconds of the message being shunted."

### Embedded UI

**Definition:** The static HTML, CSS, and JavaScript files that constitute the browser-based preview interface. These assets are compiled into the `shunt_web` binary using `rust-embed`, eliminating runtime file dependencies.

**Usage:** "The *embedded UI* renders without any build step or file serving configuration."

---

## Architecture and Design

### Workspace

**Definition:** The Cargo workspace containing all shunt crates. The workspace root defines shared dependencies and configuration. Individual crates are located in the `crates/` directory.

**Usage:** "All five crates are members of the shunt *workspace*."

### Convenience Crate

**Definition:** The `shunt` crate at the workspace root that re-exports all sub-crates. Developers who want the full shunt experience can depend on this single crate instead of listing individual sub-crates.

**Usage:** "Add the *convenience crate* to get email interception, SMS interception, and the preview server with one dependency line."

### Adapter

**Definition:** A component that translates between an external interface and shunt's internal domain model. The `EmailInterceptor` is an adapter from lettre's `AsyncTransport` to `ShuntedMessage`. The `SmsInterceptor` is an adapter from the `SmsSender` trait to `ShuntedMessage`.

**Usage:** "The email *adapter* converts lettre's `Message` type into a `ShuntedMessage`."

### Anti-Corruption Layer (ACL)

**Definition:** The boundary between external crate APIs (lettre, provider SDKs) and shunt's internal domain types. The adapters in `shunt_email` and `shunt_sms` form the anti-corruption layer, ensuring that external type changes do not propagate into `shunt_core`.

**Usage:** "The *anti-corruption layer* in `shunt_email` parses lettre's RFC 5322 output into domain types."

---

## Development and Testing

### Dev-Dependency

**Definition:** A Cargo dependency that is only compiled and linked when running tests or examples, not in production builds. Shunt is designed to be added as a dev-dependency in the consuming application's `Cargo.toml`.

**Usage:** "Add shunt as a *dev-dependency* so it is not included in production builds."

### Shunt Directory

**Definition:** The `.shunt/` directory in the project root where the file store persists messages. This directory should typically be added to `.gitignore`.

**Usage:** "Add `.shunt/` to your `.gitignore` to keep intercepted messages out of version control."

### Message Fixture

**Definition:** A pre-built `ShuntedMessage` used in tests. Fixtures provide known message content for assertions without requiring real email or SMS construction.

**Usage:** "The integration test stores a *message fixture* and verifies it appears in the API response."

---

## Terminology Boundaries

The following terms are intentionally **not** used in the shunt project to avoid confusion:

| Avoided Term   | Reason                                                                 | Use Instead          |
|---------------|------------------------------------------------------------------------|----------------------|
| Inbox          | Implies a production email client; shunt is a development tool.       | Preview, message list |
| Mock           | Shunt is not a mock; it is a real implementation of a different behavior. | Interceptor, adapter |
| Fake           | Similar to "mock"; implies something that does not work.              | Interceptor          |
| Queue          | Messages are not queued for later delivery; they are stored for preview. | Store                |
| Forward        | Messages are not forwarded; they are diverted.                        | Shunt                |
| Capture        | Acceptable in informal usage but "shunt" or "intercept" is preferred. | Shunt, intercept     |
| Mailbox        | lettre uses this term for email addresses; avoid in shunt's own API.  | Message list, store  |
