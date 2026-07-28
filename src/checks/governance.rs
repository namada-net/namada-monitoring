use std::fmt::Display;

use crate::shared::{
    alert::{summarize, Alert, Metadata, Severity},
    namada::InnerKind,
};

use super::CheckTrait;

const PROPOSAL_CHECK_ID: &str = "governance_proposal_check";

/// Reports governance proposals as they are submitted.
///
/// A proposal that nobody notices until the voting period has closed is a
/// governance failure, and the voting window is carried in the transaction
/// itself, so no extra query is needed.
#[derive(Debug, Clone, Default)]
pub struct GovernanceCheck {}

#[async_trait::async_trait]
impl CheckTrait for GovernanceCheck {
    async fn check(&self, state: &crate::state::State) -> Vec<Alert> {
        let Some(last_state) = state.last_block() else {
            return Vec::new();
        };

        let proposals = last_state
            .block
            .transactions
            .iter()
            .filter(|wrapper| wrapper.is_successful())
            .flat_map(|wrapper| wrapper.inners.iter())
            .filter(|inner| inner.was_applied)
            .filter_map(|inner| match &inner.kind {
                InnerKind::InitProposal(proposal) => Some(format!(
                    "{:?} proposal by {}, voting from epoch {} to {}, active at {}",
                    proposal.r#type,
                    proposal.author,
                    proposal.voting_start_epoch,
                    proposal.voting_end_epoch,
                    proposal.activation_epoch
                )),
                _ => None,
            })
            .collect::<Vec<_>>();

        if proposals.is_empty() {
            return Vec::new();
        }

        vec![Alert {
            check_id: PROPOSAL_CHECK_ID.to_string(),
            title: "New governance proposal".to_string(),
            description: format!(
                "*{}* governance proposals were submitted in block {}:{}",
                proposals.len(),
                last_state.block.height,
                summarize(&proposals)
            ),
            severity: Severity::Medium,
            metadata: Metadata::new(Some(last_state.block.height as u32), None),
            trigger_after: None,
            continous: self.is_continous(),
        }]
    }

    fn is_continous(&self) -> bool {
        false
    }
}

impl Display for GovernanceCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GovernanceCheck")
    }
}
