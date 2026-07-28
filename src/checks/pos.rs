use std::{fmt::Display, time::Duration};

use super::{AppConfig, CheckTrait};

const POS_ONE_THIRD_CHECK_ID: &str = "pos_one_third_check";
const POS_TWO_THIRD_CHECK_ID: &str = "pos_two_third_check";
const POS_BONDS_CHECK_ID: &str = "pos_bonds_check";
const POS_UNBONDS_CHECK_ID: &str = "pos_unbonds_check";
const POS_CONSENSUS_CHECK_ID: &str = "pos_consensus_check";
const POS_MISSED_VOTES_CHECK_ID: &str = "pos_missed_votes_check";

/// Missed votes are evaluated on every block. Without dampening a validator
/// outage would emit an alert every few seconds for as long as it lasts.
const MISSED_VOTES_DAMPENING: Duration = Duration::from_secs(60 * 15);

pub struct PoSCheck {
    mininimum_one_third_validators: u64,
    mininimum_two_third_validators: u64,
    bond_increase_threshold: f64,
    unbond_increase_threshold: f64,
    consensus_threshold: f64,
    threshold_missed_votes: f64,
}

impl PoSCheck {
    pub fn new(config: &AppConfig) -> Self {
        let config = config.get_config();
        Self {
            mininimum_one_third_validators: config.pos.mininimum_one_third_validators,
            mininimum_two_third_validators: config.pos.mininimum_two_third_validators,
            bond_increase_threshold: config.pos.bond_increase_threshold,
            unbond_increase_threshold: config.pos.unbond_increase_threshold,
            consensus_threshold: config.pos.consensus_threshold,
            threshold_missed_votes: config.pos.threshold_missed_votes,
        }
    }
}

#[async_trait::async_trait]
impl CheckTrait for PoSCheck {
    async fn check(&self, state: &super::State) -> Vec<crate::shared::alert::Alert> {
        let (Some(last_state), Some(prev_state)) = (state.last_block(), state.prev_block()) else {
            return Vec::new();
        };

        let mut alerts = Vec::new();

        let (one_third_validators, two_third_validators) = match (
            state.validators_with_voting_power(1.0 / 3.0),
            state.validators_with_voting_power(2.0 / 3.0),
        ) {
            (Ok(one_third), Ok(two_third)) => (one_third, two_third),
            (one_third, two_third) => {
                // An empty or unreadable validator set is a query problem, not a
                // centralization problem: skip rather than alert on garbage.
                tracing::warn!(
                    "Skipping voting power distribution checks: {:?} {:?}",
                    one_third.err(),
                    two_third.err()
                );
                (
                    self.mininimum_one_third_validators,
                    self.mininimum_two_third_validators,
                )
            }
        };

        if one_third_validators < self.mininimum_one_third_validators {
            alerts.push(crate::shared::alert::Alert {
                check_id: POS_ONE_THIRD_CHECK_ID.to_string(),
                title: "Centralized voting power".to_string(),
                description: format!(
                    "Only {} validators are needed to reach 1/3 of the voting power",
                    one_third_validators
                ),
                metadata: crate::shared::alert::Metadata {
                    block_height: Some(last_state.block.height as u32),
                    tx_id: None,
                },
                severity: crate::shared::alert::Severity::Low,
                trigger_after: Some(Duration::from_secs(60 * 60 * 6)),
                continous: self.is_continous(),
            });
        }

        if two_third_validators < self.mininimum_two_third_validators {
            alerts.push(crate::shared::alert::Alert {
                check_id: POS_TWO_THIRD_CHECK_ID.to_string(),
                title: "Centralized voting power".to_string(),
                description: format!(
                    "Only {} validators are needed to reach 2/3 of the voting power",
                    two_third_validators
                ),
                metadata: crate::shared::alert::Metadata {
                    block_height: Some(last_state.block.height as u32),
                    tx_id: None,
                },
                severity: crate::shared::alert::Severity::Low,
                trigger_after: Some(Duration::from_secs(60 * 60 * 6)),
                continous: self.is_continous(),
            });
        }

        let current_consensus_validators = last_state.total_consensus_validators() as f64;
        let prev_consensus_validators = prev_state.total_consensus_validators() as f64;

        // consensus_threshold is the tolerated *drop*: 0.1 means alert once a
        // tenth of the consensus set is gone. Comparing against
        // `prev * threshold` instead would only fire after a 90% collapse.
        let consensus_floor = prev_consensus_validators * (1.0 - self.consensus_threshold);
        if current_consensus_validators < consensus_floor {
            alerts.push(crate::shared::alert::Alert {
                check_id: POS_CONSENSUS_CHECK_ID.to_string(),
                title: "Low consensus validators".to_string(),
                description: format!(
                    "Consensus validators dropped from {} to {} in epoch {}",
                    prev_consensus_validators, current_consensus_validators, last_state.block.epoch
                ),
                metadata: crate::shared::alert::Metadata {
                    block_height: Some(last_state.block.height as u32),
                    tx_id: None,
                },
                severity: crate::shared::alert::Severity::Medium,
                trigger_after: Some(Duration::from_secs(60 * 60 * 6)),
                continous: self.is_continous(),
            });
        }

        let current_bonds = last_state.bonds as f64;
        let prev_bonds = prev_state.bonds as f64;
        if prev_bonds + prev_bonds * self.bond_increase_threshold < current_bonds {
            alerts.push(crate::shared::alert::Alert {
                check_id: POS_BONDS_CHECK_ID.to_string(),
                title: "High bond increase".to_string(),
                description: format!(
                    "Bonds increased from {} to {} in epoch {}",
                    prev_bonds, current_bonds, last_state.block.epoch
                ),
                metadata: crate::shared::alert::Metadata {
                    block_height: Some(last_state.block.height as u32),
                    tx_id: None,
                },
                severity: crate::shared::alert::Severity::Low,
                trigger_after: Some(Duration::from_secs(60 * 60)),
                continous: self.is_continous(),
            });
        }

        let current_unbonds = last_state.unbonds as f64;
        let prev_unbonds = prev_state.unbonds as f64;
        if prev_unbonds + prev_unbonds * self.unbond_increase_threshold < current_unbonds {
            alerts.push(crate::shared::alert::Alert {
                check_id: POS_UNBONDS_CHECK_ID.to_string(),
                title: "High unbond increase".to_string(),
                description: format!(
                    "Unbonds increased from {} to {} in epoch {}",
                    prev_unbonds, current_unbonds, last_state.block.epoch
                ),
                metadata: crate::shared::alert::Metadata {
                    block_height: Some(last_state.block.height as u32),
                    tx_id: None,
                },
                severity: crate::shared::alert::Severity::Low,
                trigger_after: Some(Duration::from_secs(60 * 60)),
                continous: self.is_continous(),
            });
        }

        // The commit lists one entry per validator, absent ones included, so the
        // number of entries is the expected vote count and only the entries
        // flagged as commits are actual votes.
        let missed_votes_percentage = match last_state.commit_votes() {
            Some((signed, expected)) if expected > 0 => 1.0 - (signed as f64 / expected as f64),
            _ => 0.0,
        };

        if missed_votes_percentage > self.threshold_missed_votes {
            alerts.push(crate::shared::alert::Alert {
                check_id: POS_MISSED_VOTES_CHECK_ID.to_string(),
                title: "High missed votes percentage".to_string(),
                description: format!(
                    "Missed votes percentage is {:.2}% which is above the threshold of {:.2}%",
                    missed_votes_percentage * 100.0,
                    self.threshold_missed_votes * 100.0
                ),
                metadata: crate::shared::alert::Metadata {
                    block_height: Some(last_state.block.height as u32),
                    tx_id: None,
                },
                severity: crate::shared::alert::Severity::Medium,
                trigger_after: Some(MISSED_VOTES_DAMPENING),
                continous: self.is_continous(),
            });
        }

        alerts
    }

    fn is_continous(&self) -> bool {
        false
    }
}

impl Display for PoSCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PoSCheck(mininimum_one_third_validators: {}, mininimum_two_third_validators: {}, bond_increase_threshold: {}, unbond_increase_threshold: {}, consensus_threshold: {}, threshold_missed_votes: {})",
            self.mininimum_one_third_validators,
            self.mininimum_two_third_validators,
            self.bond_increase_threshold,
            self.unbond_increase_threshold,
            self.consensus_threshold,
            self.threshold_missed_votes
        )
    }
}
