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

#[tokio::main]
async fn main() {
    let config = Arc::new(Mutex::new(config::load_config()));
    let status = Arc::new(Mutex::new(None));
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

    let tray = HeadsetTray {
        status: status.clone(),
        config: config.clone(),
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
                        drop(status_lock);

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
                    Err(e) => {
                        eprintln!("Error polling headset: {}", e);
                    }
                }
            }
        }
    }
}
