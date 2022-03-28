#![feature(drain_filter)]

mod error;
mod event;
mod frame;
mod handshake;
mod message;
mod prio;
mod tcp;
mod types;
mod util;

pub use error::{InitProtocolError, ProtocolError};
pub use event::ProtocolEvent;
pub use tcp::{TcpRecvProtocol, TcpSendProtocol};
pub use types::{Bandwidth, Cid, Pid, Prio, Promises, Sid, HIGHEST_PRIO, VELOREN_NETWORK_VERSION};
///use at own risk, might change any time, for internal benchmarks
pub mod _internal {
    pub use crate::{
        frame::{ITFrame, OTFrame},
        util::SortedVec,
    };
}
use instant::Duration;
use async_trait::async_trait;

/// Handshake: Used to connect 2 Channels.
#[async_trait]
pub trait InitProtocol {
    async fn initialize(
        &mut self,
        initializer: bool,
        local_pid: Pid,
        secret: u128,
    ) -> Result<(Pid, Sid, u128), InitProtocolError>;
}

/// Generic Network Send Protocol.
/// Implement this for your Protocol of choice ( tcp, udp, mpsc, quic)
/// Allows the creation/deletions of `Streams` and sending messages via
/// [`ProtocolEvent`].
///
/// A `Stream` MUST be bound to a specific Channel. You MUST NOT switch the
/// channel to send a stream mid air. We will provide takeover options for
/// Channel closure in the future to allow keeping a `Stream` over a broker
/// Channel.
///
/// [`ProtocolEvent`]: crate::ProtocolEvent
#[async_trait]
pub trait SendProtocol {
    /// YOU MUST inform the `SendProtocol` by any Stream Open BEFORE using it in
    /// `send` and Stream Close AFTER using it in `send` via this fn.
    fn notify_from_recv(&mut self, event: ProtocolEvent);
    /// Send a Event via this Protocol. The `SendProtocol` MAY require `flush`
    /// to be called before actual data is send to the respective `Sink`.
    async fn send(&mut self, event: ProtocolEvent) -> Result<(), ProtocolError>;
    /// Flush all buffered messages according to their [`Prio`] and
    /// [`Bandwidth`]. provide the current bandwidth budget (per second) as
    /// well as the `dt` since last call. According to the budget the
    /// respective messages will be flushed.
    ///
    /// [`Prio`]: crate::Prio
    /// [`Bandwidth`]: crate::Bandwidth
    async fn flush(
        &mut self,
        bandwidth: Bandwidth,
        dt: Duration,
    ) -> Result<Bandwidth, ProtocolError>;
}

/// Generic Network Recv Protocol. See: [`SendProtocol`]
///
/// [`SendProtocol`]: crate::SendProtocol
#[async_trait]
pub trait RecvProtocol {
    /// Either recv an event or fail the Protocol, once the Recv side is closed
    /// it cannot recover from the error.
    async fn recv(&mut self) -> Result<ProtocolEvent, ProtocolError>;
}

/// This crate makes use of UnreliableDrains, they are expected to provide the
/// same guarantees like their IO-counterpart. E.g. ordered messages for TCP and
/// nothing for UDP. The respective Protocol needs then to handle this.
/// This trait is an abstraction above multiple Drains, e.g. [`tokio`](https://tokio.rs) [`async-std`] [`std`] or even [`async-channel`]
///
/// [`async-std`]: async-std
/// [`std`]: std
/// [`async-channel`]: async-channel
#[async_trait]
pub trait UnreliableDrain: Send {
    type DataFormat;
    async fn send(&mut self, data: Self::DataFormat) -> Result<(), ProtocolError>;
}

/// Sink counterpart of [`UnreliableDrain`]
///
/// [`UnreliableDrain`]: crate::UnreliableDrain
#[async_trait]
pub trait UnreliableSink: Send {
    type DataFormat;
    async fn recv(&mut self) -> Result<Self::DataFormat, ProtocolError>;
}
