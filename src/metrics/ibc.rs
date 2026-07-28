use prometheus_exporter::prometheus::GaugeVec;

use super::MetricTrait;

pub struct Ibc {
    limit: GaugeVec,
}

impl MetricTrait for Ibc {
    fn register(&self, registry: &prometheus_exporter::prometheus::Registry) -> anyhow::Result<()> {
        registry.register(Box::new(self.limit.clone()))?;
        Ok(())
    }

    fn update(&self, state: &crate::state::State) {
        let Some(last_state) = state.last_block() else {
            return;
        };

        for (token, limit) in &last_state.mint_limit {
            self.limit.with_label_values(&[token]).set(*limit as f64);
        }
    }
}

impl Default for Ibc {
    fn default() -> Self {
        let limit_opts = prometheus_exporter::prometheus::Opts::new(
            "ibc_token_limit",
            "IBC token minting limit",
        );
        Self {
            limit: GaugeVec::new(limit_opts, &["token"])
                .expect("unable to create ibc token limit metric"),
        }
    }
}
