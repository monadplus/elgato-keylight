use std::{fs::File, io::Write, net::IpAddr, path::PathBuf, string::FromUtf8Error};

use reqwest::Url;
use tokio::process::Command;

mod avahi_browse;
mod http;
mod keylight;
mod unsigned_int;

pub use avahi_browse::*;
pub use http::*;
pub use keylight::*;
pub use unsigned_int::*;

const KEYLIGHT_API_PATH: &str = "elgato/lights";

#[derive(Debug, thiserror::Error)]
pub enum FindExecError {
    #[error(transparent)]
    OutputParse(#[from] FromUtf8Error),
    #[error(transparent)]
    IO(#[from] std::io::Error),
}

pub fn get_keylight_url(host: IpAddr, port: u16) -> Result<Url, url::ParseError> {
    let base = Url::parse(&format!("http://{}:{}", host, port))?;
    base.join(KEYLIGHT_API_PATH)
}

pub async fn find_executable(executable: &str) -> Result<Option<PathBuf>, FindExecError> {
    match Command::new("which").arg(executable).output().await {
        Ok(output) => Ok(Some(PathBuf::from(String::from_utf8(output.stdout)?))),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(FindExecError::IO(err)),
    }
}

pub async fn notify(msg: &str) -> anyhow::Result<()> {
    if find_executable("notify-send").await?.is_none() {
        println!("{msg}");
        return Ok(());
    }

    let dir = tempfile::tempdir()?;
    let path = dir.path().join("elgato_logo.png");
    let mut file = File::create(&path)?;
    let bytes = include_bytes!("../assets/elgato_logo.png");
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

pub async fn toggle_power(url: Url) -> anyhow::Result<PowerStatus> {
    let mut status = get_status(url.clone()).await?;
    let mut new = PowerStatus::On;
    status.set(0, |status| {
        status.power.toggle();
        new = status.power;
    })?;
    notify(&format!("Turned {}", new)).await?;
    set_status(url, status).await?;
    Ok(new)
}

pub enum Delta {
    Incr,
    Decr,
}

pub const BRIGHTNESS_DELTA_VALUE: u8 = 10;
pub const TEMPERATURE_DELTA_VALUE: u16 = 20;

pub async fn incr_brightness(url: Url, delta: Delta) -> anyhow::Result<()> {
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

pub async fn incr_temperature(url: Url, delta: Delta) -> anyhow::Result<()> {
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
