// Copyright (C) 2024 Parity Technologies (UK) Ltd. (admin@parity.io)
// This file is a part of the scale-info-legacy crate.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//         http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! This module provides the name used to insert types in a registry.

use crate::lookup_name::{LookupName, LookupNameDef};
use alloc::borrow::Cow;
use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::{String, ToString};
use core::fmt::Write;
use smallvec::SmallVec;

/// An error constructing an [`InsertName`].
#[allow(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Failed to parse the string. Expected something like 'Foo' or 'Bar<A, B>'.")]
    Invalid,
    #[error("Expected the generic params to be names like 'A' or 'B', not arrays or tuples.")]
    ExpectingNamedParam,
    #[error("Expected the generic params to be capitalized.")]
    ExpectingUppercaseParams,
}

/// A name used as a key when inserting a type into a [`crate::TypeRegistry`].
///
/// # Example
///
/// ```rust
/// use scale_info_legacy::InsertName;
///
/// // Names can be plain identifiers:
/// InsertName::parse("Foo").unwrap();
/// // Or they can have generic parameters which
/// // will be resolved during lookup:
/// InsertName::parse("Bar<A,B>").unwrap();
/// ```
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InsertName {
    pub(crate) name: String,
    pub(crate) params: SmallVec<[String; 4]>,
    pub(crate) pallet: Option<String>,
}

impl InsertName {
    /// Parse a string into an [`InsertName`].
    pub fn parse(s: &str) -> Result<Self, ParseError> {
        let ty_name = LookupName::parse(s).map_err(|_| ParseError::Invalid)?;
        ty_name.try_into()
    }

    /// Scope the inserted type to being within the given pallet. Only lookups that are
    /// also performed within this pallet will make use of this type.
    pub fn in_pallet(mut self, pallet_name: impl Into<String>) -> InsertName {
        self.pallet = Some(pallet_name.into());
        self
    }
}

impl core::str::FromStr for InsertName {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl core::convert::TryFrom<LookupName> for InsertName {
    type Error = ParseError;
    fn try_from(mut ty_name: LookupName) -> Result<Self, Self::Error> {
        let pallet = ty_name.take_pallet();

        // We only accept named types like Foo<A, B> or path::to::Bar.
        let LookupNameDef::Named(named_ty) = ty_name.def() else {
            return Err(ParseError::Invalid);
        };

        let name = named_ty.name().to_owned();
        let params = named_ty
            .param_defs()
            .map(|param| {
                // Params must be simple names and not array/tuples.
                let LookupNameDef::Named(name) = param else {
                    return Err(ParseError::ExpectingNamedParam);
                };
                // Param names must be capitalized because they represent generics.
                if name.name().starts_with(|c: char| c.is_lowercase()) {
                    return Err(ParseError::ExpectingUppercaseParams);
                }
                Ok(name.name().to_owned())
            })
            .collect::<Result<_, _>>()?;

        Ok(InsertName {
            name,
            params,
            pallet,
        })
    }
}

impl core::convert::TryFrom<&str> for InsertName {
    type Error = ParseError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl core::convert::TryFrom<String> for InsertName {
    type Error = ParseError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl core::fmt::Debug for InsertName {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self)
    }
}

impl core::fmt::Display for InsertName {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.name)?;
        if !self.params.is_empty() {
            f.write_char('<')?;
            for (idx, param) in self.params.iter().enumerate() {
                if idx != 0 {
                    f.write_str(", ")?;
                }
                f.write_str(param)?;
            }
            f.write_char('>')?;
        }
        Ok(())
    }
}

impl serde::Serialize for InsertName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for InsertName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;
        let s = <Cow<'de, str>>::deserialize(deserializer)?;
        InsertName::parse(&s)
            .map_err(|e| D::Error::custom(format!("Could not deserialize into InsertName: {e}")))
    }
}
