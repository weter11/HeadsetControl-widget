mod autostart;
mod headset_cli;
mod config;
mod notifications;
mod tray;

use std::sync::{Arc, Mutex};
use std::time::Duration;
use ksni::TrayMethods;
use crate::tray::HeadsetTray;
use crate::notifications::NotificationManager;

#[tokio::main]
async fn main() {
    let config = Arc::new(Mutex::new(config::load_config()));
    let status = Arc::new(Mutex::new(None));

    let tray = HeadsetTray {
        status: status.clone(),
        config: config.clone(),
    };

    let handle = tray.spawn().await.expect("Failed to spawn tray");

    let mut notification_manager = NotificationManager::new();

    // Initial sync
    {
        let cfg = config.lock().unwrap();
        let _ = headset_cli::set_sidetone(cfg.sidetone_level).await;
        let _ = headset_cli::set_inactive_time(cfg.inactive_time).await;
    }

    // Background polling loop
    loop {
        match headset_cli::get_headset_status().await {
            Ok(new_status) => {
                let mut status_lock = status.lock().unwrap();

                // Check for battery notifications
                if let Some(device) = new_status.devices.first() {
                    if let Some(battery) = &device.battery {
                        if let Some(level) = battery.level {
                            let charging = battery.status == "BATTERY_CHARGING";
                            let cfg = config.lock().unwrap();
                            notification_manager.check(level, charging, &cfg);
                        }
                    }
                }

                *status_lock = Some(new_status);
                let _ = handle.update(|_| {}).await;
            }
            Err(e) => {
                eprintln!("Error polling headset: {}", e);
            }
        }

        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
