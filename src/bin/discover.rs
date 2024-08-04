#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let devices = elgato_keylight::avahi::find_elgato_devices().await?;
    for device in devices {
        println!("{device}")
    }
    Ok(())
}
