use core::result::Result;

use parity_scale_codec::{
    Compact, CompactAs, Decode, DecodeWithMemTracking, Encode, EncodeLike, Error, Input, Output,
};
use static_assertions::{assert_eq_align, assert_eq_size};

use crate::FixedPoint;

#[cfg_attr(docsrs, doc(cfg(feature = "parity")))]
impl<I, P> From<Compact<Self>> for FixedPoint<I, P> {
    #[inline]
    fn from(value: Compact<Self>) -> Self {
        value.0
    }
}

macro_rules! impl_codec {
    ($layout:ty, $representation:ty) => {
        impl_codec!($layout, $representation,);
    };
    ($layout:ty, $representation:ty, $(#[$attr:meta])?) => {
        #[cfg_attr(docsrs, doc(cfg(feature = "parity")))]
        $(#[$attr])?
        impl<P> EncodeLike for FixedPoint<$layout, P> {}

        #[cfg_attr(docsrs, doc(cfg(feature = "parity")))]
        $(#[$attr])?
        impl<P> Encode for FixedPoint<$layout, P> {
            #[inline]
            fn encode_to<O: Output + ?Sized>(&self, destination: &mut O) {
                destination.write(&self.encode_as().encode());
            }
        }

        #[cfg_attr(docsrs, doc(cfg(feature = "parity")))]
        $(#[$attr])?
        impl<P> Decode for FixedPoint<$layout, P> {
            #[inline]
            fn decode<In: Input>(input: &mut In) -> Result<Self, Error> {
                let fp = <$representation as Decode>::decode(input)
                    .and_then(FixedPoint::decode_from)
                    .map_err(|_| "Error decoding FixedPoint value")?;
                Ok(fp)
            }
        }

        #[cfg_attr(docsrs, doc(cfg(feature = "parity")))]
        $(#[$attr])?
        impl<P> DecodeWithMemTracking for FixedPoint<$layout, P> {}

        #[cfg_attr(docsrs, doc(cfg(feature = "parity")))]
        $(#[$attr])?
        impl<P> CompactAs for FixedPoint<$layout, P> {
            type As = $representation;

            #[inline]
            fn encode_as(&self) -> &Self::As {
                assert_eq_size!($layout, $representation);
                assert_eq_align!($layout, $representation);
                // Representative type has the same size and memory layout so this cast is actually
                // safe.
                // TODO: Related issue: https://github.com/paritytech/parity-scale-codec/issues/205
                unsafe { &*(self.as_bits() as *const $layout as *const $representation) }
            }

            #[inline]
            fn decode_from(value: Self::As) -> Result<Self, parity_scale_codec::Error> {
                Ok(Self::from_bits(value as $layout))
            }
        }
    };
}

#[cfg(feature = "i16")]
impl_codec!(i16, u16, #[cfg_attr(docsrs, doc(cfg(feature = "i16")))]);
#[cfg(feature = "i32")]
impl_codec!(i32, u32, #[cfg_attr(docsrs, doc(cfg(feature = "i32")))]);
#[cfg(feature = "i64")]
impl_codec!(i64, u64, #[cfg_attr(docsrs, doc(cfg(feature = "i64")))]);
#[cfg(feature = "i128")]
impl_codec!(i128, u128, #[cfg_attr(docsrs, doc(cfg(feature = "i128")))]);
