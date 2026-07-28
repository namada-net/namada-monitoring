/// ## Bonds and Unbonds per epoch metrics. (bonds, unbonds)
/// These metrics track the number of bonds and unbonds per epoch in the Namada blockchains. They help monitor staking activity, validator participation, and network security dynamics. These metrics are gauges, updated at the start of each epoch based on the blockchain state.
/// * bonds: Measures the total amount of tokens bonded in a given epoch.
/// * unbonds: Measures the total amount of tokens unbonded in a given epoch.
///
/// ### Example
/// ```
/// # HELP namada_bonds Total bonds per epoch
/// # TYPE namada_bonds gauge
/// namada_bonds{epoch="2160",chain_id="local.300e84e1e16080e34547d538"} 120000000000
/// # HELP namada_unbonds Total unbonds per epoch
/// # TYPE namada_unbonds gauge
/// namada_unbonds{epoch="2160",chain_id="local.300e84e1e16080e34547d538"} 0
/// ```
use crate::state::State;
use anyhow::Result;
use prometheus_exporter::prometheus::{Gauge, Opts, Registry};

use super::MetricTrait;

pub struct Bonds {
    /// Bonds
    bonds: Gauge,
    /// Unbonds
    unbonds: Gauge,
}

impl MetricTrait for Bonds {
    fn register(&self, registry: &Registry) -> Result<()> {
        registry.register(Box::new(self.bonds.clone()))?;
        registry.register(Box::new(self.unbonds.clone()))?;
        Ok(())
    }

    fn update(&self, state: &State) {
        let Some(last_state) = state.last_block() else {
            return;
        };

        self.bonds.set(last_state.bonds as f64);
        self.unbonds.set(last_state.unbonds as f64);
    }
}

impl Default for Bonds {
    fn default() -> Self {
        let bonds_opts = Opts::new("bonds", "Total active bonds at the pipeline epoch");
        let unbonds_opts = Opts::new("unbonds", "Total active unbonds at the pipeline epoch");
        Self {
            bonds: Gauge::with_opts(bonds_opts).expect("unable to create bonds"),
            unbonds: Gauge::with_opts(unbonds_opts).expect("unable to create unbond"),
        }
    }
}
