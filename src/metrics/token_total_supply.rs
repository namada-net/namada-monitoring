/// ## Total Supply of Native Token Metric. token_total_supply
/// This metric tracks the total supply of the native token on the Namada blockchain.
/// * token_total_supply: A monotonic counter that records the latest total supply of the native token.
///
/// ### Example
/// ```
/// # HELP token_total_supply The latest total supply of the native token recorded  
/// # TYPE token_total_supply counter  
/// token_total_supply 1000000000  
/// ```
use crate::state::State;
use anyhow::Result;
use prometheus_exporter::prometheus::{GaugeVec, Opts, Registry};

use super::MetricTrait;

pub struct TokenTotalSupply {
    /// The total supply per token
    pub token_total_supply: GaugeVec,
    /// The effective supply per token (equal to the total for non-native tokens)
    pub token_effective_supply: GaugeVec,
}

impl MetricTrait for TokenTotalSupply {
    fn register(&self, registry: &Registry) -> Result<()> {
        registry.register(Box::new(self.token_total_supply.clone()))?;
        registry.register(Box::new(self.token_effective_supply.clone()))?;
        Ok(())
    }

    fn update(&self, state: &State) {
        let Some(last_state) = state.last_block() else {
            return;
        };

        for supply in &last_state.supplies {
            self.token_total_supply
                .with_label_values(&[&supply.token])
                .set(supply.total as f64);
            self.token_effective_supply
                .with_label_values(&[&supply.token])
                .set(supply.effective as f64);
        }
    }
}

impl Default for TokenTotalSupply {
    fn default() -> Self {
        let token_total_supply_opts = Opts::new("token_total_supply", "The token total supply");
        let token_effective_supply_opts =
            Opts::new("token_effective_supply", "The token effective supply");
        Self {
            token_total_supply: GaugeVec::new(token_total_supply_opts, &["token"])
                .expect("unable to create token_total_supply metric"),
            token_effective_supply: GaugeVec::new(token_effective_supply_opts, &["token"])
                .expect("unable to create token_effective_supply metric"),
        }
    }
}
