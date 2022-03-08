use crate::{
    error::{InitProtocolError, ProtocolError},
    frame::InitFrame,
    types::{
        Pid, Sid, STREAM_ID_OFFSET1, STREAM_ID_OFFSET2, VELOREN_MAGIC_NUMBER,
        VELOREN_NETWORK_VERSION,
    },
    InitProtocol,
};
use async_trait::async_trait;
use tracing::{debug, error, info, trace};

/// Implement this for auto Handshake with [`ReliableSink`].
/// You must make sure that EVERY message send this way actually is received on
/// the receiving site:
///  - exactly once
///  - in the correct order
///  - correctly
///
/// [`ReliableSink`]: crate::ReliableSink
/// [`RecvProtocol`]: crate::RecvProtocol
#[async_trait]
pub trait ReliableDrain {
    async fn send(&mut self, frame: InitFrame) -> Result<(), ProtocolError>;
}

/// Implement this for auto Handshake with [`ReliableDrain`]. See
/// [`ReliableDrain`].
///
/// [`ReliableDrain`]: crate::ReliableDrain
#[async_trait]
pub trait ReliableSink {
    async fn recv(&mut self) -> Result<InitFrame, ProtocolError>;
}

#[async_trait]
impl<D, S> InitProtocol for (D, S)
where
    D: ReliableDrain + Send,
    S: ReliableSink + Send,
{
    async fn initialize(
        &mut self,
        initializer: bool,
        local_pid: Pid,
        local_secret: u128,
    ) -> Result<(Pid, Sid, u128), InitProtocolError> {
        #[cfg(debug_assertions)]
        const WRONG_NUMBER: &str = "Handshake does not contain the magic number required by \
                                    veloren server.\nWe are not sure if you are a valid veloren \
                                    client.\nClosing the connection";
        #[cfg(debug_assertions)]
        const WRONG_VERSION: &str = "Handshake does contain a correct magic number, but invalid \
                                     version.\nWe don't know how to communicate with \
                                     you.\nClosing the connection";
        const ERR_S: &str = "Got A Raw Message, these are usually Debug Messages indicating that \
                             something went wrong on network layer and connection will be closed";

        let drain = &mut self.0;
        let sink = &mut self.1;

        if initializer {
            drain
                .send(InitFrame::Handshake {
                    magic_number: VELOREN_MAGIC_NUMBER,
                    version: VELOREN_NETWORK_VERSION,
                })
                .await?;
        }

        match sink.recv().await? {
            InitFrame::Handshake {
                magic_number,
                version,
            } => {
                trace!(?magic_number, ?version, "Recv handshake");
                if magic_number != VELOREN_MAGIC_NUMBER {
                    error!(?magic_number, "Connection with invalid magic_number");
                    #[cfg(debug_assertions)]
                    drain
                        .send(InitFrame::Raw(WRONG_NUMBER.as_bytes().to_vec()))
                        .await?;
                    Err(InitProtocolError::WrongMagicNumber(magic_number))
                } else if version[0] != VELOREN_NETWORK_VERSION[0]
                    || version[1] != VELOREN_NETWORK_VERSION[1]
                {
                    error!(?version, "Connection with wrong network version");
                    #[cfg(debug_assertions)]
                    drain
                        .send(InitFrame::Raw(
                            format!(
                                "{} Our Version: {:?}\nYour Version: {:?}\nClosing the connection",
                                WRONG_VERSION, VELOREN_NETWORK_VERSION, version,
                            )
                            .as_bytes()
                            .to_vec(),
                        ))
                        .await?;
                    Err(InitProtocolError::WrongVersion(version))
                } else {
                    trace!("Handshake Frame completed");
                    if initializer {
                        drain
                            .send(InitFrame::Init {
                                pid: local_pid,
                                secret: local_secret,
                            })
                            .await?;
                    } else {
                        drain
                            .send(InitFrame::Handshake {
                                magic_number: VELOREN_MAGIC_NUMBER,
                                version: VELOREN_NETWORK_VERSION,
                            })
                            .await?;
                    }
                    Ok(())
                }
            },
            InitFrame::Raw(bytes) => {
                match std::str::from_utf8(bytes.as_slice()) {
                    Ok(string) => error!(?string, ERR_S),
                    _ => error!(?bytes, ERR_S),
                }
                Err(InitProtocolError::Closed)
            },
            _ => {
                info!("Handshake failed");
                Err(InitProtocolError::Closed)
            },
        }?;

        match sink.recv().await? {
            InitFrame::Init { pid, secret } => {
                debug!(?pid, "Participant send their ID");
                let stream_id_offset = if initializer {
                    STREAM_ID_OFFSET1
                } else {
                    drain
                        .send(InitFrame::Init {
                            pid: local_pid,
                            secret: local_secret,
                        })
                        .await?;
                    STREAM_ID_OFFSET2
                };
                info!(?pid, "This Handshake is now configured!");
                Ok((pid, stream_id_offset, secret))
            },
            InitFrame::Raw(bytes) => {
                match std::str::from_utf8(bytes.as_slice()) {
                    Ok(string) => error!(?string, ERR_S),
                    _ => error!(?bytes, ERR_S),
                }
                Err(InitProtocolError::Closed)
            },
            _ => {
                info!("Handshake failed");
                Err(InitProtocolError::Closed)
            },
        }
    }
}
