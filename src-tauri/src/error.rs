use thiserror::Error;
use serde::Serialize;

#[derive(Error, Debug, Serialize)]
#[allow(dead_code)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Connection not found: {0}")]
    ConnectionNotFound(String),
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("Query error: {0}")]
    Query(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        AppError::Database(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Serialization(e.to_string())
    }
}

#[allow(dead_code)]
pub type Result<T> = std::result::Result<T, AppError>;
