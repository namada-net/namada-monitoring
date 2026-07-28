use async_trait::async_trait;

use crate::shared::{alert::Alert, block_explorer::BlockExplorer};

use super::AlertTrait;

pub struct Log {
    pub block_explorer: BlockExplorer,
}

#[async_trait]
impl AlertTrait for Log {
    async fn send_alerts(&self, alert: Alert) -> Result<Option<String>, String> {
        let title = alert.title.clone();
        let description = alert.description.clone();
        let metadata = alert.metadata.clone();

        let tx_url = metadata.tx_id.map(|tx| self.block_explorer.get_tx_url(&tx));
        let block_url = metadata
            .block_height
            .map(|height| self.block_explorer.get_block_url(height));

        for text in [
            ("Title", title),
            ("Description", description),
            (
                "Transaction URL",
                tx_url.unwrap_or_else(|| "N/A".to_string()),
            ),
            ("Block URL", block_url.unwrap_or_else(|| "N/A".to_string())),
        ] {
            println!("{}: {}", text.0, text.1);
        }

        Ok(Some(alert.check_id))
    }

    async fn send_resolve(&self, alert: Alert, date: &str) -> Result<(), String> {
        let title = alert.title.clone();

        println!("Resolved Alert:");
        for text in [("Title", title), ("Resolved issue at", date.to_string())] {
            println!("{}: {}", text.0, text.1);
        }

        Ok(())
    }

    fn get_id(&self) -> String {
        "log".to_string()
    }
}

impl Log {
    pub fn new(block_explorer: BlockExplorer) -> Self {
        Self { block_explorer }
    }
}
