use anyhow::Context;
use async_trait::async_trait;
use telegrama_rs::{ClientOptions, FormattingOptions, Response, Telegrama};

use crate::{
    alerts::AlertTrait,
    shared::{
        alert::{Alert, Severity},
        block_explorer::BlockExplorer,
        config::TelegramAlertConfig,
    },
};

pub struct TelegramAlert {
    pub block_explorer: BlockExplorer,
    pub telegram_token: String,
    pub telegram_chat_id: String,
    pub network_id: String,
    pub minimum_severity: Option<Severity>,
}

#[async_trait]
impl AlertTrait for TelegramAlert {
    async fn send_alerts(&self, alert: Alert) -> Result<Option<String>, String> {
        if alert.severity < self.minimum_severity.clone().unwrap_or(Severity::Low) {
            return Ok(None);
        }

        let block_explorer = self.block_explorer.clone();

        let title = alert.title.clone();
        let description = alert.description.clone();
        let metadata = alert.metadata.clone();

        // MarkdownV2 links are `[text](url)`; the Slack `<url|text>` form would
        // be rendered verbatim.
        let message = format!(
            "*{title} - {network}*\n\
            {description}.\n\
            *Block*: {height}\n\
            *Transaction*: {transaction}",
            title = title,
            description = description,
            height = metadata
                .block_height
                .map(|h| format!("[{}]({})", h, block_explorer.get_block_url(h)))
                .unwrap_or_else(|| "N/A".to_string()),
            transaction = metadata
                .tx_id
                .as_ref()
                .map(|tx| format!("[{}]({})", tx, block_explorer.get_tx_url(tx)))
                .unwrap_or_else(|| "N/A".to_string()),
            network = self.network_id,
        );

        let res = self.send_telegram_message(message).await;

        match res {
            Ok(response) if response.ok => return Ok(Some(alert.check_id)),
            Ok(response) => {
                return Err(format!(
                    "Failed to send telegram alert, status: {}",
                    response.description.unwrap_or_default()
                ));
            }
            Err(err) => {
                return Err(format!("Failed to send telegram alert: {}", err));
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

        let res = self.send_telegram_message(message).await;

        match res {
            Ok(response) if response.ok => return Ok(()),
            Ok(response) => {
                return Err(format!(
                    "Failed to send telegram alert, status: {}",
                    response.description.unwrap_or_default()
                ));
            }
            Err(err) => {
                return Err(format!("Failed to send telegram alert: {}", err));
            }
        }
    }

    fn get_id(&self) -> String {
        "telegram".to_string()
    }
}

impl TelegramAlert {
    pub fn new(
        block_explorer: BlockExplorer,
        telegram_config: TelegramAlertConfig,
        network_id: String,
    ) -> Self {
        Self {
            block_explorer,
            telegram_token: telegram_config.telegram_token,
            telegram_chat_id: telegram_config.telegram_chat_id,
            network_id,
            minimum_severity: telegram_config.minimum_severity,
        }
    }

    async fn send_telegram_message(&self, message: String) -> anyhow::Result<Response> {
        Telegrama::configure(|config| {
            config.set_bot_token(&self.telegram_token);
            config.set_chat_id(&self.telegram_chat_id);

            config.set_default_parse_mode("MarkdownV2"); // or "HTML"
            config.set_disable_web_page_preview(true);

            let formatting = FormattingOptions {
                escape_markdown: true,
                obfuscate_emails: true,
                escape_html: false,
                truncate: Some(4096),
            };
            config.set_formatting_options(formatting);

            let client_options = ClientOptions {
                timeout: 30,
                retry_count: 3,
                retry_delay: 1,
            };
            config.set_client_options(client_options);
        });

        Telegrama::send_message(
            message,
            &[
                ("parse_mode", "MarkdownV2"),
                ("disable_web_page_preview", "true"),
            ],
        )
        .context("sending telegram message")
    }
}
