#![deny(unsafe_code)]
#![cfg_attr(test, deny(rust_2018_idioms))]
#![cfg_attr(test, deny(warnings))]
#![deny(clippy::clone_on_ref_ptr)]

mod api;
mod channel;
mod message;
mod metrics;
mod participant;
mod scheduler;
mod util;

pub use api::{
    ConnectAddr, ListenAddr, Network, NetworkConnectError, NetworkError, Participant,
    ParticipantError, Stream, StreamError, StreamParams,
};
pub use message::Message;
pub use network_protocol::{InitProtocolError, Pid, Promises};
