use crate::models::Transaction;
use csv::Error as CsvError;
use std::{fmt, io};
use tokio::sync::mpsc::error::SendError;

/// Custom error type for the transaction processing engine
#[derive(Debug)]
pub enum EngineError {
    IoError(io::Error),
    CsvError(CsvError),
    TransactionError(String),
    TransactionNotFound(u32),
    InvalidOperation(String),
    SendError(SendError<Transaction>),
    ShutDownError(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::IoError(err) => write!(f, "I/O Error: {}", err),
            EngineError::CsvError(err) => write!(f, "CSV Error: {}", err),
            EngineError::TransactionError(err) => write!(f, "Transaction Error: {}", err),
            EngineError::TransactionNotFound(tx_id) => {
                write!(f, "Transaction not found: {}", tx_id)
            }
            EngineError::InvalidOperation(err) => write!(f, "Invalid Operation: {}", err),
            EngineError::SendError(err) => write!(f, "Send Error: {}", err),
            EngineError::ShutDownError(err) => write!(f, "ShutDown Error: {}", err),
        }
    }
}

impl From<io::Error> for EngineError {
    fn from(err: io::Error) -> Self {
        EngineError::IoError(err)
    }
}

impl From<CsvError> for EngineError {
    fn from(err: CsvError) -> Self {
        EngineError::CsvError(err)
    }
}

impl From<SendError<Transaction>> for EngineError {
    fn from(err: SendError<Transaction>) -> Self {
        EngineError::SendError(err)
    }
}
