use crate::{
    constants::MICRO,
    shared::alert::{summarize, Alert, Metadata, Severity},
    state::State,
};

use super::{AppConfig, CheckTrait};
use std::{collections::HashMap, fmt::Display, time::Duration};

const FEE_CHECK_ID: &str = "fee_check";

/// A busy chain can have over-paying transactions in every block. Report the
/// offenders of one block together, then stay quiet for a while.
const DAMPENING: Duration = Duration::from_secs(60 * 15);

#[derive(Debug, Clone)]
struct DefaultFeeThreshold {
    name: String,
    value: f64,
}

#[derive(Debug, Clone, Default)]
pub struct FeeCheck {
    thresholds: HashMap<String, DefaultFeeThreshold>,
}

impl FeeCheck {
    fn populate_thresholds(&mut self, config: &AppConfig) {
        for (token, threshold) in config.get_config().tokens_fees() {
            self.thresholds.insert(
                token.clone(),
                DefaultFeeThreshold {
                    name: token,
                    value: threshold * MICRO,
                },
            );
        }
    }

    pub fn new(config: &AppConfig) -> Self {
        let mut instance = Self {
            thresholds: HashMap::new(),
        };
        instance.populate_thresholds(config);
        instance
    }
}

#[async_trait::async_trait]
impl CheckTrait for FeeCheck {
    async fn check(&self, state: &State) -> Vec<Alert> {
        let Some(last_state) = state.last_block() else {
            return Vec::new();
        };

        let offenders = last_state
            .block
            .transactions
            .iter()
            .filter_map(|tx| {
                let threshold = self.thresholds.get(&tx.fee.gas_token)?;
                let fee = tx.compute_fee();

                if fee <= threshold.value {
                    return None;
                }

                Some(format!(
                    "{} paid {} {} (threshold {})",
                    tx.id, fee, threshold.name, threshold.value
                ))
            })
            .collect::<Vec<_>>();

        if offenders.is_empty() {
            return Vec::new();
        }

        vec![Alert {
            check_id: FEE_CHECK_ID.to_string(),
            title: "Fee too high".to_string(),
            description: format!(
                "*{}* transactions in block {} paid a fee above the configured threshold:{}",
                offenders.len(),
                last_state.block.height,
                summarize(&offenders)
            ),
            metadata: Metadata::new(Some(last_state.block.height as u32), None),
            severity: Severity::Low,
            trigger_after: Some(DAMPENING),
            continous: self.is_continous(),
        }]
    }

    fn is_continous(&self) -> bool {
        false
    }
}

impl Display for FeeCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FeeCheck with {} thresholds", self.thresholds.len())
    }
}
