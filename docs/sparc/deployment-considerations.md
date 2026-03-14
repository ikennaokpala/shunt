# Deployment Considerations

**Project:** shunt
**Date:** 2026-03-14
**Framework:** SPARC (Situation, Problem, Analysis, Recommendation, Conclusion)

---

## Situation

Shunt is a Rust library published to [crates.io](https://crates.io) and consumed as a `[dev-dependency]` by other Rust projects. Unlike a web application or microservice, shunt is not "deployed" in the traditional sense. Instead, its deployment concerns are:

1. **Publishing** crate versions to crates.io so that downstream projects can depend on them.
2. **Cross-platform compatibility** so that developers on Linux, macOS, and Windows can all use the library.
3. **CI/CD** for the shunt project itself, ensuring that every change is tested before it reaches crates.io.
4. **Container compatibility** so that shunt works correctly inside Docker containers and CI runners.

The library must work seamlessly in all of these environments without requiring platform-specific instructions or workarounds.

---

## Problem

Several deployment and distribution challenges must be addressed:

1. **Workspace publishing order:** The Cargo workspace contains five crates with inter-dependencies. Publishing them in the wrong order causes resolution failures for downstream users. `shunt_core` must be published before `shunt_email`, `shunt_sms`, and `shunt_web`, which must all be published before the convenience crate `shunt`.

2. **Version coordination:** All crates in the workspace must maintain compatible version constraints. A version bump in `shunt_core` must be reflected in the dependency specifications of all crates that depend on it.

3. **Cross-platform file I/O:** The `FileStore` writes JSON files to disk. Path separators, file permissions, temporary directory locations, and filesystem case sensitivity vary across operating systems.

4. **Browser opening:** The preview server optionally opens the developer's default browser. The mechanism for this differs across Linux (`xdg-open`), macOS (`open`), and Windows (`start`).

5. **Headless environments:** CI runners and Docker containers have no display server. The browser-open feature must degrade gracefully (skip without error) when no display is available.

6. **Minimum Supported Rust Version (MSRV):** The library must declare and enforce a minimum Rust version to prevent downstream projects from encountering unexpected compilation failures.

---

## Analysis

### Crate Publishing Topology

The dependency graph determines the required publishing order:

```
shunt_core          (no internal deps)
    ▲
    │
    ├── shunt_email (depends on shunt_core)
    ├── shunt_sms   (depends on shunt_core)
    └── shunt_web   (depends on shunt_core)
            ▲
            │
            └── shunt (depends on all four)
```

**Publishing order:**
1. `shunt_core`
2. `shunt_email`, `shunt_sms`, `shunt_web` (can be published in parallel)
3. `shunt` (convenience crate, published last)

### Version Strategy

All crates in the workspace share the same version number during initial development (0.x series). This simplifies communication and reduces cognitive overhead for users. The workspace `Cargo.toml` declares the version once, and member crates inherit it:

```toml
# Workspace Cargo.toml
[workspace.package]
version = "0.1.0"

# Member Cargo.toml
[package]
version.workspace = true
```

Inter-crate dependencies use exact version constraints during the 0.x series:

```toml
# In shunt_email/Cargo.toml
[dependencies]
shunt_core = { version = "=0.1.0", path = "../shunt_core" }
```

After reaching 1.0, dependencies will relax to minor-version ranges (`"^1.0"`) to allow compatible updates.

### Cross-Platform Compatibility

| Concern                | Linux                 | macOS                | Windows               | Solution                          |
|------------------------|-----------------------|----------------------|------------------------|-----------------------------------|
| Path separators        | `/`                   | `/`                  | `\`                    | Use `std::path::Path` everywhere; never construct paths with string concatenation. |
| Temp directories       | `/tmp`                | `/var/folders/...`   | `%TEMP%`               | Use `tempfile::TempDir` which abstracts platform differences. |
| File permissions       | Unix permissions      | Unix permissions     | ACLs                   | Do not set explicit permissions; rely on OS defaults. |
| Case sensitivity       | Case-sensitive        | Case-insensitive (default) | Case-insensitive | Use UUIDs for filenames, which are inherently unique regardless of case. |
| Line endings           | LF                    | LF                   | CRLF                   | Serialize JSON without trailing newlines; use `serde_json` which is consistent. |
| Browser opening        | `xdg-open`            | `open`               | `start`                | Use the `open` crate which abstracts all three. |

### Browser Opening in Headless Environments

The `open` crate attempts to launch the default browser. In headless environments (CI, Docker, SSH sessions), this fails silently or returns an error. Shunt handles this gracefully:

```rust
// Attempt to open browser; log but do not fail if it cannot
if let Err(e) = open::that(format!("http://localhost:{}", port)) {
    tracing::info!(
        "Could not open browser (headless environment?): {}. \
         Preview is available at http://localhost:{}",
        e,
        port
    );
}
```

The preview server starts regardless of whether the browser opens. The developer can always navigate to the URL manually.

### MSRV Policy

Shunt targets the Rust version from approximately 6 months prior to the current stable release. This balances access to recent language features with compatibility for projects that do not update Rust immediately.

The MSRV is declared in the workspace `Cargo.toml`:

```toml
[workspace.package]
rust-version = "1.75"
```

The MSRV is enforced in CI by testing against both the MSRV and the latest stable release.

### CI Pipeline (GitHub Actions)

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  check:
    name: Check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo check --workspace --all-targets

  test:
    name: Test (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --workspace

  msrv:
    name: MSRV
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: "1.75"
      - run: cargo check --workspace --all-targets

  fmt:
    name: Format
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - run: cargo fmt --all -- --check

  clippy:
    name: Clippy
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - run: cargo clippy --workspace --all-targets -- -D warnings

  docs:
    name: Documentation
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo doc --workspace --no-deps
        env:
          RUSTDOCFLAGS: -D warnings
```

### Container Considerations

Shunt works inside Docker containers with the following considerations:

- **No display server:** Browser opening is skipped automatically (see above).
- **Ephemeral filesystems:** The default store path (`.shunt/`) is inside the container filesystem. If the container is ephemeral, messages are lost when it stops. For CI, this is expected and desirable.
- **Port exposure:** If the preview server is needed inside a container (e.g., for integration testing in CI), the port must be exposed in the `Dockerfile` or `docker-compose.yml`. However, most CI use cases will access the API programmatically within the same container, making port exposure unnecessary.
- **Permissions:** The default `FileStore` writes with the process UID's default permissions. No `chmod` or `chown` is needed.

---

## Recommendation

### Publishing Checklist

Before publishing a new version to crates.io:

- [ ] All CI checks pass on main (check, test, msrv, fmt, clippy, docs).
- [ ] Version number bumped in workspace `Cargo.toml`.
- [ ] Inter-crate version constraints updated to match new version.
- [ ] `CHANGELOG.md` updated with release notes.
- [ ] `cargo publish --dry-run` succeeds for all crates.
- [ ] Crates published in dependency order: `shunt_core` first, `shunt` last.

### Publishing Commands

```bash
# Dry run (verify everything builds and packages correctly)
cargo publish -p shunt_core --dry-run
cargo publish -p shunt_email --dry-run
cargo publish -p shunt_sms --dry-run
cargo publish -p shunt_web --dry-run
cargo publish -p shunt --dry-run

# Actual publish (in order)
cargo publish -p shunt_core
cargo publish -p shunt_email
cargo publish -p shunt_sms
cargo publish -p shunt_web
cargo publish -p shunt
```

### Feature Flags

Optional functionality is gated behind feature flags to keep the default dependency footprint minimal:

| Feature Flag     | Crate        | What It Enables                                    | Default |
|-----------------|--------------|----------------------------------------------------|---------|
| `browser-open`  | `shunt_web`  | Automatically open browser when preview server starts | On    |
| `sse`           | `shunt_web`  | Server-Sent Events for live message updates         | On      |

Features are declared in the crate's `Cargo.toml`:

```toml
[features]
default = ["browser-open", "sse"]
browser-open = ["dep:open"]
sse = []
```

Users in headless environments can disable browser opening:

```toml
[dev-dependencies]
shunt_web = { version = "0.1", default-features = false, features = ["sse"] }
```

---

## Conclusion

Shunt's deployment model is crate publication, not service deployment. The primary concerns are cross-platform compatibility, correct crate publishing order, version coordination, and graceful degradation in headless environments.

The CI pipeline tests on all three major platforms (Linux, macOS, Windows), enforces the MSRV, and runs formatting and linting checks. Feature flags allow users to tailor the dependency footprint to their environment.

Publishing follows a strict dependency-order protocol to ensure that crates.io users always resolve a consistent set of versions. The workspace-level version declaration and inter-crate version constraints make coordinated releases straightforward.

The library requires no runtime configuration, no environment variables, and no external services. It works out of the box on any platform where `cargo build` succeeds.
