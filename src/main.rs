mod engine;
mod errors;
mod io;
mod models;

use crate::engine::ShardedEngine;
use crate::errors::EngineError;
use futures::stream::StreamExt;
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
        let mut stream = io::stream_transactions(&args[1]).await?;

        // Process each transaction by routing it to the appropriate shard
        while let Some(record_result) = stream.next().await {
            let transaction = record_result
                .map_err(|err| EngineError::TransactionError(err.to_string()))
                .and_then(|record| io::validate_and_parse_transaction(record));

            match transaction {
                Ok(trans) => {
                    if let Err(err) = engine.route_transaction(trans) {
                        error!("Failed to route transaction: {}", err);
                    }
                }
                Err(err) => {
                    error!("{}", err);
                }
            }
        }

        engine.shutdown();
        engine.wait_for_completion().await;
        engine.write_accounts().await?;
        Ok(())
    })
}
