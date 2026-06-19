use notify_rust::Notification;
use crate::config::Config;

pub struct NotificationManager {
    last_level: Option<i32>,
    discharged_notified: bool,
    charged_notified: bool,
    was_connected: bool,
}

impl NotificationManager {
    pub fn new() -> Self {
        Self {
            last_level: None,
            discharged_notified: false,
            charged_notified: false,
            was_connected: false,
        }
    }

    pub fn check_connection(&mut self, currently_connected: bool, device_name: &str, enabled: bool) {
        if enabled {
            if currently_connected && !self.was_connected {
                self.send_notification("Headset Connected", device_name);
            } else if !currently_connected && self.was_connected {
                self.send_notification("Headset Disconnected", device_name);
            }
        }
        self.was_connected = currently_connected;
    }

    pub fn check_battery(&mut self, current_level: i32, charging: bool, config: &Config) {
        if let Some(discharge_threshold) = config.discharge_level {
            if !charging && current_level <= discharge_threshold as i32 {
                if !self.discharged_notified {
                    if config.notifications_enabled {
                        self.send_notification(
                            "Headset Battery Low",
                            &format!("Battery level is at {}%", current_level),
                        );
                    }
                    self.discharged_notified = true;
                }
            } else if current_level > discharge_threshold as i32 {
                self.discharged_notified = false;
            }
        }

        if let Some(charge_threshold) = config.charge_level {
            if charging && current_level >= charge_threshold as i32 {
                if !self.charged_notified {
                    if config.notifications_enabled {
                        self.send_notification(
                            "Headset Battery Charged",
                            &format!("Battery level is at {}%", current_level),
                        );
                    }
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
