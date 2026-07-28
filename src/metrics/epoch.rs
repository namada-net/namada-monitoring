/// ## Epoch Counter (epoch)
/// This metric tracks the latest epoch recorded on a Namada blockchain, providing visibility into epoch progression and chain
/// activity over time. A steadily increasing counter indicates normal epoch progression.
///
/// ### Example
/// ```
/// # HELP epoch The latest epoch recorded  
/// # TYPE epoch counter  
/// epoch 256
/// ```
use crate::state::State;
use anyhow::Result;
use prometheus_exporter::prometheus::core::{AtomicU64, GenericCounter};
use prometheus_exporter::prometheus::Registry;

use super::MetricTrait;

pub struct Epoch {
    /// The latest epoch recorded
    epoch: GenericCounter<AtomicU64>,
}

impl MetricTrait for Epoch {
    fn register(&self, registry: &Registry) -> Result<()> {
        registry.register(Box::new(self.epoch.clone()))?;
        Ok(())
    }

    fn update(&self, state: &State) {
        let Some(last_state) = state.last_block() else {
            return;
        };

        // A counter cannot go backwards; saturate rather than underflow when
        // the monitor is restarted at an earlier height.
        let latest_epoch = self.epoch.get();
        self.epoch
            .inc_by(last_state.block.epoch.saturating_sub(latest_epoch));
    }
}

impl Default for Epoch {
    fn default() -> Self {
        Self {
            epoch: GenericCounter::<AtomicU64>::new("epoch", "the latest epoch recorded")
                .expect("unable to create counter epoch"),
        }
    }
}
