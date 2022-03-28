use std::net::SocketAddr;



#[derive(Clone, Debug)]
pub enum ConnectionArgs {
    ///hostname: (hostname|ip):[<port>]
    Tcp {
        hostname: String,
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
pub(crate) async fn resolve(address: &str) -> Result<Vec<SocketAddr>, std::io::Error> {
    // `lookup_host` will internally try to parse it as a SocketAddr
    // 1. Assume it's a hostname + port
  
    match lookup_host(address).await {
        Ok(s) => {
            log::trace!("Host lookup succeeded");
            Ok(sort_ipv6(s))
        },
        Err(e) => {
            // 2. Assume its a hostname without port
            match lookup_host((address, ConnectionArgs::DEFAULT_PORT)).await {
                Ok(s) => {
                    log::trace!("Host lookup without ports succeeded");
                    Ok(sort_ipv6(s))
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
    f: F,
) -> Result<network::Participant, crate::error::Error>
where
    F: Fn(std::net::SocketAddr) -> network::ConnectAddr,
{
    use crate::error::Error;

    log::info!("start try_connect:  {}", address);

    //tcp连接
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut participant = None;
        for addr in resolve(address)
            .await
            .map_err(Error::HostnameLookupFailed)?
        {
            let address = f(addr);

            log::info!("try_connect get address: {:?}", address);
            match network.connect(address).await {
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
        use std::net::{IpAddr, Ipv4Addr};

        let mut participant = None;

        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1404);
        log::warn!("socket addr: {:?}", addr);
        let address = f(addr);

        let connect_result = network.connect(address).await;
        match connect_result {
            Ok(p) => participant = Some(Ok(p)),
            Err(e) => participant = Some(Err(Error::NetworkErr(e))),
        }
        participant.unwrap_or_else(|| Err(Error::Other("No Ip Addr provided".to_string())))
        //Err(Error::Other("todo addr resolve and network connect".to_string()))
    }
}

fn sort_ipv6(s: impl Iterator<Item = SocketAddr>) -> Vec<SocketAddr> {
    let prefer_ipv6 = false;
    let (mut first_addrs, mut second_addrs) = s.partition::<Vec<_>, _>(|a| a.is_ipv6() == prefer_ipv6);
    std::iter::Iterator::chain(first_addrs.drain(..), second_addrs.drain(..)).collect::<Vec<_>>()
}