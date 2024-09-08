
---


# Sharded Payments Engine

This repository implements a Sharded Payments Engine in Rust, designed to handle financial transactions efficiently using a sharded architecture. The engine is capable of processing deposits, withdrawals, disputes, resolves, and chargebacks for multiple clients concurrently, with a focus on efficient resource usage and concurrency.

## Assumptions
- **Amount**: 4 Decimal points, I assumed truncate not rounded decimal.
- **Negative Balance**: Clients Can Have a Negative Balance. In this system, clients can have a negative balance under certain conditions, such as when a chargeback occurs on a transaction that has already been disputed.
- **Locked Accounts**: Locked Accounts Cannot Perform Any Transactions. When an account is locked, the client is unable to perform any transactions, including deposits, withdrawals, disputes, resolves, and chargebacks.
- **Transaction Order Handling**: The current implementation processes transactions in the order they are received. However, it does not account for the logical order required by some transaction types. For example, a Resolve transaction that is received before a Dispute transaction will be ignored because the transaction is not under dispute yet

## Input Validation


The `validate_and_parse_transaction` function  is responsible for taking a raw CSV record and converting it into a well-formed `Transaction` struct. 
This process involves strict validation to ensure that only valid transactions are processed, while malformed or incomplete records are rejected to maintain the integrity of the transaction data.

#### Features

- **Parse CSV Records**: The function takes a `StringRecord` (a row from a CSV file) as input and attempts to parse it into a `Transaction` struct.
- **Validate Data**: It ensures that all required fields are present and correctly formatted. If any field is missing or malformed, the function will return an error.
- **Reject Malformed Records**: If a record cannot be parsed correctly due to insufficient data, incorrect types,..., the function rejects the record by returning an error.

#### Detailed Check

1. **Field Length Check**:
   - The function expects each record to contain exactly four fields: `transaction type`, `client ID`, `transaction ID`, and `amount`.
   - If the record does not contain exactly four fields, it is considered malformed, and the function returns an error indicating "Insufficient data in transaction string."

2. **Field Presence and Validation**:
   - **Transaction Type**:
      - The first field is parsed into a `TransactionType` enum.
      - The transaction type is **case-insensitive**, meaning `"Deposit"`, `"DEPOSIT"`, and `"deposit"` are treated the same.
      - If the type is missing or invalid (i.e., not one of the expected types such as `deposit`, `withdrawal`, `dispute`, etc.), the function returns an error with a message indicating the invalid transaction type.
   - **Client ID**:
      - The second field is parsed into a `u16` integer.
      - If the client ID is missing or cannot be parsed as a `u16`, the function returns an error indicating "Invalid client ID."
   - **Transaction ID**:
      - The third field is parsed into a `u32` integer.
      - If the transaction ID is missing or cannot be parsed as a `u32`, the function returns an error indicating "Invalid transaction ID."
   - **Amount**:
      - For `deposit` and `withdrawal` transactions, the fourth field (amount) is parsed into a `Decimal`.
      - If the amount is missing, zero (can't deposit or withdraw 0), or not a positive number, the function returns an error indicating that the amount must be positive.
      - For `dispute`, `resolve`, and `chargeback` transactions, the amount field is not required and can be ignored if present.

3. **Error Handling and Skipping Malformed Records**:
   - When a record fails any of the validation checks mentioned above, the function returns an `EngineError::TransactionError` with a detailed error message.

4. **Correct Transaction Construction**:
   - If all validations pass, the function constructs a `Transaction` struct with the parsed and validated data.
   - The transaction is then returned in an `Ok` variant of the `Result` type, ready for further processing by the payment engine.

#### Example Scenarios

- **Valid Record**: A record like `["deposit", "1", "1001", "100.0"]` will be successfully parsed into a `Transaction` struct with a deposit of 100.0000 for client 1.

- **Case-Insensitive Transaction Type**: A record with a transaction type of `["DEPOSIT", "1", "1001", "100.0"]` will be treated the same as `["deposit", "1", "1001", "100.0"]` and successfully parsed.

- **Invalid Transaction Type**: A record with an invalid transaction type, such as `["invalid", "1", "1001", "100.0"]`, will be rejected with an error indicating the invalid type.

- **Missing Amount**: A deposit record missing the amount, such as `["deposit", "1", "1001", ""]`, will be rejected with an error indicating that the amount is missing.


## Technologies Used

- **Rust**: The core programming language used for implementing the engine, chosen for its performance, memory safety, and concurrency support.
- **Tokio**: An asynchronous runtime for Rust, used to manage concurrent tasks efficiently.
- **Serde**: A framework for serializing and deserializing Rust data structures.
- **CSV**: The engine reads and writes data in CSV format, making use of the `csv` crate for this purpose.
- **Log**: Rust's logging system is used to report errors and other information during transaction processing.

## Workflow

### Sharding and Concurrency

The engine is designed with a sharded architecture, where client accounts and their associated transactions are distributed across multiple shards. Each shard is represented by a `ShardState` struct, which maintains the state of client accounts and a transaction log specific to that shard. The number of shards is configurable at the time of the engine's initialization.

**Steps:**

1. **Initialization**:
    - The engine is initialized with a specified number of shards.
    - Each shard is associated with a `ClientShard`, which is a thread-safe structure protected by a `Mutex`. The shards are stored in a vector.

2. **Transaction Streaming**:
    - Transactions are streamed and validated directly from a CSV file, which means that transactions are read and processed in real-time without loading the entire file into memory. This approach optimizes memory usage, especially when dealing with large datasets.

3. **Transaction Routing**:
    - Incoming transactions are routed to a shard based on the client's ID, ensuring that all transactions for a particular client are handled by the same shard.
    - The engine uses **channels** provided by the `tokio::sync::mpsc` module to send transactions to the appropriate shard asynchronously. Each shard has its own transaction channel, allowing it to process transactions concurrently.
   
4. **Duplicate Transaction Detection**: 
   - The engine includes a mechanism to detect and handle duplicate transactions. If a transaction  is encountered more than once, the engine will skip the duplicate and only process the transaction the first time it is received. This ensures the integrity of transaction processing by preventing double processing.

5. **Transaction Processing**:
    - Each shard processes transactions asynchronously. The engine handles deposits, withdrawals, disputes, resolves, and chargebacks, updating the client account states accordingly.
    - If the engine is in the process of shutting down, new transactions are rejected to ensure consistency.

6. **Shutdown and Completion**:
    - The engine currently supports a basic shutdown mechanism. However, the full graceful shutdown—where all ongoing transactions are processed before the engine shuts down—is not yet implemented.

7. **State Output**:
    - The final state of all client accounts is output to a CSV file, which includes the client's available balance, held balance, total balance, and locked status.

### Error Handling

The engine is robust in error handling, with custom errors defined in the `EngineError` enum. Errors are logged using the `log` crate, and appropriate error messages are provided to help diagnose issues such as invalid operations or transactions not found.

## Payment Engine Logic

### Core Structures

- **ShardedEngine**: The main struct that orchestrates the entire engine, holding the shards, transaction channels, and control mechanisms for shutdown.
- **ShardState**: Holds the state for each shard, including client accounts and their associated transactions.
- **ClientAccount**: Represents a client's account, tracking available, held, total funds, and whether the account is locked.
- **Transaction**: Represents a financial transaction, including its type, amount, and client information.

### Transaction Types

- **Deposit**: Adds funds to a client's available balance.
- **Withdrawal**: Deducts funds from a client's available balance, ensuring sufficient funds are available.
- **Dispute**: Flags a transaction under dispute, moving the disputed amount to the held balance.
- **Resolve**: Resolves a dispute, returning the disputed amount to the available balance.
- **Chargeback**: Finalizes a dispute by permanently removing the disputed amount from the account and locking the account.

### Functionality

- **new(num_shards: usize) -> Self**: Initializes the engine with a specified number of shards.
- **route_transaction(&self, transaction: Transaction) -> Result<(), EngineError>**: Routes an incoming transaction to the appropriate shard based on the client ID using a channel.
- **shutdown(&mut self)**: Initiates a basic shutdown of the engine, (Note: Full graceful shutdown is not yet implemented.)
- **wait_for_completion(&self) -> Result<(), EngineError>**: Waits for all shards to complete processing before proceeding with a full shutdown. (Note: This feature is still in progress.)
- **process_transaction_in_shard(shard_state: &mut ShardState, transaction: Transaction) -> Result<(), EngineError>**: Handles the core logic for processing a transaction within a shard.
- **write_accounts(&self) -> Result<(), EngineError>**: Writes the final state of all client accounts to a CSV file.

## How to Run

1. **Prerequisites**:
    - Install Rust and Cargo.
    - Ensure the Tokio, Serde, and CSV crates are included in your `Cargo.toml`.

2. **Build and Run**:
    - Compile the project using Cargo: `cargo build --release`
    - Run the engine with a specified number of shards and input transactions.

3. **Output**:
    - The final state of all client accounts will be printed to the console or redirected to a CSV file.

## Example

To run the engine:

```bash
cargo run --release -- <input_file> > <output_file>
```

Where `<input_file>` is the path to the CSV file containing the transactions, and `<output_file>` is the path where the output should be saved.

## Future Improvements

- **Pending Queue**: To address the issue of out-of-order transactions, a pending queue can be introduced. This queue would temporarily hold transactions that cannot be processed immediately due to the required preceding transaction not being present (e.g., a Resolve transaction waiting for its corresponding Dispute to arrive). When a new transaction is received, the engine would check the pending queue and attempt to process any transactions that have become valid due to the new input.
- **Graceful Shutdown**: Fully implement a graceful shutdown process that ensures all in-flight transactions are processed before the engine shuts down.
- **Persistence**: Add persistence mechanisms to save the state of accounts and transactions in case of a system crash.
- **Optimizations**: Investigate further optimizations for handling large volumes of transactions efficiently.
- **Pipeline and Queue System**: Implement a pipeline and queue system for transaction processing. This could involve queuing incoming transactions and processing them in stages (e.g., validation, execution, finalization) to improve throughput and ensure consistency even under high load.

---
