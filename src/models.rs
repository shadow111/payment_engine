use crate::errors::EngineError;
use serde::Deserialize;

/// Enum representing the types of transactions
#[derive(Debug, Copy, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

/// Struct representing a single transaction
#[derive(Debug, Copy, Clone, Deserialize)]
pub struct Transaction {
    #[serde(rename = "type")]
    pub tx_type: TransactionType,
    pub client: u16,
    #[serde(rename = "tx")]
    pub tx_id: u32,
    pub amount: Option<f64>,
    #[serde(skip_deserializing)]
    pub under_dispute: bool,
}

/// Struct representing a client's account
#[derive(Debug, Default)]
pub struct ClientAccount {
    pub available: f64,
    pub held: f64,
    pub total: f64,
    pub locked: bool,
}

impl ClientAccount {
    pub fn new() -> Self {
        ClientAccount {
            available: 0.0,
            held: 0.0,
            total: 0.0,
            locked: false,
        }
    }

    /// Handle a deposit by adding to available funds and total
    pub fn deposit(&mut self, amount: f64) {
        if !self.locked {
            self.available += amount;
            self.total += amount;
        }
    }

    /// Handle a withdrawal by subtracting from available funds
    /// Returns an error if funds are insufficient
    pub fn withdraw(&mut self, amount: f64) -> Result<(), EngineError> {
        if !self.locked && self.available >= amount {
            self.available -= amount;
            self.total -= amount;
            Ok(())
        } else {
            Err(EngineError::TransactionError("Insufficient funds".into()))
        }
    }

    /// Handle a dispute by moving funds from available to held
    pub fn dispute(&mut self, amount: f64) {
        if !self.locked {
            self.available -= amount;
            self.held += amount;
        }
    }

    /// Resolve a dispute by moving funds from held back to available
    pub fn resolve(&mut self, amount: f64) {
        if !self.locked {
            self.held -= amount;
            self.available += amount;
        }
    }

    /// Handle a chargeback by removing funds from held and total, and locking the account
    pub fn chargeback(&mut self, amount: f64) -> Result<(), EngineError> {
        if !self.locked {
            self.held -= amount;
            self.total -= amount;
            self.locked = true;
            Ok(())
        } else {
            Err(EngineError::InvalidOperation(
                "Attempted to process a chargeback on an already locked account.".into(),
            ))
        }
    }
}
