use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use anyhow::anyhow;
use namada_sdk::proof_of_stake::types::ValidatorState;

use crate::shared::{
    namada::{Block, Transfer, Validator},
    supply::Supply,
};

/// Number of recent blocks kept in memory.
///
/// Only the halt check looks further back than a single block, and it caps its
/// lookback at 50 blocks. Anything beyond that is pure memory overhead: every
/// retained block holds the full tendermint block plus its transactions.
const RETAINED_BLOCKS: usize = 128;

#[derive(Debug, Clone)]
pub struct BlockState {
    pub block: Block,
    pub bonds: u64,
    pub unbonds: u64,
    /// Shared between all the blocks of an epoch: the validator set only
    /// changes on epoch boundaries, so cloning it per block wastes memory.
    pub validators: Arc<Vec<Validator>>,
    pub supplies: Vec<Supply>,
    pub mint_limit: HashMap<String, u64>,
}

impl BlockState {
    pub fn new(
        block: Block,
        bonds: u64,
        unbonds: u64,
        validators: Arc<Vec<Validator>>,
        supplies: Vec<Supply>,
        mint_limit: HashMap<String, u64>,
    ) -> Self {
        Self {
            block,
            bonds,
            unbonds,
            validators,
            supplies,
            mint_limit,
        }
    }

    pub fn consensus_validators(&self) -> impl Iterator<Item = &Validator> {
        self.validators
            .iter()
            .filter(|validator| matches!(validator.state, ValidatorState::Consensus))
    }

    pub fn total_consensus_validators(&self) -> usize {
        self.consensus_validators().count()
    }

    /// Number of validators that actually signed the commit carried by this
    /// block, together with the size of the commit's validator set.
    ///
    /// The commit contains one entry per validator, including the ones that did
    /// not vote (`BlockIdFlagAbsent`) and the ones that voted nil, so the length
    /// of `signatures` is *not* the number of signers.
    pub fn commit_votes(&self) -> Option<(usize, usize)> {
        let commit = self.block.block.last_commit.as_ref()?;
        let signed = commit
            .signatures
            .iter()
            .filter(|signature| signature.is_commit())
            .count();
        Some((signed, commit.signatures.len()))
    }
}

#[derive(Debug, Clone)]
pub struct State {
    blocks: VecDeque<BlockState>,
    size: usize,
    /// Height of the chain tip as of the last poll. Lets a check tell "the
    /// chain stopped producing blocks" apart from "the monitor is behind".
    chain_head: Option<u64>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            blocks: VecDeque::with_capacity(RETAINED_BLOCKS),
            size: RETAINED_BLOCKS,
            chain_head: None,
        }
    }
}

impl State {
    pub fn add_block(&mut self, block_state: BlockState) {
        while self.blocks.len() >= self.size {
            self.blocks.pop_front();
        }
        self.blocks.push_back(block_state);
    }

    pub fn set_chain_head(&mut self, height: u64) {
        self.chain_head = Some(height);
    }

    pub fn chain_head(&self) -> Option<u64> {
        self.chain_head
    }

    /// How many blocks behind the chain tip the monitor is.
    pub fn lag(&self) -> Option<u64> {
        let head = self.chain_head?;
        let last = self.last_block()?.block.height;
        Some(head.saturating_sub(last))
    }

    /// Whether the monitor has caught up with the chain tip. Checks that reason
    /// about wall-clock time are only meaningful once this holds.
    pub fn is_caught_up(&self, max_lag_blocks: u64) -> bool {
        self.lag().is_some_and(|lag| lag <= max_lag_blocks)
    }

    pub fn total_blocks(&self) -> usize {
        self.blocks.len()
    }

    pub fn last_block(&self) -> Option<&BlockState> {
        self.blocks.back()
    }

    pub fn prev_block(&self) -> Option<&BlockState> {
        let len = self.blocks.len();
        if len < 2 {
            return None;
        }
        self.blocks.get(len - 2)
    }

    /// The `n` most recent blocks, oldest first.
    pub fn last_n_blocks(&self, n: usize) -> impl DoubleEndedIterator<Item = &BlockState> {
        let skip = self.blocks.len().saturating_sub(n);
        self.blocks.iter().skip(skip)
    }

    /// Number of validators needed to control `fraction` of the voting power.
    pub fn validators_with_voting_power(&self, fraction: f64) -> anyhow::Result<u64> {
        let Some(block) = self.last_block() else {
            return Err(anyhow!("No block in state"));
        };

        // Only the voting powers matter here, so sort those instead of cloning
        // the whole validator set.
        let mut voting_powers = block
            .validators
            .iter()
            .map(|validator| validator.voting_power)
            .collect::<Vec<_>>();
        voting_powers.sort_unstable_by(|a, b| b.cmp(a));

        let total_voting_power: u64 = voting_powers.iter().sum();
        let threshold_voting_power = (total_voting_power as f64 * fraction) as u64;
        let mut accumulated_voting_power = 0u64;

        for (index, voting_power) in voting_powers.iter().enumerate() {
            if accumulated_voting_power >= threshold_voting_power {
                return Ok(index as u64);
            }
            accumulated_voting_power += voting_power;
        }
        Err(anyhow!(
            "No validators can hold {} of the voting power",
            fraction
        ))
    }

    pub fn total_voting_power(&self) -> u64 {
        self.last_block()
            .map(|block| {
                block
                    .validators
                    .iter()
                    .map(|validator| validator.voting_power)
                    .sum()
            })
            .unwrap_or_default()
    }

    pub fn get_all_transfers(&self) -> Vec<Transfer> {
        self.last_block()
            .map(|state| state.block.get_all_transfers())
            .unwrap_or_default()
    }
}
