/// ## Fees Metric. (fees)
/// This metric tracks the total transaction fees paid per token. It helps monitor the gas costs of transactions on
/// the network, providing insight into network congestion and transaction fee trends.
/// * The metric is a counter, meaning it only increases over time.
/// * Fees are labeled by the token used for gas payments.
///
/// ### Example
/// ```
/// # HELP namada_fees Total fees paid per token over time
/// # TYPE namada_fees counter
/// namada_fees{token="tnam1q9gr66cvu4hrzm0sd5kmlnjje82gs3xlfg3v6nu7",chain_id="housefire-alpaca.cc0d3e0c033be"} 0.5845009999999999
/// namada_fees{token="tnam1q9gr66cvu4hrzm0sd5kmlnjje82gs3xlfg3v6nu7",chain_id="housefire-alpaca.cc0d3e0c033be"} 0.154409
/// ```
use crate::state::State;
use prometheus_exporter::prometheus::{CounterVec, HistogramOpts, HistogramVec, Opts, Registry};

use super::MetricTrait;

pub struct Fees {
    /// fees counters
    fees: CounterVec,
    /// histogram of fees by tx
    fees_by_tx: HistogramVec,
}

impl MetricTrait for Fees {
    fn register(&self, registry: &Registry) -> anyhow::Result<()> {
        registry.register(Box::new(self.fees.clone()))?;
        registry.register(Box::new(self.fees_by_tx.clone()))?;
        Ok(())
    }

    fn update(&self, state: &State) {
        let Some(last_state) = state.last_block() else {
            return;
        };

        for tx in &last_state.block.transactions {
            let amount_per_gas = tx.fee.amount_per_gas_unit.parse::<f64>();
            let gas_limit = tx.fee.gas.parse::<f64>();

            let fee = match (amount_per_gas, gas_limit) {
                (Ok(amount_per_gas), Ok(gas_limit)) => amount_per_gas * gas_limit,
                _ => continue,
            };

            self.fees
                .with_label_values(&[&tx.fee.gas_token])
                .inc_by(fee);
            self.fees_by_tx
                .with_label_values(&[&tx.fee.gas_token])
                .observe(fee);
        }
    }
}

impl Default for Fees {
    fn default() -> Self {
        let fees_opts = Opts::new("fees", "Total fees paid per token");
        let fees = CounterVec::new(fees_opts, &["token"])
            .expect("unable to create gauge vector for transaction fees");

        let fees_by_tx_opts = HistogramOpts::new("fees_by_tx", "Total fees paid per transaction")
            .buckets(vec![
                0.01, 0.02, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0,
            ]);
        let fees_by_tx = HistogramVec::new(fees_by_tx_opts, &["token"])
            .expect("unable to create histogram vec for transaction fees by tx");

        Self { fees, fees_by_tx }
    }
}
