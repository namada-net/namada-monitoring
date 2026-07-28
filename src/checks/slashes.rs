use std::fmt::Display;

use namada_sdk::tendermint::evidence;

use crate::{
    shared::alert::{Alert, Metadata, Severity},
    state::State,
};

use super::CheckTrait;

const SLASHES_CHECK_ID: &str = "slash_check";

#[derive(Debug, Clone, Default)]
pub struct SlashCheck {}

#[async_trait::async_trait]
impl CheckTrait for SlashCheck {
    async fn check(&self, state: &State) -> Vec<Alert> {
        let Some(last_state) = state.last_block() else {
            return Vec::new();
        };

        last_state.block.block.evidence.iter().map(|evidence| {
            match evidence {
                evidence::Evidence::DuplicateVote(duplicate_vote_evidence) => {
                    let description = format!(
                        "✂️ Duplicate vote evidence found. Total power: {}. Validator {} voted {} at height {}. Validator {} voted {} at height {}.",
                        duplicate_vote_evidence.total_voting_power,
                        duplicate_vote_evidence.vote_a.validator_address,
                        duplicate_vote_evidence.vote_a.vote_type,
                        duplicate_vote_evidence.vote_a.height,
                        duplicate_vote_evidence.vote_b.validator_address,
                        duplicate_vote_evidence.vote_b.vote_type,
                        duplicate_vote_evidence.vote_b.height
                    );
                    let title = format!(
                        "Duplicate vote evidence found for block {}",
                        last_state.block.block.header.height
                    );
                    Alert {
                        title,
                        description,
                        metadata: Metadata::new(
                            Some(last_state.block.block.header.height.value() as u32),
                            None
                        ),
                        severity: Severity::Critical,
                        check_id: SLASHES_CHECK_ID.to_string(),
                        trigger_after: None,
                        continous: self.is_continous(),
                    }
                }
                evidence::Evidence::LightClientAttack(light_client_attack_evidence) => {
                    let description = format!(
                        "✂️ Light client attack evidence found. Total power: {}. Conflicting block height: {}. Conflicting block proposer: {}. Common height: {}. Byzantine validators: {:?}.",
                        light_client_attack_evidence.total_voting_power,
                        light_client_attack_evidence
                            .conflicting_block
                            .signed_header
                            .header
                            .height,
                        light_client_attack_evidence
                            .conflicting_block
                            .signed_header
                            .header
                            .proposer_address,
                        light_client_attack_evidence.common_height,
                        light_client_attack_evidence.byzantine_validators
                    );
                    let title = format!(
                        "Light client attack evidence found for block {}",
                        last_state.block.block.header.height
                    );
                    Alert {
                        title,
                        description,
                        metadata: Metadata::new(
                            Some(last_state.block.block.header.height.value() as u32),
                            None
                        ),
                        severity: Severity::Critical,
                        check_id: SLASHES_CHECK_ID.to_string(),
                        trigger_after: None,
                        continous: self.is_continous()
                    }
                }
            }
        }).collect()
    }

    fn is_continous(&self) -> bool {
        false
    }
}

impl Display for SlashCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SlashCheck")
    }
}
