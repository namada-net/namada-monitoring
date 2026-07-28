use std::{
    fmt::Display,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::shared::alert::{Alert, Metadata, Severity};

use super::{AppConfig, CheckTrait};

const AVG_BLOCK_TIME_CHECK_ID: &str = "average_block_time_check";
const HALT_CHECK_ID: &str = "halt_check";

const MIN_LOOKBACK_WINDOW: usize = 2;

pub struct HaltCheck {
    estimated_block_time: f64,
    halt_threshold: u64,
    avg_block_time_factor: f64,
    lookback_window: usize,
    max_lag_blocks: u64,
}

#[async_trait::async_trait]
impl CheckTrait for HaltCheck {
    async fn check(&self, state: &super::State) -> Vec<Alert> {
        let mut alerts = Vec::new();

        let blocks = state
            .last_n_blocks(self.lookback_window)
            .collect::<Vec<_>>();
        let num_blocks = blocks.len();

        // Comparing wall-clock time against the last *processed* block only says
        // anything about the chain once the monitor has caught up. While
        // backfilling, every block is legitimately old.
        if state.is_caught_up(self.max_lag_blocks) {
            if let Some(last_block) = blocks.last() {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|elapsed| elapsed.as_secs() as i64)
                    .unwrap_or_default();
                let last_block_time = last_block.block.timestamp;

                if now > last_block_time && (now - last_block_time) > self.halt_threshold as i64 {
                    alerts.push(Alert {
                        check_id: HALT_CHECK_ID.to_string(),
                        title: "Chain might be halted".to_string(),
                        description: format!(
                            "No new block has been produced for more than {} seconds.",
                            self.halt_threshold
                        ),
                        severity: Severity::Critical,
                        metadata: Metadata::new(Some(last_block.block.height as u32), None),
                        trigger_after: Some(Duration::from_secs(60)),
                        continous: self.is_continous(),
                    });
                }
            }
        }

        if num_blocks >= MIN_LOOKBACK_WINDOW {
            let mut total_diffs = 0f64;
            let mut count = 0u32;

            for window in blocks.windows(2) {
                let t1 = window[0].block.timestamp;
                let t2 = window[1].block.timestamp;
                if t2 > t1 {
                    total_diffs += (t2 - t1) as f64;
                    count += 1;
                }
            }

            if count > 0 {
                let avg = total_diffs / count as f64;
                if avg > self.estimated_block_time * self.avg_block_time_factor {
                    alerts.push(Alert {
                        check_id: AVG_BLOCK_TIME_CHECK_ID.to_string(),
                        title: "High average block time".to_string(),
                        description: format!(
                            "Average block time over the last {} blocks is *{:.2}* seconds, above the expected {:.2}.",
                            num_blocks,
                            avg,
                            self.estimated_block_time
                        ),
                        severity: Severity::Medium,
                        metadata: Metadata::new(
                            blocks.last().map(|block| block.block.height as u32),
                            None,
                        ),
                        trigger_after: Some(Duration::from_secs(60)),
                        continous: self.is_continous(),
                    });
                }
            }
        }

        alerts
    }

    fn is_continous(&self) -> bool {
        true
    }
}

impl Display for HaltCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "HaltCheck(estimated_block_time: {}, halt_threshold: {}, avg_block_time_factor: {}, lookback_window: {})",
            self.estimated_block_time,
            self.halt_threshold,
            self.avg_block_time_factor,
            self.lookback_window
        )
    }
}

impl HaltCheck {
    pub fn new(config: &AppConfig) -> Self {
        let config = config.get_config();
        Self {
            estimated_block_time: config.chain.block_time,
            halt_threshold: config.chain.halt_threshold,
            avg_block_time_factor: config.chain.avg_block_time_factor,
            lookback_window: config.chain.block_time_lookback,
            max_lag_blocks: config.chain.max_lag_blocks,
        }
    }
}
