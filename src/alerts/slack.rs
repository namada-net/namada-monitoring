use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;

use crate::shared::{
    alert::{Alert, Severity},
    block_explorer::BlockExplorer,
    config::SlackAlertConfig,
};

use super::AlertTrait;

#[derive(Serialize)]
struct SlackPayload {
    username: String,
    icon_emoji: String,
    attachments: Vec<Attachment>,
}

#[derive(Serialize)]
struct Attachment {
    color: String,
    blocks: Vec<Block>,
}

#[derive(Serialize)]
struct Block {
    #[serde(rename = "type")]
    block_type: String,
    text: Text,
}

#[derive(Serialize)]
struct Text {
    #[serde(rename = "type")]
    text_type: String,
    text: String,
}

pub struct SlackAlert {
    pub block_explorer: BlockExplorer,
    pub slack_hook_url: String,
    pub channel: String,
    pub network_id: String,
    pub mentions: Vec<String>,
    pub minimum_severity: Option<Severity>,
}

#[async_trait]
impl AlertTrait for SlackAlert {
    async fn send_alerts(&self, alert: Alert) -> Result<Option<String>, String> {
        if alert.severity < self.minimum_severity.clone().unwrap_or(Severity::Low) {
            return Ok(None);
        }

        let block_explorer = self.block_explorer.clone();

        let title = alert.title.clone();
        let description = alert.description.clone();
        let metadata = alert.metadata.clone();
        let emoji = alert.severity.to_emoji();
        let color = alert.severity.to_color();

        let message = format!(
            "*{title} - {network}*\n\
            {description}.\n\
            *Block*: {height}\n\
            *Transaction*: {transaction}",
            title = title,
            description = description,
            height = metadata
                .clone()
                .block_height
                .map(|h| format!("<{}|{}>", block_explorer.get_block_url(h), h))
                .unwrap_or_else(|| "N/A".to_string()),
            transaction = metadata
                .clone()
                .tx_id
                .map(|tx| format!("<{}|{}>", block_explorer.get_tx_url(&tx), tx))
                .unwrap_or_else(|| "N/A".to_string()),
            network = self.network_id,
        );

        let message = if matches!(alert.severity, Severity::Critical | Severity::High) {
            format!(
                "{}\n\
                *Ping*: {}",
                message,
                self.mentions.join(" ")
            )
        } else {
            message
        };

        let payload = SlackPayload {
            username: "Namada Alert Manager".into(),
            icon_emoji: emoji,
            attachments: vec![Attachment {
                color,
                blocks: vec![Block {
                    block_type: "section".into(),
                    text: Text {
                        text_type: "mrkdwn".into(),
                        text: message,
                    },
                }],
            }],
        };

        let res = Client::new()
            .post(self.slack_hook_url.clone())
            .json(&payload)
            .send()
            .await;

        match res {
            Ok(response) if response.status().is_success() => return Ok(Some(alert.check_id)),
            Ok(response) => {
                return Err(format!(
                    "Failed to send slack alert, status: {:?}",
                    response
                ));
            }
            Err(err) => {
                return Err(format!("Failed to send slack alert: {}", err));
            }
        }
    }

    async fn send_resolve(&self, alert: Alert, date: &str) -> Result<(), String> {
        let title = alert.title.clone();

        let message = format!(
            "*{title} - {network}*\n\
            Issue from {data} was resolved.",
            title = title,
            data = date,
            network = self.network_id,
        );

        let payload = SlackPayload {
            username: "Namada Alert Manager".into(),
            icon_emoji: ":white_check_mark:".into(),
            attachments: vec![Attachment {
                color: "#5df542".into(),
                blocks: vec![Block {
                    block_type: "section".into(),
                    text: Text {
                        text_type: "mrkdwn".into(),
                        text: message,
                    },
                }],
            }],
        };

        let res = Client::new()
            .post(self.slack_hook_url.clone())
            .json(&payload)
            .send()
            .await;

        match res {
            Ok(response) if response.status().is_success() => return Ok(()),
            Ok(response) => {
                return Err(format!(
                    "Failed to send slack alert, status: {}",
                    response.status()
                ));
            }
            Err(err) => {
                return Err(format!("Failed to send slack alert: {}", err));
            }
        }
    }

    fn get_id(&self) -> String {
        "slack".to_string()
    }
}

impl SlackAlert {
    pub fn new(
        block_explorer: BlockExplorer,
        slack_config: SlackAlertConfig,
        network_id: String,
    ) -> Self {
        Self {
            block_explorer,
            slack_hook_url: slack_config.slack_webhook,
            channel: slack_config.channel,
            network_id,
            mentions: slack_config.mentions,
            minimum_severity: slack_config.minimum_severity,
        }
    }
}
