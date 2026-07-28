/// ## Transaction metrics. transaction_batch_size and transaction_kind
/// This set of metrics tracks transaction activity in the Namada blockchain, capturing both batch sizes and transaction kinds per epoch.
/// These metrics help monitor network load, transaction diversity, and failure rates.
///
/// ### transaction_batch_size (Histogram)
///
/// Measures the number of inner transactions within a batch. Provides insights into how transactions are grouped and processed. Uses predefined buckets (1, 2, 4, 8, ..., 256)
///
/// #### Usage and Interpretation
/// * A skewed distribution in transaction_batch_size may suggest inefficient batch processing.
///
/// ### Example
/// ```
/// # HELP transaction_batch_size The number of inner transactions in the batch
/// # TYPE transaction_batch_size histogram
/// transaction_batch_size_bucket{le="1"} 5
/// transaction_batch_size_bucket{le="2"} 10
/// transaction_batch_size_bucket{le="4"} 20
/// transaction_batch_size_bucket{le="8"} 30
/// transaction_batch_size_bucket{le="16"} 40
/// transaction_batch_size_bucket{le="32"} 50
/// transaction_batch_size_count 80
/// transaction_batch_size_sum 3200
/// ```
///
/// ### transaction_kind (CounterVec)
/// Tracks the count of different transaction types per epoch. A high failure rate (failed = "true") should be considered abnormal.
///
/// #### Labels:
///         - kind: The specific type of transaction:
///         - transfer: Standard token transfer.
///         - ibc_transfer: Cross-chain transfer via IBC.
///         - bond/unbond/redelegate: Staking-related actions.
///         - claim_rewards/withdraw: Reward and withdrawal operations.
///         - vote_proposal/init_proposal: Governance voting and proposal creation.
///         - metadata_change/commission_change”: Validator updates.
///         - reveal_public_key: Public key revelation.
///         - become_validator/deactivate_validator/reactivate_validator/unjail_validator: Validator lifecycle actions.
///         - epoch: The epoch in which the transaction was included.
///         - failed: A boolean (true/false) indicating if the transaction failed.
///
/// ### Example
/// ```
/// # HELP transaction_kind Transaction kind count per epoch
/// # TYPE transaction_kind counter
/// transaction_kind{kind="transfer", failed="false"} 120
/// transaction_kind{kind="bond", failed="false"} 15
/// transaction_kind{kind="vote_proposal", failed="true"} 3
/// ```
use crate::state::State;
use anyhow::Result;
use prometheus_exporter::prometheus::{
    core::{AtomicU64, GenericCounterVec},
    Histogram, HistogramOpts, IntCounterVec, Opts, Registry,
};

use super::MetricTrait;

pub struct Transactions {
    /// inner transactions count in the batch histogram
    transaction_batch_size: Histogram,
    /// inner transaction kind counter by epoch
    transaction_kind: GenericCounterVec<AtomicU64>,
}

impl MetricTrait for Transactions {
    fn register(&self, registry: &Registry) -> Result<()> {
        registry.register(Box::new(self.transaction_batch_size.clone()))?;
        registry.register(Box::new(self.transaction_kind.clone()))?;
        Ok(())
    }

    fn update(&self, state: &State) {
        let Some(last_state) = state.last_block() else {
            return;
        };

        for tx in &last_state.block.transactions {
            self.transaction_batch_size.observe(tx.inners.len() as f64);
            for inner in &tx.inners {
                let inner_kind = inner.kind.to_string();
                let failed = !inner.was_applied;
                self.transaction_kind
                    .with_label_values(&[&inner_kind, &failed.to_string()])
                    .inc();
            }
        }
    }
}

impl Default for Transactions {
    fn default() -> Self {
        let transaction_batch_size_opts = HistogramOpts::new(
            "transaction_batch_size",
            "The number of inner transactions in the batch",
        )
        .buckets(vec![1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0, 256.0]);
        let transaction_batch_size = Histogram::with_opts(transaction_batch_size_opts)
            .expect("unable to create histogram transaction batch size");

        let transaction_kind = Opts::new("transaction_kind", "Transaction kind count");
        let transaction_kind = IntCounterVec::new(transaction_kind, &["kind", "failed"])
            .expect("unable to create int counter for transaction kinds");

        Self {
            transaction_batch_size,
            transaction_kind,
        }
    }
}
