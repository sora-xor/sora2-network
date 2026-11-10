// Copyright 2015-2020 Benjamin Fry <benjaminfry@me.com>
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// https://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// https://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

mod net;
#[cfg(test)]
mod tests;
mod time;

use self::{
    net::{SmolTcpStream, SmolUdpSocket},
    time::SmolTime,
};
use hickory_resolver::net::runtime::{RuntimeProvider, Spawn};
use std::{future::Future, net::SocketAddr, pin::Pin, time::Duration};

/// Smol runtime adapter for Hickory DNS.
#[derive(Clone, Copy, Default)]
pub struct SmolRuntimeProvider;

/// Spawn handle for the smol executor.
#[derive(Clone, Copy)]
pub struct SmolRuntimeHandle;
impl Spawn for SmolRuntimeHandle {
    fn spawn_bg(&mut self, future: impl Future<Output = ()> + Send + 'static) {
        ::smol::spawn(future).detach();
    }
}

impl RuntimeProvider for SmolRuntimeProvider {
    type Handle = SmolRuntimeHandle;
    type Timer = SmolTime;
    type Udp = SmolUdpSocket;
    type Tcp = SmolTcpStream;

    fn create_handle(&self) -> Self::Handle {
        SmolRuntimeHandle
    }

    fn connect_tcp(
        &self,
        server_addr: SocketAddr,
        bind_addr: Option<SocketAddr>,
        timeout: Option<Duration>,
    ) -> Pin<Box<dyn Send + Future<Output = std::io::Result<Self::Tcp>>>> {
        Box::pin(SmolTcpStream::connect_with_bind(
            server_addr,
            bind_addr,
            timeout,
        ))
    }

    fn bind_udp(
        &self,
        local_addr: SocketAddr,
        _server_addr: SocketAddr,
    ) -> Pin<Box<dyn Send + Future<Output = std::io::Result<Self::Udp>>>> {
        Box::pin(SmolUdpSocket::bind(local_addr))
    }
}
