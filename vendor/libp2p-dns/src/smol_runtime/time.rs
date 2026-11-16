// Copyright 2015-2020 Benjamin Fry <benjaminfry@me.com>
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// https://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// https://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use std::future::Future;
use std::time::Duration;

use async_trait::async_trait;
use hickory_resolver::net::runtime::Time;

/// Smol backed timer implementation
#[derive(Clone, Copy)]
pub struct SmolTime;

#[async_trait]
impl Time for SmolTime {
    async fn delay_for(duration: Duration) {
        ::smol::Timer::after(duration).await;
    }

    async fn timeout<F: 'static + Future + Send>(
        duration: Duration,
        future: F,
    ) -> Result<F::Output, std::io::Error> {
        ::smol::future::race(async { Ok(future.await) }, async {
            ::smol::Timer::after(duration).await;
            Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "future timed out",
            ))
        })
        .await
    }
}
