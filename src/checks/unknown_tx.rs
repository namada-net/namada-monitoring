use std::{collections::BTreeMap, fmt::Display, time::Duration};

use crate::shared::{
    alert::{summarize, Alert, Metadata, Severity},
    namada::InnerKind,
};

use super::CheckTrait;

const UNKNOWN_TX_CHECK_ID: &str = "unknown_tx_check";

/// Once the monitor cannot decode a transaction kind it silently stops counting
/// it, so this is dampened but never fully quiet.
const DAMPENING: Duration = Duration::from_secs(60 * 60 * 6);

/// Reports transaction kinds the monitor cannot decode.
///
/// These appear after a chain upgrade introduces or renames a transaction, and
/// every check and metric that classifies transactions silently under-reports
/// until the monitor is updated. Without this the only trace is a log line.
#[derive(Debug, Clone, Default)]
pub struct UnknownTxCheck {}

#[async_trait::async_trait]
impl CheckTrait for UnknownTxCheck {
    async fn check(&self, state: &crate::state::State) -> Vec<Alert> {
        let Some(last_state) = state.last_block() else {
            return Vec::new();
        };

        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for inner in last_state
            .block
            .transactions
            .iter()
            .flat_map(|wrapper| wrapper.inners.iter())
        {
            if let InnerKind::Unknown(code_name, _) = &inner.kind {
                *counts.entry(code_name.as_str()).or_default() += 1;
            }
        }

        if counts.is_empty() {
            return Vec::new();
        }

        let total = counts.values().sum::<usize>();
        let kinds = counts
            .into_iter()
            .map(|(code_name, count)| format!("{} ({} times)", code_name, count))
            .collect::<Vec<_>>();

        vec![Alert {
            check_id: UNKNOWN_TX_CHECK_ID.to_string(),
            title: "Undecodable transaction kind".to_string(),
            description: format!(
                "*{}* inner transactions in block {} could not be decoded. Transaction metrics and checks under-report until the monitor understands them:{}",
                total,
                last_state.block.height,
                summarize(&kinds)
            ),
            severity: Severity::Medium,
            metadata: Metadata::new(Some(last_state.block.height as u32), None),
            trigger_after: Some(DAMPENING),
            continous: self.is_continous(),
        }]
    }

    fn is_continous(&self) -> bool {
        false
    }
}

impl Display for UnknownTxCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UnknownTxCheck")
    }
}
