pub mod log;
pub mod slack;
pub mod telegram;

use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use log::Log;
use ttl_cache::TtlCache;

use crate::{checks::AppConfig, shared::alert::Alert};

/// How long a continuous alert with no explicit `trigger_after` stays on record
/// so that a resolution can be reported for it.
const DEFAULT_CONTINUOUS_TTL: Duration = Duration::from_secs(60 * 60);

/// Number of simultaneously firing alerts kept on record. Alert ids are per
/// channel and can be per token or per IBC channel, so this needs headroom.
const ON_FIRE_CAPACITY: usize = 1024;

pub struct AlertManager {
    pub communication: Vec<Box<dyn AlertTrait>>,
    on_fire: TtlCache<String, (Alert, String)>,
}

impl AlertManager {
    pub fn new(app_config: &AppConfig) -> Self {
        let config = app_config.get_config();
        let any_alert_config = config.slack.is_some() || config.telegram.is_some();

        if !any_alert_config {
            return Self {
                communication: vec![Box::new(Log::new(config.block_explorer.clone()))],
                on_fire: TtlCache::new(ON_FIRE_CAPACITY),
            };
        };

        let mut alerts: Vec<Box<dyn AlertTrait>> = vec![];

        if let Some(slack_config) = config.slack.clone() {
            let slack_alert = slack::SlackAlert::new(
                config.block_explorer.clone(),
                slack_config,
                app_config.chain_id.clone(),
            );
            alerts.push(Box::new(slack_alert));
        };

        if let Some(telegram_config) = config.telegram.clone() {
            let telegram_alert = telegram::TelegramAlert::new(
                config.block_explorer.clone(),
                telegram_config,
                app_config.chain_id.clone(),
            );
            alerts.push(Box::new(telegram_alert));
        };

        Self {
            communication: alerts,
            on_fire: TtlCache::new(ON_FIRE_CAPACITY),
        }
    }

    /// Key an alert is de-duplicated under, per communication channel.
    fn alert_key(communication: &dyn AlertTrait, alert: &Alert) -> String {
        format!("{}-{}", communication.get_id(), alert.check_id)
    }

    pub async fn run_alerts(&mut self, alerts: Vec<Alert>) {
        for communication in &self.communication {
            for alert in &alerts {
                let id = Self::alert_key(communication.as_ref(), alert);
                let alert_trigger_after = alert.trigger_after;

                if alert_trigger_after.is_some() && self.on_fire.contains_key(&id) {
                    tracing::debug!(
                        "Alert {} already on fire, skipping sending again",
                        alert.check_id
                    );
                    continue;
                }

                if let Err(err) = communication.send_alerts(alert.clone()).await {
                    tracing::error!("Failed to send alert: {}", err);
                    continue;
                }

                // A zero ttl expires on insert, which would leave a continuous
                // alert with no record to resolve against.
                let ttl = match alert_trigger_after {
                    Some(trigger_after) => trigger_after,
                    None if alert.continous => DEFAULT_CONTINUOUS_TTL,
                    None => continue,
                };

                let utc: DateTime<Utc> = Utc::now();
                self.on_fire
                    .insert(id, (alert.clone(), utc.to_rfc3339()), ttl);
            }
        }

        // Anything still on fire that this round did not re-trigger has
        // recovered. Collect first so the cache is not borrowed across awaits.
        let resolved = self
            .on_fire
            .iter()
            .filter(|(_, (alert, _))| alert.continous)
            .filter(|(_, (firing_alert, _))| {
                !alerts
                    .iter()
                    .any(|alert| firing_alert.check_id.eq(&alert.check_id))
            })
            .map(|(id, (alert, date))| (id.clone(), alert.clone(), date.clone()))
            .collect::<Vec<_>>();

        for (id, firing_alert, date) in resolved {
            for communication in &self.communication {
                if id != Self::alert_key(communication.as_ref(), &firing_alert) {
                    continue;
                }

                if let Err(err) = communication
                    .send_resolve(firing_alert.clone(), &date)
                    .await
                {
                    tracing::error!("Failed to send resolved alert: {}", err);
                    continue;
                }
            }

            self.on_fire.remove(&id);
        }
    }
}

#[async_trait]
pub trait AlertTrait: Send + Sync {
    async fn send_alerts(&self, alert: Alert) -> Result<Option<String>, String>;
    async fn send_resolve(&self, alert: Alert, date: &str) -> Result<(), String>;
    fn get_id(&self) -> String;
}
