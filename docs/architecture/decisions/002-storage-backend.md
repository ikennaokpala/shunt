# ADR-002: Storage Backend

**Date:** 2026-03-14
**Status:** Accepted
**Context:** Intercepted messages need to be persisted so they survive process restarts and can be browsed in the web UI.
**Decision:** Use filesystem storage with JSON metadata files alongside raw content files, organized in a structured directory hierarchy.
**Consequences:** Zero external dependencies for storage, human-readable message archives, and straightforward debugging. Trade-off is no indexed querying, which is acceptable for development-tool volumes.

## Context and Problem Statement

When Shunt intercepts an outbound email or SMS, it needs to store the message so that:

1. The web UI can list and display messages after they are captured.
2. Messages survive application restarts (a developer may send a test email, then restart their app before checking the preview).
3. Test code can query captured messages for assertions (e.g., "verify that a welcome email was sent to user@example.com").
4. Developers can manually inspect captured messages when debugging (e.g., checking if an HTML email renders correctly).

The storage mechanism must be simple, require no setup or external services, and work reliably across macOS, Linux, and Windows. As a development tool, Shunt will handle at most hundreds of messages per session, not millions.

## Considered Options

1. **In-memory only (HashMap behind a Mutex/RwLock)**
   - Pros: Fastest possible reads and writes. No filesystem I/O. No serialization overhead. Trivial implementation.
   - Cons: All messages are lost when the process exits. This is a dealbreaker for the common workflow where a developer sends a test message, restarts their application, and then opens the browser to check the result. Also prevents message inspection across test runs.

2. **SQLite (via rusqlite or sqlx)**
   - Pros: Indexed querying for listing and filtering. Single-file database is portable. Well-understood technology. Could support advanced features like full-text search on message bodies.
   - Cons: Adds a native dependency (libsqlite3) that complicates cross-compilation and increases binary size. Schema migrations add operational complexity for a dev tool. Message content (HTML emails, attachments) stored as BLOBs are not human-inspectable without tooling. Overkill for the expected data volumes (tens to hundreds of messages).

3. **Filesystem with JSON metadata + content files (chosen)**
   - Pros: Zero external dependencies beyond `std::fs` and `serde_json`. Messages are stored as regular files that developers can open, inspect, copy, and delete with standard tools. Directory listing provides natural chronological ordering. Attachments are stored as separate files, directly viewable. The storage directory can be wiped with `rm -rf` for a clean slate. Works identically on all platforms.
   - Cons: No indexed querying; listing messages requires reading directory entries and parsing JSON metadata. Not suitable for high-volume production use. Concurrent access requires file-level coordination. No atomic multi-file writes (mitigated by write-then-rename pattern).

## Decision Outcome

Chosen option: **Filesystem with JSON metadata + content files**

The storage layout follows this structure:

```
$SHUNT_DIR/                          # Default: .shunt/ in project root
  messages/
    <timestamp>-<uuid>/
      metadata.json                  # Sender, recipients, subject, channel, timestamps
      content.html                   # Rendered HTML preview (pre-rendered at storage time)
      raw.eml                        # Original RFC 5322 bytes (emails only)
      body.txt                       # Plain text body (SMS, or plain-text email fallback)
      attachments/
        invoice.pdf                  # Named by original filename
        logo.png
```

**Rationale:**
- Shunt is a development tool, not a production message queue. The expected volume is tens to low hundreds of messages per development session. Filesystem storage handles this comfortably.
- Developers frequently need to inspect intercepted messages outside the web UI (e.g., checking raw email headers, verifying attachment encoding). Filesystem storage makes every message directly accessible with standard tools like `cat`, `open`, or a file browser.
- Eliminating the SQLite dependency simplifies installation, cross-compilation, and the dependency tree. Users of `shunt-core` get storage without pulling in native libraries.
- The write-then-rename pattern (write to a temporary directory, then atomically rename into place) prevents the web UI from reading partially-written messages.
- Ruby's letter_opener, which inspired this project, also uses filesystem storage and has proven the approach works well for this use case over many years.

**Implications:**
- Listing messages requires scanning the `messages/` directory and reading each `metadata.json` file. For the expected volumes (under 1,000 messages), this completes in under 50ms, which is acceptable.
- The `MessageStore` trait in `shunt-core` abstracts over the storage mechanism, so an in-memory implementation can be used in tests that do not need persistence.
- Cleanup is the user's responsibility. We will provide a `shunt clean` command or API, but will not implement automatic expiration in the initial version.
- File locking or atomic operations must be used carefully if multiple processes write to the same store directory concurrently (uncommon in development, but possible).

## Validation

How we'll know if this was the right decision:
- Developers report that they can find and inspect intercepted messages without difficulty.
- No bug reports related to message corruption or partial reads under normal development usage patterns.
- The storage implementation remains under 300 lines of Rust, confirming that filesystem storage keeps complexity low.
- No user requests for SQLite or database-backed storage emerge as a recurring theme (if they do, the `MessageStore` trait allows adding an alternative backend without changing existing code).
- Cross-platform CI (Linux, macOS, Windows) passes without filesystem-related test failures.
