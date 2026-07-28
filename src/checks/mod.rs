mod block;
mod fees;
mod gas;
mod governance;
mod halt;
mod ibc;
mod ibc_limit;
mod lag;
mod pos;
mod slashes;
mod transfer_limit;
mod tx;
mod unknown_tx;
mod validator_state;

use std::fmt::Display;

use async_trait::async_trait;

pub use crate::config::AppConfig;
pub use crate::state::State;
use crate::{
    checks::{
        block::BlockCheck, fees::FeeCheck, gas::GasCheck, governance::GovernanceCheck,
        halt::HaltCheck, ibc::IbcCheck, ibc_limit::IbcLimitCheck, lag::LagCheck, pos::PoSCheck,
        slashes::SlashCheck, transfer_limit::TransferLimitCheck, tx::TxCheck,
        unknown_tx::UnknownTxCheck, validator_state::ValidatorStateCheck,
    },
    shared::alert::Alert,
};

#[async_trait]
pub trait CheckTrait: Send + Sync + Display {
    async fn check(&self, state: &State) -> Vec<Alert>;
    fn is_continous(&self) -> bool;
}

pub struct CheckManager {
    checks: Vec<Box<dyn CheckTrait>>,
}

impl CheckManager {
    pub fn new(config: &AppConfig) -> Self {
        let enabled = &config.get_config().checks;
        let mut checks: Vec<Box<dyn CheckTrait>> = Vec::new();

        // Each entry is built lazily so a disabled check does not pay for its
        // construction, which for some of them means an RPC client.
        let mut register = |is_enabled: bool, build: &dyn Fn() -> Box<dyn CheckTrait>| {
            if is_enabled {
                checks.push(build());
            }
        };

        register(enabled.fee, &|| Box::new(FeeCheck::new(config)));
        register(enabled.block, &|| Box::new(BlockCheck::new(config)));
        register(enabled.pos, &|| Box::new(PoSCheck::new(config)));
        register(enabled.tx, &|| Box::new(TxCheck::new(config)));
        register(enabled.halt, &|| Box::new(HaltCheck::new(config)));
        register(enabled.gas, &|| Box::new(GasCheck::new(config)));
        register(enabled.ibc, &|| Box::new(IbcCheck::new(config)));
        register(enabled.transfer_limit, &|| {
            Box::new(TransferLimitCheck::new(config))
        });
        register(enabled.slashes, &|| Box::new(SlashCheck::default()));
        register(enabled.ibc_limit, &|| Box::new(IbcLimitCheck::new(config)));
        register(enabled.lag, &|| Box::new(LagCheck::new(config)));
        register(enabled.validator_state, &|| {
            Box::new(ValidatorStateCheck::default())
        });
        register(enabled.governance, &|| Box::new(GovernanceCheck::default()));
        register(enabled.unknown_tx, &|| Box::new(UnknownTxCheck::default()));

        Self { checks }
    }

    pub async fn run_block_checks(&self, states: &State) -> Vec<Alert> {
        let mut results = Vec::new();
        for check in &self.checks {
            if check.is_continous() {
                continue;
            }
            results.extend(check.check(states).await);
        }
        results
    }

    pub async fn run_continous_checks(&self, states: &State) -> Vec<Alert> {
        let mut results = Vec::new();
        for check in &self.checks {
            if check.is_continous() {
                results.extend(check.check(states).await);
            }
        }
        results
    }

    pub fn get_checks(&self) -> &Vec<Box<dyn CheckTrait>> {
        &self.checks
    }
}

impl Display for CheckManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for check in &self.checks {
            write!(f, "{}", check)?;
        }
        write!(f, "CheckManager with {} checks", self.checks.len())
    }
}
