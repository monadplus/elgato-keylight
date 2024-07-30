use anyhow::bail;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::unsigned_int::{Brightness, Temperature};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStatus {
    pub number_of_lights: usize,
    pub lights: Vec<KeyLightStatus>,
}

#[derive(Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq, Debug)]
#[repr(u8)]
pub enum PowerStatus {
    Off = 0,
    On = 1,
}

impl PowerStatus {
    pub fn toggle(&mut self) {
        let toggled = match self {
            PowerStatus::Off => PowerStatus::On,
            PowerStatus::On => PowerStatus::Off,
        };
        *self = toggled;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyLightStatus {
    #[serde(rename = "on")]
    pub power: PowerStatus,
    pub brightness: Brightness,
    pub temperature: Temperature,
}

impl DeviceStatus {
    pub fn set<F>(&mut self, index: usize, update: F) -> anyhow::Result<()>
    where
        F: FnOnce(&mut KeyLightStatus),
    {
        if index > self.number_of_lights - 1 {
            bail!("Invalid index");
        }
        update(self.lights.get_mut(index).unwrap());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::unsigned_int::UnsignedInt;

    use super::*;

    #[test]
    fn serde() {
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
