use crate::models::Transaction;
use csv_async::Error as AsyncCsvError;
use std::{fmt, io};
use tokio::sync::mpsc::error::SendError;
/// Custom error type for the transaction processing engine
#[derive(Debug)]
pub enum EngineError {
    IoError(io::Error),
    AsyncCsvError(AsyncCsvError),
    TransactionError(String),
    TransactionNotFound(u32),
    InvalidOperation(String),
    SendError(SendError<Transaction>),
    ShutDownError(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::IoError(err) => write!(f, "IoError: {}", err),
            EngineError::TransactionError(err) => write!(f, "TransactionError: {}", err),
            EngineError::TransactionNotFound(tx_id) => {
                write!(f, "TransactionNotFound: {}", tx_id)
            }
            EngineError::InvalidOperation(err) => write!(f, "InvalidOperation: {}", err),
            EngineError::SendError(err) => write!(f, "SendError: {}", err),
            EngineError::ShutDownError(err) => write!(f, "ShutDownError: {}", err),
            EngineError::AsyncCsvError(err) => write!(f, "AsyncCsvError: {}", err),
        }
    }
}

impl From<io::Error> for EngineError {
    fn from(err: io::Error) -> Self {
        EngineError::IoError(err)
    }
}
impl From<AsyncCsvError> for EngineError {
    fn from(err: AsyncCsvError) -> Self {
        EngineError::AsyncCsvError(err)
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
        assert_eq!(format!("{}", engine_error), "IoError: some io error");
    }

    #[test]
    fn test_transaction_error_display() {
        let engine_error = EngineError::TransactionError("invalid transaction".into());
        assert_eq!(
            format!("{}", engine_error),
            "TransactionError: invalid transaction"
        );
    }

    #[test]
    fn test_transaction_not_found_display() {
        let engine_error = EngineError::TransactionNotFound(42);
        assert_eq!(format!("{}", engine_error), "TransactionNotFound: 42");
    }

    #[test]
    fn test_invalid_operation_display() {
        let engine_error = EngineError::InvalidOperation("invalid operation".into());
        assert_eq!(
            format!("{}", engine_error),
            "InvalidOperation: invalid operation"
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
        assert!(format!("{}", engine_error).contains("SendError"));
    }

    #[test]
    fn test_shutdown_error_display() {
        let engine_error = EngineError::ShutDownError("shutdown failed".into());
        assert_eq!(
            format!("{}", engine_error),
            "ShutDownError: shutdown failed"
        );
    }
}
