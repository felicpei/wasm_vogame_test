use crate::{
    error::ProtocolError,
    event::ProtocolEvent,
    frame::{ITFrame, InitFrame, OTFrame},
    handshake::{ReliableDrain, ReliableSink},
    message::{ITMessage, ALLOC_BLOCK},
    metrics::{ProtocolMetricCache, RemoveReason},
    prio::PrioManager,
    types::{Bandwidth, Mid, Promises, Sid},
    RecvProtocol, SendProtocol, UnreliableDrain, UnreliableSink,
};
use async_trait::async_trait;
use bytes::BytesMut;
use hashbrown::HashMap;
use std::time::{Duration, Instant};

/// TCP implementation of [`SendProtocol`]
///
/// [`SendProtocol`]: crate::SendProtocol
#[derive(Debug)]
pub struct TcpSendProtocol<D>
where
    D: UnreliableDrain<DataFormat = BytesMut>,
{
    buffer: BytesMut,
    store: PrioManager,
    next_mid: Mid,
    closing_streams: Vec<Sid>,
    notify_closing_streams: Vec<Sid>,
    pending_shutdown: bool,
    drain: D,
    #[allow(dead_code)]
    last: Instant,
    metrics: ProtocolMetricCache,
}

/// TCP implementation of [`RecvProtocol`]
///
/// [`RecvProtocol`]: crate::RecvProtocol
#[derive(Debug)]
pub struct TcpRecvProtocol<S>
where
    S: UnreliableSink<DataFormat = BytesMut>,
{
    buffer: BytesMut,
    itmsg_allocator: BytesMut,
    incoming: HashMap<Mid, ITMessage>,
    sink: S,
    metrics: ProtocolMetricCache,
}

impl<D> TcpSendProtocol<D>
where
    D: UnreliableDrain<DataFormat = BytesMut>,
{
    pub fn new(drain: D, metrics: ProtocolMetricCache) -> Self {
        Self {
            buffer: BytesMut::new(),
            store: PrioManager::new(metrics.clone()),
            next_mid: 0u64,
            closing_streams: vec![],
            notify_closing_streams: vec![],
            pending_shutdown: false,
            drain,
            last: Instant::now(),
            metrics,
        }
    }

    /// returns all promises that this Protocol can take care of
    /// If you open a Stream anyway, unsupported promises are ignored.
    pub fn supported_promises() -> Promises {
        Promises::ORDERED
            | Promises::CONSISTENCY
            | Promises::GUARANTEED_DELIVERY
            | Promises::COMPRESSED
    }
}

impl<S> TcpRecvProtocol<S>
where
    S: UnreliableSink<DataFormat = BytesMut>,
{
    pub fn new(sink: S, metrics: ProtocolMetricCache) -> Self {
        Self {
            buffer: BytesMut::new(),
            itmsg_allocator: BytesMut::with_capacity(ALLOC_BLOCK),
            incoming: HashMap::new(),
            sink,
            metrics,
        }
    }
}

#[async_trait]
impl<D> SendProtocol for TcpSendProtocol<D>
where
    D: UnreliableDrain<DataFormat = BytesMut>,
{
    fn notify_from_recv(&mut self, event: ProtocolEvent) {
        match event {
            ProtocolEvent::OpenStream {
                sid,
                prio,
                promises,
                guaranteed_bandwidth,
            } => {
                self.store
                    .open_stream(sid, prio, promises, guaranteed_bandwidth);
            },
            ProtocolEvent::CloseStream { sid } => {
                if !self.store.try_close_stream(sid) {
                    #[cfg(feature = "trace_pedantic")]
                    trace!(?sid, "hold back notify close stream");
                    self.notify_closing_streams.push(sid);
                }
            },
            _ => {},
        }
    }

    async fn send(&mut self, event: ProtocolEvent) -> Result<(), ProtocolError> {
        #[cfg(feature = "trace_pedantic")]
        trace!(?event, "send");
        match event {
            ProtocolEvent::OpenStream {
                sid,
                prio,
                promises,
                guaranteed_bandwidth,
            } => {
                self.store
                    .open_stream(sid, prio, promises, guaranteed_bandwidth);
                event.to_frame().write_bytes(&mut self.buffer);
                self.drain.send(self.buffer.split()).await?;
            },
            ProtocolEvent::CloseStream { sid } => {
                if self.store.try_close_stream(sid) {
                    event.to_frame().write_bytes(&mut self.buffer);
                    self.drain.send(self.buffer.split()).await?;
                } else {
                    #[cfg(feature = "trace_pedantic")]
                    trace!(?sid, "hold back close stream");
                    self.closing_streams.push(sid);
                }
            },
            ProtocolEvent::Shutdown => {
                if self.store.is_empty() {
                    event.to_frame().write_bytes(&mut self.buffer);
                    self.drain.send(self.buffer.split()).await?;
                } else {
                    #[cfg(feature = "trace_pedantic")]
                    trace!("hold back shutdown");
                    self.pending_shutdown = true;
                }
            },
            ProtocolEvent::Message { data, sid } => {
                self.metrics.smsg_ib(sid, data.len() as u64);
                self.store.add(data, self.next_mid, sid);
                self.next_mid += 1;
            },
        }
        Ok(())
    }

    async fn flush(
        &mut self,
        bandwidth: Bandwidth,
        dt: Duration,
    ) -> Result</* actual */ Bandwidth, ProtocolError> {
        let (frames, total_bytes) = self.store.grab(bandwidth, dt);
        self.buffer.reserve(total_bytes as usize);
        let mut data_frames = 0;
        let mut data_bandwidth = 0;
        for (_, frame) in frames {
            if let OTFrame::Data { mid: _, data } = &frame {
                data_bandwidth += data.len();
                data_frames += 1;
            }
            frame.write_bytes(&mut self.buffer);
        }
        self.drain.send(self.buffer.split()).await?;
        self.metrics
            .sdata_frames_b(data_frames, data_bandwidth as u64);

        let mut finished_streams = vec![];
        for (i, &sid) in self.closing_streams.iter().enumerate() {
            if self.store.try_close_stream(sid) {
                #[cfg(feature = "trace_pedantic")]
                trace!(?sid, "close stream, as it's now empty");
                OTFrame::CloseStream { sid }.write_bytes(&mut self.buffer);
                self.drain.send(self.buffer.split()).await?;
                finished_streams.push(i);
            }
        }
        for i in finished_streams.iter().rev() {
            self.closing_streams.remove(*i);
        }

        let mut finished_streams = vec![];
        for (i, sid) in self.notify_closing_streams.iter().enumerate() {
            if self.store.try_close_stream(*sid) {
                #[cfg(feature = "trace_pedantic")]
                trace!(?sid, "close stream, as it's now empty");
                finished_streams.push(i);
            }
        }
        for i in finished_streams.iter().rev() {
            self.notify_closing_streams.remove(*i);
        }

        if self.pending_shutdown && self.store.is_empty() {
            #[cfg(feature = "trace_pedantic")]
            trace!("shutdown, as it's now empty");
            OTFrame::Shutdown {}.write_bytes(&mut self.buffer);
            self.drain.send(self.buffer.split()).await?;
            self.pending_shutdown = false;
        }
        Ok(data_bandwidth as u64)
    }
}

#[async_trait]
impl<S> RecvProtocol for TcpRecvProtocol<S>
where
    S: UnreliableSink<DataFormat = BytesMut>,
{
    async fn recv(&mut self) -> Result<ProtocolEvent, ProtocolError> {
        'outer: loop {
            loop {
                match ITFrame::read_frame(&mut self.buffer) {
                    Ok(Some(frame)) => {
                        #[cfg(feature = "trace_pedantic")]
                        trace!(?frame, "recv");
                        match frame {
                            ITFrame::Shutdown => break 'outer Ok(ProtocolEvent::Shutdown),
                            ITFrame::OpenStream {
                                sid,
                                prio,
                                promises,
                                guaranteed_bandwidth,
                            } => {
                                break 'outer Ok(ProtocolEvent::OpenStream {
                                    sid,
                                    prio: prio.min(crate::types::HIGHEST_PRIO),
                                    promises,
                                    guaranteed_bandwidth,
                                });
                            },
                            ITFrame::CloseStream { sid } => {
                                break 'outer Ok(ProtocolEvent::CloseStream { sid });
                            },
                            ITFrame::DataHeader { sid, mid, length } => {
                                let m = ITMessage::new(sid, length, &mut self.itmsg_allocator);
                                self.metrics.rmsg_ib(sid, length);
                                self.incoming.insert(mid, m);
                            },
                            ITFrame::Data { mid, data } => {
                                self.metrics.rdata_frames_b(data.len() as u64);
                                let m = match self.incoming.get_mut(&mid) {
                                    Some(m) => m,
                                    None => {
                                        log::info!(
                                            "protocol violation by remote side: send Data before \
                                             Header {}",
                                             mid
                                        );
                                        break 'outer Err(ProtocolError::Violated);
                                    },
                                };
                                m.data.extend_from_slice(&data);
                                if m.data.len() == m.length as usize {
                                    // finished, yay
                                    let m = self.incoming.remove(&mid).unwrap();
                                    self.metrics.rmsg_ob(
                                        m.sid,
                                        RemoveReason::Finished,
                                        m.data.len() as u64,
                                    );
                                    break 'outer Ok(ProtocolEvent::Message {
                                        sid: m.sid,
                                        data: m.data.freeze(),
                                    });
                                }
                            },
                        };
                    },
                    Ok(None) => break, //inner => read more data
                    Err(()) => return Err(ProtocolError::Violated),
                }
            }
            let chunk = self.sink.recv().await?;
            if self.buffer.is_empty() {
                self.buffer = chunk;
            } else {
                self.buffer.extend_from_slice(&chunk);
            }
        }
    }
}

#[async_trait]
impl<D> ReliableDrain for TcpSendProtocol<D>
where
    D: UnreliableDrain<DataFormat = BytesMut>,
{
    async fn send(&mut self, frame: InitFrame) -> Result<(), ProtocolError> {
        let mut buffer = BytesMut::with_capacity(500);
        frame.write_bytes(&mut buffer);
        self.drain.send(buffer).await
    }
}

#[async_trait]
impl<S> ReliableSink for TcpRecvProtocol<S>
where
    S: UnreliableSink<DataFormat = BytesMut>,
{
    async fn recv(&mut self) -> Result<InitFrame, ProtocolError> {
        while self.buffer.len() < 100 {
            let chunk = self.sink.recv().await?;
            self.buffer.extend_from_slice(&chunk);
            if let Some(frame) = InitFrame::read_frame(&mut self.buffer) {
                return Ok(frame);
            }
        }
        Err(ProtocolError::Violated)
    }
}
