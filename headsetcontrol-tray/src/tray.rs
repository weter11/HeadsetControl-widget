use ksni::{self, menu::*, Tray, ToolTip};
use std::sync::{Arc, Mutex};
use tokio::sync::watch;
use crate::autostart;
use crate::config::{Config, save_config};
use crate::headset_cli::{self, HeadsetControlOutput, BatteryInfo};

const ASSUMED_CHARGE_TIME_HOURS: f32 = 3.0;
const ASSUMED_BATTERY_LIFE_HOURS: f32 = 20.0;

pub struct HeadsetTray {
    pub status: Arc<Mutex<Option<HeadsetControlOutput>>>,
    pub config: Arc<Mutex<Config>>,
    pub shutdown_tx: watch::Sender<bool>,
}

fn format_battery_info(b: &BatteryInfo) -> (String, String) {
    let level_str = b.level.map(|l| format!("{}%", l)).unwrap_or_else(|| "Unknown".into());
    let state = if b.status == "BATTERY_CHARGING" {
        let remaining = b.level.map(|l| format!(", ~{:.1}h to full", (100.0 - l as f32) / 100.0 * ASSUMED_CHARGE_TIME_HOURS)).unwrap_or_default();
        format!(" (Charging{})", remaining)
    } else {
        let remaining = b.level.map(|l| format!(", ~{:.1}h remaining", (l as f32) / 100.0 * ASSUMED_BATTERY_LIFE_HOURS)).unwrap_or_default();
        format!(" (Discharging{})", remaining)
    };
    (level_str, state)
}

impl Tray for HeadsetTray {
    fn id(&self) -> String {
        "headsetcontrol-tray".into()
    }

    fn icon_name(&self) -> String {
        let status_lock = match self.status.lock() {
            Ok(lock) => lock,
            Err(_) => return "audio-headset-disconnected".into(),
        };
        if let Some(ref output) = *status_lock {
            if let Some(device) = output.devices.first() {
                if let Some(ref b) = device.battery {
                    if b.status == "BATTERY_CHARGING" {
                        return "battery-charging".into();
                    }
                    if let Some(level) = b.level {
                        if level < 20 {
                            return "battery-caution".into();
                        }
                    }
                }
                return "audio-headset".into();
            }
        }
        "audio-headset-disconnected".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![ksni::Icon {
            width: 32,
            height: 32,
            data: include_bytes!("../icons/headset.argb").to_vec(),
        }]
    }

    fn title(&self) -> String {
        let status_lock = match self.status.lock() {
            Ok(lock) => lock,
            Err(_) => return "HeadsetControl — Not connected".into(),
        };
        if let Some(ref output) = *status_lock {
            if let Some(device) = output.devices.first() {
                if let Some(ref b) = device.battery {
                    if let Some(level) = b.level {
                        return format!("HeadsetControl — {}% 🔋", level);
                    }
                }
                return "HeadsetControl — Connected".into();
            }
        }
        "HeadsetControl — Not connected".into()
    }

    fn tool_tip(&self) -> ToolTip {
        let config = match self.config.lock() {
            Ok(lock) => lock.clone(),
            Err(_) => Default::default(),
        };

        let sidetone_info = format!("Sidetone: {}/128", config.sidetone_level);
        let inactive_info = if config.inactive_time == 0 {
            "Auto-off: Disabled".into()
        } else {
            format!("Auto-off: {} min", config.inactive_time)
        };

        let status_lock = match self.status.lock() {
            Ok(lock) => lock,
            Err(_) => return ToolTip {
                title: "No headset found".into(),
                description: format!("Connection status: Disconnected\n{}\n{}", sidetone_info, inactive_info),
                icon_name: self.icon_name(),
                icon_pixmap: self.icon_pixmap(),
            },
        };

        if let Some(ref output) = *status_lock {
            if let Some(device) = output.devices.first() {
                let conn_status = if device.status == "success" { "Connected" } else { "Disconnected" };
                let battery_info = if let Some(ref b) = device.battery {
                    let (level_str, state) = format_battery_info(b);
                    format!("Battery: {}{}", level_str, state)
                } else {
                    "Battery: Unknown".into()
                };
                ToolTip {
                    title: device.device.clone(),
                    description: format!("Status: {}\n{}\n{}\n{}", conn_status, battery_info, sidetone_info, inactive_info),
                    icon_name: self.icon_name(),
                    icon_pixmap: self.icon_pixmap(),
                }
            } else {
                ToolTip {
                    title: "No headset found".into(),
                    description: format!("Connection status: Not found\n{}\n{}", sidetone_info, inactive_info),
                    icon_name: self.icon_name(),
                    icon_pixmap: self.icon_pixmap(),
                }
            }
        } else {
            ToolTip {
                title: "No headset found".into(),
                description: format!("Connection status: Disconnected\n{}\n{}", sidetone_info, inactive_info),
                icon_name: self.icon_name(),
                icon_pixmap: self.icon_pixmap(),
            }
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let config = match self.config.lock() {
            Ok(lock) => lock.clone(),
            Err(_) => Default::default(),
        };

        let mut menu_items = Vec::new();

        // Get connected device stats (cloning only necessary fields to keep lock-holding minimal)
        let status_lock = self.status.lock().ok();
        let device_data = status_lock.as_ref()
            .and_then(|lock| lock.as_ref())
            .and_then(|output| output.devices.first())
            .map(|device| (device.device.clone(), device.battery.clone(), device.status.clone()));
        drop(status_lock);

        if let Some((device_name, battery, status)) = device_data {
            if status == "success" {
                // Add Device Name
                menu_items.push(StandardItem {
                    label: format!("Device: {}", device_name),
                    enabled: false,
                    ..Default::default()
                }.into());

                // Add Battery Info
                if let Some(ref b) = battery {
                    let (level_str, state) = format_battery_info(b);
                    menu_items.push(StandardItem {
                        label: format!("Battery: {}{}", level_str, state),
                        enabled: false,
                        ..Default::default()
                    }.into());
                }

                // Add Sidetone Info
                menu_items.push(StandardItem {
                    label: format!("Sidetone: {}/128", config.sidetone_level),
                    enabled: false,
                    ..Default::default()
                }.into());

                // Add Auto-off Info
                let inactive_str = if config.inactive_time == 0 {
                    "Disabled".into()
                } else {
                    format!("{} min", config.inactive_time)
                };
                menu_items.push(StandardItem {
                    label: format!("Auto-off: {}", inactive_str),
                    enabled: false,
                    ..Default::default()
                }.into());

                menu_items.push(MenuItem::Separator);
            }
        }

        // Add standard controls
        menu_items.extend(vec![
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
                            let mut cfg = match this.config.lock() {
                                Ok(lock) => lock,
                                Err(_) => return,
                            };
                            cfg.sidetone_level = level;
                            let _ = save_config(&cfg);
                            std::thread::spawn(move || {
                                let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
                                rt.block_on(async {
                                    let _ = headset_cli::set_sidetone(level).await;
                                });
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
                            let mut cfg = match this.config.lock() {
                                Ok(lock) => lock,
                                Err(_) => return,
                            };
                            cfg.inactive_time = m;
                            let _ = save_config(&cfg);
                            std::thread::spawn(move || {
                                let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
                                rt.block_on(async {
                                    let _ = headset_cli::set_inactive_time(m).await;
                                });
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
                                        let mut cfg = match this.config.lock() {
                                            Ok(lock) => lock,
                                            Err(_) => return,
                                        };
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
                                    let mut cfg = match this.config.lock() {
                                        Ok(lock) => lock,
                                        Err(_) => return,
                                    };
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
                                        let mut cfg = match this.config.lock() {
                                            Ok(lock) => lock,
                                            Err(_) => return,
                                        };
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
                                    let mut cfg = match this.config.lock() {
                                        Ok(lock) => lock,
                                        Err(_) => return,
                                    };
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

        menu_items
    }
}
