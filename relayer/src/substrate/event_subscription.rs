use std::collections::VecDeque;

use crate::prelude::*;
use subxt::*;

pub struct EventSubscription<T: Config> {
    events: VecDeque<RawEvent>,
    decoder: EventsDecoder<T>,
    subscription: EventStorageSubscription<T>,
}

impl<T: Config> EventSubscription<T> {
    pub fn new(subscription: EventStorageSubscription<T>, decoder: EventsDecoder<T>) -> Self {
        Self {
            subscription,
            events: Default::default(),
            decoder,
        }
    }

    pub async fn next(&mut self) -> AnyResult<Option<RawEvent>> {
        loop {
            if let Some(event) = self.events.pop_front() {
                return Ok(Some(event));
            }

            let changes = if let Some(data) = self.subscription.next().await {
                data.changes
            } else {
                return Ok(None);
            };

            for (_, data) in changes {
                if let Some(data) = data {
                    let events = self.decoder.decode_events(&mut data.0.as_ref())?;
                    for (phase, event) in events {
                        if let Phase::ApplyExtrinsic(_) = phase {
                            self.events.push_back(event);
                        }
                    }
                }
            }
        }
    }
}
