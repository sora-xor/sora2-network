use super::{time::SmolTime, SmolRuntimeProvider};
use hickory_resolver::{
    config::{ResolverConfig, ResolverOpts},
    net::runtime::{DnsUdpSocket, RuntimeProvider, Spawn, Time},
    Resolver,
};
use std::time::Duration;

#[test]
fn resolver_is_send_and_sync() {
    fn assert_traits<T: Send + Sync>() {}
    assert_traits::<Resolver<SmolRuntimeProvider>>();
}

#[test]
fn literal_lookup_without_network() {
    ::smol::block_on(async {
        let resolver =
            Resolver::builder_with_config(ResolverConfig::default(), SmolRuntimeProvider)
                .with_options(ResolverOpts::default())
                .build()
                .unwrap();
        let addresses = resolver.lookup_ip("127.0.0.1").await.unwrap();
        assert_eq!(
            addresses.iter().collect::<Vec<_>>(),
            vec!["127.0.0.1".parse::<std::net::IpAddr>().unwrap()]
        );
    });
}

#[test]
fn tcp_and_udp_runtime_adapters() {
    ::smol::block_on(async {
        let provider = SmolRuntimeProvider;
        let listener = ::smol::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let peer = listener.local_addr().unwrap();
        let client = provider
            .connect_tcp(peer, None, Some(Duration::from_secs(2)))
            .await
            .unwrap();
        let (_server, _) = listener.accept().await.unwrap();
        drop(client);
        let socket = ::smol::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let peer = socket.local_addr().unwrap();
        let adapter = provider
            .bind_udp("127.0.0.1:0".parse().unwrap(), peer)
            .await
            .unwrap();
        adapter.send_to(b"dns", peer).await.unwrap();
        let mut bytes = [0; 32];
        let (len, addr) = socket.recv_from(&mut bytes).await.unwrap();
        assert_eq!(&bytes[..len], b"dns");
        socket.send_to(b"reply", addr).await.unwrap();
        let (len, _) = adapter.recv_from(&mut bytes).await.unwrap();
        assert_eq!(&bytes[..len], b"reply");
    });
}

#[test]
fn timers_and_background_tasks() {
    ::smol::block_on(async {
        let error = SmolTime::timeout(Duration::from_millis(1), futures::future::pending::<()>())
            .await
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        let (tx, rx) = futures::channel::oneshot::channel();
        SmolRuntimeProvider.create_handle().spawn_bg(async move {
            tx.send(42).unwrap();
        });
        assert_eq!(
            SmolTime::timeout(Duration::from_secs(2), rx)
                .await
                .unwrap()
                .unwrap(),
            42
        );
    });
}
