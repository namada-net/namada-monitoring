use std::{collections::HashMap, fmt::Display};

use namada_sdk::{ibc, token::Amount};

use crate::{
    checks::AppConfig,
    shared::alert::{summarize, Alert, Metadata, Severity},
};

use super::CheckTrait;

const FLOW_IBC_CHECK_ID: &str = "transfer_limit_check";

#[derive(Debug, Clone)]
struct DefaultFeeThreshold {
    name: String,
    value: u64,
}

#[derive(Default)]
pub struct TransferLimitCheck {
    thresholds: HashMap<String, DefaultFeeThreshold>,
}

impl TransferLimitCheck {
    fn populate_thresholds(&mut self, config: &AppConfig) {
        for (address, threshold) in config.get_config().tokens_thresholds() {
            let token = address.clone();
            self.thresholds.insert(
                token.clone(),
                DefaultFeeThreshold {
                    name: token,
                    value: threshold,
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
impl CheckTrait for TransferLimitCheck {
    async fn check(&self, state: &crate::state::State) -> Vec<Alert> {
        let Some(last_state) = state.last_block() else {
            return Vec::new();
        };

        let applied_inners = last_state
            .block
            .transactions
            .iter()
            .filter(|wrapper| wrapper.is_successful())
            .flat_map(|wrapper| wrapper.inners.iter())
            .filter(|inner| inner.was_applied);

        let mut offenders = Vec::new();

        for inner in applied_inners {
            let (sources, is_ibc) = match &inner.kind {
                crate::shared::namada::InnerKind::Transfer(transfer) => (&transfer.sources, false),
                crate::shared::namada::InnerKind::IbcMsgTransfer(ibc::IbcMessage::Transfer(
                    msg_transfer,
                )) => match &msg_transfer.transfer {
                    Some(transfer) => (&transfer.sources, true),
                    None => continue,
                },
                _ => continue,
            };

            for (account, amount) in sources {
                let Some(threshold) = self.thresholds.get(&account.token.to_string()) else {
                    continue;
                };
                let token_threshold = Amount::from_u64(threshold.value);

                if amount.amount() <= token_threshold {
                    continue;
                }

                offenders.push(format!(
                    "{} sent {} {} in {}{}",
                    account.owner,
                    amount.amount(),
                    threshold.name,
                    inner.id,
                    if is_ibc { " (IBC)" } else { "" }
                ));
            }
        }

        if offenders.is_empty() {
            return Vec::new();
        }

        // Deliberately not dampened: a large transfer is a security signal and
        // suppressing later blocks could hide a second one. Aggregating per
        // block already caps this at one alert per block, and only when a
        // transfer actually crosses the threshold.
        vec![Alert {
            check_id: FLOW_IBC_CHECK_ID.to_string(),
            severity: Severity::Medium,
            title: "Transfer exceeded threshold".to_string(),
            description: format!(
                "*{}* transfers in block {} exceeded their configured threshold:{}",
                offenders.len(),
                last_state.block.height,
                summarize(&offenders)
            ),
            metadata: Metadata::new(Some(last_state.block.height as u32), None),
            continous: self.is_continous(),
            trigger_after: None,
        }]
    }

    fn is_continous(&self) -> bool {
        false
    }
}

impl Display for TransferLimitCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TransferLimitCheck with {} thresholds",
            self.thresholds.len()
        )
    }
}
