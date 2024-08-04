use std::net::IpAddr;

use clap::{Parser, Subcommand};

use reqwest::Url;

use elgato_keylight::*;

pub const BRIGHTNESS_DELTA_VALUE: u8 = 10;
pub const TEMPERATURE_DELTA_VALUE: u16 = 20;

/// Elgato Keylight controller
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// IP address
    #[arg(long)]
    ip: IpAddr,
    /// API port
    #[arg(long)]
    port: u16,
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

    let url = Url::parse(&format!("http://{}:{}", args.ip, args.port))?;

    match args.command {
        Commands::Toggle => {
            toggle_power(url).await?;
        }
        Commands::Status => {
            let status = get_status(url.clone()).await?;
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
        Commands::IncrBrightness => incr_brightness(url, Delta::Incr).await?,
        Commands::DecrBrightness => incr_brightness(url, Delta::Decr).await?,
        Commands::IncrTemperature => incr_temperature(url, Delta::Incr).await?,
        Commands::DecrTemperature => incr_temperature(url, Delta::Incr).await?,
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

/// Toggle device power
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

/// Increase device brightness by delta
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

/// Increase device temperature by delta
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
