use crate::errors::EngineError;
use crate::models::{Transaction, TransactionType, MAX_DISPLAY_PRECISION};
use csv::{ReaderBuilder, StringRecord};
use rust_decimal::Decimal;
use std::fs::File;

type TransactionResult = Result<Transaction, EngineError>;
/// Stream transactions from a CSV file without loading the entire file into memory
pub fn stream_transactions(
    file_path: &str,
) -> Result<impl Iterator<Item = TransactionResult>, EngineError> {
    let file = File::open(file_path).map_err(|err| EngineError::IoError(err))?;

    let rdr = ReaderBuilder::new().trim(csv::Trim::All).from_reader(file);

    Ok(rdr.into_records().map(|result| match result {
        Ok(record) => {
            let transaction = validate_and_parse_transaction(record)?;
            Ok(transaction)
        }
        Err(e) => Err(EngineError::CsvError(e)),
    }))
}

fn validate_and_parse_transaction(record: StringRecord) -> Result<Transaction, EngineError> {
    if record.len() < 4 {
        return Err(EngineError::TransactionError(
            "Insufficient data in transaction string".into(),
        ));
    }

    // Extract and trim each field
    let transaction_type_str = record
        .get(0)
        .ok_or_else(|| EngineError::TransactionError("Missing transaction type".into()))?;

    let client_id_str = record
        .get(1)
        .ok_or_else(|| EngineError::TransactionError("Missing client ID".into()))?;

    let transaction_id_str = record
        .get(2)
        .ok_or_else(|| EngineError::TransactionError("Missing transaction ID".into()))?;

    let amount_str = record.get(3);

    // Parse and validate transaction type
    let transaction_type = transaction_type_str
        .to_lowercase()
        .parse::<TransactionType>()
        .map_err(|_| EngineError::TransactionError("Invalid transaction type".into()))?;

    // Parse client_id
    let client_id = client_id_str
        .parse::<u16>()
        .map_err(|_| EngineError::TransactionError("Invalid client ID".into()))?;

    // Parse transaction_id
    let transaction_id = transaction_id_str
        .parse::<u32>()
        .map_err(|_| EngineError::TransactionError("Invalid transaction ID".into()))?;

    // Validate and parse amount for deposit and withdrawal
    let amount = match transaction_type {
        TransactionType::Deposit | TransactionType::Withdrawal => {
            let amount_str =
                amount_str.ok_or_else(|| EngineError::TransactionError("Missing amount".into()))?;
            let amount = amount_str
                .parse::<Decimal>()
                .map_err(|_| EngineError::TransactionError("Invalid amount".into()))?;
            if amount <= Decimal::ZERO {
                return Err(EngineError::TransactionError(
                    "Amount must be positive".into(),
                ));
            }
            Some(amount.trunc_with_scale(MAX_DISPLAY_PRECISION))
        }
        _ => None, // Dispute, Resolve, Chargeback don't require an amount
    };

    Ok(Transaction {
        tx_type: transaction_type,
        client: client_id,
        tx_id: transaction_id,
        amount,
        under_dispute: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use csv::StringRecord;
    use rust_decimal::Decimal;
    use std::fs;
    use std::fs::File;
    use std::io::Write;
    use std::str::FromStr;

    fn create_temp_csv(file_path: &str, data: &str) -> String {
        let mut file = File::create(file_path).expect("Unable to create test file");
        file.write_all(data.as_bytes())
            .expect("Unable to write to test file");
        file_path.to_string()
    }

    #[test]
    fn test_stream_transactions_valid_csv() {
        let csv_data = "type,client,tx,amount\n\
                        deposit,1,1,1000.0\n\
                        withdrawal,1,2,500.0\n";

        let file_path = create_temp_csv("test_stream_transactions_valid.csv", csv_data);
        let transactions = stream_transactions(&file_path).expect("Failed to stream transactions");

        let mut count = 0;
        for transaction in transactions {
            count += 1;
            assert!(transaction.is_ok());
        }
        assert_eq!(count, 2);

        fs::remove_file(file_path).expect("Failed to delete test file");
    }

    #[test]
    fn test_stream_transactions_invalid_csv() {
        let csv_data = "type,client,tx,amount\n\
                        deposit,1,1,1000.0\n\
                        withdrawal,1,,500.0\n"; // Missing tx field

        let file_path = create_temp_csv("test_stream_transactions_invalid.csv", csv_data);
        let transactions = stream_transactions(&file_path).expect("Failed to stream transactions");

        let mut count = 0;
        for transaction in transactions {
            count += 1;
            if count == 2 {
                assert!(transaction.is_err());
            }
        }
        assert_eq!(count, 2);

        fs::remove_file(file_path).expect("Failed to delete test file");
    }

    #[test]
    fn test_stream_transactions_corrupted_data() {
        let csv_data = "type,client,tx,amount\n\
                        deposit,1,1,abc\n"; // 'abc' is not a valid amount

        let file_path = create_temp_csv("test_stream_transactions_corrupted_data.csv", csv_data);
        let transactions = stream_transactions(&file_path).expect("Failed to stream transactions");

        let mut count = 0;
        for transaction in transactions {
            count += 1;
            if count == 1 {
                assert!(transaction.is_err());
            }
        }
        assert_eq!(count, 1);
        fs::remove_file(file_path).expect("Failed to delete test file");
    }

    #[test]
    fn test_validate_and_parse_transaction_success() {
        let record = StringRecord::from(vec!["deposit", "1", "1001", "123.4567"]);
        let transaction = validate_and_parse_transaction(record).unwrap();

        assert_eq!(transaction.tx_type, TransactionType::Deposit);
        assert_eq!(transaction.client, 1);
        assert_eq!(transaction.tx_id, 1001);
        assert_eq!(
            transaction.amount.unwrap(),
            Decimal::from_str("123.4567").unwrap()
        );
        assert_eq!(transaction.under_dispute, false);
    }

    #[test]
    fn test_validate_and_parse_transaction_missing_amount() {
        let record = StringRecord::from(vec!["deposit", "1", "1001"]);
        let result = validate_and_parse_transaction(record);

        assert!(result.is_err());
    }

    #[test]
    fn test_validate_and_parse_transaction_invalid_transaction_type() {
        let record = StringRecord::from(vec!["invalid_type", "1", "1001", "123.4567"]);
        let result = validate_and_parse_transaction(record);

        assert!(result.is_err());
    }

    #[test]
    fn test_validate_and_parse_transaction_invalid_client_id() {
        let record = StringRecord::from(vec!["deposit", "invalid_client", "1001", "123.4567"]);
        let result = validate_and_parse_transaction(record);

        assert!(result.is_err());
    }

    #[test]
    fn test_validate_and_parse_transaction_invalid_transaction_id() {
        let record = StringRecord::from(vec!["deposit", "1", "invalid_tx", "123.4567"]);
        let result = validate_and_parse_transaction(record);

        assert!(result.is_err());
    }

    #[test]
    fn test_validate_and_parse_transaction_invalid_amount() {
        let record = StringRecord::from(vec!["deposit", "1", "1001", "invalid_amount"]);
        let result = validate_and_parse_transaction(record);

        assert!(result.is_err());
    }

    #[test]
    fn test_validate_and_parse_transaction_amount_must_be_positive() {
        let record = StringRecord::from(vec!["deposit", "1", "1001", "-123.4567"]);
        let result = validate_and_parse_transaction(record);

        assert!(result.is_err());
    }

    #[test]
    fn test_validate_and_parse_transaction_insufficient_data() {
        let record = StringRecord::from(vec!["deposit", "1"]);
        let result = validate_and_parse_transaction(record);

        assert!(result.is_err());
    }

    #[test]
    fn test_validate_and_parse_transaction_dispute_type_without_amount() {
        let record = StringRecord::from(vec!["dispute", "1", "1001", ","]);
        let transaction = validate_and_parse_transaction(record).unwrap();

        assert_eq!(transaction.tx_type, TransactionType::Dispute);
        assert_eq!(transaction.client, 1);
        assert_eq!(transaction.tx_id, 1001);
        assert!(transaction.amount.is_none());
        assert_eq!(transaction.under_dispute, false);
    }
}
