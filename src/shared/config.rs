use serde::Deserialize;

use crate::shared::alert::Severity;

use super::block_explorer::BlockExplorer;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub block_explorer: BlockExplorer,
    pub chain: Chain,
    pub pos: Pos,
    pub tx: Tx,
    /// The IBC channels to watch. Distinct from the `[ibc]` table below, which
    /// holds the tuning knobs of the IBC checks themselves.
    pub ibcs: Vec<Ibc>,
    #[serde(default)]
    pub ibc: IbcSettings,
    pub tokens: Vec<TokenConfig>,
    #[serde(default)]
    pub checks: ChecksConfig,
    pub slack: Option<SlackAlertConfig>,
    pub telegram: Option<TelegramAlertConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TokenConfig {
    pub alias: String,
    pub token: String,
    pub fee_threshold: f64,
    pub transfer_threshold: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Chain {
    pub block_time: f64,
    pub block_time_max_deviation: f64,
    /// Seconds without a new block before the chain is considered halted.
    pub halt_threshold: u64,
    /// Multiple of `block_time` the windowed average may reach before alerting.
    #[serde(default = "default_avg_block_time_factor")]
    pub avg_block_time_factor: f64,
    /// How many recent blocks the average block time is computed over.
    #[serde(default = "default_block_time_lookback")]
    pub block_time_lookback: usize,
    /// How far the monitor may fall behind the chain head, in blocks, before
    /// alerting. Also suppresses halt detection while catching up.
    #[serde(default = "default_max_lag_blocks")]
    pub max_lag_blocks: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Pos {
    pub mininimum_one_third_validators: u64,
    pub mininimum_two_third_validators: u64,
    /// Relative increase in bonds, between two blocks, that triggers an alert.
    pub bond_increase_threshold: f64,
    /// Relative increase in unbonds, between two blocks, that triggers an alert.
    pub unbond_increase_threshold: f64,
    /// Relative *drop* in the size of the consensus set that triggers an alert:
    /// 0.1 means "alert once a tenth of the consensus validators are gone".
    pub consensus_threshold: f64,
    pub threshold_missed_votes: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Tx {
    pub threshold_batch: u64,
    pub threshold_sections: u64,
    pub gas_limit_threshold: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Ibc {
    pub alias: String,
    pub channel: u64,
    pub connection: u64,
    pub client_id: u64,
}

/// Tuning knobs of the IBC checks.
#[derive(Debug, Clone, Deserialize)]
pub struct IbcSettings {
    /// How close to expiry, in seconds, an IBC client may get before alerting.
    #[serde(default = "default_client_expiration_threshold")]
    pub client_expiration_threshold: i64,
    /// Minimum delay, in seconds, between two IBC client queries. The check runs
    /// on every block but the condition moves on a scale of days.
    #[serde(default = "default_ibc_poll_interval")]
    pub poll_interval: i64,
    /// Fraction of a token's IBC mint limit that its supply may reach before
    /// alerting.
    #[serde(default = "default_supply_limit_threshold")]
    pub supply_limit_threshold: f64,
}

impl Default for IbcSettings {
    fn default() -> Self {
        Self {
            client_expiration_threshold: default_client_expiration_threshold(),
            poll_interval: default_ibc_poll_interval(),
            supply_limit_threshold: default_supply_limit_threshold(),
        }
    }
}

/// Which checks to run. Every check is enabled unless it is explicitly turned
/// off, so an existing configuration keeps its current behaviour.
#[derive(Debug, Clone, Deserialize)]
pub struct ChecksConfig {
    #[serde(default = "enabled")]
    pub block: bool,
    #[serde(default = "enabled")]
    pub fee: bool,
    #[serde(default = "enabled")]
    pub gas: bool,
    #[serde(default = "enabled")]
    pub governance: bool,
    #[serde(default = "enabled")]
    pub halt: bool,
    #[serde(default = "enabled")]
    pub ibc: bool,
    #[serde(default = "enabled")]
    pub ibc_limit: bool,
    #[serde(default = "enabled")]
    pub lag: bool,
    #[serde(default = "enabled")]
    pub pos: bool,
    #[serde(default = "enabled")]
    pub slashes: bool,
    #[serde(default = "enabled")]
    pub transfer_limit: bool,
    #[serde(default = "enabled")]
    pub tx: bool,
    #[serde(default = "enabled")]
    pub unknown_tx: bool,
    #[serde(default = "enabled")]
    pub validator_state: bool,
}

impl Default for ChecksConfig {
    fn default() -> Self {
        Self {
            block: true,
            fee: true,
            gas: true,
            governance: true,
            halt: true,
            ibc: true,
            ibc_limit: true,
            lag: true,
            pos: true,
            slashes: true,
            transfer_limit: true,
            tx: true,
            unknown_tx: true,
            validator_state: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlackAlertConfig {
    pub slack_webhook: String,
    pub channel: String,
    pub mentions: Vec<String>,
    pub minimum_severity: Option<Severity>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TelegramAlertConfig {
    pub telegram_token: String,
    pub telegram_chat_id: String,
    pub minimum_severity: Option<Severity>,
}

impl Config {
    pub fn tokens_thresholds(&self) -> Vec<(String, u64)> {
        self.tokens
            .iter()
            .map(|token| (token.token.clone(), token.transfer_threshold))
            .collect()
    }

    pub fn tokens_fees(&self) -> Vec<(String, f64)> {
        self.tokens
            .iter()
            .map(|token| (token.token.clone(), token.fee_threshold))
            .collect()
    }
}

fn enabled() -> bool {
    true
}

fn default_avg_block_time_factor() -> f64 {
    1.2
}

fn default_block_time_lookback() -> usize {
    50
}

fn default_max_lag_blocks() -> u64 {
    10
}

fn default_client_expiration_threshold() -> i64 {
    60 * 60 * 24 * 2
}

fn default_ibc_poll_interval() -> i64 {
    60 * 5
}

fn default_supply_limit_threshold() -> f64 {
    0.80
}
