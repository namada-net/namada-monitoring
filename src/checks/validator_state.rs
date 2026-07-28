use std::{collections::HashMap, fmt::Display};

use namada_sdk::proof_of_stake::types::ValidatorState as PosValidatorState;

use crate::shared::alert::{summarize, Alert, Metadata, Severity};

use super::CheckTrait;

const JAILED_CHECK_ID: &str = "validator_jailed_check";
const LEFT_CONSENSUS_CHECK_ID: &str = "validator_left_consensus_check";

/// Alerts on validators changing proof-of-stake state.
///
/// The validator set is shared between all the blocks of an epoch, so this only
/// has anything to compare at an epoch boundary. That is also the only moment
/// the states can actually change.
#[derive(Debug, Clone, Default)]
pub struct ValidatorStateCheck {}

#[async_trait::async_trait]
impl CheckTrait for ValidatorStateCheck {
    async fn check(&self, state: &crate::state::State) -> Vec<Alert> {
        let (Some(last_state), Some(prev_state)) = (state.last_block(), state.prev_block()) else {
            return Vec::new();
        };

        if last_state.block.epoch == prev_state.block.epoch {
            return Vec::new();
        }

        let previous = prev_state
            .validators
            .iter()
            .map(|validator| (validator.address.as_str(), validator.state))
            .collect::<HashMap<_, _>>();

        let mut newly_jailed = Vec::new();
        let mut left_consensus = Vec::new();

        for validator in last_state.validators.iter() {
            let Some(previous_state) = previous.get(validator.address.as_str()) else {
                continue;
            };

            if validator.state == *previous_state {
                continue;
            }

            if matches!(validator.state, PosValidatorState::Jailed) {
                newly_jailed.push(format!(
                    "{} ({:?} -> jailed)",
                    validator.address, previous_state
                ));
            } else if matches!(previous_state, PosValidatorState::Consensus) {
                left_consensus.push(format!(
                    "{} (consensus -> {:?})",
                    validator.address, validator.state
                ));
            }
        }

        let epoch = last_state.block.epoch;
        let height = last_state.block.height as u32;
        let mut alerts = Vec::new();

        if !newly_jailed.is_empty() {
            alerts.push(Alert {
                check_id: JAILED_CHECK_ID.to_string(),
                title: "Validators jailed".to_string(),
                description: format!(
                    "*{}* validators were jailed in epoch {}:{}",
                    newly_jailed.len(),
                    epoch,
                    summarize(&newly_jailed)
                ),
                severity: Severity::High,
                metadata: Metadata::new(Some(height), None),
                trigger_after: None,
                continous: self.is_continous(),
            });
        }

        if !left_consensus.is_empty() {
            alerts.push(Alert {
                check_id: LEFT_CONSENSUS_CHECK_ID.to_string(),
                title: "Validators left the consensus set".to_string(),
                description: format!(
                    "*{}* validators left the consensus set in epoch {}:{}",
                    left_consensus.len(),
                    epoch,
                    summarize(&left_consensus)
                ),
                severity: Severity::Medium,
                metadata: Metadata::new(Some(height), None),
                trigger_after: None,
                continous: self.is_continous(),
            });
        }

        alerts
    }

    fn is_continous(&self) -> bool {
        false
    }
}

impl Display for ValidatorStateCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ValidatorStateCheck")
    }
}
