use clap::{Parser, Subcommand};

mod keylight;
mod unsigned_int;

pub use keylight::{DeviceStatus, KeyLightStatus, PowerStatus};
pub use unsigned_int::{Brightness, Temperature};

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
    /// Status: on/off, brightness, temperature, etc.
    Status,
    /// Toggle (on/off)
    Toggle,
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

// 0-100, increments/decrements by 10 units
const BRIGHTNESS_DELTA_VALUE: u8 = 10;

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
        Commands::Incr => brightness(url, Delta::Incr).await?,
        Commands::Decr => brightness(url, Delta::Decr).await?,
        Commands::Set {
            brightness: _,
            temperature: _,
        } => todo!(),
    }

    Ok(())
}

async fn get_status(url: reqwest::Url) -> anyhow::Result<DeviceStatus> {
    Ok(reqwest::get(url).await?.json::<DeviceStatus>().await?)
}

async fn toggle_power(url: reqwest::Url) -> anyhow::Result<()> {
    let mut status = get_status(url.clone()).await?;
    status.set(0, |status| {
        status.power.toggle();
        println!("Keylight turned {}", status.power)
    })?;
    let _ = reqwest::Client::new().put(url).json(&status).send().await?;
    Ok(())
}

enum Delta {
    Incr,
    Decr,
}

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
