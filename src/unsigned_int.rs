use std::{num::ParseIntError, str::FromStr};

use serde::{de::Error, Deserialize, Serialize};

pub type Brightness = UnsignedInt<u8, 0, 100>;

pub type Temperature = UnsignedInt<u16, 143, 344>;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(transparent)]
pub struct UnsignedInt<I, const S: usize, const E: usize>(I);

impl<const S: usize, const E: usize, I: std::fmt::Debug + Copy + PartialEq + Into<usize>>
    UnsignedInt<I, S, E>
{
    pub fn new(i: I) -> Result<Self, String> {
        let n: usize = i.into();
        if n < S || n > E {
            return Err(format!("Outside range [{}, {}]", S, E));
        }
        Ok(UnsignedInt(i))
    }
}

impl<
        const S: usize,
        const E: usize,
        I: std::fmt::Debug + Copy + PartialEq + FromStr<Err = ParseIntError> + Into<usize>,
    > FromStr for UnsignedInt<I, S, E>
{
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        UnsignedInt::new(I::from_str(s).map_err(|e| format!("{e}"))?)
    }
}

impl<
        'de,
        const S: usize,
        const E: usize,
        I: std::fmt::Debug + Copy + PartialEq + Deserialize<'de> + Into<usize>,
    > Deserialize<'de> for UnsignedInt<I, S, E>
{
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let inner = I::deserialize(d)?;
        UnsignedInt::new(inner).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsigned_int() {
        let x: Result<UnsignedInt<u8, 5, 10>, _> = UnsignedInt::new(6);
        assert!(x.is_ok());

        let x: Result<UnsignedInt<u8, 5, 10>, _> = UnsignedInt::new(3);
        assert!(x.is_err());
    }
}
