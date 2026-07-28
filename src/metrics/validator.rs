/// ## Validator State Metrics
/// One gauge per proof-of-stake validator state, counting how many validators
/// sit in that state in the latest observed block.
///
/// ### Example
/// ```
/// # HELP consensus_validators Number of validators that are in consensus state
/// # TYPE consensus_validators gauge
/// consensus_validators 100
/// ```
use super::MetricTrait;
use crate::state::State;
use anyhow::Result;
use namada_sdk::proof_of_stake::types::ValidatorState as EnumValidatorState;
use prometheus_exporter::prometheus::{Gauge, Opts, Registry};

pub struct ValidatorState {
    pub consensus_validators: Gauge,
    pub jailed_validators: Gauge,
    pub inactive_validators: Gauge,
    pub below_threshold_validators: Gauge,
    pub below_capacity_validators: Gauge,
}

impl MetricTrait for ValidatorState {
    fn register(&self, registry: &Registry) -> Result<()> {
        registry.register(Box::new(self.consensus_validators.clone()))?;
        registry.register(Box::new(self.jailed_validators.clone()))?;
        registry.register(Box::new(self.inactive_validators.clone()))?;
        registry.register(Box::new(self.below_threshold_validators.clone()))?;
        registry.register(Box::new(self.below_capacity_validators.clone()))?;
        Ok(())
    }

    fn update(&self, state: &State) {
        let Some(last_state) = state.last_block() else {
            return;
        };

        let mut total_consensus_validators = 0;
        let mut total_jailed_validators = 0;
        let mut total_inactive_validators = 0;
        let mut total_below_threshold_validators = 0;
        let mut total_below_capacity_validators = 0;

        for validator in last_state.validators.iter() {
            match validator.state {
                EnumValidatorState::Consensus => total_consensus_validators += 1,
                EnumValidatorState::Jailed => total_jailed_validators += 1,
                EnumValidatorState::Inactive => total_inactive_validators += 1,
                EnumValidatorState::BelowThreshold => total_below_threshold_validators += 1,
                EnumValidatorState::BelowCapacity => total_below_capacity_validators += 1,
            }
        }

        self.consensus_validators
            .set(total_consensus_validators as f64);
        self.jailed_validators.set(total_jailed_validators as f64);
        self.inactive_validators
            .set(total_inactive_validators as f64);
        self.below_threshold_validators
            .set(total_below_threshold_validators as f64);
        self.below_capacity_validators
            .set(total_below_capacity_validators as f64);
    }
}

impl Default for ValidatorState {
    fn default() -> Self {
        let gauge = |name: &str, description: &str| {
            Gauge::with_opts(Opts::new(name, description))
                .unwrap_or_else(|_| panic!("unable to create {} metric", name))
        };

        Self {
            consensus_validators: gauge(
                "consensus_validators",
                "Number of validators that are in consensus state",
            ),
            jailed_validators: gauge(
                "jailed_validators",
                "Number of validators that are in jailed state",
            ),
            inactive_validators: gauge(
                "inactive_validators",
                "Number of validators that are in inactive state",
            ),
            below_threshold_validators: gauge(
                "below_threshold_validators",
                "Number of validators that are below the voting power threshold",
            ),
            below_capacity_validators: gauge(
                "below_capacity_validators",
                "Number of validators that are below the capacity threshold",
            ),
        }
    }
}
