/// ## Block Height (block_height)
/// This metric tracks the latest block height of a Namada blockchain. It provides a real-time view of
/// block progression, and helps monitor chain liveness and ensure continuous block production.
/// It is updated at each block by fetching the latest block height from the blockchain state.
///
/// ### Example
/// ```
/// # HELP namada_block_height the latest block height recorded
/// # TYPE namada_block_height counter
/// namada_block_height 12960
/// ```
///
/// ## Alert: Block Height Stalled:
/// If no blocks are registered in 5 minutes, the block height has stalled. Alert the team to investigate the issue.
/// ```
/// (time() - min_over_time(time()[5m])) > 300 and (absent_over_time(namada_block_height[5m]) or increase(namada_block_height[5m]) == 0)
/// ```
use crate::state::State;
use anyhow::Result;
use prometheus_exporter::prometheus::core::{AtomicU64, GenericCounter};
use prometheus_exporter::prometheus::{Histogram, HistogramOpts, Registry};

use super::MetricTrait;

pub struct Block {
    block_height: GenericCounter<AtomicU64>,
    block_time: Histogram,
}

impl MetricTrait for Block {
    fn register(&self, registry: &Registry) -> Result<()> {
        registry.register(Box::new(self.block_height.clone()))?;
        registry.register(Box::new(self.block_time.clone()))?;
        Ok(())
    }

    fn update(&self, state: &State) {
        let (Some(last_state), Some(prev_state)) = (state.last_block(), state.prev_block()) else {
            return;
        };

        // A counter cannot go backwards; saturate rather than underflow when
        // the monitor is restarted at an earlier height.
        let block_height = last_state.block.height;
        let latest_block = self.block_height.get();
        self.block_height
            .inc_by(block_height.saturating_sub(latest_block));

        let process_time = last_state.block.timestamp - prev_state.block.timestamp;
        self.block_time.observe(process_time as f64);
    }
}

impl Default for Block {
    fn default() -> Self {
        let buckets = vec![5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0];
        let block_time_opts =
            HistogramOpts::new("block_time", "The time spent processing block").buckets(buckets);
        let block_time = Histogram::with_opts(block_time_opts)
            .expect("unable to create histogram blocks used time");

        Self {
            block_height: GenericCounter::<AtomicU64>::new(
                "block_height",
                "the latest block height recorded",
            )
            .expect("unable to create counter block_height"),
            block_time,
        }
    }
}
