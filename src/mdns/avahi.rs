use std::{
    convert::TryFrom,
    fmt::Display,
    hash::Hash,
    io::BufRead as _,
    process::Stdio,
    string::FromUtf8Error,
    sync::{Arc, RwLock},
    thread::JoinHandle,
};

use itertools::Itertools as _;
use url::Url;

use crate::{find_executable, FindExecError, MdnsPacket, PacketParseError};

const ELGATO_SERVICE_ID: &str = "_elg._tcp";

#[derive(Debug, thiserror::Error)]
pub enum DiscoverError {
    #[error(transparent)]
    FindExec(#[from] FindExecError),
    #[error("avahi-browse not installed")]
    AvahiBrowseNotInstalled,
    #[error("avahi-browse error: {0}")]
    AvahiBrowseError(std::io::Error),
    #[error("Output parse error: {0}")]
    OutputParse(FromUtf8Error),
    #[error(transparent)]
    Parse(#[from] PacketParseError),
}

pub async fn exec_avahi_browse(filter: Option<&str>) -> Result<Vec<MdnsPacket>, DiscoverError> {
    if find_executable("avahi-browse").await?.is_none() {
        return Err(DiscoverError::AvahiBrowseNotInstalled);
    }

    let output = tokio::process::Command::new("avahi-browse")
        .arg(filter.unwrap_or_default())
        .arg("--parsable")
        .arg("--resolve")
        .arg("--terminate")
        .output()
        .await
        .map_err(DiscoverError::AvahiBrowseError)?;

    let output = String::from_utf8(output.stdout).map_err(DiscoverError::OutputParse)?;

    Ok(output
        .lines()
        .map(|line| MdnsPacket::try_from(line.to_string()))
        .collect::<Result<Vec<_>, _>>()?)
}

#[derive(Debug, Clone)]
pub struct Device {
    pub name: String,
    pub url: Url,
}

impl PartialEq for Device {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Device {}

impl Hash for Device {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state)
    }
}

impl Display for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} => {}", self.name, self.url)
    }
}

impl Device {
    pub fn from_packet(packet: MdnsPacket) -> Result<Option<Self>, url::ParseError> {
        match packet {
            MdnsPacket::New(_) | MdnsPacket::Exited(_) => Ok(None),
            MdnsPacket::Resolved { base, service } => {
                let url = Url::parse(&format!("http://{}:{}", service.ip, service.port))?;
                Ok(Some(Device {
                    name: base.hostname,
                    url,
                }))
            }
        }
    }
}

#[derive(Debug)]
pub struct AvahiState {
    pub devices: Vec<Device>,
}

impl AvahiState {
    pub fn process_packet(&mut self, packet: MdnsPacket) -> Result<(), url::ParseError> {
        match packet {
            MdnsPacket::New(_) => (),
            MdnsPacket::Resolved { .. } => {
                let new_device = Device::from_packet(packet)?.unwrap();
                if !self.devices.iter().any(|device| device == &new_device) {
                    log::info!("New device found: {new_device}");
                    self.devices.push(new_device);
                } else {
                    log::debug!("Device {new_device} already in the state");
                }
            }
            MdnsPacket::Exited(base) => {
                if let Some(idx) = self
                    .devices
                    .iter()
                    // I hope hostname are unique
                    .position(|device| device.name != base.hostname)
                {
                    self.devices.remove(idx);
                }
            }
        }

        Ok(())
    }
}

pub fn spawn_avahi_daemon(state: Arc<RwLock<AvahiState>>) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let child = std::process::Command::new("avahi-browse")
            .arg("--parsable")
            .arg("--resolve")
            .arg(ELGATO_SERVICE_ID)
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to spawn avahi-browse subprocess");

        let stream = child
            .stdout
            .expect("Failed to get stdout of avahi-browse subprocess");
        let stream = std::io::BufReader::new(stream);
        let stream = stream.lines();

        for line in stream {
            let line = line.expect("Failed to read line from avahi-browse subprocess");

            match MdnsPacket::try_from(line.to_string()) {
                Ok(packet) => {
                    log::info!("mDNS packet received: {:#?}", packet);
                    let mut state = state.write().expect("lock already held by current thread");
                    if let Err(err) = state.process_packet(packet) {
                        log::error!("Process packat failed: {}", err);
                    }
                }
                Err(err) => {
                    log::error!("Failed to parse packet: {}", err);
                }
            }
        }
    })
}

pub async fn find_elgato_devices() -> Result<Vec<Device>, DiscoverError> {
    Ok(exec_avahi_browse(ELGATO_SERVICE_ID.into())
        .await?
        .into_iter()
        .filter_map(|packet| {
            Device::from_packet(packet).unwrap_or_else(|err| {
                // Light started returning `fe80::3e6a:9dff:fe21:b16` instead of `192.168.0.92`
                log::error!("Couldn't parse url: {err}");
                None
            })
        })
        .unique()
        .collect::<Vec<Device>>())
}
