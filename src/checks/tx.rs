use std::{fmt::Display, time::Duration};

use crate::shared::alert::{summarize, Alert, Metadata, Severity};

use super::{AppConfig, CheckTrait};

const TX_SECTIONS_CHECK_ID: &str = "tx_sections_check";
const TX_BATCH_CHECK_ID: &str = "tx_batch_check";

/// Oversized transactions tend to arrive in bursts. Report a block's offenders
/// together, then stay quiet for a while.
const DAMPENING: Duration = Duration::from_secs(60 * 15);

pub struct TxCheck {
    pub threshold_sections: u64,
    pub threshold_batch: u64,
}

impl TxCheck {
    pub fn new(config: &AppConfig) -> Self {
        let config = config.get_config();
        Self {
            threshold_sections: config.tx.threshold_sections,
            threshold_batch: config.tx.threshold_batch,
        }
    }
}

#[async_trait::async_trait]
impl CheckTrait for TxCheck {
    async fn check(&self, states: &super::State) -> Vec<Alert> {
        let Some(last_block) = states.last_block() else {
            return Vec::new();
        };

        let mut wide_sections = Vec::new();
        let mut large_batches = Vec::new();

        for transaction in &last_block.block.transactions {
            if transaction.total_sections
                > self.threshold_sections * transaction.inners.len() as u64
            {
                wide_sections.push(format!(
                    "{} has {} sections over {} inner transactions",
                    transaction.id,
                    transaction.total_sections,
                    transaction.inners.len()
                ));
            }

            if transaction.inners.len() as u64 > self.threshold_batch {
                large_batches.push(format!(
                    "{} batches {} inner transactions",
                    transaction.id,
                    transaction.inners.len()
                ));
            }
        }

        let height = last_block.block.height;
        let mut alerts = Vec::new();

        if !wide_sections.is_empty() {
            alerts.push(Alert {
                check_id: TX_SECTIONS_CHECK_ID.to_string(),
                title: "Transaction sections limit exceeded".to_string(),
                description: format!(
                    "*{}* transactions in block {} exceeded the section limit of {} per inner transaction:{}",
                    wide_sections.len(),
                    height,
                    self.threshold_sections,
                    summarize(&wide_sections)
                ),
                metadata: Metadata::new(Some(height as u32), None),
                severity: Severity::Low,
                trigger_after: Some(DAMPENING),
                continous: self.is_continous(),
            });
        }

        if !large_batches.is_empty() {
            alerts.push(Alert {
                check_id: TX_BATCH_CHECK_ID.to_string(),
                title: "Transaction batch limit exceeded".to_string(),
                description: format!(
                    "*{}* transactions in block {} exceeded the batch limit of {}:{}",
                    large_batches.len(),
                    height,
                    self.threshold_batch,
                    summarize(&large_batches)
                ),
                metadata: Metadata::new(Some(height as u32), None),
                severity: Severity::Low,
                trigger_after: Some(DAMPENING),
                continous: self.is_continous(),
            });
        }

        alerts
    }

    fn is_continous(&self) -> bool {
        false
    }
}

impl Display for TxCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TxCheck(threshold_sections: {}, threshold_batch: {})",
            self.threshold_sections, self.threshold_batch
        )
    }
}
