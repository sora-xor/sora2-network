use ethabi::{Event, Param, ParamKind, Token};
use frame_support::log::{debug, warn};
use snowbridge_ethereum::log::Log;
use snowbridge_ethereum::H160;
use sp_core::RuntimeDebug;
use sp_std::convert::TryFrom;
use sp_std::prelude::*;

// Used to decode a raw Ethereum log into an [`Envelope`].
static EVENT_ABI: &Event = &Event {
    signature: "Message(address,uint64,bytes)",
    inputs: &[
        Param {
            kind: ParamKind::Address,
            indexed: false,
        },
        Param {
            kind: ParamKind::Uint(64),
            indexed: false,
        },
        Param {
            kind: ParamKind::Bytes,
            indexed: false,
        },
    ],
    anonymous: false,
};

/// An inbound message that has had its outer envelope decoded.
#[derive(Clone, PartialEq, Eq, RuntimeDebug)]
pub struct Envelope {
    /// The address of the outbound channel on Ethereum that forwarded this message.
    pub channel: H160,
    /// The application on Ethereum where the message originated from.
    pub source: H160,
    /// A nonce for enforcing replay protection and ordering.
    pub nonce: u64,
    /// The inner payload generated from the source application.
    pub payload: Vec<u8>,
}

#[derive(Copy, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct EnvelopeDecodeError;

impl TryFrom<Log> for Envelope {
    type Error = EnvelopeDecodeError;

    fn try_from(log: Log) -> Result<Self, Self::Error> {
        debug!("Decode log: {:?}", log);
        let tokens = EVENT_ABI.decode(log.topics, log.data).map_err(|err| {
            warn!("Failed to decode event: {:?}", err);
            EnvelopeDecodeError
        })?;

        let mut iter = tokens.into_iter();

        debug!("Ping");
        let source = match iter.next().ok_or(EnvelopeDecodeError)? {
            Token::Address(source) => source,
            _ => return Err(EnvelopeDecodeError),
        };

        debug!("Ping");
        let nonce = match iter.next().ok_or(EnvelopeDecodeError)? {
            Token::Uint(value) => value.low_u64(),
            _ => return Err(EnvelopeDecodeError),
        };

        debug!("Ping");
        let payload = match iter.next().ok_or(EnvelopeDecodeError)? {
            Token::Bytes(payload) => payload,
            _ => return Err(EnvelopeDecodeError),
        };

        Ok(Self {
            channel: log.address,
            source,
            nonce,
            payload,
        })
    }
}
