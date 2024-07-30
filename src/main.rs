use std::{num::ParseIntError, str::FromStr};

use anyhow::bail;
use clap::{Parser, Subcommand};
use serde::{de::Error, Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

// TODO:
// * incr/decr
// * set
// * notify-send
// * discover lights

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
        brightness: Option<Brightness>,
        #[arg(short, long)]
        temperature: Option<Temperature>,
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
            let _ = reqwest::Client::new().put(url).json(&status).send().await?;
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
    let _ = reqwest::Client::new().put(url).json(&status).send().await?;
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
    brightness: Brightness,
    temperature: Temperature,
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

    #[allow(dead_code)]
    fn set_power_all(&mut self, status: PowerStatus) {
        (0..self.number_of_lights).for_each(|index| self.set_power(index, status).unwrap())
    }
}

type Brightness = UnsignedInt<u8, 0, 100>;
type Temperature = UnsignedInt<u16, 143, 344>;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(transparent)]
struct UnsignedInt<I, const S: usize, const E: usize>(I);

impl<const S: usize, const E: usize, I: std::fmt::Debug + Copy + PartialEq + Into<usize>>
    UnsignedInt<I, S, E>
{
    pub fn new(i: I) -> Result<Self, String> {
        let n: usize = i.into();
        if n < S || n > E {
            return Err(format!("Outside range [{}, {}]", S, E));
        }
        Ok(UnsignedInt(i))
    }
}

impl<
        const S: usize,
        const E: usize,
        I: std::fmt::Debug + Copy + PartialEq + FromStr<Err = ParseIntError> + Into<usize>,
    > FromStr for UnsignedInt<I, S, E>
{
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        UnsignedInt::new(I::from_str(s).map_err(|e| format!("{e}"))?)
    }
}

impl<
        'de,
        const S: usize,
        const E: usize,
        I: std::fmt::Debug + Copy + PartialEq + Deserialize<'de> + Into<usize>,
    > Deserialize<'de> for UnsignedInt<I, S, E>
{
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let inner = I::deserialize(d)?;
        UnsignedInt::new(inner).map_err(D::Error::custom)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsigned_int() {
        let x: Result<UnsignedInt<u8, 5, 10>, _> = UnsignedInt::new(6);
        assert!(x.is_ok());

        let x: Result<UnsignedInt<u8, 5, 10>, _> = UnsignedInt::new(3);
        assert!(x.is_err());
    }

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
                    brightness: UnsignedInt::new(3).unwrap(),
                    temperature: UnsignedInt::new(191).unwrap(),
                }),
            }
        );

        let obj = serde_json::json!({
            "numberOfLights":1,
            "lights":[{"on":1,"brightness":-1,"temperature":360}]
        });
        assert!(serde_json::from_value::<DeviceStatus>(obj).is_err());
    }
}
