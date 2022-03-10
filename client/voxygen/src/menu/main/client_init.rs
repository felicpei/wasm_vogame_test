use client::{
    addr::ConnectionArgs,
    error::{Error as ClientError, NetworkConnectError, NetworkError},
    Client, ServerInfo,
};
use crossbeam_channel::{unbounded, Receiver, TryRecvError};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::runtime;

#[derive(Debug)]
#[allow(clippy::enum_variant_names)] //TODO: evaluate ClientError ends with Enum name
pub enum Error {
    ClientError {
        error: ClientError,
        mismatched_server_info: Option<ServerInfo>,
    },
    ClientCrashed,
    ServerNotFound,
}

#[allow(clippy::large_enum_variant)] // TODO: Pending review in #587
pub enum Msg {
    Done(Result<Client, Error>),
}

// Used to asynchronously parse the server address, resolve host names,
// and create the client (which involves establishing a connection to the
// server).
pub struct ClientInit {
    rx: Receiver<Msg>,
    cancel: Arc<AtomicBool>,
}
impl ClientInit {
    pub fn new(
        connection_args: ConnectionArgs,
        username: String,
        password: String,
        runtime: Arc<runtime::Runtime>,
    ) -> Self {
        let (tx, rx) = unbounded();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel2 = Arc::clone(&cancel);

        log::info!("# start ClientInit");
        let runtime2 = Arc::clone(&runtime);
        runtime.spawn(async move {
         
            let mut last_err = None;

            const FOUR_MINUTES_RETRIES: u64 = 48;
            'tries: for _ in 0..FOUR_MINUTES_RETRIES {
                if cancel2.load(Ordering::Relaxed) {
                    break;
                }
                let mut mismatched_server_info = None;
                match Client::new(
                    connection_args.clone(),
                    Arc::clone(&runtime2),
                    &mut mismatched_server_info,
                )
                .await
                {
                    Ok(mut client) => {

                        //验证登录模块
                        if let Err(e) = client.register(username, password).await {
                            last_err = Some(Error::ClientError {
                                error: e,
                                mismatched_server_info: None,
                            });
                            break 'tries;
                        }
                        let _ = tx.send(Msg::Done(Ok(client)));

                        
                        //########## 去掉多线程 rt_multi_thread
                        tokio::task::spawn_blocking(move || drop(runtime2));
                        //tokio::task::block_in_place(move || drop(runtime2));
                        return;
                    },

                    Err(ClientError::NetworkErr(NetworkError::ConnectFailed(
                        NetworkConnectError::Io(e),
                    ))) => {
                        log::warn!("{:?} Failed to connect to the server. Retrying...", e);
                    },

                    Err(e) => {
                        log::trace!("{:?}  Aborting server connection attempt", e);
                        last_err = Some(Error::ClientError {
                            error: e,
                            mismatched_server_info,
                        });
                        break 'tries;
                    },
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
            }

            // Parsing/host name resolution successful but no connection succeeded
            // If last_err is None this typically means there was no server up at the input
            // address and all the attempts timed out.
            let _ = tx.send(Msg::Done(Err(last_err.unwrap_or(Error::ServerNotFound))));

            // Safe drop runtime
            //########## 去掉多线程 rt_multi_thread
            //tokio::task::block_in_place(move || drop(runtime2));
            tokio::task::spawn_blocking(move || drop(runtime2));
        });

        ClientInit {
            rx,
            cancel,
        }
    }

    /// Poll if the thread is complete.
    /// Returns None if the thread is still running, otherwise returns the
    /// Result of client creation.
    pub fn poll(&self) -> Option<Msg> {
        match self.rx.try_recv() {
            Ok(msg) => Some(msg),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Msg::Done(Err(Error::ClientCrashed))),
        }
    }

    pub fn cancel(&mut self) { self.cancel.store(true, Ordering::Relaxed); }
}

impl Drop for ClientInit {
    fn drop(&mut self) { self.cancel(); }
}
