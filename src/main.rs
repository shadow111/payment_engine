mod engine;
mod errors;
mod io;
mod models;

use crate::engine::ShardedEngine;
use crate::errors::EngineError;
use log::error;
use std::env;
use tokio::runtime::Runtime;

fn main() -> Result<(), EngineError> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let runtime = Runtime::new()?;
    runtime.block_on(async {
        let args: Vec<String> = env::args().collect();
        if args.len() < 2 {
            eprintln!("Usage: {} <input_file>", args[0]);
            std::process::exit(1);
        }

        let num_shards = 4;
        let mut engine = ShardedEngine::new(num_shards);
        let transactions = io::stream_transactions(&args[1])?;

        // Process each transaction by routing it to the appropriate shard
        for transaction_result in transactions {
            match transaction_result {
                Ok(transaction) => engine.route_transaction(transaction)?,
                Err(err) => {
                    error!("{}", err)
                }
            }
        }

        engine.shutdown();
        engine.wait_for_completion().await;

        engine.write_accounts().await?;
        Ok(())
    })
}

/*if let Some(&rhs) = shard_state.transactions.get(&transaction.tx_id) {
if transaction == rhs {
return  Err(EngineError::TransactionError("Duplicate transaction".into()))
}

}*/
