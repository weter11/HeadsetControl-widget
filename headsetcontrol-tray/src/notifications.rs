use notify_rust::Notification;
use crate::config::Config;

pub struct NotificationManager {
    last_level: Option<i32>,
    discharged_notified: bool,
    charged_notified: bool,
}

impl NotificationManager {
    pub fn new() -> Self {
        Self {
            last_level: None,
            discharged_notified: false,
            charged_notified: false,
        }
    }

    pub fn check(&mut self, current_level: i32, charging: bool, config: &Config) {
        if let Some(discharge_threshold) = config.discharge_level {
            if !charging && current_level <= discharge_threshold as i32 {
                if !self.discharged_notified {
                    self.send_notification(
                        "Headset Battery Low",
                        &format!("Battery level is at {}%", current_level),
                    );
                    self.discharged_notified = true;
                }
            } else if current_level > discharge_threshold as i32 {
                self.discharged_notified = false;
            }
        }

        if let Some(charge_threshold) = config.charge_level {
            if charging && current_level >= charge_threshold as i32 {
                if !self.charged_notified {
                    self.send_notification(
                        "Headset Battery Charged",
                        &format!("Battery level is at {}%", current_level),
                    );
                    self.charged_notified = true;
                }
            } else if current_level < charge_threshold as i32 {
                self.charged_notified = false;
            }
        }

        self.last_level = Some(current_level);
    }

    fn send_notification(&self, summary: &str, body: &str) {
        let summary = summary.to_string();
        let body = body.to_string();
        // Run on blocking thread pool to avoid nested runtime panic.
        // notify-rust's .show() calls zbus::block_on() which creates a new
        // tokio runtime, panicking if called from within an existing runtime.
        let _ = tokio::task::spawn_blocking(move || {
            let _ = Notification::new()
                .summary(&summary)
                .body(&body)
                .icon("headset")
                .show();
        });
    }
}
