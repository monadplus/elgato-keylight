use anyhow::bail;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

const KEYLIGHT_API_PATH: &str = "elgato/lights";

/// Elgato Keylight controller using the http API.
/// Use `avahi-browse -t _elg._tcp --resolve` to discover the IP of your device.
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
    /// Turn on
    On,
    /// Turn off
    Off,
    /// Toggle (on/off)
    Toggle,
    /// Status
    Status,
    /// Increase brightness by 10%
    Incr,
    /// Decrease brightness by 10%
    Decr,
    /// Set value
    Set {
        #[arg(short, long)]
        brightness: Option<u8>,
        #[arg(short, long)]
        temperature: Option<u16>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let base = reqwest::Url::parse(&format!("http://{}:{}", args.host, args.port))?;
    let url = base.join(KEYLIGHT_API_PATH)?;

    // curl 192.168.0.92:9123/elgato/lights -XPUT -H 'Content-Type: application/json' -d '{"numberOfLights":1,"lights":[{"temperature":344}]}'

    match args.command {
        Commands::On => {
            power(url.clone(), PowerStatus::On).await?;
            println!("Keylight turned on");
        }
        Commands::Off => {
            power(url.clone(), PowerStatus::Off).await?;
            println!("Keylight turned off");
        }
        Commands::Toggle => {
            let mut status = status(url.clone()).await?;
            status.toggle_power(0)?;
            let _ = reqwest::Client::new()
                .put(url)
                .json(&status)
                .send()
                .await?;
            println!("Keylight toggled");
        }
        Commands::Status => {
            let status = status(url.clone()).await?;
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
        Commands::Incr => todo!(),
        Commands::Decr => todo!(),
        Commands::Set {
            brightness,
            temperature,
        } => todo!(),
    }

    Ok(())
}

async fn status(url: reqwest::Url) -> anyhow::Result<DeviceStatus> {
    Ok(reqwest::get(url).await?.json::<DeviceStatus>().await?)
}

async fn power(url: reqwest::Url, power: PowerStatus) -> anyhow::Result<()> {
    let mut status = status(url.clone()).await?;
    status.set_power(0, power)?;
    let _ = reqwest::Client::new()
        .put(url)
        .json(&status)
        .send()
        .await?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceStatus {
    number_of_lights: usize,
    lights: Vec<KeyLightStatus>,
}

#[derive(Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq, Debug)]
#[repr(u8)]
enum PowerStatus {
    Off = 0,
    On = 1,
}

impl PowerStatus {
    fn toggle(&mut self) {
        let toggled = match self {
            PowerStatus::Off => PowerStatus::On,
            PowerStatus::On => PowerStatus::Off,
        };
        *self = toggled;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KeyLightStatus {
    #[serde(rename = "on")]
    power: PowerStatus,
    // TODO: range 0-100
    brightness: u8,
    // TODO: range 143-344
    temperature: u16,
}

impl DeviceStatus {
    fn set_power(&mut self, index: usize, status: PowerStatus) -> anyhow::Result<()> {
        if index > self.number_of_lights - 1 {
            bail!("Invalid index");
        }

        let light = self.lights.get_mut(index).unwrap();
        light.power = status;

        Ok(())
    }

    fn toggle_power(&mut self, index: usize) -> anyhow::Result<()> {
        if index > self.number_of_lights - 1 {
            bail!("Invalid index");
        }

        let light = self.lights.get_mut(index).unwrap();
        light.power.toggle();

        Ok(())
    }

    fn set_power_all(&mut self, status: PowerStatus) {
        (0..self.number_of_lights).for_each(|index| self.set_power(index, status).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deser() {
        let obj = serde_json::json!({
            "numberOfLights":1,
            "lights":[{"on":1,"brightness":3,"temperature":191}]
        });
        let status = serde_json::from_value::<DeviceStatus>(obj).unwrap();
        assert_eq!(
            status,
            DeviceStatus {
                number_of_lights: 1,
                lights: vec!(KeyLightStatus {
                    power: PowerStatus::On,
                    brightness: 3,
                    temperature: 191,
                }),
            }
        )
    }
}
