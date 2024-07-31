use std::time::Duration;

use crate::DeviceStatus;

const CONNECTION_TIMEOUT: Duration = Duration::from_millis(500);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(1);

fn get_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .connect_timeout(CONNECTION_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
}

pub async fn get_status(url: reqwest::Url) -> anyhow::Result<DeviceStatus> {
    let client = get_client()?;
    let resp = client.get(url).send().await?;
    Ok(resp.json().await?)
}

pub async fn set_status(url: reqwest::Url, status: DeviceStatus) -> anyhow::Result<()> {
    let client = get_client()?;
    let _resp = client.put(url).json(&status).send().await?;
    Ok(())
}
