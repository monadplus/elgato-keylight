use clap::{Parser, Subcommand};

use elgato_keylight::*;

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
