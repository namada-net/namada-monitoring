/// ## Voting Power Metrics (one_third_threshold, two_third_threshold)
/// These metrics track the number of validators required to reach 1/3 and 2/3 of the total voting power. They provide insight into consensus formation and validator distribution in the Namada blockchain.
///
///  - one_third_threshold: The number of validators needed to reach 1/3 of the voting power.
///  - two_third_threshold: The number of validators needed to reach 2/3 of the voting power.
///  - total_voting_power: The total voting power of the network.
///
/// ### Example
/// ```
/// # HELP one_third_threshold Number of validators to reach 1/3 of the voting power
/// # TYPE one_third_threshold gauge
/// one_third_threshold 5
///
/// # HELP two_third_threshold Number of validators to reach 2/3 of the voting power
/// # TYPE two_third_threshold gauge
/// two_third_threshold 12
///
/// # HELP total_voting_power The total voting power of the network
/// # TYPE total_voting_power gauge
/// total_voting_power 20
///
/// ```
use crate::state::State;
use anyhow::Result;
use prometheus_exporter::prometheus::{Gauge, Opts, Registry};

use super::MetricTrait;

pub struct VotingPower {
    pub one_third_threshold: Gauge,
    pub two_third_threshold: Gauge,
    pub total_voting_power: Gauge,
}

impl MetricTrait for VotingPower {
    fn register(&self, registry: &Registry) -> Result<()> {
        registry.register(Box::new(self.one_third_threshold.clone()))?;
        registry.register(Box::new(self.two_third_threshold.clone()))?;
        registry.register(Box::new(self.total_voting_power.clone()))?;
        Ok(())
    }

    fn update(&self, state: &State) {
        if state.last_block().is_none() {
            return;
        }

        let one_third_vp = state
            .validators_with_voting_power(1.0 / 3.0)
            .unwrap_or_default();
        self.one_third_threshold.set(one_third_vp as f64);

        let two_third_vp = state
            .validators_with_voting_power(2.0 / 3.0)
            .unwrap_or_default();
        self.two_third_threshold.set(two_third_vp as f64);
        self.total_voting_power
            .set(state.total_voting_power() as f64);
    }
}

impl Default for VotingPower {
    fn default() -> Self {
        let gauge = |name: &str, description: &str| {
            Gauge::with_opts(Opts::new(name, description))
                .unwrap_or_else(|_| panic!("unable to create {} metric", name))
        };

        Self {
            one_third_threshold: gauge(
                "one_third_threshold",
                "The number of validators to reach 1/3 of the voting power",
            ),
            two_third_threshold: gauge(
                "two_third_threshold",
                "The number of validators to reach 2/3 of the voting power",
            ),
            total_voting_power: gauge(
                "total_voting_power",
                "The total voting power of the network",
            ),
        }
    }
}
