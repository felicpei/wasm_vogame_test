use std::net::SocketAddr;


#[derive(Clone, Debug)]
pub enum ConnectionArgs {
    ///hostname: (hostname|ip):[<port>]
    Tcp {
        hostname: String,
        prefer_ipv6: bool,
    },
}

impl ConnectionArgs {
    const DEFAULT_PORT: u16 = 14004;
}

#[cfg(not(target_arch = "wasm32"))]
use tokio::net::lookup_host;

/// Parse ip address or resolves hostname.
/// Note: If you use an ipv6 address, the number after the last
/// colon will be used as the port unless you use [] around the address.

#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn resolve(
    address: &str,
    prefer_ipv6: bool,
) -> Result<Vec<SocketAddr>, std::io::Error> {
    // `lookup_host` will internally try to parse it as a SocketAddr
    // 1. Assume it's a hostname + port
  
    match lookup_host(address).await {
        Ok(s) => {
            log::trace!("Host lookup succeeded");
            Ok(sort_ipv6(s, prefer_ipv6))
        },
        Err(e) => {
            // 2. Assume its a hostname without port
            match lookup_host((address, ConnectionArgs::DEFAULT_PORT)).await {
                Ok(s) => {
                    log::trace!("Host lookup without ports succeeded");
                    Ok(sort_ipv6(s, prefer_ipv6))
                },
                Err(_) => Err(e), // Todo: evaluate returning both errors
            }
        },
    }
}

//建立网络连接
pub(crate) async fn try_connect<F>(
    network: &network::Network,
    address: &str,
    prefer_ipv6: bool,
    f: F,
) -> Result<network::Participant, crate::error::Error>
where
    F: Fn(std::net::SocketAddr) -> network::ConnectAddr,
{
    use crate::error::Error;

    //tcp连接
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut participant = None;
        for addr in resolve(address, prefer_ipv6)
            .await
            .map_err(Error::HostnameLookupFailed)?
        {
            match network.connect(f(addr)).await {
                Ok(p) => {
                    participant = Some(Ok(p));
                    break;
                },
                Err(e) => participant = Some(Err(Error::NetworkErr(e))),
            }
        }
        participant.unwrap_or_else(|| Err(Error::Other("No Ip Addr provided".to_string())))
    }

    //websocket连接 todo
    #[cfg(target_arch = "wasm32")]
    {
        log::error!("########## todo addr resolve and network connect");
        Err(Error::Other("todo addr resolve and network connect".to_string()))
    }
}

fn sort_ipv6(s: impl Iterator<Item = SocketAddr>, prefer_ipv6: bool) -> Vec<SocketAddr> {
    let (mut first_addrs, mut second_addrs) =
        s.partition::<Vec<_>, _>(|a| a.is_ipv6() == prefer_ipv6);
    std::iter::Iterator::chain(first_addrs.drain(..), second_addrs.drain(..)).collect::<Vec<_>>()
}