use std::{fs::File, io::Write, path::PathBuf};

use anyhow::bail;
use tokio::process::Command;

mod avahi_browse;
mod http;
mod keylight;
mod unsigned_int;

pub use avahi_browse::discover_elgato_devices;
pub use http::{get_status, set_status};
pub use keylight::{DeviceStatus, KeyLightStatus, PowerStatus};
pub use unsigned_int::{Brightness, Temperature};

pub async fn find_executable(executable: &str) -> anyhow::Result<Option<PathBuf>> {
    match Command::new("which").arg(executable).output().await {
        Ok(output) => Ok(Some(PathBuf::from(String::from_utf8(output.stdout)?))),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => bail!(err),
    }
}

pub async fn notify(msg: &str) -> anyhow::Result<()> {
    if find_executable("notify-send").await?.is_none() {
        println!("{msg}");
        return Ok(());
    }

    let dir = tempfile::tempdir()?;
    let path = dir.path().join("elgato.png");
    let mut file = File::create(&path)?;
    let bytes = include_bytes!("../assets/elgato.png");
    file.write_all(bytes)?;
    file.flush()?;

    Command::new("notify-send")
        .arg(format!("--icon={}", path.display()))
        .arg("Key Light Controller")
        .arg(msg)
        .output()
        .await?;

    Ok(())
}

pub async fn toggle_power(url: reqwest::Url) -> anyhow::Result<()> {
    let mut status = get_status(url.clone()).await?;
    let mut new = PowerStatus::On;
    status.set(0, |status| {
        status.power.toggle();
        new = status.power;
    })?;
    notify(&format!("Turned {}", new)).await?;
    let _ = reqwest::Client::new().put(url).json(&status).send().await?;
    Ok(())
}

pub enum Delta {
    Incr,
    Decr,
}

pub const BRIGHTNESS_DELTA_VALUE: u8 = 10;
pub const TEMPERATURE_DELTA_VALUE: u16 = 20;

pub async fn brightness(url: reqwest::Url, delta: Delta) -> anyhow::Result<()> {
    let mut status = get_status(url.clone()).await?;
    status.set(0, |status| {
        let new_raw_value = match delta {
            Delta::Incr => status.brightness.0.saturating_add(BRIGHTNESS_DELTA_VALUE),
            Delta::Decr => status.brightness.0.saturating_sub(BRIGHTNESS_DELTA_VALUE),
        };
        if let Ok(new_brightness) = Brightness::new(new_raw_value) {
            status.brightness = new_brightness;
        }
    })?;
    let _ = reqwest::Client::new().put(url).json(&status).send().await?;
    Ok(())
}

pub async fn temperature(url: reqwest::Url, delta: Delta) -> anyhow::Result<()> {
    let mut status = get_status(url.clone()).await?;
    status.set(0, |status| {
        let new_raw_value = match delta {
            Delta::Incr => status.temperature.0.saturating_add(TEMPERATURE_DELTA_VALUE),
            Delta::Decr => status.temperature.0.saturating_sub(TEMPERATURE_DELTA_VALUE),
        };
        if let Ok(new_temperature) = Temperature::new(new_raw_value) {
            status.temperature = new_temperature;
        }
    })?;
    let _ = reqwest::Client::new().put(url).json(&status).send().await?;
    Ok(())
}
