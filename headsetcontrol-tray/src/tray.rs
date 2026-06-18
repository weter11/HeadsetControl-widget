use ksni::{self, menu::*, Tray, ToolTip};
use std::sync::{Arc, Mutex};
use crate::autostart;
use crate::config::{Config, save_config};
use crate::headset_cli::{self, HeadsetControlOutput};

pub struct HeadsetTray {
    pub status: Arc<Mutex<Option<HeadsetControlOutput>>>,
    pub config: Arc<Mutex<Config>>,
}

impl Tray for HeadsetTray {
    fn id(&self) -> String {
        "headsetcontrol-tray".into()
    }

    fn icon_name(&self) -> String {
        let status_lock = self.status.lock().unwrap();
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
        let status_lock = self.status.lock().unwrap();
        if let Some(ref output) = *status_lock {
            if let Some(device) = output.devices.first() {
                let conn_status = if device.status == "success" { "Connected" } else { "Disconnected" };
                let battery_info = if let Some(ref b) = device.battery {
                    let level = b.level.map(|l| format!(": {}%", l)).unwrap_or_default();
                    let state = if b.status == "BATTERY_CHARGING" { " (Charging)" } else { " (Discharging)" };
                    format!("Battery{}{}", level, state)
                } else {
                    "Battery: Unknown".into()
                };
                ToolTip {
                    title: device.device.clone(),
                    description: format!("Status: {}\n{}", conn_status, battery_info),
                    ..Default::default()
                }
            } else {
                ToolTip {
                    title: "HeadsetControl".into(),
                    description: "No device found".into(),
                    ..Default::default()
                }
            }
        } else {
            ToolTip {
                title: "HeadsetControl".into(),
                description: "Disconnected".into(),
                ..Default::default()
            }
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let config = self.config.lock().unwrap().clone();

        vec![
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
                activate: Box::new(|_| std::process::exit(0)),
                ..Default::default()
            }.into(),
        ]
    }
}
