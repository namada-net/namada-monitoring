use crate::shared::{
    checksums::Checksums,
    namada::{Address, Block, BlockResult, Epoch, Height, Validator},
    supply::Supply,
};
use anyhow::Context;
use futures::StreamExt;

use crate::shared::client::Client as OwnClient;
use namada_sdk::tendermint::block::Height as TenderHeight;
use namada_sdk::{
    address::Address as NamadaAddress,
    hash::Hash,
    proof_of_stake::types::ValidatorState,
    rpc,
    state::{Epoch as NamadaEpoch, Key},
};
use std::str::FromStr;
use tendermint_rpc::Client;

/// Upper bound on in-flight per-validator queries when refreshing the set.
const MAX_CONCURRENT_VALIDATOR_QUERIES: usize = 30;

pub struct Rpc {
    client: OwnClient,
}

impl Rpc {
    pub async fn new(url: &str) -> anyhow::Result<Self> {
        let client = OwnClient::new(url);

        Ok(Self { client })
    }

    pub async fn query_checksums_at_height(&self, height: u64) -> anyhow::Result<Checksums> {
        tracing::debug!("Getting checksums at height {}", height);
        let mut checksums = Checksums::default();
        for code_path in Checksums::code_paths() {
            let code = self
                .query_tx_code_hash(&code_path, height)
                .await?
                .with_context(|| {
                    format!(
                        "{} must be defined in namada storage at height {}",
                        code_path, height
                    )
                })?;
            checksums.add(code_path, code);
        }
        Ok(checksums)
    }

    pub async fn query_tx_code_hash(
        &self,
        tx_code_path: &str,
        height: Height,
    ) -> anyhow::Result<Option<String>> {
        let hash_key = Key::wasm_hash(tx_code_path);

        let res = rpc::query_storage_value_bytes(
            self.client.as_ref(),
            &hash_key,
            Some(height.into()),
            false,
        )
        .await;

        if let Some(tx_code_bytes) = res.context("Should be able to get tx code")?.0 {
            Ok(Hash::try_from(&tx_code_bytes[..])
                .ok()
                .map(|hash| hash.to_string()))
        } else {
            Ok(None)
        }
    }

    pub async fn query_epoch_at_height(
        &self,
        block_height: Height,
    ) -> anyhow::Result<Option<Epoch>> {
        let res = rpc::query_epoch_at_height(self.client.as_ref(), block_height.into()).await;

        res.map(|epoch| epoch.map(|epoch| epoch.0))
            .context("Should be able to get epoch")
    }

    pub async fn query_lastest_height(&self) -> anyhow::Result<u32> {
        let res = self.client.as_ref().latest_block().await;

        res.map(|response| response.block.header.height.value() as u32)
            .context("Should be able to query for block")
    }

    pub async fn query_block_at_height(
        &self,
        block_height: Height,
        checksums: &Checksums,
        epoch: Epoch,
    ) -> anyhow::Result<Block> {
        let block_height = TenderHeight::try_from(block_height)
            .with_context(|| format!("Invalid block height: {}", block_height))?;

        let events_res = self.client.as_ref().block_results(block_height).await;
        let events = events_res.map(BlockResult::from).context(format!(
            "Should be able to query for block events for height: {}",
            block_height
        ))?;

        let block = self.client.as_ref().block(block_height).await;
        block
            .map(|response| Block::from(response, events, checksums, epoch))
            .context(format!(
                "Should be able to query for block for height: {}",
                block_height
            ))
    }

    pub async fn query_validator_state(
        &self,
        validator: &NamadaAddress,
        epoch: Epoch,
    ) -> anyhow::Result<ValidatorState> {
        let res =
            rpc::get_validator_state(self.client.as_ref(), validator, Some(epoch.into())).await;
        let (validator_state, _epoch) = res.context("Should be able to query validator state")?;

        match validator_state {
            Some(state_info) => Ok(state_info),
            None => Err(anyhow::anyhow!("Validator state not found")),
        }
    }

    pub async fn query_stake(
        &self,
        validator: &NamadaAddress,
        epoch: Epoch,
    ) -> anyhow::Result<u64> {
        let res = rpc::get_validator_stake(self.client.as_ref(), epoch.into(), validator).await;
        let stake = res.context("Should be able to query validator stake")?;

        Ok(stake.raw_amount().as_u64())
    }

    pub async fn query_validators(&self, epoch: Epoch) -> anyhow::Result<Vec<Validator>> {
        let res = rpc::get_all_validators(self.client.as_ref(), NamadaEpoch(epoch)).await;

        let validators = res.context("Should be able to query the validator set")?;
        let results: Vec<Result<Validator, anyhow::Error>> =
            futures::stream::iter(validators.into_iter().map(|validator_address| async move {
                let voting_power = self.query_stake(&validator_address, epoch).await?;
                let state = self
                    .query_validator_state(&validator_address, epoch)
                    .await?;
                Ok::<_, anyhow::Error>(Validator {
                    address: validator_address.to_string(),
                    voting_power,
                    state,
                })
            }))
            .buffer_unordered(MAX_CONCURRENT_VALIDATOR_QUERIES)
            .collect()
            .await;

        // A partial validator set silently corrupts every voting-power check,
        // so fail the whole block instead and let the retry loop try again.
        results
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .context("Should be able to query every validator")
    }

    pub async fn query_native_token(&self) -> anyhow::Result<Address> {
        let res = rpc::query_native_token(self.client.as_ref()).await;

        res.context("Should be able to query native token")
            .map(|address| address.to_string())
    }

    pub async fn query_future_bonds_and_unbonds(&self, epoch: Epoch) -> anyhow::Result<(u64, u64)> {
        let pipeline_epoch = NamadaEpoch(epoch + 1);
        let res =
            rpc::enriched_bonds_and_unbonds(self.client.as_ref(), pipeline_epoch, &None, &None)
                .await;

        res.context("Should be able to query bonds and unbonds")
            .map(|summary| {
                (
                    summary
                        .bonds_total_active()
                        .map(|amount| amount.raw_amount().as_u64())
                        .unwrap_or(0),
                    summary
                        .unbonds_total_active()
                        .map(|amount| amount.raw_amount().as_u64())
                        .unwrap_or(0),
                )
            })
    }

    pub async fn query_native_token_supply(&self, token: &str) -> anyhow::Result<Supply> {
        let address = NamadaAddress::from_str(token)
            .context("Should be able to convert string to address")?;
        let total_supply_res = rpc::get_token_total_supply(self.client.as_ref(), &address).await;
        let effect_supply_res = rpc::get_effective_native_supply(self.client.as_ref()).await;

        let total_native_supply = total_supply_res
            .context("Should be able to query total supply native token")
            .map(|amount| amount.raw_amount().as_u64())?;
        let effective_native_supply = effect_supply_res
            .context("Should be able to query effective supply native token")
            .map(|amount| amount.raw_amount().as_u64())?;

        Ok(Supply {
            total: total_native_supply,
            effective: effective_native_supply,
            token: token.to_string(),
        })
    }

    pub async fn query_token_supply(&self, token: &str) -> anyhow::Result<Supply> {
        let address = NamadaAddress::from_str(token)
            .context("Should be able to convert string to address")?;
        let total_supply_res = rpc::get_token_total_supply(self.client.as_ref(), &address).await?;
        let total_supply = total_supply_res.raw_amount().as_u64();

        Ok(Supply {
            total: total_supply,
            effective: total_supply,
            token: token.to_string(),
        })
    }

    pub async fn query_token_ibc_limit(&self, token: &str) -> anyhow::Result<u64> {
        let token = NamadaAddress::from_str(token)
            .context("Should be able to convert string to address")?;
        let res = rpc::query_ibc_rate_limits(self.client.as_ref(), &token).await;

        res.context("Should be able to query token IBC limit")
            .map(|amount| amount.mint_limit.raw_amount().as_u64())
    }
}
