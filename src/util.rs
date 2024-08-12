use std::{fs::File, io::Write as _, path::PathBuf, string::FromUtf8Error};

use log::{error, info};
use tokio::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum FindExecError {
    #[error(transparent)]
    OutputParse(#[from] FromUtf8Error),
    #[error(transparent)]
    IO(#[from] std::io::Error),
}

/// Find executable in process PATH
pub async fn find_executable(executable: &str) -> Result<Option<PathBuf>, FindExecError> {
    match Command::new("which").arg(executable).output().await {
        Ok(output) => Ok(Some(PathBuf::from(String::from_utf8(output.stdout)?))),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(FindExecError::IO(err)),
    }
}

/// Notify to the user using `libnotify`
pub async fn notify(msg: &str) -> anyhow::Result<()> {
    if find_executable("notify-send").await?.is_none() {
        info!("notify-send not found. Using stdout");
        println!("{msg}");
        return Ok(());
    }

    let Ok(icon_path) = inject_icon() else {
        error!("Inject icon failed. Using stdout");
        println!("{msg}");
        return Ok(());
    };

    if let Err(err) = Command::new("notify-send")
        .arg(format!("--icon={}", icon_path.display()))
        .arg("Key Light Controller")
        .arg(msg)
        .output()
        .await
    {
        error!("`notify-send` failed: {err}. Using stdout");
        println!("{msg}");
    }

    Ok(())
}

fn inject_icon() -> anyhow::Result<PathBuf> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("elgato_logo.png");
    let mut file = File::create(&path)?;

    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/elgato_logo.png"
    ));
    file.write_all(bytes)?;
    file.flush()?;
    Ok(path)
}
