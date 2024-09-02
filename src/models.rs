use crate::errors::EngineError;
use serde::Deserialize;

/// Enum representing the types of transactions
#[derive(Debug, Copy, Clone, PartialEq, Deserialize)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_deserialization() {
        let csv_data = "type,client,tx,amount\n\
                        deposit,1,1,1000.0\n";
        let mut rdr = csv::ReaderBuilder::new().from_reader(csv_data.as_bytes());
        let transaction: Transaction = rdr
            .deserialize()
            .next()
            .expect("Failed to deserialize transaction")
            .expect("Invalid CSV data");

        assert_eq!(transaction.tx_type, TransactionType::Deposit);
        assert_eq!(transaction.client, 1);
        assert_eq!(transaction.tx_id, 1);
        assert_eq!(transaction.amount, Some(1000.0));
        assert_eq!(transaction.under_dispute, false);
    }

    #[test]
    fn test_deposit() {
        let mut account = ClientAccount::new();
        account.deposit(1000.0);

        assert_eq!(account.available, 1000.0);
        assert_eq!(account.total, 1000.0);
        assert_eq!(account.held, 0.0);
        assert_eq!(account.locked, false);
    }

    #[test]
    fn test_withdraw_sufficient_funds() {
        let mut account = ClientAccount::new();
        account.deposit(1000.0);
        let result = account.withdraw(500.0);

        assert!(result.is_ok());
        assert_eq!(account.available, 500.0);
        assert_eq!(account.total, 500.0);
        assert_eq!(account.held, 0.0);
    }

    #[test]
    fn test_withdraw_insufficient_funds() {
        let mut account = ClientAccount::new();
        account.deposit(500.0);
        let result = account.withdraw(1000.0);

        assert!(result.is_err());
        assert_eq!(account.available, 500.0);
        assert_eq!(account.total, 500.0);
        assert_eq!(account.held, 0.0);
    }

    #[test]
    fn test_dispute() {
        let mut account = ClientAccount::new();
        account.deposit(1000.0);
        account.dispute(500.0);

        assert_eq!(account.available, 500.0);
        assert_eq!(account.held, 500.0);
        assert_eq!(account.total, 1000.0);
    }

    #[test]
    fn test_resolve_dispute() {
        let mut account = ClientAccount::new();
        account.deposit(1000.0);
        account.dispute(500.0);
        account.resolve(500.0);

        assert_eq!(account.available, 1000.0);
        assert_eq!(account.held, 0.0);
        assert_eq!(account.total, 1000.0);
    }

    #[test]
    fn test_chargeback() {
        let mut account = ClientAccount::new();
        account.deposit(1000.0);
        account.dispute(500.0);
        let result = account.chargeback(500.0);

        assert!(result.is_ok());
        assert_eq!(account.available, 500.0);
        assert_eq!(account.held, 0.0);
        assert_eq!(account.total, 500.0);
        assert_eq!(account.locked, true);
    }

    #[test]
    fn test_chargeback_on_locked_account() {
        let mut account = ClientAccount::new();
        account.deposit(1000.0);
        account.dispute(500.0);
        account.chargeback(500.0).expect("First chargeback failed");
        let result = account.chargeback(500.0);

        assert!(result.is_err());
        assert_eq!(account.available, 500.0);
        assert_eq!(account.held, 0.0);
        assert_eq!(account.total, 500.0);
        assert_eq!(account.locked, true);
    }
}
