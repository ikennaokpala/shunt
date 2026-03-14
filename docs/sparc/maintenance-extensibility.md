# Maintenance and Extensibility

**Project:** shunt
**Date:** 2026-03-14
**Framework:** SPARC (Situation, Problem, Analysis, Recommendation, Conclusion)

---

## Situation

Shunt depends on several external Rust crates, most notably `lettre` for email transport trait definitions and `axum` for the preview web server. These dependencies evolve on their own release schedules, and breaking changes in their public APIs directly affect shunt's implementation.

At the same time, shunt must remain extensible. New communication channels (push notifications, webhook capture, chat messages) may be added in the future. New storage backends (SQLite, Redis, in-memory) may be needed for different use cases. New UI features (search, filtering, export) will be requested as adoption grows.

The architecture must accommodate this evolution without requiring rewrites of existing, stable code.

---

## Problem

Several maintenance and extensibility risks must be addressed:

1. **lettre API stability:** lettre's `AsyncTransport` trait is the foundation of shunt's email interception. A breaking change in this trait (new methods, changed signatures, altered associated types) would require changes to `shunt_email`. Since lettre is the de facto Rust email crate, shunt cannot simply switch to an alternative.

2. **axum version churn:** axum has historically made breaking changes between minor versions (0.6 to 0.7, for example). Since `shunt_web` builds its server on axum, major axum updates require migration effort.

3. **Adding new channels:** Adding support for a new communication channel (e.g., push notifications) should not require modifying `shunt_core`, `shunt_email`, `shunt_sms`, or `shunt_web`. The cost of adding a channel should be proportional to the channel's complexity, not to the size of the existing codebase.

4. **Adding new storage backends:** The `MessageStore` trait defines the persistence contract. A new backend (e.g., SQLite for better querying) should be addable without changing any code that writes to or reads from the store.

5. **UI evolution:** The embedded UI is compiled into the binary. Changes to the UI require recompiling `shunt_web`. The UI should be structured so that adding support for a new message type (channel) requires minimal changes.

6. **Semver compliance:** As a published crate, shunt must follow semantic versioning. Understanding which changes are breaking and which are additive is critical for version management.

---

## Analysis

### Dependency Version Strategy

#### lettre

lettre is shunt's most critical external dependency. The `AsyncTransport` trait is the integration point that makes shunt useful for email.

**Current approach:** Pin to a minor version range in `Cargo.toml`:

```toml
[dependencies]
lettre = { version = "0.11", default-features = false, features = ["async-std1", "builder"] }
```

**Rationale for minor-version pinning:**
- lettre follows semver. Within `0.11.x`, the `AsyncTransport` trait signature is stable.
- Pinning to `0.11` (not `0.11.3`) allows patch updates automatically.
- When lettre releases `0.12` with breaking changes, shunt can evaluate the impact and release a new major or minor version accordingly.

**Migration strategy for lettre breaking changes:**
1. Track lettre's changelog and pre-release announcements.
2. When a new major/minor version is released, assess the impact on `AsyncTransport`.
3. If the trait signature changes, implement the new version in a branch.
4. Release a new shunt version with updated lettre support.
5. If necessary, maintain a compatibility matrix (shunt 0.1.x supports lettre 0.11, shunt 0.2.x supports lettre 0.12).

#### axum

axum is used only in `shunt_web` and does not leak into the public API. This containment limits the blast radius of axum upgrades.

**Current approach:** Pin to minor version range:

```toml
[dependencies]
axum = "0.8"
```

**Migration strategy for axum breaking changes:**
1. axum changes are contained to `shunt_web`.
2. Handler signatures and routing are internal implementation details.
3. The REST API contract (URL paths, request/response formats) is shunt's public interface, not axum's.
4. Upgrading axum requires changing handler code but not the API contract.

#### Other Dependencies

| Dependency    | Stability | Risk  | Strategy                                          |
|--------------|-----------|-------|---------------------------------------------------|
| `serde`      | Very high | Low   | Pin to `1.x`. Extremely stable API.               |
| `tokio`      | High      | Low   | Pin to `1.x`. Runtime API is stable.              |
| `uuid`       | High      | Low   | Pin to `1.x`. Simple API surface.                 |
| `chrono`     | High      | Low   | Pin to `0.4.x`. Well-established API.             |
| `mail-parser`| Medium    | Medium| Pin to minor version. Less mature than lettre.    |
| `rust-embed` | Medium    | Low   | Pin to `8.x`. Internal to `shunt_web`.            |
| `thiserror`  | Very high | Low   | Pin to `2.x`. Derive macro, minimal API surface.  |

### Extensibility Architecture

#### Adding a New Channel

The architecture is designed so that adding a new channel follows a cookie-cutter pattern. Here is the process for adding hypothetical push notification support:

**Step 1:** Add content type to `shunt_core` (the only core change required):

```rust
// In shunt_core/src/types.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageKind {
    Email,
    Sms,
    Push,  // New variant
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum MessageContent {
    Email(EmailContent),
    Sms(SmsContent),
    Push(PushContent),  // New variant
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushContent {
    pub device_token: String,
    pub title: String,
    pub body: String,
    pub data: HashMap<String, serde_json::Value>,
}
```

**Step 2:** Create a new crate:

```
crates/shunt_push/
├── Cargo.toml
└── src/
    └── lib.rs    # PushInterceptor implementing a PushSender trait
```

**Step 3:** Update the convenience crate to re-export the new crate.

**What does NOT change:**
- `shunt_email` -- no modifications needed.
- `shunt_sms` -- no modifications needed.
- `shunt_web` -- the REST API returns `MessageContent` as JSON; the new variant is automatically serialized. The UI may need a new rendering template for push notifications, but the API layer requires no changes.
- `FileStore` -- stores `ShuntedMessage` as JSON; the new content variant is handled by serde automatically.

**Estimated effort:** 1-2 days for the new crate, minimal changes to `shunt_core`, optional UI template in `shunt_web`.

#### Adding a New Storage Backend

The `MessageStore` trait is the extension point for storage:

```rust
#[async_trait]
pub trait MessageStore: Send + Sync {
    async fn store(&self, message: &ShuntedMessage) -> Result<Uuid>;
    async fn get(&self, id: Uuid) -> Result<Option<ShuntedMessage>>;
    async fn list(&self) -> Result<Vec<ShuntedMessage>>;
    async fn delete(&self, id: Uuid) -> Result<()>;
    async fn clear(&self) -> Result<()>;
}
```

A new backend (e.g., `SqliteStore`) implements this trait and can be used as a drop-in replacement:

```rust
// In a new crate or behind a feature flag
pub struct SqliteStore {
    pool: SqlitePool,
}

#[async_trait]
impl MessageStore for SqliteStore {
    async fn store(&self, message: &ShuntedMessage) -> Result<Uuid> {
        // INSERT into messages table
    }
    // ... other methods
}
```

**What does NOT change:**
- `EmailInterceptor` and `SmsInterceptor` accept `Arc<dyn MessageStore>`, so they work with any backend.
- `shunt_web` accepts `Arc<dyn MessageStore>`, so the preview server works with any backend.
- No conditional compilation or feature-flag gymnastics needed for the basic case.

#### Adding UI Features

The embedded UI is structured as static files:

```
crates/shunt_web/ui/
├── index.html
├── style.css
└── app.js
```

Adding a new UI feature (e.g., search, filtering by date range, message export) involves:

1. Modify the HTML/CSS/JS files.
2. If new API endpoints are needed, add them to the axum router in `shunt_web`.
3. Recompile `shunt_web` (rust-embed picks up the changed files automatically).

The UI is intentionally framework-free (vanilla JS) to avoid frontend build toolchain dependencies and to keep the contribution barrier low.

### Semver Impact Analysis

Understanding which changes are breaking is essential for correct versioning:

| Change                                      | Breaking? | Version Bump |
|---------------------------------------------|-----------|--------------|
| Add new variant to `MessageKind`             | Yes       | Minor (0.x) or Major (1.x+) |
| Add new variant to `MessageContent`          | Yes       | Minor (0.x) or Major (1.x+) |
| Add new field to `EmailContent`              | Yes (struct is not `#[non_exhaustive]`) | Minor/Major |
| Add new method to `MessageStore` trait       | Yes       | Minor/Major  |
| Change `MessageStore` method signature       | Yes       | Minor/Major  |
| Add new optional field to `MessageSummary`   | Yes (unless `#[non_exhaustive]`) | Minor/Major |
| Add new REST API endpoint                    | No        | Patch/Minor  |
| Change REST API response format              | Yes (for API consumers) | Minor/Major |
| Add new feature flag                         | No        | Patch/Minor  |
| Bump dependency version (internal change)    | No (if API unchanged) | Patch |

**Mitigation:** Use `#[non_exhaustive]` on enums and key structs to allow additive changes without semver breaks:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MessageKind {
    Email,
    Sms,
}
```

With `#[non_exhaustive]`, adding a `Push` variant is not a breaking change because downstream code is already required to handle unknown variants (via a wildcard match arm).

### Deprecation Process

When functionality needs to be removed or replaced:

1. **Mark as deprecated** with `#[deprecated(since = "0.3.0", note = "Use X instead")]`.
2. **Document the migration path** in the changelog and rustdoc.
3. **Maintain for at least one minor version** before removal.
4. **Remove in the next major version** (or next minor version during 0.x development).

---

## Recommendation

### Architectural Principles for Maintainability

1. **Traits at every boundary.** `MessageStore`, `SmsSender`, and lettre's `AsyncTransport` are the extension points. New implementations can be added without modifying existing code.

2. **`#[non_exhaustive]` on public enums and key structs.** This allows adding variants and fields in minor versions without breaking downstream code.

3. **Contain external dependencies.** lettre types do not appear in `shunt_core`. axum types do not appear outside `shunt_web`. If an external dependency makes a breaking change, only the containing crate needs updating.

4. **One crate per channel.** Adding a new channel means adding a new crate, not modifying existing ones. The `shunt_core` types may need a new enum variant (mitigated by `#[non_exhaustive]`), but no existing crate logic changes.

5. **Workspace-level dependency versions.** All crates share the same versions of common dependencies, preventing version conflicts and ensuring consistent behaviour.

6. **Minimal public API surface.** Expose only what users need. Internal types and functions are `pub(crate)` or private. A smaller API surface means fewer breaking changes to manage.

### Dependency Update Schedule

| Frequency   | Action                                                       |
|-------------|--------------------------------------------------------------|
| Weekly      | Run `cargo update` to pick up patch releases.                |
| Monthly     | Review `cargo outdated` for new minor/major versions.        |
| Per release | Audit `cargo deny` for security advisories and license issues. |
| As needed   | Respond to security advisories within 48 hours.              |

### Monitoring External Changes

- **lettre:** Watch the [lettre GitHub repository](https://github.com/lettre/lettre) releases and changelogs. Subscribe to release notifications.
- **axum:** Watch the [axum GitHub repository](https://github.com/tokio-rs/axum) for pre-release announcements of breaking changes.
- **RustSec Advisory Database:** Run `cargo audit` in CI to catch known vulnerabilities in dependencies.

---

## Conclusion

Shunt's extensibility is grounded in three architectural decisions:

1. **Trait-based boundaries** (`MessageStore`, `SmsSender`, `AsyncTransport`) allow new implementations without modifying existing code.
2. **One crate per channel** isolates channel-specific logic and ensures that adding a new channel is an additive operation.
3. **Dependency containment** limits the blast radius of external API changes to a single crate.

Adding a new message type (channel) requires creating a new crate, adding a variant to `MessageKind` and `MessageContent` (non-breaking with `#[non_exhaustive]`), and optionally adding a UI template. No changes to existing interceptors, the storage layer, or the web server's API logic are needed.

Maintaining compatibility with lettre is the highest-risk dependency concern. The mitigation is minor-version pinning, proactive changelog monitoring, and architectural containment (lettre types do not leak into `shunt_core`). When lettre makes a breaking change, only `shunt_email` needs updating, and the rest of the workspace remains stable.

The library is designed to grow channel by channel, backend by backend, without accumulating cross-cutting complexity. Each new addition follows the same pattern, and each existing component remains untouched.
