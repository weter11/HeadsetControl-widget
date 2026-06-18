use std::fs;
use std::path::PathBuf;

const DESKTOP_FILE_NAME: &str = "headsetcontrol-tray.desktop";

const DESKTOP_FILE_CONTENT: &str = "[Desktop Entry]
Name=HeadsetControl Tray
Comment=Control your headset from the system tray
Exec=headsetcontrol-tray
Icon=headset
Terminal=false
Type=Application
Categories=Settings;HardwareSettings;
StartupNotify=false
X-GNOME-Autostart-enabled=true
";

fn autostart_path() -> Option<PathBuf> {
    let config_dir = dirs_next::config_dir()?;
    Some(config_dir.join("autostart").join(DESKTOP_FILE_NAME))
}

pub fn is_autostart_enabled() -> bool {
    autostart_path()
        .map(|p| p.exists())
        .unwrap_or(false)
}

pub fn set_autostart(enabled: bool) -> Result<(), String> {
    let path = autostart_path().ok_or("Could not determine autostart directory")?;

    if enabled {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&path, DESKTOP_FILE_CONTENT).map_err(|e| e.to_string())?;
    } else if path.exists() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }

    Ok(())
}
