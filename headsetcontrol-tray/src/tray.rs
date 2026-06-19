use ksni::{self, menu::*, Tray, ToolTip};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::watch;
use crate::autostart;
use crate::config::{Config, save_config};
use crate::headset_cli::{self, HeadsetControlOutput};
use crate::battery_rate::BatteryRateTracker;

pub struct HeadsetTray {
    pub status: Arc<Mutex<Option<HeadsetControlOutput>>>,
    pub config: Arc<Mutex<Config>>,
    pub battery_tracker: Arc<Mutex<BatteryRateTracker>>,
    pub shutdown_tx: watch::Sender<bool>,
}

impl Tray for HeadsetTray {
    fn id(&self) -> String {
        "headsetcontrol-tray".into()
    }

    fn icon_name(&self) -> String {
        let status_lock = match self.status.lock() {
            Ok(lock) => lock,
            Err(_) => return "audio-headphones".into(),
        };
        if let Some(ref output) = *status_lock {
            if let Some(device) = output.devices.first() {
                if device.status == "success" {
                    return "audio-headset".into();
                }
            }
        }
        // Disconnected or no data yet — use a dimmed/generic icon
        "audio-headphones".into()
    }

    fn title(&self) -> String {
        "HeadsetControl".into()
    }

    fn tool_tip(&self) -> ToolTip {
        let icon_name = self.icon_name();
        let status_lock = match self.status.lock() {
            Ok(lock) => lock,
            Err(_) => return ToolTip {
                title: "HeadsetControl".into(),
                description: "No headset connected".into(),
                icon_name,
                ..Default::default()
            },
        };
        if let Some(ref output) = *status_lock {
            if let Some(device) = output.devices.first() {
                if device.status == "success" {
                    let battery_tracker = self.battery_tracker.lock().unwrap();
                    let (percentage, details) = if let Some(ref b) = device.battery {
                        let level = b.level.unwrap_or(0);
                        let charging = b.status == "BATTERY_CHARGING";
                        let estimate = battery_tracker.estimated_remaining(level, charging);

                        format_battery_info(level, charging, estimate)
                    } else {
                        ("Unknown".into(), "".into())
                    };
                    ToolTip {
                        title: device.device.clone(),
                        description: format!("Status: Connected\nBattery: {} {}", percentage, details),
                        icon_name,
                        ..Default::default()
                    }
                } else {
                    ToolTip {
                        title: "HeadsetControl".into(),
                        description: "No headset connected".into(),
                        icon_name,
                        ..Default::default()
                    }
                }
            } else {
                ToolTip {
                    title: "HeadsetControl".into(),
                    description: "No headset connected".into(),
                    icon_name,
                    ..Default::default()
                }
            }
        } else {
            ToolTip {
                title: "HeadsetControl".into(),
                description: "No headset connected".into(),
                icon_name,
                ..Default::default()
            }
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let config = self.config.lock().unwrap().clone();
        let mut menu = Vec::new();

        // Info Block
        {
            let status_lock = self.status.lock().unwrap();
            if let Some(ref output) = *status_lock {
                if let Some(device) = output.devices.first() {
                    if device.status == "success" {
                        let battery_tracker = self.battery_tracker.lock().unwrap();
                        let (percentage, details) = if let Some(ref b) = device.battery {
                            let level = b.level.unwrap_or(0);
                            let charging = b.status == "BATTERY_CHARGING";
                            let estimate = battery_tracker.estimated_remaining(level, charging);

                            format_battery_info(level, charging, estimate)
                        } else {
                            ("Unknown".into(), "".into())
                        };

                        menu.push(StandardItem {
                            label: format!("{} — {}", device.device, percentage),
                            enabled: false,
                            ..Default::default()
                        }.into());

                        if !details.is_empty() {
                            menu.push(StandardItem {
                                label: details,
                                enabled: false,
                                ..Default::default()
                            }.into());
                        }
                    } else {
                        menu.push(StandardItem {
                            label: "No headset connected".into(),
                            enabled: false,
                            ..Default::default()
                        }.into());
                    }
                } else {
                    menu.push(StandardItem {
                        label: "No headset connected".into(),
                        enabled: false,
                        ..Default::default()
                    }.into());
                }
            } else {
                menu.push(StandardItem {
                    label: "No headset connected".into(),
                    enabled: false,
                    ..Default::default()
                }.into());
            }
        }

        menu.push(MenuItem::Separator);

        menu.push(CheckmarkItem {
            label: "Show Notifications".into(),
            checked: config.notifications_enabled,
            activate: Box::new(|this: &mut Self| {
                let mut cfg = this.config.lock().unwrap();
                cfg.notifications_enabled = !cfg.notifications_enabled;
                let _ = save_config(&cfg);
            }),
            ..Default::default()
        }.into());

        menu.push(CheckmarkItem {
            label: "Reapply settings on startup".into(),
            checked: config.reapply_on_startup,
            activate: Box::new(|this: &mut Self| {
                let mut cfg = this.config.lock().unwrap();
                cfg.reapply_on_startup = !cfg.reapply_on_startup;
                let _ = save_config(&cfg);
            }),
            ..Default::default()
        }.into());

        menu.push(MenuItem::Separator);

        menu.extend(vec![
            // Sidetone Submenu
            SubMenu {
                label: "Sidetone Level".into(),
                submenu: (0..10).map(|i| {
                    let percentiles = [0, 14, 28, 42, 57, 71, 85, 99, 113, 128];
                    let level = percentiles[i];
                    let label = format!("Level {}", level);
                    let is_active = (config.sidetone_level as i32 - level as i32).abs() < 5;
                    CheckmarkItem {
                        label,
                        checked: is_active,
                        activate: Box::new(move |this: &mut Self| {
                            let mut cfg = this.config.lock().unwrap();
                            cfg.sidetone_level = level;
                            let _ = save_config(&cfg);
                            tokio::spawn(async move {
                                let _ = headset_cli::set_sidetone(level).await;
                            });
                        }),
                        ..Default::default()
                    }.into()
                }).collect(),
                ..Default::default()
            }.into(),

            // Inactive Time Submenu
            SubMenu {
                label: "Inactive Time".into(),
                submenu: vec![0, 1, 2, 3, 5, 10, 15, 20, 30, 45, 60, 90].into_iter().map(|m| {
                    let label = if m == 0 { "Disabled".into() } else { format!("{} min", m) };
                    CheckmarkItem {
                        label,
                        checked: config.inactive_time == m,
                        activate: Box::new(move |this: &mut Self| {
                            let mut cfg = this.config.lock().unwrap();
                            cfg.inactive_time = m;
                            let _ = save_config(&cfg);
                            tokio::spawn(async move {
                                let _ = headset_cli::set_inactive_time(m).await;
                            });
                        }),
                        ..Default::default()
                    }.into()
                }).collect(),
                ..Default::default()
            }.into(),

            // Battery Notifications Submenu
            SubMenu {
                label: "Battery Notifications".into(),
                submenu: vec![
                    SubMenu {
                        label: "Discharge Level".into(),
                        submenu: {
                            let mut items: Vec<MenuItem<Self>> = (1..=9).map(|i| {
                                let level = i * 10;
                                CheckmarkItem {
                                    label: format!("{}%", level),
                                    checked: config.discharge_level == Some(level),
                                    activate: Box::new(move |this: &mut Self| {
                                        let mut cfg = this.config.lock().unwrap();
                                        cfg.discharge_level = Some(level);
                                        let _ = save_config(&cfg);
                                    }),
                                    ..Default::default()
                                }.into()
                            }).collect();
                            items.push(CheckmarkItem {
                                label: "Disable".into(),
                                checked: config.discharge_level.is_none(),
                                activate: Box::new(|this: &mut Self| {
                                    let mut cfg = this.config.lock().unwrap();
                                    cfg.discharge_level = None;
                                    let _ = save_config(&cfg);
                                }),
                                ..Default::default()
                            }.into());
                            items
                        },
                        ..Default::default()
                    }.into(),
                    SubMenu {
                        label: "Charge Level".into(),
                        submenu: {
                            let mut items: Vec<MenuItem<Self>> = (1..=10).map(|i| {
                                let level = i * 10;
                                CheckmarkItem {
                                    label: format!("{}%", level),
                                    checked: config.charge_level == Some(level),
                                    activate: Box::new(move |this: &mut Self| {
                                        let mut cfg = this.config.lock().unwrap();
                                        cfg.charge_level = Some(level);
                                        let _ = save_config(&cfg);
                                    }),
                                    ..Default::default()
                                }.into()
                            }).collect();
                            items.push(CheckmarkItem {
                                label: "Disable".into(),
                                checked: config.charge_level.is_none(),
                                activate: Box::new(|this: &mut Self| {
                                    let mut cfg = this.config.lock().unwrap();
                                    cfg.charge_level = None;
                                    let _ = save_config(&cfg);
                                }),
                                ..Default::default()
                            }.into());
                            items
                        },
                        ..Default::default()
                    }.into(),
                ],
                ..Default::default()
            }.into(),

            MenuItem::Separator,

            CheckmarkItem {
                label: "Start on Login".into(),
                checked: autostart::is_autostart_enabled(),
                activate: Box::new(|_this: &mut Self| {
                    let currently_enabled = autostart::is_autostart_enabled();
                    if let Err(e) = autostart::set_autostart(!currently_enabled) {
                        eprintln!("Failed to toggle autostart: {}", e);
                    }
                }),
                ..Default::default()
            }.into(),

            MenuItem::Separator,

            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.shutdown_tx.send(true);
                }),
                ..Default::default()
            }.into(),
        ]);

        menu
    }
}

fn format_battery_info(level: i32, charging: bool, estimate: Option<Duration>) -> (String, String) {
    let percentage = format!("{}%", level);
    let state = if charging { "Charging" } else { "Discharging" };
    let time_clause = if let Some(duration) = estimate {
        let hours = duration.as_secs() as f64 / 3600.0;
        let suffix = if charging { " to full" } else { "" };
        format!(", ~{:.1}h{}", hours, suffix)
    } else {
        "".to_string()
    };
    (percentage, format!("({}{})", state, time_clause))
}
