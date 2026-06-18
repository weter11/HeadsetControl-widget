use ksni::{self, menu::*, Tray, ToolTip};
use std::sync::{Arc, Mutex};
use tokio::sync::watch;
use crate::autostart;
use crate::config::{Config, save_config};
use crate::headset_cli::{self, HeadsetControlOutput};

pub struct HeadsetTray {
    pub status: Arc<Mutex<Option<HeadsetControlOutput>>>,
    pub config: Arc<Mutex<Config>>,
    pub shutdown_tx: watch::Sender<bool>,
}

fn get_autostart_path() -> Option<std::path::PathBuf> {
    dirs_next::config_dir().map(|mut p| {
        p.push("autostart");
        p.push("headsetcontrol-tray.desktop");
        p
    })
}

fn is_autostart_enabled() -> bool {
    get_autostart_path().map_or(false, |p| p.exists())
}

fn set_autostart(enabled: bool) {
    if let Some(path) = get_autostart_path() {
        if enabled {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let content = r#"[Desktop Entry]
Name=HeadsetControl Tray
Comment=Control your headset from the system tray
Exec=headsetcontrol-tray
Icon=headset
Terminal=false
Type=Application
Categories=Settings;HardwareSettings;
StartupNotify=false
X-GNOME-Autostart-enabled=true
"#;
            let _ = std::fs::write(path, content);
        } else {
            let _ = std::fs::remove_file(path);
        }
    }
}

impl Tray for HeadsetTray {
    fn id(&self) -> String {
        "headsetcontrol-tray".into()
    }

    fn icon_name(&self) -> String {
<<<<<<< HEAD
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
=======
        let status_lock = self.status.lock().unwrap();
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
>>>>>>> origin/master
    }

    fn title(&self) -> String {
        let status_lock = self.status.lock().unwrap();
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
<<<<<<< HEAD
        let status_lock = match self.status.lock() {
            Ok(lock) => lock,
            Err(_) => return ToolTip {
                title: "HeadsetControl".into(),
                description: "No headset connected".into(),
                ..Default::default()
            },
        };
        if let Some(ref output) = *status_lock {
            if let Some(device) = output.devices.first() {
                if device.status == "success" {
                    let battery_info = if let Some(ref b) = device.battery {
                        let level = b.level.map(|l| format!(": {}%", l)).unwrap_or_default();
                        let state = if b.status == "BATTERY_CHARGING" { " (Charging)" } else { " (Discharging)" };
                        format!("Battery{}{}", level, state)
                    } else {
                        "Battery: Unknown".into()
                    };
                    ToolTip {
                        title: device.device.clone(),
                        description: format!("Status: Connected\n{}", battery_info),
                        ..Default::default()
                    }
                } else {
                    ToolTip {
                        title: "HeadsetControl".into(),
                        description: "No headset connected".into(),
                        ..Default::default()
                    }
                }
            } else {
                ToolTip {
                    title: "HeadsetControl".into(),
                    description: "No headset connected".into(),
                    ..Default::default()
=======
        let status_lock = self.status.lock().unwrap();
        let config_lock = self.config.lock().unwrap();
        let config = config_lock.clone();
        drop(config_lock);
        drop(status_lock);

        let sidetone_info = format!("Sidetone: {}/128", config.sidetone_level);
        let inactive_info = if config.inactive_time == 0 {
            "Auto-off: Disabled".into()
        } else {
            format!("Auto-off: {} min", config.inactive_time)
        };

        let status_lock = self.status.lock().unwrap();
        if let Some(ref output) = *status_lock {
            if let Some(device) = output.devices.first() {
                let conn_status = if device.status == "success" { "Connected" } else { "Disconnected" };
                let battery_info = if let Some(ref b) = device.battery {
                    let level_str = b.level.map(|l| format!("{}%", l)).unwrap_or_else(|| "Unknown".into());
                    let state = if b.status == "BATTERY_CHARGING" {
                        let remaining = b.level.map(|l| format!(", ~{:.1}h to full", (100.0 - l as f32) / 100.0 * 3.0)).unwrap_or_default();
                        format!(" (Charging{})", remaining)
                    } else {
                        let remaining = b.level.map(|l| format!(", ~{:.1}h remaining", (l as f32) / 100.0 * 20.0)).unwrap_or_default();
                        format!(" (Discharging{})", remaining)
                    };
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
>>>>>>> origin/master
                }
            }
        } else {
            ToolTip {
<<<<<<< HEAD
                title: "HeadsetControl".into(),
                description: "No headset connected".into(),
                ..Default::default()
=======
                title: "No headset found".into(),
                description: format!("Connection status: Disconnected\n{}\n{}", sidetone_info, inactive_info),
                icon_name: self.icon_name(),
                icon_pixmap: self.icon_pixmap(),
>>>>>>> origin/master
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
                            let mut cfg = this.config.lock().unwrap();
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
<<<<<<< HEAD
                checked: autostart::is_autostart_enabled(),
                activate: Box::new(|_this: &mut Self| {
                    let currently_enabled = autostart::is_autostart_enabled();
                    if let Err(e) = autostart::set_autostart(!currently_enabled) {
                        eprintln!("Failed to toggle autostart: {}", e);
                    }
=======
                checked: is_autostart_enabled(),
                activate: Box::new(|_this: &mut Self| {
                    let currently_enabled = is_autostart_enabled();
                    set_autostart(!currently_enabled);
>>>>>>> origin/master
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
        ]
    }
}
