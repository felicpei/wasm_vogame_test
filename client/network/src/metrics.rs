use crate::api::{ConnectAddr, ListenAddr};
use network_protocol::{Cid, Pid};
use std::{error::Error, net::SocketAddr};

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(crate) enum ProtocolInfo {
    Tcp(SocketAddr),
    //Udp(SocketAddr),
    //Mpsc(u64),
}

impl From<ListenAddr> for ProtocolInfo {
    fn from(other: ListenAddr) -> ProtocolInfo {
        match other {
            ListenAddr::Tcp(s) => ProtocolInfo::Tcp(s),
            //ListenAddr::Udp(s) => ProtocolInfo::Udp(s),
            //ListenAddr::Mpsc(s) => ProtocolInfo::Mpsc(s),
        }
    }
}

pub struct NetworkMetrics {}

impl NetworkMetrics {
    pub fn new(_local_pid: &Pid) -> Result<Self, Box<dyn Error>> { Ok(Self {}) }

    pub(crate) fn channels_connected(&self, _remote_p: &str, _no: usize, _cid: Cid) {}

    pub(crate) fn channels_disconnected(&self, _remote_p: &str) {}

    pub(crate) fn participant_bandwidth(&self, _remote_p: &str, _bandwidth: f32) {}

    pub(crate) fn streams_opened(&self, _remote_p: &str) {}

    pub(crate) fn streams_closed(&self, _remote_p: &str) {}

    pub(crate) fn listen_request(&self, _protocol: &ListenAddr) {}

    pub(crate) fn connect_request(&self, _protocol: &ConnectAddr) {}

    pub(crate) fn cleanup_participant(&self, _remote_p: &str) {}
}

impl std::fmt::Debug for NetworkMetrics {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NetworkMetrics()")
    }
}
