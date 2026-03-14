use thiserror::Error;

#[derive(Debug, Error)]
pub enum ShuntError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Message not found: {0}")]
    NotFound(uuid::Uuid),

    #[error("Email parse error: {0}")]
    EmailParse(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Web server error: {0}")]
    Server(String),
}

pub type Result<T> = std::result::Result<T, ShuntError>;
