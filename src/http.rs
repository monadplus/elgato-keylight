use crate::DeviceStatus;

pub async fn get_status(url: reqwest::Url) -> anyhow::Result<DeviceStatus> {
    Ok(reqwest::get(url).await?.json::<DeviceStatus>().await?)
}

pub async fn set_status(url: reqwest::Url, status: DeviceStatus) -> anyhow::Result<()> {
    let _ = reqwest::Client::new().put(url).json(&status).send().await?;
    Ok(())
}
