mod block;
mod bonds;
mod epoch;
mod fees;
mod ibc;
mod signatures;
mod slashes;
mod token_total_supply;
mod transactions;
mod transfers;
mod validator;
mod voting_power;

use std::{collections::HashMap, net::SocketAddr};

use block::Block;
use bonds::Bonds;
use epoch::Epoch;
use fees::Fees;
use ibc::Ibc;
use signatures::Signatures;
use slashes::Slashes;
use token_total_supply::TokenTotalSupply;
use transactions::Transactions;
use transfers::Transfers;
use validator::ValidatorState;
use voting_power::VotingPower;

use crate::{config::AppConfig, state::State};
use anyhow::{Context, Result};
use prometheus_exporter::prometheus::Registry;

pub trait MetricTrait: Send + Sync {
    fn register(&self, registry: &Registry) -> Result<()>;
    fn update(&self, state: &State);
}

pub struct MetricsExporter {
    registry: Registry,
    metrics: Vec<Box<dyn MetricTrait>>,
    port: u64,
}

impl MetricsExporter {
    pub fn new(config: &AppConfig) -> Self {
        let registry = Registry::new_custom(
            Some("namada_monitoring".to_string()),
            Some(HashMap::from_iter([(
                "chain_id".to_string(),
                config.chain_id.clone(),
            )])),
        )
        .expect("Failed to create registry");

        let metrics = Self::default_metrics();

        for metric in &metrics {
            metric
                .register(&registry)
                .expect("Failed to register metric");
        }

        Self {
            port: config.prometheus_port,
            metrics,
            registry,
        }
    }

    pub fn default_metrics() -> Vec<Box<dyn MetricTrait>> {
        vec![
            Box::<Block>::default() as Box<dyn MetricTrait>,
            Box::<Bonds>::default() as Box<dyn MetricTrait>,
            Box::<Epoch>::default() as Box<dyn MetricTrait>,
            Box::<TokenTotalSupply>::default() as Box<dyn MetricTrait>,
            Box::<Transactions>::default() as Box<dyn MetricTrait>,
            Box::<Transfers>::default() as Box<dyn MetricTrait>,
            Box::<VotingPower>::default() as Box<dyn MetricTrait>,
            Box::<Fees>::default() as Box<dyn MetricTrait>,
            Box::<Signatures>::default() as Box<dyn MetricTrait>,
            Box::<Slashes>::default() as Box<dyn MetricTrait>,
            Box::<ValidatorState>::default() as Box<dyn MetricTrait>,
            Box::<Ibc>::default() as Box<dyn MetricTrait>,
        ]
    }

    pub fn start_exporter(&self) -> anyhow::Result<()> {
        let addr_raw = format!("0.0.0.0:{}", self.port);
        let addr: SocketAddr = addr_raw.parse().context("can not parse listen addr")?;

        let mut builder = prometheus_exporter::Builder::new(addr);
        builder.with_registry(self.registry.clone());
        builder.start().context("can not start exporter")?;

        Ok(())
    }

    pub fn update(&self, state: &State) {
        for metric in &self.metrics {
            metric.update(state);
        }
    }
}
