use std::fmt::Display;
use std::str::FromStr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use namada_sdk::ibc::context::client::{AnyClientState, AnyConsensusState};
use namada_sdk::ibc::core::host::types::path::ClientConsensusStatePath;
use namada_sdk::ibc::core::host::types::{identifiers::ClientId, path::ClientStatePath};
use namada_sdk::ibc::primitives::proto::Any;
use namada_sdk::ibc::storage;
use namada_sdk::rpc;
use prost::Message;
use tendermint_rpc::HttpClient;

use crate::shared::alert::Metadata;

use super::{AppConfig, CheckTrait};

const IBC_CHECK_ID: &str = "ibc_check";

struct IbcChannel {
    pub channel_id: u64,
    pub connection_id: u64,
    pub client_id: u64,
    pub alias: String,
}

pub struct IbcCheck {
    channels: Vec<IbcChannel>,
    client: HttpClient,
    /// How close to expiry a client may get before alerting.
    expiration_threshold: i64,
    /// Minimum delay between two runs. This is a continuous check, so without a
    /// throttle it would issue two storage queries per channel on every block
    /// for a condition that moves on a scale of days.
    poll_interval: i64,
    last_run: AtomicI64,
}

#[async_trait::async_trait]
impl CheckTrait for IbcCheck {
    async fn check(&self, _state: &crate::state::State) -> Vec<crate::shared::alert::Alert> {
        let now = chrono::Utc::now().timestamp();
        let last_run = self.last_run.load(Ordering::Relaxed);
        if now - last_run < self.poll_interval {
            return vec![];
        }
        self.last_run.store(now, Ordering::Relaxed);

        let mut alerts = vec![];

        for channel in self.channels.iter() {
            let client_id = format!("07-tendermint-{}", channel.client_id);
            let Ok(parsed_client_id) = ClientId::from_str(&client_id) else {
                tracing::error!("Invalid IBC client id: {}", client_id);
                continue;
            };

            let client_state = match self
                .query_client_state(&channel.alias, ClientStatePath(parsed_client_id.clone()))
                .await
            {
                Some(any) => match AnyClientState::try_from(any) {
                    Ok(AnyClientState::Tendermint(client_state)) => client_state.inner().clone(),
                    Err(e) => {
                        tracing::error!("Malformed IBC client state for {}: {}", channel.alias, e);
                        continue;
                    }
                },
                None => continue,
            };

            let consensus_state_path = ClientConsensusStatePath {
                client_id: parsed_client_id,
                revision_number: client_state.latest_height.revision_number(),
                revision_height: client_state.latest_height.revision_height(),
            };

            let consensus_state = match self
                .query_client_state(&channel.alias, consensus_state_path)
                .await
            {
                Some(any) => match AnyConsensusState::try_from(any) {
                    Ok(AnyConsensusState::Tendermint(consensus_state)) => {
                        consensus_state.inner().clone()
                    }
                    Err(e) => {
                        tracing::error!(
                            "Malformed IBC consensus state for {}: {}",
                            channel.alias,
                            e
                        );
                        continue;
                    }
                },
                None => continue,
            };

            let expiration_time = consensus_state.timestamp.unix_timestamp()
                + client_state.trusting_period.as_secs() as i64;

            if now + self.expiration_threshold > expiration_time {
                let hours_left = (expiration_time - now) as f64 / 3600.0;
                alerts.push(crate::shared::alert::Alert {
                    check_id: format!("{}-{}", IBC_CHECK_ID, channel.alias),
                    title: "IBC Client Expiration".to_string(),
                    description: format!(
                        "The IBC channel {} for *{}* (connection {}, client {}), is near expiration: *{:.1}* hours left.",
                        channel.channel_id,
                        channel.alias,
                        channel.connection_id,
                        channel.client_id,
                        hours_left
                    ),
                    severity: crate::shared::alert::Severity::High,
                    metadata: Metadata::new(None, None),
                    trigger_after: Some(Duration::from_secs(60 * 60)),
                    continous: self.is_continous(),
                });
            };
        }

        alerts
    }

    fn is_continous(&self) -> bool {
        true
    }
}

impl Display for IbcCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IbcCheck with {} channels", self.channels.len())
    }
}

impl IbcCheck {
    pub fn new(config: &AppConfig) -> Self {
        let settings = config.get_config().ibc.clone();
        let channels = config
            .get_config()
            .ibcs
            .iter()
            .map(|ibc| IbcChannel {
                channel_id: ibc.channel,
                connection_id: ibc.connection,
                alias: ibc.alias.clone(),
                client_id: ibc.client_id,
            })
            .collect();

        let client = HttpClient::new(config.rpc.as_str())
            .expect("Should be able to create the IBC RPC client");

        IbcCheck {
            channels,
            client,
            expiration_threshold: settings.client_expiration_threshold,
            poll_interval: settings.poll_interval,
            last_run: AtomicI64::new(0),
        }
    }

    /// Read a protobuf `Any` out of IBC storage, logging (rather than failing)
    /// when the key is missing or the node errors out.
    async fn query_client_state<P: ToString>(&self, alias: &str, path: P) -> Option<Any> {
        let key = match storage::ibc_key(path.to_string()) {
            Ok(key) => key,
            Err(e) => {
                tracing::error!("Unparsable IBC storage path for {}: {}", alias, e);
                return None;
            }
        };

        match rpc::query_storage_value_bytes(&self.client, &key, None, false).await {
            Ok((Some(value), _proof)) => match Any::decode(value.as_slice()) {
                Ok(any) => Some(any),
                Err(e) => {
                    tracing::error!("Undecodable IBC state for {}: {}", alias, e);
                    None
                }
            },
            Ok((None, _proof)) => {
                tracing::warn!("No IBC state found for {} at {}", alias, key);
                None
            }
            Err(e) => {
                tracing::error!("Failed to query IBC state for {}: {}", alias, e);
                None
            }
        }
    }
}
