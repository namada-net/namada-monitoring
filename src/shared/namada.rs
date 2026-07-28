use namada_sdk::borsh::BorshDeserialize;
use namada_sdk::governance::{InitProposalData, VoteProposalData};
use namada_sdk::ibc::{self, IbcMessage};
use namada_sdk::key::common::PublicKey;
use namada_sdk::proof_of_stake::types::ValidatorState;
use namada_sdk::tendermint::block::Block as TendermintBlock;
use namada_sdk::uint::Uint;
use std::collections::BTreeMap;
use std::fmt::Display;

use namada_sdk::token::Transfer as NamadaTransfer;
use namada_sdk::tx::action::{Bond, ClaimRewards, Redelegation, Unbond, Withdraw};
use namada_sdk::tx::data::pos::{BecomeValidator, CommissionChange, MetaDataChange};
use namada_sdk::tx::data::{TxResult, TxType};
use namada_sdk::tx::{data::compute_inner_tx_hash, either::Either, Tx};
use std::str::FromStr;
use tendermint_rpc::endpoint::block::Response;
use tendermint_rpc::endpoint::block_results::Response as TendermintBlockResultResponse;

use super::checksums::Checksums;

pub type Height = u64;
pub type Epoch = u64;
pub type TxId = String;
pub type Address = String;

/// Internal address of the multi-asset shielded pool.
pub const MASP_ADDRESS: namada_sdk::address::Address =
    namada_sdk::address::Address::Internal(namada_sdk::address::InternalAddress::Masp);

#[derive(Clone, Debug)]
pub struct Validator {
    pub address: String,
    pub voting_power: u64,
    pub state: ValidatorState,
}

#[derive(Clone, Debug)]
pub struct Block {
    pub height: Height,
    pub epoch: Epoch,
    pub timestamp: i64,
    pub transactions: Vec<Wrapper>,
    pub block: TendermintBlock,
}

#[derive(Clone, Debug)]
pub struct Wrapper {
    pub id: TxId,
    pub inners: Vec<Inner>,
    pub fee: Fee,
    pub atomic: bool,
    pub total_sections: u64,
    pub status: TransactionExitStatus,
}

impl Wrapper {
    pub fn compute_fee(&self) -> f64 {
        let gas_limit = self.fee.gas.parse::<f64>().unwrap_or_default();
        let amount_per_gas = self
            .fee
            .amount_per_gas_unit
            .parse::<f64>()
            .unwrap_or_default();
        gas_limit * amount_per_gas
    }

    pub fn is_successful(&self) -> bool {
        self.status.was_applied()
    }
}

#[derive(Debug, Clone)]
pub struct Fee {
    pub gas: String,
    pub gas_used: u64,
    pub amount_per_gas_unit: String,
    pub gas_payer: String,
    pub gas_token: String,
}

#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum InnerKind {
    Transfer(NamadaTransfer),
    IbcMsgTransfer(IbcMessage<NamadaTransfer>),
    Bond(Bond),
    Redelegation(Redelegation),
    Unbond(Unbond),
    Withdraw(Withdraw),
    ClaimRewards(ClaimRewards),
    ProposalVote(VoteProposalData),
    InitProposal(InitProposalData),
    MetadataChange(MetaDataChange),
    CommissionChange(CommissionChange),
    RevealPk(PublicKey),
    BecomeValidator(BecomeValidator),
    DeactivateValidator(Address),
    ReactivateValidator(Address),
    UnjailValidator(Address),
    Unknown(String, Vec<u8>),
}

impl Display for InnerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InnerKind::Transfer(_) => write!(f, "transfer"),
            InnerKind::IbcMsgTransfer(_) => write!(f, "ibc_transfer"),
            InnerKind::Bond(_) => write!(f, "bond"),
            InnerKind::Redelegation(_) => write!(f, "redelegate"),
            InnerKind::Unbond(_) => write!(f, "unbond"),
            InnerKind::Withdraw(_) => write!(f, "withdraw"),
            InnerKind::ClaimRewards(_) => write!(f, "claim_rewards"),
            InnerKind::ProposalVote(_) => write!(f, "vote_proposal"),
            InnerKind::InitProposal(_) => write!(f, "init_proposal"),
            InnerKind::MetadataChange(_) => write!(f, "metadata_change"),
            InnerKind::CommissionChange(_) => write!(f, "commission_change"),
            InnerKind::RevealPk(_) => write!(f, "reveal_public_key"),
            InnerKind::BecomeValidator(_) => write!(f, "become_validator"),
            InnerKind::DeactivateValidator(_) => write!(f, "deactivate_validator"),
            InnerKind::ReactivateValidator(_) => write!(f, "reactivate_validator"),
            InnerKind::UnjailValidator(_) => write!(f, "unjail_validator"),
            InnerKind::Unknown(code_name, ..) => write!(f, "unknown({})", code_name),
        }
    }
}

impl InnerKind {
    pub fn from(tx_code_name: &str, data: &[u8]) -> Self {
        let default = |_| InnerKind::Unknown(tx_code_name.into(), data.to_vec());
        match tx_code_name {
            "tx_transfer" => {
                NamadaTransfer::try_from_slice(data).map_or_else(default, InnerKind::Transfer)
            }
            "tx_bond" => Bond::try_from_slice(data).map_or_else(default, InnerKind::Bond),
            "tx_redelegate" => Redelegation::try_from_slice(data)
                .map_or_else(default, |redelegation| {
                    InnerKind::Redelegation(redelegation)
                }),
            "tx_unbond" => Unbond::try_from_slice(data)
                .map_or_else(default, |unbond| InnerKind::Unbond(Unbond::from(unbond))),
            "tx_withdraw" => {
                Withdraw::try_from_slice(data).map_or_else(default, InnerKind::Withdraw)
            }
            "tx_claim_rewards" => ClaimRewards::try_from_slice(data)
                .map_or_else(default, |claim_rewards| {
                    InnerKind::ClaimRewards(claim_rewards)
                }),
            "tx_init_proposal" => InitProposalData::try_from_slice(data)
                .map_or_else(default, |init_proposal| {
                    InnerKind::InitProposal(init_proposal)
                }),
            "tx_vote_proposal" => VoteProposalData::try_from_slice(data)
                .map_or_else(default, |vote_proposal| {
                    InnerKind::ProposalVote(vote_proposal)
                }),
            "tx_change_validator_metadata" => MetaDataChange::try_from_slice(data)
                .map_or_else(default, |metadata_change| {
                    InnerKind::MetadataChange(metadata_change)
                }),
            "tx_commission_change" => CommissionChange::try_from_slice(data)
                .map_or_else(default, |commission_change| {
                    InnerKind::CommissionChange(commission_change)
                }),
            "tx_reveal_pk" => {
                PublicKey::try_from_slice(data).map_or_else(default, InnerKind::RevealPk)
            }
            "tx_deactivate_validator" => {
                Address::try_from_slice(data).map_or_else(default, InnerKind::DeactivateValidator)
            }
            "tx_reactivate_validator" => {
                Address::try_from_slice(data).map_or_else(default, InnerKind::ReactivateValidator)
            }
            "tx_unjail_validator" => {
                Address::try_from_slice(data).map_or_else(default, InnerKind::UnjailValidator)
            }
            "tx_become_validator" => BecomeValidator::try_from_slice(data)
                .map_or_else(default, |become_validator| {
                    InnerKind::BecomeValidator(become_validator)
                }),

            "tx_ibc" => {
                if let Ok(ibc_data) = ibc::decode_message::<NamadaTransfer>(data) {
                    InnerKind::IbcMsgTransfer(ibc_data)
                } else {
                    InnerKind::Unknown(tx_code_name.into(), data.to_vec())
                }
            }
            _ => {
                tracing::warn!("Unknown transaction kind: {}", tx_code_name);
                InnerKind::Unknown(tx_code_name.into(), data.to_vec())
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Inner {
    pub id: TxId,
    pub size: usize,
    pub kind: InnerKind,

    pub was_applied: bool,
}

#[derive(Debug, Clone, Default)]
pub struct BlockResult {
    pub height: u64,
    pub begin_events: Vec<Event>,
    pub end_events: Vec<Event>,
}

#[derive(Debug, Clone)]
pub struct Event {
    pub kind: EventKind,
    pub attributes: Option<TxAttributesType>,
}

#[derive(Debug, Clone)]
pub enum EventKind {
    Applied,
    Unknown,
}

impl From<&String> for EventKind {
    fn from(value: &String) -> Self {
        match value.as_str() {
            "tx/applied" => Self::Applied,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TxAttributesType {
    TxApplied(TxApplied),
}

#[derive(Debug, Clone, Default)]
pub struct TxApplied {
    pub code: TxEventStatusCode,
    pub gas: u64,
    pub hash: String,
    pub height: u64,
    pub batch: BatchResults,
    pub info: String,
}

#[derive(Debug, Clone, Default, Copy)]
pub enum TxEventStatusCode {
    Ok,
    #[default]
    Fail,
}

impl From<&str> for TxEventStatusCode {
    fn from(value: &str) -> Self {
        match value {
            "0" => Self::Ok,
            _ => Self::Fail,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BatchResults {
    pub batch_errors: BTreeMap<String, BTreeMap<String, String>>,
    pub batch_results: BTreeMap<String, bool>,
}

impl From<TxResult<String>> for BatchResults {
    fn from(value: TxResult<String>) -> Self {
        Self {
            batch_results: value
                .iter()
                .fold(BTreeMap::default(), |mut acc, (tx_hash, result)| {
                    let tx_id = tx_hash.to_string();
                    let result = if let Ok(result) = result {
                        result.is_accepted()
                    } else {
                        false
                    };
                    acc.insert(tx_id, result);
                    acc
                }),
            batch_errors: value
                .iter()
                .fold(BTreeMap::default(), |mut acc, (tx_hash, result)| {
                    let tx_id = tx_hash.to_string().to_uppercase();
                    let result = if let Ok(result) = result {
                        result
                            .vps_result
                            .errors
                            .iter()
                            .map(|(address, error)| (address.to_string(), error.clone()))
                            .collect()
                    } else {
                        BTreeMap::default()
                    };
                    acc.insert(tx_id, result);
                    acc
                }),
        }
    }
}

impl BatchResults {
    pub fn is_successful(&self, tx_id: &str) -> bool {
        match self.batch_results.get(tx_id) {
            Some(result) => *result,
            None => false,
        }
    }
}

impl Block {
    pub fn from(
        response: Response,
        block_results: BlockResult,
        checksums: &Checksums,
        epoch: Epoch,
    ) -> Self {
        let block = response.block.clone();
        Self {
            block,
            height: response.block.header.height.value(),
            epoch,
            timestamp: response.block.header.time.unix_timestamp(),
            transactions: response
                .block
                .data
                .into_iter()
                .filter_map(|bytes| {
                    let tx = Tx::try_from(bytes.as_ref()).ok()?;
                    let wrapper_id = tx.header_hash();

                    let wrapper_tx_id = wrapper_id.to_string();
                    let wrapper_tx_status = block_results.is_wrapper_tx_applied(&wrapper_tx_id);
                    let gas_used = block_results
                        .gas_used(&wrapper_tx_id)
                        .map(|gas| gas.parse::<u64>().unwrap_or_default())
                        .unwrap_or_default();
                    let atomic = tx.header().atomic;
                    let total_sections = tx.sections.len() as u64;

                    let fee = if let TxType::Wrapper(wrapper) = tx.header().tx_type {
                        Fee {
                            gas: Uint::from(wrapper.gas_limit).to_string(),
                            gas_used,
                            amount_per_gas_unit: wrapper
                                .fee
                                .amount_per_gas_unit
                                .to_string_precise(),
                            gas_payer: wrapper.fee_payer().to_string(),
                            gas_token: wrapper.fee.token.to_string(),
                        }
                    } else {
                        return None;
                    };

                    let inners = tx
                        .header()
                        .batch
                        .into_iter()
                        .map(|tx_commitment| {
                            let tx_id = compute_inner_tx_hash(
                                Some(&wrapper_id),
                                Either::Right(&tx_commitment),
                            )
                            .to_string();

                            let tx_code_id = tx
                                .get_section(tx_commitment.code_sechash())
                                .and_then(|s| s.code_sec())
                                .map(|s| s.code.hash().0)
                                .map(|bytes| {
                                    String::from_utf8(subtle_encoding::hex::encode(bytes)).unwrap()
                                });

                            let tx_data = tx.data(&tx_commitment).unwrap_or_default();
                            let tx_size = tx_data.len();

                            let tx_code_name = match tx_code_id {
                                Some(id) => checksums
                                    .get_name_by_id(&id)
                                    .unwrap_or_else(|| format!("no_tx_code_name_with_id_{}", id)),
                                None => "no_tx_id".into(),
                            };

                            let kind = InnerKind::from(&tx_code_name, &tx_data);
                            let inner_tx_id = compute_inner_tx_hash(
                                Some(&wrapper_id),
                                Either::Right(&tx_commitment),
                            )
                            .to_string();
                            let inner_tx_status =
                                block_results.is_inner_tx_accepted(&wrapper_tx_id, &inner_tx_id);

                            Inner {
                                id: tx_id,
                                size: tx_size,
                                kind,
                                was_applied: inner_tx_status.was_applied(),
                            }
                        })
                        .collect();

                    Some(Wrapper {
                        id: wrapper_id.to_string(),
                        inners,
                        fee,
                        atomic,
                        total_sections,
                        status: wrapper_tx_status,
                    })
                })
                .collect(),
        }
    }

    /// Every token movement carried by the applied transactions of this block,
    /// aggregated per (inner transaction, token).
    pub fn get_all_transfers(&self) -> Vec<Transfer> {
        let mut transfers = Vec::new();

        let applied_inners = self
            .transactions
            .iter()
            .filter(|tx| tx.status.was_applied())
            .flat_map(|tx| tx.inners.iter())
            .filter(|inner| inner.was_applied);

        for inner in applied_inners {
            let (transfer, kind) = match &inner.kind {
                InnerKind::Transfer(transfer) => (
                    transfer,
                    masp_transfer_kind(transfer).unwrap_or(TransferKind::Native),
                ),
                InnerKind::IbcMsgTransfer(IbcMessage::Transfer(msg_transfer)) => {
                    match &msg_transfer.transfer {
                        Some(transfer) => (transfer, TransferKind::Ibc),
                        None => continue,
                    }
                }
                _ => continue,
            };

            let mut totals: BTreeMap<String, u64> = BTreeMap::new();
            for (account, amount) in &transfer.targets {
                *totals.entry(account.token.to_string()).or_default() +=
                    amount.amount().raw_amount().as_u64();
            }

            for (token, amount) in totals {
                transfers.push(Transfer {
                    height: self.height,
                    id: inner.id.clone(),
                    kind: kind.clone(),
                    token,
                    amount,
                    accepted: inner.was_applied,
                });
            }
        }

        transfers
    }
}

/// Classify a transfer with respect to the MASP: shielding, unshielding, fully
/// shielded, or a mix of transparent and shielded parts. Returns `None` for a
/// plain transparent transfer.
pub fn masp_transfer_kind(data: &NamadaTransfer) -> Option<TransferKind> {
    let has_shielded_section = data.shielded_section_hash.is_some();
    if has_shielded_section && data.sources.is_empty() && data.targets.is_empty() {
        return Some(TransferKind::Shielded);
    }

    let (all_sources_are_masp, any_sources_are_masp) =
        data.sources
            .iter()
            .fold((true, false), |(all, any), (acc, _)| {
                let is_masp = acc.owner.eq(&MASP_ADDRESS);
                (all && is_masp, any || is_masp)
            });

    let (all_targets_are_masp, any_targets_are_masp) =
        data.targets
            .iter()
            .fold((true, false), |(all, any), (acc, _)| {
                let is_masp = acc.owner.eq(&MASP_ADDRESS);
                (all && is_masp, any || is_masp)
            });

    match (
        all_sources_are_masp,
        any_sources_are_masp,
        all_targets_are_masp,
        any_targets_are_masp,
        has_shielded_section,
    ) {
        (true, _, true, _, true) => Some(TransferKind::Shielded),
        (true, _, _, false, true) => Some(TransferKind::Unshielding),
        (_, false, true, _, true) => Some(TransferKind::Shielding),
        (_, false, _, false, false) => None,
        _ => Some(TransferKind::Mixed),
    }
}

#[derive(Clone, Debug)]
pub enum TransferKind {
    Ibc,
    Native,
    Shielding,
    Unshielding,
    Shielded,
    Mixed,
}

impl Display for TransferKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransferKind::Ibc => write!(f, "ibc"),
            TransferKind::Native => write!(f, "native"),
            TransferKind::Shielding => write!(f, "shielding"),
            TransferKind::Unshielding => write!(f, "unshielding"),
            TransferKind::Shielded => write!(f, "shielded"),
            TransferKind::Mixed => write!(f, "mixed"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Transfer {
    pub height: Height,
    pub id: String,
    pub kind: TransferKind,
    pub token: String,
    pub amount: u64,
    pub accepted: bool,
}

impl BlockResult {
    pub fn is_wrapper_tx_applied(&self, tx_hash: &str) -> TransactionExitStatus {
        let tx_hash = tx_hash.to_uppercase();
        let exit_status = self
            .end_events
            .iter()
            .filter_map(|event| {
                if let Some(TxAttributesType::TxApplied(data)) = &event.attributes {
                    Some(data)
                } else {
                    None
                }
            })
            .find(|attributes| attributes.hash.eq(&tx_hash))
            .map(|attributes| attributes.clone().code)
            .map(TransactionExitStatus::from);
        exit_status.unwrap_or(TransactionExitStatus::Rejected)
    }

    pub fn gas_used(&self, tx_hash: &str) -> Option<String> {
        self.end_events
            .iter()
            .filter_map(|event| {
                if let Some(TxAttributesType::TxApplied(data)) = &event.attributes {
                    Some(data.clone())
                } else {
                    None
                }
            })
            .find(|attributes| attributes.hash.eq(tx_hash))
            .map(|attributes| attributes.gas.to_string())
    }

    pub fn is_inner_tx_accepted(
        &self,
        wrapper_hash: &str,
        inner_hash: &str,
    ) -> TransactionExitStatus {
        let wrapper_hash = wrapper_hash.to_uppercase();
        let exit_status = self
            .end_events
            .iter()
            .filter_map(|event| {
                if let Some(TxAttributesType::TxApplied(data)) = &event.attributes {
                    Some(data.clone())
                } else {
                    None
                }
            })
            .find(|attributes| attributes.hash.eq(&wrapper_hash))
            .map(|attributes| attributes.batch.is_successful(inner_hash))
            .map(|successful| match successful {
                true => TransactionExitStatus::Applied,
                false => TransactionExitStatus::Rejected,
            });
        exit_status.unwrap_or(TransactionExitStatus::Rejected)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionExitStatus {
    Applied,
    Rejected,
}

impl TransactionExitStatus {
    pub fn was_applied(&self) -> bool {
        matches!(self, Self::Applied)
    }
}

impl Display for TransactionExitStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Applied => write!(f, "Applied"),
            Self::Rejected => write!(f, "Rejected"),
        }
    }
}

impl From<TxEventStatusCode> for TransactionExitStatus {
    fn from(value: TxEventStatusCode) -> Self {
        match value {
            TxEventStatusCode::Ok => Self::Applied,
            TxEventStatusCode::Fail => Self::Rejected,
        }
    }
}

impl From<&TendermintBlockResultResponse> for BlockResult {
    fn from(value: &TendermintBlockResultResponse) -> Self {
        BlockResult::from(value.clone())
    }
}

impl From<TendermintBlockResultResponse> for BlockResult {
    fn from(value: TendermintBlockResultResponse) -> Self {
        let begin_events = Self::parse_events(value.begin_block_events.unwrap_or_default());
        let end_events = Self::parse_events(value.end_block_events.unwrap_or_default());
        Self {
            height: value.height.value(),
            begin_events,
            end_events,
        }
    }
}

impl BlockResult {
    /// Turn raw tendermint events into the subset this monitor understands.
    /// Attributes that are not valid utf-8 are dropped rather than fatal: a
    /// single odd event should never take the monitor down.
    fn parse_events(events: Vec<namada_sdk::tendermint::abci::Event>) -> Vec<Event> {
        events
            .iter()
            .map(|event| {
                let kind = EventKind::from(&event.kind);
                let raw_attributes =
                    event
                        .attributes
                        .iter()
                        .fold(BTreeMap::default(), |mut acc, attribute| {
                            match (attribute.key_str(), attribute.value_str()) {
                                (Ok(key), Ok(value)) => {
                                    acc.insert(String::from(key), String::from(value));
                                }
                                _ => tracing::warn!(
                                    "Skipping non utf-8 attribute of a {} event",
                                    event.kind
                                ),
                            }
                            acc
                        });
                let attributes = TxAttributesType::deserialize(&kind, &raw_attributes);
                Event { kind, attributes }
            })
            .collect()
    }
}

impl TxAttributesType {
    pub fn deserialize(
        event_kind: &EventKind,
        attributes: &BTreeMap<String, String>,
    ) -> Option<Self> {
        match event_kind {
            EventKind::Unknown => None,
            // A missing or malformed attribute means the monitor cannot tell
            // whether the transaction was applied. Report that as unknown
            // rather than bringing the process down over one event.
            EventKind::Applied => {
                let missing = |attribute: &str| {
                    tracing::warn!("Ignoring a tx/applied event without a usable {}", attribute);
                    None::<Self>
                };

                let Some(code) = attributes
                    .get("code")
                    .map(|code| TxEventStatusCode::from(code.as_str()))
                else {
                    return missing("code");
                };
                let Some(gas) = attributes
                    .get("gas_used")
                    .and_then(|gas| u64::from_str(gas).ok())
                else {
                    return missing("gas_used");
                };
                let Some(hash) = attributes.get("hash").map(|hash| hash.to_uppercase()) else {
                    return missing("hash");
                };
                let Some(height) = attributes
                    .get("height")
                    .and_then(|height| u64::from_str(height).ok())
                else {
                    return missing("height");
                };
                let Some(batch) = attributes
                    .get("batch")
                    .and_then(|batch| serde_json::from_str::<TxResult<String>>(batch).ok())
                    .map(BatchResults::from)
                else {
                    return missing("batch");
                };
                let Some(info) = attributes.get("info").cloned() else {
                    return missing("info");
                };

                Some(Self::TxApplied(TxApplied {
                    code,
                    gas,
                    hash,
                    height,
                    batch,
                    info,
                }))
            }
        }
    }
}
