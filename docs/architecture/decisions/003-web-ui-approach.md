# ADR-003: Web UI Approach

**Date:** 2026-03-14
**Status:** Accepted
**Context:** Shunt needs a browser-based preview UI for intercepted messages that is easy to ship as part of a Rust binary with no external build tooling.
**Decision:** Embed a single-file HTML page with vanilla JavaScript using `rust-embed`, served by the Axum web server. Use Server-Sent Events (SSE) for live updates.
**Consequences:** Zero frontend build step, single binary distribution, instant page loads from embedded assets. Trade-off is a less sophisticated UI compared to a full SPA framework.

## Context and Problem Statement

Shunt's web UI allows developers to browse intercepted emails and SMS messages in their browser. The UI must:

1. List all captured messages with sender, recipient, subject (or body preview for SMS), and timestamp.
2. Display full message content, including rendered HTML emails with inline images.
3. Update in real time when new messages arrive, without requiring a manual refresh.
4. Ship as part of the Rust binary with no separate frontend build or deployment step.
5. Work offline and without a CDN, since it is a local development tool.

The UI is intentionally simple: it is a developer debugging tool, not a production email client. Functionality and reliability matter more than visual polish.

## Considered Options

1. **React/Vue/Svelte SPA with a build pipeline**
   - Pros: Rich component model for complex UI interactions. Large ecosystem of UI libraries. Familiar to many frontend developers. Could support advanced features like search, filtering, and keyboard shortcuts with less effort.
   - Cons: Requires Node.js and a frontend build step (webpack/vite/esbuild), adding complexity to the Rust project's build process. The built assets must be embedded or served separately. Development requires running both a Rust server and a frontend dev server. Increases the barrier to contribution for Rust developers who may not have Node.js tooling. The bundle size is significant for what is fundamentally a simple list-and-detail UI.

2. **Askama (Jinja-like) server-rendered templates**
   - Pros: Templates are compiled into the Rust binary at build time. No JavaScript required for basic rendering. Familiar template syntax. Type-safe template variables.
   - Cons: Every navigation action requires a full page round-trip to the server. Live updates require either polling or a separate SSE/WebSocket mechanism layered on top. Viewing a message replaces the message list, requiring a back-button navigation. The user experience feels sluggish compared to client-side rendering. Complex interactions (toggling between HTML/plain text view, showing headers) require either JavaScript anyway or additional server endpoints.

3. **Embedded single-file HTML with vanilla JavaScript (chosen)**
   - Pros: The entire UI is a single HTML file with inline CSS and JavaScript, embedded into the binary via `rust-embed`. No build step of any kind. The file can be edited with any text editor and previewed in a browser during development. Vanilla JavaScript has zero dependencies, zero bundling, and works in all modern browsers. SSE provides a simple, native browser API for live updates. The embedded file is typically under 30KB, so page loads are instant. Contributors can modify the UI without any frontend tooling.
   - Cons: No component abstraction; UI logic lives in plain DOM manipulation. Vanilla JavaScript is more verbose than framework code for complex UI interactions. No type safety in the JavaScript layer. If the UI grows significantly in complexity, this approach will become harder to maintain.

## Decision Outcome

Chosen option: **Embedded single-file HTML with vanilla JavaScript**

The implementation works as follows:

- A single `index.html` file contains all markup, styles, and JavaScript.
- `rust-embed` embeds this file into the compiled binary at build time, so `shunt-web` ships as a single binary with no external assets.
- The Axum server serves the embedded HTML at the root path (`/`).
- A REST API (`/api/messages`, `/api/messages/:id`) provides message data as JSON.
- An SSE endpoint (`/api/events`) pushes new-message notifications to the browser.
- The JavaScript fetches the message list on load, renders it in the left panel, and displays selected message content in the right panel (two-pane layout).
- When an SSE event arrives, the new message is prepended to the list without a full reload.

**Rationale:**
- Shunt is a Rust library. Its users are Rust developers. Requiring Node.js and a frontend build pipeline to contribute to or build the project would be a significant friction point that contradicts the tool's purpose of simplifying development workflows.
- The UI requirements are genuinely simple: list messages, show message detail, live updates. This does not warrant a framework. Vanilla JavaScript handles this in under 500 lines.
- `rust-embed` is a well-maintained crate that adds negligible compile-time overhead and produces a self-contained binary. This is the same approach used by other successful Rust dev tools (e.g., trunk, miniserve).
- SSE is a better fit than WebSockets for this use case because the communication is unidirectional (server to client). SSE is simpler to implement, automatically reconnects on disconnection, and requires no additional dependencies beyond what Axum already provides.
- Ruby's letter_opener opens emails directly in the browser as HTML files. Shunt improves on this by providing a persistent list view with live updates, while keeping the same zero-dependency philosophy.

**Implications:**
- The UI should be kept intentionally simple. If requirements grow to include features like message search, filtering by date range, or multi-account support, we should re-evaluate this decision.
- The single-file approach means no CSS preprocessor and no module system. Styles and scripts should be organized with clear comments and kept under reasonable size (target: under 50KB total for the HTML file).
- Browser compatibility targets modern evergreen browsers only (Chrome, Firefox, Safari, Edge). No IE11 or legacy browser support.
- The SSE connection should include heartbeat messages to detect stale connections and trigger reconnection.

## Validation

How we'll know if this was the right decision:
- The web UI loads in under 100ms from the embedded assets (no network dependency).
- Contributors can modify the UI by editing a single HTML file and rebuilding the Rust binary, with no additional tooling required.
- The `shunt-web` crate compiles in under 15 seconds, confirming that the embedded assets do not bloat build times.
- Live updates via SSE reliably display new messages within 1 second of interception.
- No user feedback indicates that the UI is too limited for the core use case of browsing intercepted messages.
- The HTML file remains under 50KB and under 1,000 lines, confirming that complexity is contained.
