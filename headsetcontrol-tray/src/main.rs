mod autostart;
mod headset_cli;
mod config;
mod notifications;
mod tray;
mod battery_rate;

use std::sync::{Arc, Mutex};
use std::time::Duration;
use ksni::TrayMethods;
use tokio::sync::watch;
use crate::tray::HeadsetTray;
use crate::notifications::NotificationManager;
use crate::battery_rate::BatteryRateTracker;

#[tokio::main]
async fn main() {
    let config = Arc::new(Mutex::new(config::load_config()));
    let status = Arc::new(Mutex::new(None));
    let battery_tracker = Arc::new(Mutex::new(BatteryRateTracker::new()));
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

    let tray = HeadsetTray {
        status: status.clone(),
        config: config.clone(),
        battery_tracker: battery_tracker.clone(),
        shutdown_tx,
    };

    let handle = tray.spawn().await.expect("Failed to spawn tray");

    let mut notification_manager = NotificationManager::new();

    // Initial sync
    {
        let cfg = config.lock().unwrap();
        let _ = headset_cli::set_sidetone(cfg.sidetone_level).await;
        let _ = headset_cli::set_inactive_time(cfg.inactive_time).await;
    }

    let mut tray_errors: u32 = 0;

    // Background polling loop
    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    break;
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(5)) => {
                match headset_cli::get_headset_status().await {
                    Ok(new_status) => {
                        // Check for connection and battery notifications
                        if let Some(device) = new_status.devices.first() {
                            let is_connected = device.status == "success";
                            notification_manager.check_connection(is_connected, &device.device);

                            if is_connected {
                                if let Some(battery) = &device.battery {
                                    if let Some(level) = battery.level {
                                        let charging = battery.status == "BATTERY_CHARGING";

                                        // Update battery rate tracker
                                        {
                                            let mut tracker = battery_tracker.lock().unwrap();
                                            tracker.update(level, charging);
                                        }

                                        let cfg = config.lock().unwrap();
                                        notification_manager.check_battery(level, charging, &cfg);
                                    }
                                }
                            }
                        } else {
                            notification_manager.check_connection(false, "Headset");
                        }

                        let mut status_lock = status.lock().unwrap();
                        *status_lock = Some(new_status);
                        drop(status_lock);
                    }
                    Err(e) => {
                        eprintln!("Error polling headset: {}", e);
                        notification_manager.check_connection(false, "Headset");
                        // Clear status so tooltip shows disconnected state
                        let mut status_lock = status.lock().unwrap();
                        *status_lock = None;
                        drop(status_lock);
                    }
                }

                // Always update tray to reflect current state
                if handle.update(|_| {}).await.is_none() {
                    eprintln!("Tray update failed: handle returned None");
                    tray_errors += 1;
                    if tray_errors >= 3 {
                        eprintln!("Tray unrecoverable, exiting");
                        std::process::exit(1);
                    }
                } else {
                    tray_errors = 0;
                }
            }
        }
    }
}
