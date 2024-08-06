#![allow(clippy::redundant_closure)]

use super::*;
// #[cfg(feature = "std")]
use {
    serde::{Deserializer, Serializer},
    sp_std::str::FromStr,
};

use scale_info::prelude::string::String;
use serde::{Deserialize, Serialize};

/// (De)serialization implementation for AssetSymbol
impl Serialize for AssetSymbol {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}", self))
    }
}

impl<'de> Deserialize<'de> for AssetSymbol {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s).map_err(|str_err| serde::de::Error::custom(str_err))
    }
}

/// (De)serialization implementation for BalanceWrapper
#[cfg(feature = "std")]
impl Serialize for BalanceWrapper {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}", self.0))
    }
}

#[cfg(feature = "std")]
impl<'de> Deserialize<'de> for BalanceWrapper {
    fn deserialize<D>(deserializer: D) -> Result<BalanceWrapper, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let inner = Balance::from_str(&s).map_err(|str_err| serde::de::Error::custom(str_err))?;
        Ok(BalanceWrapper(inner))
    }
}

/// (De)serialization implementation for AssetName
impl Serialize for AssetName {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}", self))
    }
}

impl<'de> Deserialize<'de> for AssetName {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s).map_err(|str_err| serde::de::Error::custom(str_err))
    }
}

/// (De)serialization implementation for ContentSource
impl Serialize for ContentSource {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}", self))
    }
}

impl<'de> Deserialize<'de> for ContentSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s).map_err(|str_err| serde::de::Error::custom(str_err))
    }
}

/// (De)serialization implementation for Description
impl Serialize for Description {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}", self))
    }
}

impl<'de> Deserialize<'de> for Description {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s).map_err(|str_err| serde::de::Error::custom(str_err))
    }
}

/// (De)serialization implementation for AssetId32
impl<AssetId> Serialize for AssetId32<AssetId> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}", self))
    }
}

impl<'de, AssetId> Deserialize<'de> for AssetId32<AssetId> {
    fn deserialize<D>(deserializer: D) -> Result<AssetId32<AssetId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        AssetId32::from_str(&s).map_err(|str_err| serde::de::Error::custom(str_err))
    }
}

/// (De)serialization implementation for AssetId32
impl Serialize for SymbolName {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}", self))
    }
}

impl<'de> Deserialize<'de> for SymbolName {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s).map_err(|str_err| serde::de::Error::custom(str_err))
    }
}

/// (De)serialization implementation for AssetId32
#[cfg(feature = "std")]
impl Serialize for CrowdloanTag {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}", self))
    }
}

#[cfg(feature = "std")]
impl<'de> Deserialize<'de> for CrowdloanTag {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s).map_err(|str_err| serde::de::Error::custom(str_err))
    }
}
