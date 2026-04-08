//! A module that contains instances of `Serialize` and `Deserialize` for `FixedPoint`.
//! Also contains submodule that can be provided to `serde(with)` in order to
//! change the implementation.
//!
//! By default `FixedPoint` is serialized using `as_string` for human readable formats
//! and `as_repr` for other ones.

use core::{
    convert::TryFrom,
    fmt::{self, Display},
    marker::PhantomData,
    str::{self, FromStr},
};

use serde::{
    de::{self, Error as _},
    Deserialize, Deserializer, Serialize, Serializer,
};

use crate::{errors::ConvertError, FixedPoint};

impl<I, P> Serialize for FixedPoint<I, P>
where
    I: Serialize,
    Self: Display,
{
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            as_string::serialize(self, serializer)
        } else {
            as_repr::serialize(self, serializer)
        }
    }
}

impl<'de, I, P> Deserialize<'de> for FixedPoint<I, P>
where
    I: Deserialize<'de>,
    Self: FromStr,
{
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            as_string::deserialize(deserializer)
        } else {
            as_repr::deserialize(deserializer)
        }
    }
}

/// (De)serializer `FixedPoint` as inner representation.
pub mod as_repr {
    use super::*;

    #[inline]
    pub fn serialize<I, P, S>(fp: &FixedPoint<I, P>, serializer: S) -> Result<S::Ok, S::Error>
    where
        I: Serialize,
        S: Serializer,
    {
        fp.inner.serialize(serializer)
    }

    #[inline]
    pub fn deserialize<'de, I, P, D>(deserializer: D) -> Result<FixedPoint<I, P>, D::Error>
    where
        I: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        I::deserialize(deserializer).map(FixedPoint::from_bits)
    }
}

/// (De)serializes `FixedPoint` as a string.
pub mod as_string {
    use super::*;

    pub fn serialize<I, P, S>(fp: &FixedPoint<I, P>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        FixedPoint<I, P>: Display,
    {
        // Serialize as a string in case of human readable formats.
        // The maximum length can be calculated as `len(str(-2**bits)) + 1`,
        // where `1` is reserved for `.` after integral part.
        let res = format!("{}", fp);

        serializer.serialize_str(res.as_str())
    }

    pub fn deserialize<'de, I, P, D>(deserializer: D) -> Result<FixedPoint<I, P>, D::Error>
    where
        D: Deserializer<'de>,
        FixedPoint<I, P>: FromStr,
    {
        // Deserialize as a string in case of human readable formats.
        deserializer.deserialize_str(FixedPointVisitor::<I, P>(PhantomData))
    }

    struct FixedPointVisitor<I, P>(PhantomData<(I, P)>);

    impl<'de, I, P> de::Visitor<'de> for FixedPointVisitor<I, P>
    where
        FixedPoint<I, P>: FromStr,
    {
        type Value = FixedPoint<I, P>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a FixedPoint type representing a fixed-point number")
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
            // TODO: parse scientific form.
            // TODO: parse big ones with loss instead of an error.
            value
                .parse()
                .map_err(|_| E::invalid_value(de::Unexpected::Str(value), &self))
        }

        // TODO: visit_f64
    }
}

/// (De)serializes `FixedPoint` as `f64`.
pub mod as_f64 {
    use super::*;

    #[inline]
    pub fn serialize<I, P, S>(fp: &FixedPoint<I, P>, serializer: S) -> Result<S::Ok, S::Error>
    where
        I: Serialize,
        FixedPoint<I, P>: Into<f64> + Clone,
        S: Serializer,
    {
        serializer.serialize_f64(fp.clone().into())
    }

    #[inline]
    pub fn deserialize<'de, I, P, D>(deserializer: D) -> Result<FixedPoint<I, P>, D::Error>
    where
        I: Deserialize<'de>,
        FixedPoint<I, P>: TryFrom<f64, Error = ConvertError>,
        D: Deserializer<'de>,
    {
        let f = f64::deserialize(deserializer)?;

        FixedPoint::<I, P>::try_from(f)
            .map_err(|err| D::Error::invalid_value(de::Unexpected::Float(f), &err.as_str()))
    }
}
