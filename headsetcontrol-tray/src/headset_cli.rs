use serde::Deserialize;
use tokio::process::Command;

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct BatteryInfo {
    pub status: String,
    pub level: Option<i32>,
    pub voltage_mv: Option<i32>,
    pub time_to_empty_min: Option<i32>,
    pub time_to_full_min: Option<i32>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct DeviceInfo {
    pub status: String,
    pub device: String,
    pub battery: Option<BatteryInfo>,
    pub id_vendor: String,
    pub id_product: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct HeadsetControlOutput {
    pub devices: Vec<DeviceInfo>,
}

pub async fn get_headset_status() -> Result<HeadsetControlOutput, String> {
    let output = Command::new("headsetcontrol")
        .args(&["-o", "json", "-b"])
        .output()
        .await
        .map_err(|e| format!("Failed to execute headsetcontrol: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("headsetcontrol failed: {}", stderr));
    }

    serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse headsetcontrol output: {}", e))
}

pub async fn print_device_info() {
    let _ = Command::new("headsetcontrol")
        .arg("-b")
        .status()
        .await;
}

pub async fn set_sidetone(level: u8, verbose: bool) -> Result<(), String> {
    let mut cmd = Command::new("headsetcontrol");
    cmd.args(&["-s", &level.to_string()]);

    if verbose {
        let status = cmd.status().await
            .map_err(|e| format!("Failed to execute headsetcontrol: {}", e))?;
        if !status.success() {
            return Err("headsetcontrol failed to set sidetone".to_string());
        }
    } else {
        let output = cmd.output().await
            .map_err(|e| format!("Failed to execute headsetcontrol: {}", e))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("headsetcontrol failed to set sidetone: {}", stderr));
        }
    }
    Ok(())
}

pub async fn set_inactive_time(minutes: u8, verbose: bool) -> Result<(), String> {
    let mut cmd = Command::new("headsetcontrol");
    cmd.args(&["-i", &minutes.to_string()]);

    if verbose {
        let status = cmd.status().await
            .map_err(|e| format!("Failed to execute headsetcontrol: {}", e))?;
        if !status.success() {
            return Err("headsetcontrol failed to set inactive time".to_string());
        }
    } else {
        let output = cmd.output().await
            .map_err(|e| format!("Failed to execute headsetcontrol: {}", e))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("headsetcontrol failed to set inactive time: {}", stderr));
        }
    }
    Ok(())
}
