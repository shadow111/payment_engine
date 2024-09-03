use crate::errors::EngineError;
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::str::FromStr;

const MAX_DISPLAY_PRECISION: u32 = 4;

/// Custom deserialization function to round/truncate Decimal to MAX_DISPLAY_PRECISION decimal places
fn deserialize_decimal_with_precision<'de, D>(deserializer: D) -> Result<Option<Decimal>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt_str = Option::<String>::deserialize(deserializer)?;

    let opt_decimal = opt_str
        .map(|s| s.trim().parse::<Decimal>())
        .transpose()
        .map_err(serde::de::Error::custom)?;

    let rounded_decimal =
        opt_decimal.map(|decimal| decimal.trunc_with_scale(MAX_DISPLAY_PRECISION));
    Ok(rounded_decimal)
}

/// Custom deserialization function to trim whitespaces
fn deserialize_trimmed<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: fmt::Display,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    let trimmed = s.trim();
    trimmed.parse().map_err(serde::de::Error::custom)
}

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

impl FromStr for TransactionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "deposit" => Ok(TransactionType::Deposit),
            "withdrawal" => Ok(TransactionType::Withdrawal),
            "dispute" => Ok(TransactionType::Dispute),
            "resolve" => Ok(TransactionType::Resolve),
            "chargeback" => Ok(TransactionType::Chargeback),
            _ => Err(format!("Invalid transaction type: {}", s)),
        }
    }
}

impl fmt::Display for TransactionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                TransactionType::Deposit => "deposit",
                TransactionType::Withdrawal => "withdrawal",
                TransactionType::Dispute => "transfer",
                TransactionType::Resolve => "resolve",
                TransactionType::Chargeback => "chargeback",
            }
        )
    }
}

/// Struct representing a single transaction
#[derive(Debug, Copy, Clone, Deserialize)]
pub struct Transaction {
    #[serde(rename = "type", deserialize_with = "deserialize_trimmed")]
    pub tx_type: TransactionType,
    #[serde(deserialize_with = "deserialize_trimmed")]
    pub client: u16,
    #[serde(rename = "tx", deserialize_with = "deserialize_trimmed")]
    pub tx_id: u32,
    #[serde(deserialize_with = "deserialize_decimal_with_precision")]
    pub amount: Option<Decimal>,
    #[serde(skip_deserializing)]
    pub under_dispute: bool,
}

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
        ClientAccount {
            available: Decimal::new(0, MAX_DISPLAY_PRECISION),
            held: Decimal::new(0, MAX_DISPLAY_PRECISION),
            total: Decimal::new(0, MAX_DISPLAY_PRECISION),
            locked: false,
        }
    }

    /// Handle a deposit by adding to available funds and total
    pub fn deposit(&mut self, amount: Decimal) {
        if !self.locked {
            self.available += amount;
            self.total += amount;
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
            Err(EngineError::TransactionError("Insufficient funds".into()))
        }
    }

    /// Handle a dispute by moving funds from available to held
    pub fn dispute(&mut self, amount: Decimal) {
        if !self.locked {
            self.available -= amount;
            self.held += amount;
        }
    }

    /// Resolve a dispute by moving funds from held back to available
    pub fn resolve(&mut self, amount: Decimal) {
        if !self.locked {
            self.held -= amount;
            self.available += amount;
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
                "Attempted to process a chargeback on an already locked account.".into(),
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
                        deposit,    1,1,1000.0\n";
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
        account.deposit(dec!(1000.0));

        assert_eq!(account.available, dec!(1000.0));
        assert_eq!(account.total, dec!(1000.0));
        assert_eq!(account.held, dec!(0.0));
        assert_eq!(account.locked, false);
    }

    #[test]
    fn test_withdraw_sufficient_funds() {
        let mut account = ClientAccount::new();
        account.deposit(dec!(1000.0));
        let result = account.withdraw(dec!(500.0));

        assert!(result.is_ok());
        assert_eq!(account.available, dec!(500.0));
        assert_eq!(account.total, dec!(500.0));
        assert_eq!(account.held, dec!(0.0));
    }

    #[test]
    fn test_withdraw_insufficient_funds() {
        let mut account = ClientAccount::new();
        account.deposit(dec!(500.0));
        let result = account.withdraw(dec!(1000.0));

        assert!(result.is_err());
        assert_eq!(account.available, dec!(500.0));
        assert_eq!(account.total, dec!(500.0));
        assert_eq!(account.held, dec!(0.0));
    }

    #[test]
    fn test_dispute() {
        let mut account = ClientAccount::new();
        account.deposit(dec!(1000.0));
        account.dispute(dec!(500.0));

        assert_eq!(account.available, dec!(500.0));
        assert_eq!(account.held, dec!(500.0));
        assert_eq!(account.total, dec!(1000.0));
    }

    #[test]
    fn test_resolve_dispute() {
        let mut account = ClientAccount::new();
        account.deposit(dec!(1000.0));
        account.dispute(dec!(500.0));
        account.resolve(dec!(500.0));

        assert_eq!(account.available, dec!(1000.0));
        assert_eq!(account.held, dec!(0.0));
        assert_eq!(account.total, dec!(1000.0));
    }

    #[test]
    fn test_chargeback() {
        let mut account = ClientAccount::new();
        account.deposit(dec!(1000.0));
        account.dispute(dec!(500.0));
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
        account.deposit(dec!(1000.0));
        account.dispute(dec!(500.0));
        account
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
