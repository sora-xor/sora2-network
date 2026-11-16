// Copyright 2015-2020 Benjamin Fry <benjaminfry@me.com>
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// https://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// https://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use ::smol::unblock;
use async_trait::async_trait;
use futures::future::FutureExt;
use futures::io::{AsyncRead, AsyncWrite};
use futures::pin_mut;
use hickory_resolver::net::runtime::{DnsTcpStream, DnsUdpSocket, Time};
use socket2::{Domain, Protocol, Socket, Type};

use super::time::SmolTime;

pub struct SmolUdpSocket(::smol::net::UdpSocket);

#[async_trait]
impl DnsUdpSocket for SmolUdpSocket {
    type Time = SmolTime;
    fn poll_recv_from(
        &self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<(usize, SocketAddr)>> {
        let fut = self.0.recv_from(buf);
        pin_mut!(fut);

        fut.poll_unpin(cx)
    }

    async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.0.recv_from(buf).await
    }

    fn poll_send_to(
        &self,
        cx: &mut Context<'_>,
        buf: &[u8],
        target: SocketAddr,
    ) -> Poll<io::Result<usize>> {
        let fut = self.0.send_to(buf, target);
        pin_mut!(fut);

        fut.poll_unpin(cx)
    }

    async fn send_to(&self, buf: &[u8], target: SocketAddr) -> io::Result<usize> {
        self.0.send_to(buf, target).await
    }
}

impl SmolUdpSocket {
    pub async fn bind(addr: SocketAddr) -> io::Result<Self> {
        ::smol::net::UdpSocket::bind(addr).await.map(Self)
    }
}

pub struct SmolTcpStream(::smol::net::TcpStream);

impl DnsTcpStream for SmolTcpStream {
    type Time = SmolTime;
}

impl SmolTcpStream {
    pub async fn connect_with_bind(
        addr: SocketAddr,
        bind_addr: Option<SocketAddr>,
        timeout: Option<Duration>,
    ) -> io::Result<Self> {
        let stream = match bind_addr {
            Some(bind_addr) => {
                unblock(move || {
                    let domain = match bind_addr {
                        SocketAddr::V4(_) => Domain::IPV4,
                        SocketAddr::V6(_) => Domain::IPV6,
                    };
                    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))?;
                    socket.bind(&bind_addr.into())?;
                    match timeout {
                        Some(timeout) => socket.connect_timeout(&addr.into(), timeout)?,
                        None => socket.connect(&addr.into())?,
                    };
                    let std_stream: std::net::TcpStream = socket.into();
                    let stream = ::smol::net::TcpStream::try_from(std_stream)?;
                    Ok::<_, io::Error>(stream)
                })
                .await?
            }
            None => match timeout {
                Some(timeout) => {
                    SmolTime::timeout(timeout, ::smol::net::TcpStream::connect(addr)).await??
                }
                None => ::smol::net::TcpStream::connect(addr).await?,
            },
        };
        stream.set_nodelay(true)?;
        Ok(Self(stream))
    }
}

impl AsyncWrite for SmolTcpStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bytes: &[u8],
    ) -> std::task::Poll<std::result::Result<usize, std::io::Error>> {
        Pin::new(&mut self.0).poll_write(cx, bytes)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        Pin::new(&mut self.0).poll_close(cx)
    }
}

impl AsyncRead for SmolTcpStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bytes: &mut [u8],
    ) -> std::task::Poll<std::result::Result<usize, std::io::Error>> {
        Pin::new(&mut self.0).poll_read(cx, bytes)
    }
}
