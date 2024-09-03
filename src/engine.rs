use crate::errors::EngineError;
use crate::models::{ClientAccount, Transaction, TransactionType};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, Notify};

type ClientShard = Arc<Mutex<ShardState>>;
type TxChannel = mpsc::UnboundedSender<Transaction>;

#[derive(Clone)]
pub struct ShardedEngine {
    shards: Vec<ClientShard>,
    tx_channels: Vec<TxChannel>,
    notify: Arc<Notify>,
    completed_shards: Arc<AtomicUsize>,
    shutdown: Arc<AtomicBool>,
}

/// ShardState holds both the accounts and the transaction log for a shard.
pub struct ShardState {
    accounts: HashMap<u16, ClientAccount>,
    transactions: HashMap<u32, Transaction>,
}

impl ShardedEngine {
    pub fn new(num_shards: usize) -> Self {
        let mut shards: Vec<ClientShard> = Vec::with_capacity(num_shards);
        let mut tx_channels: Vec<TxChannel> = Vec::with_capacity(num_shards);
        let notify = Arc::new(Notify::new());
        let completed_shards = Arc::new(AtomicUsize::new(0));
        let shutdown = Arc::new(AtomicBool::new(false));

        for _ in 0..num_shards {
            let (tx, mut rx) = mpsc::unbounded_channel();
            let shard: ClientShard = Arc::new(Mutex::new(ShardState {
                accounts: HashMap::new(),
                transactions: HashMap::new(),
            }));

            let shard_clone: ClientShard = Arc::clone(&shard);
            let notify_clone = Arc::clone(&notify);
            let completed_shards_clone = Arc::clone(&completed_shards);
            let shutdown_clone = Arc::clone(&shutdown);

            tokio::spawn(async move {
                while let Some(transaction) = rx.recv().await {
                    if shutdown_clone.load(Ordering::SeqCst) {
                        break;
                    }

                    let mut shard_state = shard_clone.lock().await;
                    if let Err(e) =
                        Self::process_transaction_in_shard(&mut shard_state, transaction)
                    {
                        log::error!("Error processing transaction: {:?}", e);
                    }
                }
                completed_shards_clone.fetch_add(1, Ordering::SeqCst);
                notify_clone.notify_one();
            });

            shards.push(shard);
            tx_channels.push(tx);
        }

        ShardedEngine {
            shards,
            tx_channels,
            notify,
            completed_shards,
            shutdown,
        }
    }

    pub fn route_transaction(&self, transaction: Transaction) -> Result<(), EngineError> {
        if self.shutdown.load(Ordering::SeqCst) {
            return Err(EngineError::ShutDownError(
                "Engine is shutting down, no new transactions accepted.".into(),
            ));
        }
        let shard_index = (transaction.client as usize) % self.shards.len();
        self.tx_channels[shard_index].send(transaction)?;

        Ok(())
    }

    pub fn shutdown(&mut self) {
        //TODO graceful shutdown
        // self.shutdown.store(true, Ordering::SeqCst);
        self.tx_channels.clear();
    }

    pub async fn wait_for_completion(&self) {
        while self.completed_shards.load(Ordering::SeqCst) < self.shards.len() {
            self.notify.notified().await;
        }
    }

    pub fn process_transaction_in_shard(
        shard_state: &mut ShardState,
        transaction: Transaction,
    ) -> Result<(), EngineError> {
        let account = shard_state
            .accounts
            .entry(transaction.client)
            .or_insert_with(ClientAccount::new);

        match transaction.tx_type {
            TransactionType::Deposit => {
                if let Some(amount) = transaction.amount {
                    account.deposit(amount);
                    shard_state.transactions.insert(
                        transaction.tx_id,
                        Transaction {
                            under_dispute: false,
                            ..transaction.clone()
                        },
                    );
                }
            }

            TransactionType::Withdrawal => {
                if let Some(amount) = transaction.amount {
                    account.withdraw(amount)?;
                    shard_state.transactions.insert(
                        transaction.tx_id,
                        Transaction {
                            under_dispute: false,
                            ..transaction.clone()
                        },
                    );
                }
            }

            TransactionType::Dispute => {
                match shard_state.transactions.get_mut(&transaction.tx_id) {
                    Some(tx) => {
                        if let Some(amount) = tx.amount {
                            account.dispute(amount);
                            tx.under_dispute = true;
                        }
                    }
                    None => {
                        return Err(EngineError::TransactionNotFound(transaction.tx_id));
                    }
                }
            }

            TransactionType::Resolve => {
                match shard_state.transactions.get_mut(&transaction.tx_id) {
                    Some(tx) if tx.under_dispute => {
                        if let Some(amount) = tx.amount {
                            account.resolve(amount);
                        }
                    }
                    Some(_) => {
                        return Err(EngineError::InvalidOperation(
                            "Resolve attempted on a non-disputed transaction".into(),
                        ));
                    }
                    None => {
                        return Err(EngineError::TransactionNotFound(transaction.tx_id));
                    }
                }
            }

            TransactionType::Chargeback => {
                match shard_state.transactions.get_mut(&transaction.tx_id) {
                    Some(tx) if tx.under_dispute => {
                        if let Some(amount) = tx.amount {
                            account.chargeback(amount)?;
                        }
                    }
                    Some(_) => {
                        return Err(EngineError::InvalidOperation(
                            "Chargeback attempted on a non-disputed transaction".into(),
                        ));
                    }
                    None => {
                        return Err(EngineError::TransactionNotFound(transaction.tx_id));
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn write_accounts(&self) -> Result<(), EngineError> {
        let mut wtr = csv::Writer::from_writer(std::io::stdout());
        wtr.write_record(&["client", "available", "held", "total", "locked"])?;

        for shard in &self.shards {
            let shard_state = shard.lock().await;
            for (client_id, account) in shard_state.accounts.iter() {
                wtr.serialize((
                    client_id,
                    account.available,
                    account.held,
                    account.total,
                    account.locked,
                ))?;
            }
        }
        wtr.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_process_deposit() {
        let mut shard_state = ShardState {
            accounts: HashMap::new(),
            transactions: HashMap::new(),
        };

        let transaction = Transaction {
            tx_type: TransactionType::Deposit,
            client: 1,
            tx_id: 1,
            amount: Some(dec!(1000.0)),
            under_dispute: false,
        };

        ShardedEngine::process_transaction_in_shard(&mut shard_state, transaction).unwrap();

        let account = shard_state.accounts.get(&1).unwrap();
        assert_eq!(account.available, dec!(1000.0));
        assert_eq!(account.total, dec!(1000.0));
        assert_eq!(account.held, dec!(0.0));

        let tx = shard_state.transactions.get(&1).unwrap();
        assert_eq!(tx.tx_type, TransactionType::Deposit);
        assert_eq!(tx.amount, Some(dec!(1000.0)));
    }

    #[tokio::test]
    async fn test_process_withdrawal() {
        let mut shard_state = ShardState {
            accounts: HashMap::new(),
            transactions: HashMap::new(),
        };

        let deposit = Transaction {
            tx_type: TransactionType::Deposit,
            client: 1,
            tx_id: 1,
            amount: Some(dec!(1000.0)),
            under_dispute: false,
        };

        ShardedEngine::process_transaction_in_shard(&mut shard_state, deposit).unwrap();

        let withdrawal = Transaction {
            tx_type: TransactionType::Withdrawal,
            client: 1,
            tx_id: 2,
            amount: Some(dec!(500.0)),
            under_dispute: false,
        };

        ShardedEngine::process_transaction_in_shard(&mut shard_state, withdrawal).unwrap();

        let account = shard_state.accounts.get(&1).unwrap();
        assert_eq!(account.available, dec!(500.0));
        assert_eq!(account.total, dec!(500.0));
        assert_eq!(account.held, dec!(0.0));

        let tx = shard_state.transactions.get(&2).unwrap();
        assert_eq!(tx.tx_type, TransactionType::Withdrawal);
        assert_eq!(tx.amount, Some(dec!(500.0)));
    }

    #[tokio::test]
    async fn test_process_dispute() {
        let mut shard_state = ShardState {
            accounts: HashMap::new(),
            transactions: HashMap::new(),
        };

        let deposit = Transaction {
            tx_type: TransactionType::Deposit,
            client: 1,
            tx_id: 1,
            amount: Some(dec!(1000.0)),
            under_dispute: false,
        };

        ShardedEngine::process_transaction_in_shard(&mut shard_state, deposit).unwrap();

        let dispute = Transaction {
            tx_type: TransactionType::Dispute,
            client: 1,
            tx_id: 1,
            amount: None,
            under_dispute: true,
        };

        ShardedEngine::process_transaction_in_shard(&mut shard_state, dispute).unwrap();

        let account = shard_state.accounts.get(&1).unwrap();
        assert_eq!(account.available, dec!(0.0));
        assert_eq!(account.held, dec!(1000.0));
        assert_eq!(account.total, dec!(1000.0));

        let tx = shard_state.transactions.get(&1).unwrap();
        assert!(tx.under_dispute);
    }

    #[tokio::test]
    async fn test_process_resolve() {
        let mut shard_state = ShardState {
            accounts: HashMap::new(),
            transactions: HashMap::new(),
        };

        let deposit = Transaction {
            tx_type: TransactionType::Deposit,
            client: 1,
            tx_id: 1,
            amount: Some(dec!(1000.0)),
            under_dispute: false,
        };

        ShardedEngine::process_transaction_in_shard(&mut shard_state, deposit).unwrap();

        let dispute = Transaction {
            tx_type: TransactionType::Dispute,
            client: 1,
            tx_id: 1,
            amount: None,
            under_dispute: true,
        };

        ShardedEngine::process_transaction_in_shard(&mut shard_state, dispute).unwrap();

        let resolve = Transaction {
            tx_type: TransactionType::Resolve,
            client: 1,
            tx_id: 1,
            amount: None,
            under_dispute: false,
        };

        ShardedEngine::process_transaction_in_shard(&mut shard_state, resolve).unwrap();

        let account = shard_state.accounts.get(&1).unwrap();
        assert_eq!(account.available, dec!(1000.0));
        assert_eq!(account.held, dec!(0.0));
        assert_eq!(account.total, dec!(1000.0));

        let tx = shard_state.transactions.get(&1).unwrap();
        assert!(tx.under_dispute);
    }

    #[tokio::test]
    async fn test_process_chargeback() {
        let mut shard_state = ShardState {
            accounts: HashMap::new(),
            transactions: HashMap::new(),
        };

        let deposit = Transaction {
            tx_type: TransactionType::Deposit,
            client: 1,
            tx_id: 1,
            amount: Some(dec!(1000.0)),
            under_dispute: false,
        };

        ShardedEngine::process_transaction_in_shard(&mut shard_state, deposit).unwrap();

        let dispute = Transaction {
            tx_type: TransactionType::Dispute,
            client: 1,
            tx_id: 1,
            amount: None,
            under_dispute: true,
        };

        ShardedEngine::process_transaction_in_shard(&mut shard_state, dispute).unwrap();

        let chargeback = Transaction {
            tx_type: TransactionType::Chargeback,
            client: 1,
            tx_id: 1,
            amount: None,
            under_dispute: false,
        };

        ShardedEngine::process_transaction_in_shard(&mut shard_state, chargeback).unwrap();

        let account = shard_state.accounts.get(&1).unwrap();
        assert_eq!(account.available, dec!(0.0));
        assert_eq!(account.held, dec!(0.0));
        assert_eq!(account.total, dec!(0.0));
        assert!(account.locked);

        let tx = shard_state.transactions.get(&1).unwrap();
        assert!(tx.under_dispute);
    }

    #[tokio::test]
    async fn test_insufficient_funds_withdrawal() {
        let mut shard_state = ShardState {
            accounts: HashMap::new(),
            transactions: HashMap::new(),
        };

        let deposit = Transaction {
            tx_type: TransactionType::Deposit,
            client: 1,
            tx_id: 1,
            amount: Some(dec!(500.0)),
            under_dispute: false,
        };

        ShardedEngine::process_transaction_in_shard(&mut shard_state, deposit).unwrap();

        let withdrawal = Transaction {
            tx_type: TransactionType::Withdrawal,
            client: 1,
            tx_id: 2,
            amount: Some(dec!(1000.0)),
            under_dispute: false,
        };

        let result = ShardedEngine::process_transaction_in_shard(&mut shard_state, withdrawal);
        assert!(result.is_err());

        let account = shard_state.accounts.get(&1).unwrap();
        assert_eq!(account.available, dec!(500.0));
        assert_eq!(account.total, dec!(500.0));
        assert_eq!(account.held, dec!(0.0));

        let tx = shard_state.transactions.get(&2);
        assert!(tx.is_none());
    }

    #[tokio::test]
    async fn test_concurrent_transactions() {
        let mut engine = ShardedEngine::new(4);

        let mut handles = vec![];

        for i in 0..100 {
            let engine = engine.clone();
            let transaction = Transaction {
                tx_type: if i % 2 == 0 {
                    TransactionType::Deposit
                } else {
                    TransactionType::Withdrawal
                },
                client: i % 10,
                tx_id: i as u32,
                amount: Some(dec!(1000.0)),
                under_dispute: false,
            };

            let handle = tokio::spawn(async move {
                engine.route_transaction(transaction).unwrap();
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap();
        }

        engine.shutdown();
        engine.wait_for_completion().await;

        for shard in &engine.shards {
            let shard_state = shard.lock().await;
            for account in shard_state.accounts.values() {
                // Ensure the account is consistent.
                assert!(account.available >= dec!(0.0));
                assert!(account.total >= account.available);
            }
        }
    }
}
