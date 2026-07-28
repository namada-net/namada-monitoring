use std::{fmt::Display, time::Duration};

use crate::shared::alert::{Alert, Metadata, Severity};

use super::{AppConfig, CheckTrait};

const LAG_CHECK_ID: &str = "lag_check";

pub struct LagCheck {
    max_lag_blocks: u64,
}

/// Watches the monitor itself.
///
/// Every other check reasons about the last block the monitor managed to
/// process. If the monitor falls behind the chain, those checks keep reporting
/// on stale data and quietly stop covering the present, which is the worst
/// failure mode for a monitoring tool: silence that looks like health.
#[async_trait::async_trait]
impl CheckTrait for LagCheck {
    async fn check(&self, state: &crate::state::State) -> Vec<Alert> {
        let (Some(lag), Some(last_state), Some(head)) =
            (state.lag(), state.last_block(), state.chain_head())
        else {
            return Vec::new();
        };

        if lag <= self.max_lag_blocks {
            return Vec::new();
        }

        vec![Alert {
            check_id: LAG_CHECK_ID.to_string(),
            title: "Monitor is behind the chain".to_string(),
            description: format!(
                "The monitor is *{}* blocks behind the chain head: last processed {}, chain head {}. Checks are reporting on stale data.",
                lag, last_state.block.height, head
            ),
            severity: Severity::High,
            metadata: Metadata::new(Some(last_state.block.height as u32), None),
            trigger_after: Some(Duration::from_secs(60 * 5)),
            continous: self.is_continous(),
        }]
    }

    fn is_continous(&self) -> bool {
        true
    }
}

impl Display for LagCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LagCheck(max_lag_blocks: {})", self.max_lag_blocks)
    }
}

impl LagCheck {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            max_lag_blocks: config.get_config().chain.max_lag_blocks,
        }
    }
}
