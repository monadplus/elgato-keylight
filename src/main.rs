use std::{fs::File, io::Write};

use clap::{Parser, Subcommand};

mod keylight;
mod unsigned_int;

pub use keylight::{DeviceStatus, KeyLightStatus, PowerStatus};
use tempfile::tempdir;
use tokio::process::Command;
pub use unsigned_int::{Brightness, Temperature};

// TODO:
// * UI with iced
// * discover lights

const KEYLIGHT_API_PATH: &str = "elgato/lights";

/// Elgato Keylight controller
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// IP address
    #[arg(long)]
    host: String,
    /// API port
    #[arg(long)]
    port: String,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Status: on/off, brightness, temperature, etc.
    Status,
    /// Toggle (on/off)
    Toggle,
    /// Increase brightness by 10%
    IncrBrightness,
    /// Decrease brightness by 10%
    DecrBrightness,
    /// Increase temperature by 10%
    IncrTemperature,
    /// Decrease temperature by 10%
    DecrTemperature,
    /// Set values for brightness and temperature
    Set(SetArgs),
}

#[derive(Debug, clap::Args)]
#[group(required = true, multiple = true)]
pub struct SetArgs {
    #[arg(short, long)]
    brightness: Option<Brightness>,
    #[arg(short, long)]
    temperature: Option<Temperature>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let base = reqwest::Url::parse(&format!("http://{}:{}", args.host, args.port))?;
    let url = base.join(KEYLIGHT_API_PATH)?;

    match args.command {
        Commands::Toggle => {
            toggle_power(url).await?;
        }
        Commands::Status => {
            let status = get_status(url.clone()).await?;
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
        Commands::IncrBrightness => brightness(url, Delta::Incr).await?,
        Commands::DecrBrightness => brightness(url, Delta::Decr).await?,
        Commands::IncrTemperature => temperature(url, Delta::Incr).await?,
        Commands::DecrTemperature => temperature(url, Delta::Incr).await?,
        Commands::Set(SetArgs {
            brightness,
            temperature,
        }) => {
            let mut status = get_status(url.clone()).await?;
            status.set(0, move |status| {
                status.brightness = brightness.unwrap_or(status.brightness);
                status.temperature = temperature.unwrap_or(status.temperature);
            })?;
            let _ = reqwest::Client::new().put(url).json(&status).send().await?;
        }
    }

    Ok(())
}

async fn get_status(url: reqwest::Url) -> anyhow::Result<DeviceStatus> {
    Ok(reqwest::get(url).await?.json::<DeviceStatus>().await?)
}

async fn toggle_power(url: reqwest::Url) -> anyhow::Result<()> {
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

enum Delta {
    Incr,
    Decr,
}

const BRIGHTNESS_DELTA_VALUE: u8 = 10;
const TEMPERATURE_DELTA_VALUE: u16 = 20;

async fn brightness(url: reqwest::Url, delta: Delta) -> anyhow::Result<()> {
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

async fn temperature(url: reqwest::Url, delta: Delta) -> anyhow::Result<()> {
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

async fn notify(msg: &str) -> anyhow::Result<()> {
    let dir = tempdir()?;
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
