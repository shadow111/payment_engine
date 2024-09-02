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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TransactionType;
    use std::io;
    use tokio::sync::mpsc;

    #[test]
    fn test_io_error_display() {
        let io_err = io::Error::new(io::ErrorKind::Other, "some io error");
        let engine_error = EngineError::from(io_err);
        assert_eq!(format!("{}", engine_error), "I/O Error: some io error");
    }

    #[test]
    fn test_transaction_error_display() {
        let engine_error = EngineError::TransactionError("invalid transaction".into());
        assert_eq!(
            format!("{}", engine_error),
            "Transaction Error: invalid transaction"
        );
    }

    #[test]
    fn test_transaction_not_found_display() {
        let engine_error = EngineError::TransactionNotFound(42);
        assert_eq!(format!("{}", engine_error), "Transaction not found: 42");
    }

    #[test]
    fn test_invalid_operation_display() {
        let engine_error = EngineError::InvalidOperation("invalid operation".into());
        assert_eq!(
            format!("{}", engine_error),
            "Invalid Operation: invalid operation"
        );
    }

    #[test]
    fn test_send_error_display() {
        let (_tx, _rx) = mpsc::channel::<Transaction>(1);
        let transaction = Transaction {
            tx_type: TransactionType::Deposit,
            client: 0,
            tx_id: 0,
            amount: None,
            under_dispute: false,
        };

        let send_error: Result<(), SendError<Transaction>> = Err(SendError(transaction));
        let engine_error = EngineError::from(send_error.err().unwrap());
        assert!(format!("{}", engine_error).contains("Send Error"));
    }

    #[test]
    fn test_shutdown_error_display() {
        let engine_error = EngineError::ShutDownError("shutdown failed".into());
        assert_eq!(
            format!("{}", engine_error),
            "ShutDown Error: shutdown failed"
        );
    }
}
