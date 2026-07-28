use std::fmt::Display;

use super::{AppConfig, CheckTrait};

const LIMIT_IBC_CHECK_ID: &str = "ibc_limit_check";

pub struct IbcLimitCheck {
    supply_limit_threshold: f64,
}

impl IbcLimitCheck {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            supply_limit_threshold: config.get_config().ibc.supply_limit_threshold,
        }
    }
}

#[async_trait::async_trait]
impl CheckTrait for IbcLimitCheck {
    async fn check(&self, state: &crate::state::State) -> Vec<crate::shared::alert::Alert> {
        let Some(last_state) = state.last_block() else {
            return Vec::new();
        };
        let mut alerts = Vec::new();

        for supply in &last_state.supplies {
            let limit = last_state.mint_limit.get(&supply.token);
            if let Some(limit) = limit {
                if *limit > 0 && supply.total as f64 > *limit as f64 * self.supply_limit_threshold {
                    let alert = crate::shared::alert::Alert {
                        check_id: format!("{}_{}", LIMIT_IBC_CHECK_ID, supply.token),
                        severity: crate::shared::alert::Severity::Low,
                        title: "IBC Token Supply Limit Alert".to_string(),
                        description: format!(
                            "IBC token {} supply {} almost reached limit {}",
                            supply.token, supply.total, limit
                        ),
                        metadata: crate::shared::alert::Metadata {
                            block_height: Some(last_state.block.height as u32),
                            tx_id: None,
                        },
                        continous: self.is_continous(),
                        trigger_after: Some(std::time::Duration::from_secs(60 * 60 * 6)), // Trigger after 6 hours
                    };
                    alerts.push(alert);
                }
            }
        }

        alerts
    }

    fn is_continous(&self) -> bool {
        false
    }
}

impl Display for IbcLimitCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "IbcLimitCheck(supply_limit_threshold: {})",
            self.supply_limit_threshold
        )
    }
}
