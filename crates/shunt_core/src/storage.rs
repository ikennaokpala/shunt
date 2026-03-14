use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs;
use uuid::Uuid;

use crate::error::Result;
use crate::types::ShuntedMessage;
use crate::ShuntConfig;

/// Trait for storing and retrieving shunted messages.
#[async_trait]
pub trait MessageStore: Send + Sync {
    /// Store a shunted message and return its ID.
    async fn store(&self, message: &ShuntedMessage) -> Result<Uuid>;

    /// Retrieve a message by ID.
    async fn get(&self, id: Uuid) -> Result<ShuntedMessage>;

    /// List all stored messages, newest first.
    async fn list(&self) -> Result<Vec<ShuntedMessage>>;

    /// Delete a message by ID.
    async fn delete(&self, id: Uuid) -> Result<()>;

    /// Delete all stored messages.
    async fn clear(&self) -> Result<()>;
}

/// Filesystem-based message store. Stores each message as a JSON file.
#[derive(Debug, Clone)]
pub struct FileStore {
    dir: PathBuf,
}

impl FileStore {
    pub fn new(config: &ShuntConfig) -> Self {
        Self {
            dir: config.storage_dir.clone(),
        }
    }

    pub fn from_dir(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn message_path(&self, id: Uuid) -> PathBuf {
        self.dir.join(format!("{}.json", id))
    }

    async fn ensure_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.dir).await?;
        Ok(())
    }
}

#[async_trait]
impl MessageStore for FileStore {
    async fn store(&self, message: &ShuntedMessage) -> Result<Uuid> {
        self.ensure_dir().await?;
        let path = self.message_path(message.id);
        let json = serde_json::to_string_pretty(message)?;
        fs::write(&path, json).await?;
        Ok(message.id)
    }

    async fn get(&self, id: Uuid) -> Result<ShuntedMessage> {
        let path = self.message_path(id);
        if !path.exists() {
            return Err(crate::error::ShuntError::NotFound(id));
        }
        let json = fs::read_to_string(&path).await?;
        let message: ShuntedMessage = serde_json::from_str(&json)?;
        Ok(message)
    }

    async fn list(&self) -> Result<Vec<ShuntedMessage>> {
        self.ensure_dir().await?;
        let mut messages = Vec::new();
        let mut entries = fs::read_dir(&self.dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let json = fs::read_to_string(&path).await?;
                if let Ok(message) = serde_json::from_str::<ShuntedMessage>(&json) {
                    messages.push(message);
                }
            }
        }

        // Sort by created_at descending (newest first)
        messages.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(messages)
    }

    async fn delete(&self, id: Uuid) -> Result<()> {
        let path = self.message_path(id);
        if !path.exists() {
            return Err(crate::error::ShuntError::NotFound(id));
        }
        fs::remove_file(&path).await?;
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        if self.dir.exists() {
            fs::remove_dir_all(&self.dir).await?;
        }
        self.ensure_dir().await?;
        Ok(())
    }
}
