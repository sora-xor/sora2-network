use super::{BalanceOf, Config};
use ethabi::{Event, EventParam, ParamType};
use once_cell::race::OnceBox;
use snowbridge_ethereum::log::Log;
use snowbridge_ethereum::H160;

use sp_core::RuntimeDebug;
use sp_runtime::traits::Convert;
use sp_std::convert::TryFrom;
use sp_std::prelude::*;

pub static EVENT_ABI: OnceBox<Event> = OnceBox::new();

fn get_event_abi() -> &'static Event {
    EVENT_ABI.get_or_init(event_abi)
}

fn event_abi() -> Box<Event> {
    Box::new(Event {
        name: "Message".into(),
        inputs: vec![
            EventParam {
                kind: ParamType::Address,
                name: "source".into(),
                indexed: false,
            },
            EventParam {
                kind: ParamType::Uint(64),
                name: "nonce".into(),
                indexed: false,
            },
            EventParam {
                kind: ParamType::Uint(256),
                name: "fee".into(),
                indexed: false,
            },
            EventParam {
                kind: ParamType::Bytes,
                name: "payload".into(),
                indexed: false,
            },
        ],
        anonymous: false,
    })
}

/// An inbound message that has had its outer envelope decoded.
#[derive(Clone, PartialEq, Eq, RuntimeDebug)]
pub struct Envelope<T>
where
    T: Config,
{
    /// The address of the outbound channel on Ethereum that forwarded this message.
    pub channel: H160,
    /// The application on Ethereum where the message originated from.
    pub source: H160,
    /// A nonce for enforcing replay protection and ordering.
    pub nonce: u64,
    /// Fee paid by user for relaying the message
    pub fee: BalanceOf<T>,
    /// The inner payload generated from the source application.
    pub payload: Vec<u8>,
}

#[derive(Copy, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct EnvelopeDecodeError;

impl<T: Config> TryFrom<Log> for Envelope<T> {
    type Error = EnvelopeDecodeError;

    fn try_from(log: Log) -> Result<Self, Self::Error> {
        let address = log.address;
        let log = get_event_abi()
            .parse_log(log.into())
            .map_err(|_| EnvelopeDecodeError)?;

        let mut source = None;
        let mut nonce = None;
        let mut payload = None;
        let mut fee = None;
        for param in log.params {
            match param.name.as_str() {
                "source" => source = param.value.into_address(),
                "nonce" => nonce = param.value.into_uint().map(|x| x.low_u64()),
                "payload" => payload = param.value.into_bytes(),
                "fee" => fee = param.value.into_uint().map(|x| T::FeeConverter::convert(x)),
                _ => return Err(EnvelopeDecodeError),
            }
        }

        Ok(Self {
            channel: address,
            fee: fee.ok_or(EnvelopeDecodeError)?,
            source: source.ok_or(EnvelopeDecodeError)?,
            nonce: nonce.ok_or(EnvelopeDecodeError)?,
            payload: payload.ok_or(EnvelopeDecodeError)?,
        })
    }
}
