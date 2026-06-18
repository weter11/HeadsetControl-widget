mod autostart;
mod headset_cli;
mod config;
mod notifications;
mod tray;

use std::sync::{Arc, Mutex};
use std::time::Duration;
use ksni::TrayMethods;
use tokio::sync::watch;
use crate::tray::HeadsetTray;
use crate::notifications::NotificationManager;

const MAX_TRAY_ERRORS: u32 = 3;

#[tokio::main]
async fn main() {
    let config = Arc::new(Mutex::new(config::load_config()));
    let status = Arc::new(Mutex::new(None));
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

    let tray = HeadsetTray {
        status: status.clone(),
        config: config.clone(),
        shutdown_tx: shutdown_tx.clone(),
    };

    let handle = tray.spawn().await.expect("Failed to spawn tray");

    let mut notification_manager = NotificationManager::new();

    // Initial sync
    {
        if let Ok(cfg) = config.lock() {
            let _ = headset_cli::set_sidetone(cfg.sidetone_level).await;
            let _ = headset_cli::set_inactive_time(cfg.inactive_time).await;
        }
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
                        // Check for battery notifications only for connected devices
                        if let Some(device) = new_status.devices.first() {
                            if device.status == "success" {
                                if let Some(battery) = &device.battery {
                                    if let Some(level) = battery.level {
                                        let charging = battery.status == "BATTERY_CHARGING";
                                        if let Ok(cfg) = config.lock() {
                                            notification_manager.check(level, charging, &cfg);
                                        }
                                    }
                                }
                            }
                        }

                        if let Ok(mut status_lock) = status.lock() {
                            *status_lock = Some(new_status);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error polling headset: {}", e);
                        // Clear status so tooltip shows disconnected state
                        if let Ok(mut status_lock) = status.lock() {
                            *status_lock = None;
                        }
                    }
                }

                // Always update tray to reflect current state
                if handle.update(|_| {}).await.is_none() {
                    eprintln!("Tray update failed: handle returned None");
                    tray_errors += 1;
                    if tray_errors >= MAX_TRAY_ERRORS {
                        eprintln!("Tray unrecoverable, exiting");
                        let _ = shutdown_tx.send(true);
                        break;
                    }
                } else {
                    tray_errors = 0;
                }
            }
        }
    }
}
