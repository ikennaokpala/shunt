# ADR-004: Email Parsing Strategy

**Date:** 2026-03-14
**Status:** Accepted
**Context:** Shunt intercepts emails sent via lettre's `Transport` trait and must parse them into a structured, displayable format for the web UI.
**Decision:** Use the `mail_parser` crate to parse raw RFC 5322 bytes obtained from lettre's `Message::formatted()` method. Pre-render the HTML preview at storage time so the web UI can serve it instantly.
**Consequences:** Reliable parsing of complex email formats without hand-written parsers. Instant preview serving with no parse-time latency. Trade-off is a dependency on `mail_parser` and storing both raw and rendered forms.

## Context and Problem Statement

When a developer's application sends an email through Shunt's `EmailInterceptor` (which implements lettre's `Transport` trait), Shunt receives the email as a lettre `Message` struct. To display this email in the web UI, Shunt must:

1. Extract structured fields: sender, recipients (to, cc, bcc), subject, date, and headers.
2. Extract the message body in both HTML and plain text forms, handling MIME multipart structures correctly.
3. Extract inline images and file attachments, preserving filenames and content types.
4. Handle character encoding (UTF-8, ISO-8859-1, etc.) and transfer encoding (base64, quoted-printable).
5. Produce a rendered HTML preview that can be displayed in the browser, including inline images rendered via data URIs or local file references.

Email is one of the oldest and most complex internet standards. RFC 5322 (message format), RFC 2045-2049 (MIME), and numerous extensions create a parsing surface that is notoriously difficult to implement correctly.

## Considered Options

1. **Hand-written parser for RFC 5322 and MIME**
   - Pros: No external dependency. Full control over parsing behavior. Could be optimized for the subset of email features that development emails actually use.
   - Cons: Extremely fragile. Email standards span thousands of pages across dozens of RFCs. Edge cases are innumerable: nested multipart messages, mixed encodings, malformed headers from various email libraries, non-standard line endings. A hand-written parser would be an ongoing source of bugs and maintenance burden. This is a solved problem that should not be re-solved.

2. **Client-side parsing in the browser (JavaScript)**
   - Pros: Offloads parsing complexity from Rust. JavaScript email parsing libraries exist (e.g., postal-mime, mailparser for Node). Raw `.eml` files could be served directly and parsed in the browser.
   - Cons: Adds significant JavaScript complexity to what should be a simple UI. Client-side parsing introduces latency when viewing messages, especially for large emails with attachments. Attachment handling in the browser is awkward (Blob URLs, memory management). Duplicates parsing logic if the Rust side also needs structured access for test assertions. Breaks the principle of keeping the frontend minimal.

3. **Use `mail_parser` crate with pre-rendering at storage time (chosen)**
   - Pros: `mail_parser` is a mature, well-tested Rust crate that handles the full complexity of RFC 5322, MIME multipart, character encoding, and transfer encoding. By parsing at storage time (when the email is intercepted), we pay the parsing cost once and store the results. The web UI serves pre-rendered HTML instantly with no parsing delay. Structured fields are available in the JSON metadata for listing and test assertions. The raw `.eml` file is preserved alongside the parsed output for debugging.
   - Cons: Adds a dependency on `mail_parser` (and transitively on its dependencies). Stores both raw and rendered forms, using more disk space (acceptable for a dev tool). If `mail_parser` has a bug, we inherit it (mitigated by the crate's maturity and active maintenance).

## Decision Outcome

Chosen option: **Use `mail_parser` crate with pre-rendering at storage time**

The parsing pipeline works as follows:

1. The `EmailInterceptor` receives a lettre `Message` via the `Transport::send_raw` trait method.
2. Raw RFC 5322 bytes are obtained from the message envelope and body.
3. `mail_parser::MessageParser` parses the raw bytes into a structured `Message` representation.
4. Shunt extracts metadata (from, to, cc, bcc, subject, date, message-id, headers) into a `MessageMetadata` struct and serializes it to `metadata.json`.
5. The HTML body (if present) is extracted and written to `content.html`. If only a plain text body exists, it is wrapped in minimal HTML with `<pre>` tags.
6. Inline images are extracted, decoded from base64/quoted-printable, and saved as files in the `attachments/` directory. References in the HTML body (`cid:` URIs) are rewritten to point to the local file paths (or served via a dedicated endpoint).
7. File attachments are similarly extracted and saved.
8. The raw bytes are saved as `raw.eml` for inspection and debugging.

**Rationale:**
- `mail_parser` is authored by the Stalwart Mail Server project and handles real-world email parsing in production mail servers. It is battle-tested against the full spectrum of email quirks and edge cases that a hand-written parser would struggle with.
- Pre-rendering at storage time means the web UI is a simple file server for the HTML preview. No parsing logic in the frontend, no latency when clicking on a message, no JavaScript email libraries needed.
- Storing the raw `.eml` file preserves the original message for cases where a developer needs to inspect headers, encoding, or other details that the rendered preview does not surface. Standard email clients can also open `.eml` files directly.
- The structured metadata in `metadata.json` supports both the web UI's list view and programmatic access from test code, without either needing to re-parse the raw email.

**Implications:**
- `shunt-email` depends on `mail_parser`, which increases the crate's dependency count. This is acceptable because email parsing is `shunt-email`'s core responsibility, and users who do not need email interception will not depend on this crate.
- If lettre changes its `Transport` trait signature or the way it exposes raw message bytes, the parsing pipeline will need to be updated. This risk is low given lettre's API stability.
- HTML email content is user-generated and potentially includes JavaScript or external resource references. The web UI must sandbox the email preview (e.g., using an iframe with appropriate sandbox attributes) to prevent script execution and external resource loading.
- The pre-rendering approach means that if we improve the rendering logic, previously stored messages will not benefit from the improvements unless re-rendered. This is acceptable for a development tool where messages are ephemeral.

## Validation

How we'll know if this was the right decision:
- Emails sent from lettre with HTML bodies, plain text bodies, inline images, and file attachments all render correctly in the web UI preview.
- Emails with non-ASCII characters (UTF-8, ISO-8859-1, Shift-JIS) display correctly without encoding artifacts.
- Multipart emails (mixed, alternative, related) are parsed and the appropriate body is displayed.
- The parsing and storage pipeline completes in under 100ms for typical development emails (under 1MB).
- No parsing-related bug reports that trace back to edge cases in our code (as opposed to upstream `mail_parser` issues).
- Test code can assert on structured metadata (sender, subject, recipient) without parsing raw email bytes.
