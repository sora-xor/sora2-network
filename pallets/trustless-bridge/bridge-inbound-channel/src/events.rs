//! Ethereum event logs decoders.

use super::{BalanceOf, Config};
use bridge_types::log::Log;
use bridge_types::H160;
use ethabi::{Event, EventParam, ParamType};
use once_cell::race::OnceBox;
use sp_core::RuntimeDebug;
use sp_runtime::traits::Convert;
use sp_std::convert::TryFrom;
use sp_std::prelude::*;

pub static MESSAGE_EVENT_ABI: OnceBox<Event> = OnceBox::new();

fn get_message_event_abi() -> &'static Event {
    MESSAGE_EVENT_ABI.get_or_init(message_event_abi)
}

/// ABI for OutboundChannel Message event
fn message_event_abi() -> Box<Event> {
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
        let log = get_message_event_abi()
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

pub static BATCH_DISPATCHED_EVENT_ABI: OnceBox<Event> = OnceBox::new();

fn get_batch_dispatched_event_abi() -> &'static Event {
    BATCH_DISPATCHED_EVENT_ABI.get_or_init(batch_dispatched_event_abi)
}

/// ABI for InboundChannel BatchDispatched event
fn batch_dispatched_event_abi() -> Box<Event> {
    Box::new(Event {
        name: "BatchDispatched".into(),
        inputs: vec![
            EventParam {
                kind: ParamType::Uint(64),
                name: "batch_nonce".into(),
                indexed: false,
            },
            EventParam {
                kind: ParamType::Address,
                name: "relayer".into(),
                indexed: false,
            },
            EventParam {
                kind: ParamType::Uint(256),
                name: "results".into(),
                indexed: false,
            },
            EventParam {
                kind: ParamType::Uint(256),
                name: "results_length".into(),
                indexed: false,
            },
            EventParam {
                kind: ParamType::Uint(256),
                name: "gas_spent".into(),
                indexed: false,
            },
            EventParam {
                kind: ParamType::Uint(256),
                name: "base_fee".into(),
                indexed: false,
            },
        ],
        anonymous: false,
    })
}

#[derive(Clone, PartialEq, Eq, RuntimeDebug)]
pub struct BatchDispatched {
    /// The address of the inbound channel on Ethereum that processed this message.
    pub channel: H160,
    /// A nonce for enforcing replay protection and ordering.
    pub batch_nonce: u64,
    /// Ethereum address of batch sender
    pub relayer: H160,
    /// A bitfield status of message delivery.
    pub results: u64,
    /// A number of messages in a batch.
    pub results_length: u64,
    /// Gas spent for batch submission, but not a full gas for tx, at least 10500 gas should be
    /// added.
    pub gas_spent: u64,
    /// Base fee in the block.
    pub base_fee: u64,
}

#[derive(Copy, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct BatchDispatchedEventDecodeError;

impl TryFrom<Log> for BatchDispatched {
    type Error = BatchDispatchedEventDecodeError;

    fn try_from(log: Log) -> Result<Self, Self::Error> {
        let address = log.address;
        let mut batch_nonce = None;
        let mut relayer = None;
        let mut results = None;
        let mut results_length = None;
        let mut gas_spent = None;
        let mut base_fee = None;

        let log = get_batch_dispatched_event_abi()
            .parse_log((log.topics, log.data).into())
            .map_err(|_| BatchDispatchedEventDecodeError)?;

        for param in log.params {
            match param.name.as_str() {
                "batch_nonce" => batch_nonce = param.value.into_uint().map(|x| x.low_u64()),
                "relayer" => relayer = param.value.into_address(),
                "results" => results = param.value.into_uint().map(|x| x.low_u64()),
                "results_length" => results_length = param.value.into_uint().map(|x| x.low_u64()),
                "gas_spent" => gas_spent = param.value.into_uint().map(|x| x.low_u64()),
                "base_fee" => base_fee = param.value.into_uint().map(|x| x.low_u64()),
                _ => return Err(BatchDispatchedEventDecodeError),
            }
        }

        Ok(Self {
            channel: address,
            batch_nonce: batch_nonce.ok_or(BatchDispatchedEventDecodeError)?,
            relayer: relayer.ok_or(BatchDispatchedEventDecodeError)?,
            results: results.ok_or(BatchDispatchedEventDecodeError)?,
            results_length: results_length.ok_or(BatchDispatchedEventDecodeError)?,
            gas_spent: gas_spent.ok_or(BatchDispatchedEventDecodeError)?,
            base_fee: base_fee.ok_or(BatchDispatchedEventDecodeError)?,
        })
    }
}
