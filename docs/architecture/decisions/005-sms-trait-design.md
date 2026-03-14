# ADR-005: SMS Trait Design

**Date:** 2026-03-14
**Status:** Accepted
**Context:** Unlike email (where lettre provides a standard `Transport` trait), the Rust ecosystem has no standard trait or interface for sending SMS messages. Shunt needs a way to intercept SMS in development that mirrors how it intercepts emails.
**Decision:** Define a custom `SmsSender` trait with a simple async method signature. Provide `SmsInterceptor` as the development implementation that stores messages instead of sending them. Users swap their production SMS provider for `SmsInterceptor` in development via dependency injection.
**Consequences:** Users get a clean abstraction for SMS sending that works with any provider. The trait is simple enough to implement in minutes. Trade-off is that users must adopt our trait rather than an ecosystem standard (which does not exist).

## Context and Problem Statement

Shunt's email interception works by implementing lettre's existing `Transport` trait, so applications that already use lettre can swap in Shunt's `EmailInterceptor` with minimal code changes. For SMS, no equivalent exists in the Rust ecosystem:

- There is no widely adopted SMS crate analogous to lettre for email.
- SMS providers (Twilio, Vonage, AWS SNS, MessageBird) each have their own Rust SDKs with incompatible APIs.
- Some applications use HTTP clients directly to call SMS provider REST APIs, with no abstraction at all.

Shunt needs to provide an SMS interception mechanism that:

1. Is easy to adopt regardless of which SMS provider an application uses in production.
2. Has a minimal API surface that covers the common denominator of SMS functionality.
3. Allows developers to swap between real sending and interception with a single line of configuration.
4. Integrates with the same storage and web preview infrastructure used for email.

## Considered Options

1. **Provider-specific adapter crates (shunt-twilio, shunt-vonage, etc.)**
   - Pros: Could intercept at the SDK level with zero application code changes. Each adapter would wrap the specific provider's client and redirect messages to Shunt storage.
   - Cons: Requires writing and maintaining an adapter for every SMS provider. New providers require new crates. Applications using direct HTTP calls would not be covered. Enormous maintenance burden for marginal benefit. Does not solve the fundamental problem of having no common abstraction.

2. **HTTP proxy that intercepts outbound API calls**
   - Pros: Language-agnostic; could intercept SMS API calls from any application. No application code changes needed.
   - Cons: Requires configuring HTTP proxy settings. HTTPS interception requires certificate manipulation. Parsing provider-specific API request formats is fragile. Does not integrate cleanly with the Rust library approach. Overly complex for a development convenience tool.

3. **Custom `SmsSender` trait with `SmsInterceptor` implementation (chosen)**
   - Pros: Simple, idiomatic Rust. The trait has a single async method, making it trivial to implement for any provider. Applications gain a clean abstraction layer they likely should have anyway. The `SmsInterceptor` implementation stores messages to the same `MessageStore` as emails, unifying the preview experience. Dependency injection via generics or trait objects is standard Rust practice. The trait is small enough to stabilize quickly and rarely need breaking changes.
   - Cons: Requires application code changes to adopt the trait. Applications must wrap their existing SMS sending code in a trait implementation. Does not help applications that cannot modify their SMS sending code path.

## Decision Outcome

Chosen option: **Custom `SmsSender` trait with `SmsInterceptor` implementation**

The trait definition:

```rust
#[async_trait]
pub trait SmsSender: Send + Sync {
    async fn send_sms(
        &self,
        from: &str,
        to: &str,
        body: &str,
        metadata: Option<&SmsMetadata>,
    ) -> Result<SmsResponse, SmsError>;
}
```

Where `SmsMetadata` is an optional struct for provider-specific data:

```rust
pub struct SmsMetadata {
    /// Arbitrary key-value pairs for provider-specific options
    pub extra: HashMap<String, String>,
}
```

And `SmsInterceptor` implements this trait by storing messages:

```rust
pub struct SmsInterceptor {
    store: Arc<dyn MessageStore>,
}

#[async_trait]
impl SmsSender for SmsInterceptor {
    async fn send_sms(
        &self,
        from: &str,
        to: &str,
        body: &str,
        metadata: Option<&SmsMetadata>,
    ) -> Result<SmsResponse, SmsError> {
        // Store the message instead of sending it
        self.store.save(Message::sms(from, to, body, metadata)).await?;
        Ok(SmsResponse::intercepted())
    }
}
```

Usage pattern in application code:

```rust
// Production
let sms: Box<dyn SmsSender> = Box::new(TwilioSender::new(account_sid, auth_token));

// Development
let sms: Box<dyn SmsSender> = Box::new(SmsInterceptor::new(store));

// Same code path regardless of environment
sms.send_sms("+1234567890", "+0987654321", "Your code is 1234", None).await?;
```

**Rationale:**
- The trait has the smallest possible API surface that covers SMS use cases: a sender, a recipient, a body, and optional metadata. This maps directly to what every SMS provider needs and nothing more.
- The `metadata` parameter accommodates provider-specific options (e.g., Twilio's `MessagingServiceSid`, Vonage's `type` field) without polluting the core trait with provider-specific types. In practice, most SMS sends only need `from`, `to`, and `body`.
- Making the trait async with `Send + Sync` bounds ensures it works in any async runtime (tokio, async-std) and can be shared across threads, which is the standard pattern for service clients in Rust web applications.
- The pattern of swapping implementations via dependency injection is identical to how Shunt's email interception works with lettre's `Transport`. This consistency reduces the learning curve.
- Wrapping an existing SMS provider takes about 20 lines of code (implement the trait, call the provider SDK inside), making adoption low-friction.

**Implications:**
- Users must adopt the `SmsSender` trait in their application code. This is an intentional design choice: the trait encourages clean architecture by separating SMS sending concerns from business logic. The migration guide should provide examples for Twilio, Vonage, and direct HTTP implementations.
- The trait uses `&str` for phone numbers rather than a dedicated `PhoneNumber` type. This keeps the trait simple and avoids forcing users to convert between types. Validation of phone number format is the caller's responsibility, consistent with how SMS providers accept arbitrary strings and validate on their end.
- If the Rust ecosystem eventually converges on a standard SMS trait (unlikely in the near term), Shunt can provide a bridge implementation. The current trait is simple enough to adapt.
- The `SmsResponse` type should include enough information for tests to verify that a message was intercepted (e.g., the assigned message ID) but should not attempt to replicate the full response structure of any specific provider.

## Validation

How we'll know if this was the right decision:
- Users can implement the `SmsSender` trait for their SMS provider in under 30 lines of code.
- The trait signature remains stable (no breaking changes) through at least the first 3 minor version releases.
- Applications can swap between `SmsInterceptor` and their production implementation with a single line of configuration (e.g., an environment variable check).
- SMS messages appear in the same web preview UI as emails, with sender, recipient, body, and timestamp displayed correctly.
- No user feedback requests additional parameters on the core trait method (confirming that `from`, `to`, `body`, and `metadata` cover the common cases).
- At least 2 community-contributed provider implementations (e.g., Twilio, Vonage) validate that the trait is practical to implement against real SDKs.
