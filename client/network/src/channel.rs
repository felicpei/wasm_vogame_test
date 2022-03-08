use crate::api::NetworkConnectError;
use async_trait::async_trait;
use bytes::BytesMut;
use futures_util::FutureExt;
use network_protocol::{
    Bandwidth, Cid, InitProtocolError, Pid,
    ProtocolError, ProtocolEvent, ProtocolMetricCache, ProtocolMetrics, Sid, TcpRecvProtocol,
    TcpSendProtocol, UnreliableDrain, UnreliableSink,
};
use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net,
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
    select,
    sync::{mpsc, oneshot},
};
use tracing::{info, trace, warn};

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum Protocols {
    Tcp((TcpSendProtocol<TcpDrain>, TcpRecvProtocol<TcpSink>)),
}

#[derive(Debug)]
pub(crate) enum SendProtocols {
    Tcp(TcpSendProtocol<TcpDrain>),
}

#[derive(Debug)]
pub(crate) enum RecvProtocols {
    Tcp(TcpRecvProtocol<TcpSink>),
}

impl Protocols {

    pub(crate) async fn with_tcp_connect(
        addr: SocketAddr,
        metrics: ProtocolMetricCache,
    ) -> Result<Self, NetworkConnectError> {
        let stream = net::TcpStream::connect(addr)
            .await
            .and_then(|s| {
                s.set_nodelay(true)?;
                Ok(s)
            })
            .map_err(NetworkConnectError::Io)?;
        info!(
            "Connecting Tcp to: {}",
            stream.peer_addr().map_err(NetworkConnectError::Io)?
        );
        Ok(Self::new_tcp(stream, metrics))
    }

    pub(crate) async fn with_tcp_listen(
        addr: SocketAddr,
        cids: Arc<AtomicU64>,
        metrics: Arc<ProtocolMetrics>,
        s2s_stop_listening_r: oneshot::Receiver<()>,
        c2s_protocol_s: mpsc::UnboundedSender<(Self, Cid)>,
    ) -> std::io::Result<()> {

        use socket2::{Domain, Socket, Type};
        let domain = Domain::for_address(addr);
        let socket2_socket = Socket::new(domain, Type::STREAM, None)?;
        if domain == Domain::IPV6 {
            socket2_socket.set_only_v6(true)?
        }
        socket2_socket.set_nonblocking(true)?; // Needed by Tokio
        // See https://docs.rs/tokio/latest/tokio/net/struct.TcpSocket.html
        #[cfg(not(windows))]
        socket2_socket.set_reuse_address(true)?;
        let socket2_addr = addr.into();
        socket2_socket.bind(&socket2_addr)?;
        socket2_socket.listen(1024)?;
        let std_listener: std::net::TcpListener = socket2_socket.into();
        let listener = tokio::net::TcpListener::from_std(std_listener)?;
        trace!(?addr, "Tcp Listener bound");
        let mut end_receiver = s2s_stop_listening_r.fuse();
        tokio::spawn(async move {
            while let Some(data) = select! {
                    next = listener.accept().fuse() => Some(next),
                    _ = &mut end_receiver => None,
            } {
                let (stream, remote_addr) = match data {
                    Ok((s, p)) => (s, p),
                    Err(e) => {
                        trace!(?e, "TcpStream Error, ignoring connection attempt");
                        continue;
                    },
                };
                if let Err(e) = stream.set_nodelay(true) {
                    warn!(
                        ?e,
                        "Failed to set TCP_NODELAY, client may have degraded latency"
                    );
                }
                let cid = cids.fetch_add(1, Ordering::Relaxed);
                info!(?remote_addr, ?cid, "Accepting Tcp from");
                let metrics = ProtocolMetricCache::new(&cid.to_string(), Arc::clone(&metrics));
                let _ = c2s_protocol_s.send((Self::new_tcp(stream, metrics.clone()), cid));
            }
        });
        Ok(())
    }

    pub(crate) fn new_tcp(stream: tokio::net::TcpStream, metrics: ProtocolMetricCache) -> Self {
        let (r, w) = stream.into_split();
        let sp = TcpSendProtocol::new(TcpDrain { half: w }, metrics.clone());
        let rp = TcpRecvProtocol::new(
            TcpSink {
                half: r,
                buffer: BytesMut::new(),
            },
            metrics,
        );
        Protocols::Tcp((sp, rp))
    }

    pub(crate) fn split(self) -> (SendProtocols, RecvProtocols) {
        match self {
            Protocols::Tcp((s, r)) => (SendProtocols::Tcp(s), RecvProtocols::Tcp(r)),
        }
    }
}

#[async_trait]
impl network_protocol::InitProtocol for Protocols {
    async fn initialize(
        &mut self,
        initializer: bool,
        local_pid: Pid,
        secret: u128,
    ) -> Result<(Pid, Sid, u128), InitProtocolError> {
        match self {
            Protocols::Tcp(p) => p.initialize(initializer, local_pid, secret).await,
        }
    }
}

#[async_trait]
impl network_protocol::SendProtocol for SendProtocols {
    fn notify_from_recv(&mut self, event: ProtocolEvent) {
        match self {
            SendProtocols::Tcp(s) => s.notify_from_recv(event),
        }
    }

    async fn send(&mut self, event: ProtocolEvent) -> Result<(), ProtocolError> {
        match self {
            SendProtocols::Tcp(s) => s.send(event).await,
        }
    }

    async fn flush(
        &mut self,
        bandwidth: Bandwidth,
        dt: Duration,
    ) -> Result<Bandwidth, ProtocolError> {
        match self {
            SendProtocols::Tcp(s) => s.flush(bandwidth, dt).await,
        }
    }
}

#[async_trait]
impl network_protocol::RecvProtocol for RecvProtocols {
    async fn recv(&mut self) -> Result<ProtocolEvent, ProtocolError> {
        match self {
            RecvProtocols::Tcp(r) => r.recv().await,
        }
    }
}

///////////////////////////////////////
//// TCP
#[derive(Debug)]
pub struct TcpDrain {
    half: OwnedWriteHalf,
}

#[derive(Debug)]
pub struct TcpSink {
    half: OwnedReadHalf,
    buffer: BytesMut,
}

#[async_trait]
impl UnreliableDrain for TcpDrain {
    type DataFormat = BytesMut;

    async fn send(&mut self, data: Self::DataFormat) -> Result<(), ProtocolError> {
        match self.half.write_all(&data).await {
            Ok(()) => Ok(()),
            Err(_) => Err(ProtocolError::Closed),
        }
    }
}

#[async_trait]
impl UnreliableSink for TcpSink {
    type DataFormat = BytesMut;

    async fn recv(&mut self) -> Result<Self::DataFormat, ProtocolError> {
        self.buffer.resize(1500, 0u8);
        match self.half.read(&mut self.buffer).await {
            Ok(0) => Err(ProtocolError::Closed),
            Ok(n) => Ok(self.buffer.split_to(n)),
            Err(_) => Err(ProtocolError::Closed),
        }
    }
}
