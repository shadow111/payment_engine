use crate::errors::EngineError;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::str::FromStr;

pub const MAX_DISPLAY_PRECISION: u32 = 4;

/// Enum representing the types of transactions
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

impl FromStr for TransactionType {
    type Err = EngineError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "deposit" => Ok(TransactionType::Deposit),
            "withdrawal" => Ok(TransactionType::Withdrawal),
            "dispute" => Ok(TransactionType::Dispute),
            "resolve" => Ok(TransactionType::Resolve),
            "chargeback" => Ok(TransactionType::Chargeback),
            _ => Err(EngineError::TransactionError(
                "Invalid transaction type".into(),
            )),
        }
    }
}

/// Struct representing a single transaction
#[derive(Debug, Copy, Clone, Hash, Deserialize)]
pub struct Transaction {
    #[serde(rename = "type")]
    pub tx_type: TransactionType,
    pub client: u16,
    #[serde(rename = "tx")]
    pub tx_id: u32,
    pub amount: Option<Decimal>,
    #[serde(skip_deserializing)]
    pub under_dispute: bool,
}

impl PartialEq for Transaction {
    fn eq(&self, other: &Self) -> bool {
        self.tx_type == other.tx_type
            && self.client == other.client
            && self.tx_id == other.tx_id
            && self.amount == other.amount
    }
}

impl Eq for Transaction {}

/// Struct representing a client's account
#[derive(Debug)]
pub struct ClientAccount {
    pub available: Decimal,
    pub held: Decimal,
    pub total: Decimal,
    pub locked: bool,
}

impl ClientAccount {
    pub fn new() -> Self {
        Self {
            available: Decimal::new(0, MAX_DISPLAY_PRECISION),
            held: Decimal::new(0, MAX_DISPLAY_PRECISION),
            total: Decimal::new(0, MAX_DISPLAY_PRECISION),
            locked: false,
        }
    }

    /// Handle a deposit by adding to available funds and total
    pub fn deposit(&mut self, amount: Decimal) -> Result<(), EngineError> {
        if !self.locked {
            self.available += amount;
            self.total += amount;
            Ok(())
        } else {
            Err(EngineError::InvalidOperation(
                "Attempted to process invalid deposit".into(),
            ))
        }
    }

    /// Handle a withdrawal by subtracting from available funds
    /// Returns an error if funds are insufficient
    pub fn withdraw(&mut self, amount: Decimal) -> Result<(), EngineError> {
        if !self.locked && self.available >= amount {
            self.available -= amount;
            self.total -= amount;
            Ok(())
        } else {
            Err(EngineError::InvalidOperation(
                "Attempted to process invalid withdraw".into(),
            ))
        }
    }

    /// Handle a dispute by moving funds from available to held
    pub fn dispute(&mut self, amount: Decimal) -> Result<(), EngineError> {
        if !self.locked {
            self.available -= amount;
            self.held += amount;
            Ok(())
        } else {
            Err(EngineError::InvalidOperation(
                "Attempted to process invalid dispute".into(),
            ))
        }
    }

    /// Resolve a dispute by moving funds from held back to available
    pub fn resolve(&mut self, amount: Decimal) -> Result<(), EngineError> {
        if !self.locked {
            self.held -= amount;
            self.available += amount;
            Ok(())
        } else {
            Err(EngineError::InvalidOperation(
                "Attempted to process invalid resolve".into(),
            ))
        }
    }

    /// Handle a chargeback by removing funds from held and total, and locking the account
    pub fn chargeback(&mut self, amount: Decimal) -> Result<(), EngineError> {
        if !self.locked {
            self.held -= amount;
            self.total -= amount;
            self.locked = true;
            Ok(())
        } else {
            Err(EngineError::InvalidOperation(
                "Attempted to process invalid chargeback.".into(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

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
        assert_eq!(transaction.amount, Some(dec!(1000.0)));
        assert_eq!(transaction.under_dispute, false);
    }

    #[test]
    fn test_deposit() {
        let mut account = ClientAccount::new();
        let _ = account.deposit(dec!(1000.0));

        assert_eq!(account.available, dec!(1000.0));
        assert_eq!(account.total, dec!(1000.0));
        assert_eq!(account.held, dec!(0.0));
        assert_eq!(account.locked, false);
    }

    #[test]
    fn test_withdraw_sufficient_funds() {
        let mut account = ClientAccount::new();
        let _ = account.deposit(dec!(1000.0));
        let result = account.withdraw(dec!(500.0));

        assert!(result.is_ok());
        assert_eq!(account.available, dec!(500.0));
        assert_eq!(account.total, dec!(500.0));
        assert_eq!(account.held, dec!(0.0));
    }

    #[test]
    fn test_withdraw_insufficient_funds() {
        let mut account = ClientAccount::new();
        let _ = account.deposit(dec!(500.0));
        let result = account.withdraw(dec!(1000.0));

        assert!(result.is_err());
        assert_eq!(account.available, dec!(500.0));
        assert_eq!(account.total, dec!(500.0));
        assert_eq!(account.held, dec!(0.0));
    }

    #[test]
    fn test_dispute() {
        let mut account = ClientAccount::new();
        let _ = account.deposit(dec!(1000.0));
        let _ = account.dispute(dec!(500.0));

        assert_eq!(account.available, dec!(500.0));
        assert_eq!(account.held, dec!(500.0));
        assert_eq!(account.total, dec!(1000.0));
    }

    #[test]
    fn test_resolve_dispute() {
        let mut account = ClientAccount::new();
        let _ = account.deposit(dec!(1000.0));
        let _ = account.dispute(dec!(500.0));
        let _ = account.resolve(dec!(500.0));

        assert_eq!(account.available, dec!(1000.0));
        assert_eq!(account.held, dec!(0.0));
        assert_eq!(account.total, dec!(1000.0));
    }

    #[test]
    fn test_chargeback() {
        let mut account = ClientAccount::new();
        let _ = account.deposit(dec!(1000.0));
        let _ = account.dispute(dec!(500.0));
        let result = account.chargeback(dec!(500.0));

        assert!(result.is_ok());
        assert_eq!(account.available, dec!(500.0));
        assert_eq!(account.held, dec!(0.0));
        assert_eq!(account.total, dec!(500.0));
        assert_eq!(account.locked, true);
    }

    #[test]
    fn test_chargeback_on_locked_account() {
        let mut account = ClientAccount::new();
        let _ = account.deposit(dec!(1000.0));
        let _ = account.dispute(dec!(500.0));
        let _ = account
            .chargeback(dec!(500.0))
            .expect("First chargeback failed");
        let result = account.chargeback(dec!(500.0));

        assert!(result.is_err());
        assert_eq!(account.available, dec!(500.0));
        assert_eq!(account.held, dec!(0.0));
        assert_eq!(account.total, dec!(500.0));
        assert_eq!(account.locked, true);
    }
}
