use crate::errors::EngineError;
use crate::models::Transaction;
use csv::ReaderBuilder;

/// Stream transactions from a CSV file without loading the entire file into memory
pub fn stream_transactions(
    file_path: &str,
) -> Result<impl Iterator<Item = Result<Transaction, EngineError>>, EngineError> {
    let rdr = ReaderBuilder::new().from_path(file_path)?;

    Ok(rdr
        .into_deserialize()
        .map(|result| result.map_err(EngineError::from)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::fs::File;
    use std::io::Write;

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
}
