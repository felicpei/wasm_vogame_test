use crate::api::ListenAddr;
use std::net::SocketAddr;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(crate) enum ProtocolInfo {
    Tcp(SocketAddr),
}

impl From<ListenAddr> for ProtocolInfo {
    fn from(other: ListenAddr) -> ProtocolInfo {
        match other {
            ListenAddr::Tcp(s) => ProtocolInfo::Tcp(s),
        }
    }
}
