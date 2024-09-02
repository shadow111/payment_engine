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
