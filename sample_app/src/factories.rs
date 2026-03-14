use fabricate::builder::BuildableFactory;
use fabricate::traits::TraitRegistry;
use fabricate::{FactoryContext, FactoryTrait, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestUser {
    pub id: String,
    pub email: String,
    pub phone: String,
    pub full_name: String,
    pub role: String,
    pub is_verified: bool,
    pub locale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestNotification {
    pub id: String,
    pub user_id: String,
    pub channel: String, // "email" or "sms"
    pub subject: Option<String>,
    pub body: String,
    pub html_body: Option<String>,
    pub to: String,
    pub metadata: std::collections::HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// User factory
// ---------------------------------------------------------------------------

pub struct UserFactory {
    traits: TraitRegistry<TestUser>,
}

impl UserFactory {
    pub fn new() -> Self {
        let mut traits = TraitRegistry::new();
        traits.register(Box::new(VerifiedTrait));
        traits.register(Box::new(UnverifiedTrait));
        traits.register(Box::new(DriverTrait));
        traits.register(Box::new(AdminTrait));
        traits.register(Box::new(JapaneseLocaleTrait));
        traits.register(Box::new(ArabicLocaleTrait));
        traits.register(Box::new(EmojiNameTrait));
        traits.register(Box::new(LongNameTrait));
        traits.register(Box::new(SpecialCharEmailTrait));
        Self { traits }
    }
}

impl BuildableFactory<TestUser> for UserFactory {
    fn build_base(&self, ctx: &mut FactoryContext) -> TestUser {
        TestUser {
            id: uuid::Uuid::new_v4().to_string(),
            email: ctx.email("user"),
            phone: ctx.phone(),
            full_name: ctx.full_name(),
            role: "passenger".to_string(),
            is_verified: false,
            locale: "en".to_string(),
        }
    }

    fn trait_registry(&self) -> &TraitRegistry<TestUser> {
        &self.traits
    }

    fn apply_overrides(&self, entity: &mut TestUser, overrides: &[(String, Value)]) {
        for (field, value) in overrides {
            match field.as_str() {
                "email" => {
                    if let Some(v) = value.as_str() {
                        entity.email = v.to_string();
                    }
                }
                "phone" => {
                    if let Some(v) = value.as_str() {
                        entity.phone = v.to_string();
                    }
                }
                "full_name" => {
                    if let Some(v) = value.as_str() {
                        entity.full_name = v.to_string();
                    }
                }
                "role" => {
                    if let Some(v) = value.as_str() {
                        entity.role = v.to_string();
                    }
                }
                "locale" => {
                    if let Some(v) = value.as_str() {
                        entity.locale = v.to_string();
                    }
                }
                _ => {}
            }
        }
    }

    async fn persist(&self, entity: TestUser, _ctx: &mut FactoryContext) -> Result<TestUser> {
        Ok(entity)
    }
}

// User traits

struct VerifiedTrait;
impl FactoryTrait<TestUser> for VerifiedTrait {
    fn name(&self) -> &str {
        "verified"
    }
    fn apply(&self, user: &mut TestUser) {
        user.is_verified = true;
    }
}

struct UnverifiedTrait;
impl FactoryTrait<TestUser> for UnverifiedTrait {
    fn name(&self) -> &str {
        "unverified"
    }
    fn apply(&self, user: &mut TestUser) {
        user.is_verified = false;
    }
}

struct DriverTrait;
impl FactoryTrait<TestUser> for DriverTrait {
    fn name(&self) -> &str {
        "driver"
    }
    fn apply(&self, user: &mut TestUser) {
        user.role = "driver".to_string();
    }
}

struct AdminTrait;
impl FactoryTrait<TestUser> for AdminTrait {
    fn name(&self) -> &str {
        "admin"
    }
    fn apply(&self, user: &mut TestUser) {
        user.role = "admin".to_string();
        user.is_verified = true;
    }
}

struct JapaneseLocaleTrait;
impl FactoryTrait<TestUser> for JapaneseLocaleTrait {
    fn name(&self) -> &str {
        "japanese"
    }
    fn apply(&self, user: &mut TestUser) {
        user.locale = "ja".to_string();
        user.full_name = "\u{7530}\u{4e2d}\u{592a}\u{90ce}".to_string(); // 田中太郎
    }
}

struct ArabicLocaleTrait;
impl FactoryTrait<TestUser> for ArabicLocaleTrait {
    fn name(&self) -> &str {
        "arabic"
    }
    fn apply(&self, user: &mut TestUser) {
        user.locale = "ar".to_string();
        user.full_name = "\u{0645}\u{062d}\u{0645}\u{062f} \u{0623}\u{062d}\u{0645}\u{062f}".to_string(); // محمد أحمد
    }
}

struct EmojiNameTrait;
impl FactoryTrait<TestUser> for EmojiNameTrait {
    fn name(&self) -> &str {
        "emoji_name"
    }
    fn apply(&self, user: &mut TestUser) {
        user.full_name = "J\u{00f6}hn \u{1f600} D\u{00f6}e".to_string(); // Jöhn 😀 Döe
    }
}

struct LongNameTrait;
impl FactoryTrait<TestUser> for LongNameTrait {
    fn name(&self) -> &str {
        "long_name"
    }
    fn apply(&self, user: &mut TestUser) {
        user.full_name = "A".repeat(500);
    }
}

struct SpecialCharEmailTrait;
impl FactoryTrait<TestUser> for SpecialCharEmailTrait {
    fn name(&self) -> &str {
        "special_email"
    }
    fn apply(&self, user: &mut TestUser) {
        user.email = "user+tag@sub.domain.example.com".to_string();
    }
}

// ---------------------------------------------------------------------------
// Notification factory
// ---------------------------------------------------------------------------

pub struct NotificationFactory {
    traits: TraitRegistry<TestNotification>,
}

impl NotificationFactory {
    pub fn new() -> Self {
        let mut traits = TraitRegistry::new();
        traits.register(Box::new(EmailChannelTrait));
        traits.register(Box::new(SmsChannelTrait));
        traits.register(Box::new(HtmlEmailTrait));
        traits.register(Box::new(LongBodyTrait));
        traits.register(Box::new(EmptyBodyTrait));
        traits.register(Box::new(UnicodeBodyTrait));
        traits.register(Box::new(XssBodyTrait));
        Self { traits }
    }
}

impl BuildableFactory<TestNotification> for NotificationFactory {
    fn build_base(&self, ctx: &mut FactoryContext) -> TestNotification {
        let n = ctx.sequence("notification");
        TestNotification {
            id: uuid::Uuid::new_v4().to_string(),
            user_id: uuid::Uuid::new_v4().to_string(),
            channel: "email".to_string(),
            subject: Some(format!("Notification #{}", n)),
            body: format!("This is notification body #{}", n),
            html_body: None,
            to: ctx.email("recipient"),
            metadata: std::collections::HashMap::new(),
        }
    }

    fn trait_registry(&self) -> &TraitRegistry<TestNotification> {
        &self.traits
    }

    fn apply_overrides(
        &self,
        entity: &mut TestNotification,
        overrides: &[(String, Value)],
    ) {
        for (field, value) in overrides {
            match field.as_str() {
                "to" => {
                    if let Some(v) = value.as_str() {
                        entity.to = v.to_string();
                    }
                }
                "subject" => {
                    entity.subject = value.as_str().map(|s| s.to_string());
                }
                "body" => {
                    if let Some(v) = value.as_str() {
                        entity.body = v.to_string();
                    }
                }
                "channel" => {
                    if let Some(v) = value.as_str() {
                        entity.channel = v.to_string();
                    }
                }
                _ => {}
            }
        }
    }

    async fn persist(
        &self,
        entity: TestNotification,
        _ctx: &mut FactoryContext,
    ) -> Result<TestNotification> {
        Ok(entity)
    }
}

// Notification traits

struct EmailChannelTrait;
impl FactoryTrait<TestNotification> for EmailChannelTrait {
    fn name(&self) -> &str {
        "email"
    }
    fn apply(&self, n: &mut TestNotification) {
        n.channel = "email".to_string();
    }
}

struct SmsChannelTrait;
impl FactoryTrait<TestNotification> for SmsChannelTrait {
    fn name(&self) -> &str {
        "sms"
    }
    fn apply(&self, n: &mut TestNotification) {
        n.channel = "sms".to_string();
        n.subject = None;
        n.html_body = None;
    }
}

struct HtmlEmailTrait;
impl FactoryTrait<TestNotification> for HtmlEmailTrait {
    fn name(&self) -> &str {
        "html"
    }
    fn apply(&self, n: &mut TestNotification) {
        n.html_body = Some(format!(
            "<html><body><h1>{}</h1><p>{}</p></body></html>",
            n.subject.as_deref().unwrap_or(""),
            n.body
        ));
    }
}

struct LongBodyTrait;
impl FactoryTrait<TestNotification> for LongBodyTrait {
    fn name(&self) -> &str {
        "long_body"
    }
    fn apply(&self, n: &mut TestNotification) {
        n.body = "Lorem ipsum dolor sit amet. ".repeat(1000);
    }
}

struct EmptyBodyTrait;
impl FactoryTrait<TestNotification> for EmptyBodyTrait {
    fn name(&self) -> &str {
        "empty_body"
    }
    fn apply(&self, n: &mut TestNotification) {
        n.body = String::new();
    }
}

struct UnicodeBodyTrait;
impl FactoryTrait<TestNotification> for UnicodeBodyTrait {
    fn name(&self) -> &str {
        "unicode"
    }
    fn apply(&self, n: &mut TestNotification) {
        n.body = "Hello \u{1f30d}\u{1f30e}\u{1f30f}! \u{2764}\u{fe0f} \u{00e9}\u{00e8}\u{00ea} \u{00fc}\u{00f6}\u{00e4} \u{4f60}\u{597d} \u{0645}\u{0631}\u{062d}\u{0628}\u{0627} \u{d55c}\u{ad6d}\u{c5b4}".to_string();
    }
}

struct XssBodyTrait;
impl FactoryTrait<TestNotification> for XssBodyTrait {
    fn name(&self) -> &str {
        "xss"
    }
    fn apply(&self, n: &mut TestNotification) {
        n.body = "<script>alert('xss')</script><img src=x onerror=alert(1)>".to_string();
        n.html_body = Some(
            "<html><body><p>Hello</p><script>alert('xss')</script></body></html>".to_string(),
        );
    }
}

// ---------------------------------------------------------------------------
// Helper to create a context for in-memory builds
// ---------------------------------------------------------------------------

pub fn test_context() -> FactoryContext {
    FactoryContext {
        sequences: fabricate::Sequence::new(),
        http_client: None,
        base_url: None,
        test_key: "test-key".to_string(),
        overrides: std::collections::HashMap::new(),
    }
}
