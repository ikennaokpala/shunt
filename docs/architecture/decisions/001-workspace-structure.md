# ADR-001: Workspace Structure

**Date:** 2026-03-14
**Status:** Accepted
**Context:** Shunt needs a modular architecture that lets users opt into only the messaging channels they care about.
**Decision:** Use a Cargo workspace with 4 focused crates (shunt-core, shunt-email, shunt-sms, shunt-web) plus a convenience re-export crate (shunt).
**Consequences:** Users pay only for what they use; each crate can evolve independently; the convenience crate keeps simple setups simple.

## Context and Problem Statement

Shunt is a development tool that intercepts outbound emails and SMS messages, storing them locally and presenting them in a browser preview. Different projects have different needs: some only send emails, some only send SMS, and some send both. The web preview UI is optional if a developer only wants to capture messages for test assertions.

We need a crate structure that:

- Allows users to depend on only the functionality they need (e.g., email interception without SMS or the web UI).
- Keeps compilation times reasonable by avoiding unnecessary dependency graphs.
- Maintains clear separation of concerns so that email parsing logic, SMS trait definitions, web server code, and shared storage types do not become entangled.
- Remains approachable for contributors by having obvious boundaries between components.

## Considered Options

1. **Single monolith crate**
   - Pros: Simplest initial setup. One `Cargo.toml`. No inter-crate dependency management. Easy for contributors to find code.
   - Cons: Users who only need email interception still pull in SMS types, the web server, and all transitive dependencies (axum, tower, etc.). Feature flags could gate functionality, but feature-flag combinatorics become unwieldy quickly. Tight coupling between unrelated subsystems makes refactoring risky. Compile times grow as all code is always compiled.

2. **Two-crate split (core + everything-else)**
   - Pros: Separates shared types from implementation. Simpler than a full workspace.
   - Cons: The "everything-else" crate still forces the web UI and SMS together. A user who wants email + web but not SMS still compiles SMS code. Does not solve the coupling problem, only moves it one level down.

3. **Four crates plus convenience re-export crate (chosen)**
   - Pros: Each crate has a single responsibility. Users declare exactly the dependencies they need. The convenience crate (`shunt`) re-exports all sub-crates for users who want everything with a single dependency line. Compile times are scoped to the crates in use. Clear ownership boundaries make the codebase easier to navigate and maintain.
   - Cons: More `Cargo.toml` files to maintain. Inter-crate version coordination requires care (mitigated by workspace-level dependency declarations). Slightly higher initial setup cost.

## Decision Outcome

Chosen option: **Four crates plus convenience re-export crate**

The workspace is structured as follows:

| Crate | Responsibility |
|---|---|
| `shunt-core` | Shared types (`Message`, `MessageId`, `Channel`), the `MessageStore` trait, filesystem storage implementation, and common error types. |
| `shunt-email` | `EmailInterceptor` that implements lettre's `Transport` trait, email parsing via `mail_parser`, and HTML rendering of email content. |
| `shunt-sms` | `SmsSender` trait definition, `SmsInterceptor` implementation, and SMS message types. |
| `shunt-web` | Axum-based web server, embedded HTML/JS UI, SSE live-update endpoint, and REST API for listing/viewing messages. |
| `shunt` | Convenience crate that re-exports all of the above, so users can write `shunt = "0.1"` and get everything. |

**Rationale:**
- A web-only project adds `shunt-email` and `shunt-web` to `[dev-dependencies]` and never touches SMS code.
- A test harness that asserts on captured messages adds only `shunt-email` (or `shunt-sms`) and `shunt-core`, avoiding the web server entirely.
- The convenience crate eliminates ceremony for users who want the full experience.
- Workspace-level `[workspace.dependencies]` ensures all crates share the same versions of common dependencies (serde, tokio, etc.).

**Implications:**
- Public API surfaces must be carefully designed at crate boundaries. Breaking changes in `shunt-core` propagate to all dependents.
- Integration tests that exercise email-to-browser workflows will live in the `shunt` convenience crate or a dedicated `tests/` workspace member, since they span multiple crates.
- Documentation must clearly guide users on which crates to depend on for their use case.

## Validation

How we'll know if this was the right decision:
- Users can add a single crate (`shunt-email`) and intercept emails without compiling web server or SMS code.
- Adding a new messaging channel (e.g., push notifications) requires a new crate without modifying existing ones.
- Compile times for individual crates remain under 10 seconds on a typical developer machine.
- No circular dependencies emerge between crates.
- The convenience crate's re-export surface stays thin (just `pub use` statements and optional glue code).
