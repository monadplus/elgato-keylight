use elgato_keylight::discover_elgato_devices;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let devices = discover_elgato_devices().await?;

    println!("{devices:#?}");

    Ok(())
}
