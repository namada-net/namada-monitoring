use std::{fmt::Display, time::Duration};

use crate::shared::alert::{summarize, Metadata};

use super::{AppConfig, CheckTrait};

const GAS_CHECK_ID: &str = "gas_check";

/// Over-provisioned gas is common and rarely urgent: report a block's offenders
/// together, then stay quiet for a while.
const DAMPENING: Duration = Duration::from_secs(60 * 15);

pub struct GasCheck {
    gas_limit_threshold: f64,
}

#[async_trait::async_trait]
impl CheckTrait for GasCheck {
    async fn check(&self, state: &super::State) -> Vec<crate::shared::alert::Alert> {
        let Some(last_state) = state.last_block() else {
            return Vec::new();
        };

        let offenders = last_state
            .block
            .transactions
            .iter()
            .filter_map(|transaction| {
                let gas_limit = transaction.fee.gas.parse::<f64>().unwrap_or_default();
                let gas_used = transaction.fee.gas_used as f64;

                // A transaction that consumed no gas was rejected, or its result
                // event was missing. Either way the ratio says nothing about how
                // well the gas limit was chosen.
                if gas_used <= 0.0 {
                    return None;
                }

                if gas_used * (1.0 + self.gas_limit_threshold) >= gas_limit {
                    return None;
                }

                Some(format!(
                    "{} reserved {} gas and used {}",
                    transaction.id, gas_limit, gas_used
                ))
            })
            .collect::<Vec<_>>();

        if offenders.is_empty() {
            return Vec::new();
        }

        vec![crate::shared::alert::Alert {
            check_id: GAS_CHECK_ID.to_string(),
            title: "Gas limit too high".to_string(),
            description: format!(
                "*{}* transactions in block {} reserved far more gas than they used:{}",
                offenders.len(),
                last_state.block.height,
                summarize(&offenders)
            ),
            metadata: Metadata::new(Some(last_state.block.height as u32), None),
            severity: crate::shared::alert::Severity::Low,
            trigger_after: Some(DAMPENING),
            continous: self.is_continous(),
        }]
    }

    fn is_continous(&self) -> bool {
        false
    }
}

impl Display for GasCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "GasCheck(gas_limit_threshold: {})",
            self.gas_limit_threshold
        )
    }
}

impl GasCheck {
    pub fn new(config: &AppConfig) -> Self {
        let config = config.get_config();
        Self {
            gas_limit_threshold: config.tx.gas_limit_threshold,
        }
    }
}
