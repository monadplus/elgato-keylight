use std::panic;
use std::{convert::TryFrom, net::IpAddr, str::FromStr};

use anyhow::bail;
use regex::{Captures, Regex};
use tokio::process::Command;

use crate::find_executable;

const ELGATO_SERVICE_ID: &str = "_elg._tcp";

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum PacketParseError {
    #[error("Failed to parse mode: {0}")]
    ModeParse(char),
    #[error("Failed to parse internet protocol: {0}")]
    IpTypeParse(String),
    #[error("Not enough arguments")]
    NotEnoughArgs,
    #[error(transparent)]
    AddrParse(#[from] std::net::AddrParseError),
    #[error(transparent)]
    IntParse(#[from] std::num::ParseIntError),
}

pub async fn discover_elgato_devices() -> anyhow::Result<Vec<MdnsPacket>> {
    if find_executable("avahi-browse").await?.is_none() {
        bail!("avahi-browse not installed");
    }

    let output = Command::new("avahi-browse")
        .arg(ELGATO_SERVICE_ID)
        .arg("--parsable")
        .arg("--resolve")
        .arg("--terminate")
        .output()
        .await?;
    let output = String::from_utf8(output.stdout)?;
    Ok(output
        .lines()
        .map(|line| MdnsPacket::try_from(line.to_string()))
        .collect::<Result<Vec<_>, _>>()?)
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PacketMode {
    New,
    Resolved,
    Exited,
}

impl TryFrom<char> for PacketMode {
    type Error = PacketParseError;

    fn try_from(c: char) -> Result<Self, Self::Error> {
        match c {
            '+' => Ok(PacketMode::New),
            '=' => Ok(PacketMode::Resolved),
            '-' => Ok(PacketMode::Exited),
            _ => Err(PacketParseError::ModeParse(c)),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum IpType {
    V4,
    V6,
}

impl TryFrom<String> for IpType {
    type Error = PacketParseError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.as_str() {
            "IPv4" => Ok(IpType::V4),
            "IPv6" => Ok(IpType::V6),
            _ => Err(PacketParseError::IpTypeParse(s)),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum MdnsPacket {
    New(MdnsPacketBase),
    Resolved {
        base: MdnsPacketBase,
        service: Service,
    },
    Exited(MdnsPacketBase),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MdnsPacketBase {
    /// The interface the packet was received on
    pub interface_name: String,
    /// The internet protocol type of the packet
    pub internet_protocol: IpType,
    /// The hostname / service name
    pub hostname: String,
    /// The type of the service
    pub service_type: String,
    /// The domain of the service
    pub domain: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Service {
    /// The name of the service
    pub name: String,
    /// The mDNS hostname of the service
    pub hostname: String,
    /// The IP address serving the service
    pub ip: IpAddr,
    /// The port the service is listening on
    pub port: u16,
    /// All additional data
    pub data: Vec<String>,
}

impl TryFrom<String> for MdnsPacket {
    type Error = PacketParseError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        let mut iter = s.split(';');

        let mode = PacketMode::try_from(
            try_unwrap_arg(iter.next())?
                .chars()
                .next()
                .ok_or(PacketParseError::NotEnoughArgs)?,
        )?;

        let interface_name = try_unwrap_arg(iter.next())?.to_string();

        let internet_protocol = IpType::try_from(try_unwrap_arg(iter.next())?.to_string())?;

        let mut hostname = try_unwrap_arg(iter.next())?.to_string().replace("\\.", ".");
        hostname = parse_escaped_ascii(&hostname);

        let service_type = try_unwrap_arg(iter.next())?.to_string();

        let domain = try_unwrap_arg(iter.next())?.to_string();

        let base = MdnsPacketBase {
            interface_name,
            internet_protocol,
            hostname,
            service_type: service_type.clone(),
            domain,
        };

        let mdns_packet = match mode {
            PacketMode::New => Self::New(base),
            PacketMode::Resolved => Self::Resolved {
                base,
                service: Service {
                    name: service_type,
                    hostname: try_unwrap_arg(iter.next())?.to_string(),
                    ip: IpAddr::from_str(try_unwrap_arg(iter.next())?)?,
                    port: u16::from_str(try_unwrap_arg(iter.next())?)?,
                    data: iter.map(|s| s.to_string()).collect(),
                },
            },
            PacketMode::Exited => Self::Exited(base),
        };

        Ok(mdns_packet)
    }
}

fn parse_escaped_ascii(s: &str) -> String {
    let re = Regex::new(r"\\(\d{1,3})").unwrap();
    let replacement = |caps: &Captures| -> String {
        match caps[1].parse::<u8>() {
            Err(_) => panic!("Couldn't parse ascii code as u8"),
            Ok(n) => char::from_u32(n as u32).unwrap().to_string(),
        }
    };
    re.replace_all(s, &replacement).to_string()
}

fn try_unwrap_arg(arg: Option<&str>) -> Result<&str, PacketParseError> {
    arg.ok_or(PacketParseError::NotEnoughArgs)
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    #[test]
    fn parse_escaped_ascii_test() {
        let input = r#"Elgato\032Key\032Light\0328D7C"#;
        assert_eq!(parse_escaped_ascii(input), "Elgato Key Light 8D7C");
    }

    #[test]
    fn parse_mdns_packet_test() {
        let input = r#"+;enp6s0;IPv6;Elgato\032Key\032Light\0328D7C;_elg._tcp;local"#.to_string();
        let res = MdnsPacket::try_from(input);
        assert_eq!(
            res,
            Ok(MdnsPacket::New(MdnsPacketBase {
                interface_name: "enp6s0".to_string(),
                internet_protocol: IpType::V6,
                hostname: "Elgato Key Light 8D7C".to_string(),
                service_type: "_elg._tcp".to_string(),
                domain: "local".to_string(),
            }))
        );

        let input = r#"=;enp6s0;IPv4;Elgato\032Key\032Light\0328D7C;_elg._tcp;local;elgato-key-light-8d7c.local;192.168.0.92;9123;"pv=1.0" "md=Elgato Key Light 20GAK9901" "id=3C:6A:9D:21:B1:6E" "dt=53" "mf=Elgato"#.to_string();
        let res = MdnsPacket::try_from(input);
        assert_eq!(
            res,
            Ok(MdnsPacket::Resolved {
                base: MdnsPacketBase {
                    interface_name: "enp6s0".to_string(),
                    internet_protocol: IpType::V4,
                    hostname: "Elgato Key Light 8D7C".to_string(),
                    service_type: "_elg._tcp".to_string(),
                    domain: "local".to_string(),
                },
                service: Service {
                    name: "_elg._tcp".to_string(),
                    hostname: "elgato-key-light-8d7c.local".to_string(),
                    ip: IpAddr::V4(Ipv4Addr::new(192, 168, 0, 92)),
                    port: 9123,
                    data: vec!(r#""pv=1.0" "md=Elgato Key Light 20GAK9901" "id=3C:6A:9D:21:B1:6E" "dt=53" "mf=Elgato"#.to_string()),
                }
            })
        );
    }
}
