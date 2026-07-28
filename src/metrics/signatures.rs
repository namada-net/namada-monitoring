/// ## Signature Counter (signatures)
/// This metric tracks the number of validators that signed the block, providing visibility into the block
/// signing activity over time.
///
/// ### Example
/// ```text
/// # HELP block_signatures Number of validators that signed the block
/// # TYPE block_signatures gauge
/// block_signatures 98
/// # HELP block_expected_signatures Number of validators expected to sign
/// # TYPE block_expected_signatures gauge
/// block_expected_signatures 100
/// ```
use crate::state::State;
use anyhow::Result;
use prometheus_exporter::prometheus::{Gauge, Opts, Registry};

use super::MetricTrait;

pub struct Signatures {
    /// Validators that signed the commit carried by the latest block
    signatures: Gauge,
    /// Size of the commit's validator set, signers and non-signers alike
    expected_signatures: Gauge,
}

impl MetricTrait for Signatures {
    fn register(&self, registry: &Registry) -> Result<()> {
        registry.register(Box::new(self.signatures.clone()))?;
        registry.register(Box::new(self.expected_signatures.clone()))?;
        Ok(())
    }

    fn update(&self, state: &State) {
        // The commit holds one entry per validator, absent ones included, so
        // only the entries flagged as commits are real signatures.
        let Some((signed, expected)) = state.last_block().and_then(|block| block.commit_votes())
        else {
            return;
        };

        self.signatures.set(signed as f64);
        self.expected_signatures.set(expected as f64);
    }
}

impl Default for Signatures {
    fn default() -> Self {
        let signature_opts = Opts::new(
            "block_signatures",
            "Number of validators that signed the latest block's commit",
        );
        let expected_opts = Opts::new(
            "block_expected_signatures",
            "Number of validators expected to sign the latest block's commit",
        );
        Self {
            signatures: Gauge::with_opts(signature_opts)
                .expect("unable to create signatures metric"),
            expected_signatures: Gauge::with_opts(expected_opts)
                .expect("unable to create expected signatures metric"),
        }
    }
}
