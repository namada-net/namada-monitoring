use std::{collections::HashMap, sync::Arc, time::Duration};

use futures::future::try_join_all;
use tokio::sync::RwLock;

use crate::{
    alerts::AlertManager,
    checks::CheckManager,
    error::BlockNotAvailable,
    metrics::MetricsExporter,
    rpc::Rpc,
    shared::supply::Supply,
    state::{BlockState, State},
};

/// How many times the initial height query is retried before giving up.
const STARTUP_ATTEMPTS: u32 = 10;

pub struct Manager {
    pub metrics_exporter: MetricsExporter,
    pub checks: CheckManager,
    pub alerts: AlertManager,
    pub rpc: Rpc,
    pub state: State,
    /// Token addresses to track, from the toml configuration.
    tokens: Vec<String>,
    /// Address of the chain's native token. Used to pick the supply query:
    /// only the native token has an effective supply. Resolved lazily on the
    /// first processed block so a flaky RPC delays startup instead of aborting
    /// it.
    native_token: Option<String>,
}

impl Manager {
    pub async fn new(config: &crate::config::AppConfig) -> (Arc<RwLock<Self>>, u32) {
        let metrics_exporter = MetricsExporter::new(config);
        let checks = CheckManager::new(config);
        let alerts = AlertManager::new(config);
        let rpc = Rpc::new(&config.rpc)
            .await
            .expect("Should be able to create RPC client");

        let initial_block_height = match config.initial_block_height {
            u32::MAX => Self::query_initial_height(&rpc, config.sleep_for).await,
            height => height,
        };

        metrics_exporter
            .start_exporter()
            .expect("Should be able to start metrics exporter");

        let manager = Self {
            metrics_exporter,
            checks,
            alerts,
            rpc,
            state: State::default(),
            tokens: config
                .get_config()
                .tokens
                .iter()
                .map(|token| token.token.clone())
                .collect(),
            native_token: None,
        };

        for check in manager.checks.get_checks() {
            tracing::info!("Loaded check: {}", check);
        }

        (Arc::new(RwLock::new(manager)), initial_block_height)
    }

    /// Height to start monitoring from. A node that is briefly unreachable at
    /// boot should not crash-loop the monitor, so this retries rather than
    /// failing on the first error.
    async fn query_initial_height(rpc: &Rpc, sleep_for: u64) -> u32 {
        for attempt in 1..=STARTUP_ATTEMPTS {
            match rpc.query_lastest_height().await {
                Ok(height) => return height,
                Err(err) => {
                    tracing::warn!(
                        "Could not query the latest block height (attempt {}/{}): {}",
                        attempt,
                        STARTUP_ATTEMPTS,
                        err
                    );
                    tokio::time::sleep(Duration::from_secs(sleep_for)).await;
                }
            }
        }
        panic!("Could not query the latest block height after {STARTUP_ATTEMPTS} attempts");
    }

    pub fn has_enough_blocks(&self) -> bool {
        self.state.total_blocks() > 1
    }

    pub async fn update_next_state(&mut self, block_height: u64) -> anyhow::Result<()> {
        // Once the monitor has caught up, it asks for a block the chain has not
        // produced yet. Detect that up front so it can be retried quietly,
        // instead of surfacing as a failed storage query.
        let latest_height = self.rpc.query_lastest_height().await? as u64;
        self.state.set_chain_head(latest_height);
        if block_height > latest_height {
            return Err(BlockNotAvailable(block_height).into());
        }

        if self.native_token.is_none() {
            self.native_token = Some(self.rpc.query_native_token().await?);
        }

        let last_epoch = self.state.last_block().map(|block| block.block.epoch);

        let checksums = self.rpc.query_checksums_at_height(block_height).await?;
        let epoch = self
            .rpc
            .query_epoch_at_height(block_height)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No epoch defined at height {}", block_height))?;
        let block = self
            .rpc
            .query_block_at_height(block_height, &checksums, epoch)
            .await?;

        // The validator set only changes on epoch boundaries, so within an
        // epoch the previous block's (shared) set is reused as-is.
        let validators = match (last_epoch, self.state.last_block()) {
            (Some(last_epoch), Some(previous)) if last_epoch == epoch => {
                Arc::clone(&previous.validators)
            }
            _ => Arc::new(self.rpc.query_validators(epoch).await?),
        };

        let (bonds, unbonds) = self.rpc.query_future_bonds_and_unbonds(epoch).await?;

        let (supplies, mint_limits) = self.query_token_state().await?;

        let block_state = BlockState::new(block, bonds, unbonds, validators, supplies, mint_limits);
        self.state.add_block(block_state);

        Ok(())
    }

    /// Supply and IBC mint limit of every configured token, queried concurrently.
    async fn query_token_state(&self) -> anyhow::Result<(Vec<Supply>, HashMap<String, u64>)> {
        let queries = self.tokens.iter().map(|address| async move {
            let supply = if Some(address) == self.native_token.as_ref() {
                self.rpc.query_native_token_supply(address).await?
            } else {
                self.rpc.query_token_supply(address).await?
            };
            let limit = self.rpc.query_token_ibc_limit(address).await?;
            Ok::<_, anyhow::Error>((supply, (address.clone(), limit)))
        });

        Ok(try_join_all(queries).await?.into_iter().unzip())
    }
}
