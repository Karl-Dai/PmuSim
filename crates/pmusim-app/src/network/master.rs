use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use log::{error, info, warn};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::OwnedReadHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::task::JoinHandle;

use pmusim_core::protocol::builder::{build_command, build_config};
use pmusim_core::protocol::constants::{
    Cmd, FrameType, ProtocolVersion, IDCODE_LEN, SYNC_BYTE,
};
use pmusim_core::protocol::frame::{CommandFrame, ConfigFrame, Frame};
use pmusim_core::protocol::parser::parse;
use pmusim_core::time_utils::current_soc;
use crate::events::{ConfigInfo, DataInfo, PmuEvent};

/// Async event sink. Tauri side forwards these to `emit("pmu-event", ...)`;
/// tests collect them directly.
pub type EventSender = mpsc::UnboundedSender<PmuEvent>;
use crate::network::session::{SessionState, SubStationSession};

/// Internal command dispatched from the UI thread via mpsc.
#[derive(Debug)]
enum MasterCmd {
    Connect {
        host: String,
        port: u16,
        data_port: u16,        // 0 = use mgmt+1 default
        version: ProtocolVersion,
    },
    RequestCfg1 {
        idcode: String,
    },
    SendCfg2Cmd {
        idcode: String,
    },
    SendCfg2 {
        idcode: String,
        period: Option<u16>,
    },
    RequestCfg2 {
        idcode: String,
    },
    OpenData {
        idcode: String,
    },
    CloseData {
        idcode: String,
    },
    /// Master-side "联网触发" (CMD 0xA000, bit15-13=101) per V3 §8 表 3.
    /// Substation reacts implementation-defined; we just send the frame.
    Trigger {
        idcode: String,
    },
    AutoHandshake {
        idcode: String,
        period: Option<u16>,
    },
    Disconnect {
        idcode: String,
    },
}

pub struct MasterStation {
    pub data_port: u16,
    /// Heartbeat interval in milliseconds. Held as an Arc<AtomicU64> so the
    /// already-spawned `heartbeat_loop` reads the current value on every
    /// iteration — letting the UI re-tune it live without re-spawning the
    /// task.
    pub heartbeat_interval_ms: Arc<AtomicU64>,
    pub protocol: ProtocolVersion,
    pub sessions: Arc<RwLock<HashMap<String, SubStationSession>>>,
    cmd_tx: mpsc::Sender<MasterCmd>,
    cmd_rx: Option<mpsc::Receiver<MasterCmd>>,
    event_tx: EventSender,
    tasks: Vec<JoinHandle<()>>,
}

impl MasterStation {
    pub fn new(
        event_tx: EventSender,
        data_port: u16,
        heartbeat_interval: f64,
        protocol: ProtocolVersion,
    ) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel(64);
        Self {
            data_port,
            heartbeat_interval_ms: Arc::new(AtomicU64::new(
                (heartbeat_interval * 1000.0) as u64,
            )),
            protocol,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            cmd_tx,
            cmd_rx: Some(cmd_rx),
            event_tx,
            tasks: Vec::new(),
        }
    }

    /// Re-tune the heartbeat interval at runtime. The existing
    /// `heartbeat_loop` picks up the new value on its next iteration —
    /// no task respawn required.
    pub fn set_heartbeat_interval(&self, secs: f64) {
        let ms = (secs.max(0.1) * 1000.0) as u64;
        self.heartbeat_interval_ms.store(ms, Ordering::Relaxed);
    }

    /// Start the data TCP listener (V2 only), command loop, and heartbeat loop.
    /// V3 (master = data client) skips the listener; data_port is reset to 0.
    pub async fn start(&mut self) -> Result<(), String> {
        match self.protocol {
            ProtocolVersion::V2 => {
                let listener = TcpListener::bind(("0.0.0.0", self.data_port))
                    .await
                    .map_err(|e| format!("Failed to bind data port {}: {e}", self.data_port))?;
                self.data_port = listener
                    .local_addr()
                    .map(|a| a.port())
                    .unwrap_or(self.data_port);

                info!("MasterStation started (V2), data listener on port {}", self.data_port);

                let sessions = self.sessions.clone();
                let handle = self.event_tx.clone();
                self.tasks.push(tokio::spawn(async move {
                    Self::data_listener_loop(listener, sessions, handle).await;
                }));
            }
            ProtocolVersion::V3 => {
                self.data_port = 0;
                info!("MasterStation started (V3), no local data listener (master-outbound only)");
            }
        }

        // Spawn command loop.
        let cmd_rx = self
            .cmd_rx
            .take()
            .ok_or_else(|| "start() called twice".to_string())?;
        let cmd_tx_loopback = self.cmd_tx.clone();
        let sessions = self.sessions.clone();
        let handle = self.event_tx.clone();
        let hb_interval = self.heartbeat_interval_ms.clone();
        self.tasks.push(tokio::spawn(async move {
            Self::command_loop(cmd_rx, cmd_tx_loopback, sessions.clone(), handle.clone()).await;
        }));

        // Spawn heartbeat loop.
        let sessions = self.sessions.clone();
        let handle = self.event_tx.clone();
        self.tasks.push(tokio::spawn(async move {
            Self::heartbeat_loop(sessions, handle, hb_interval).await;
        }));

        Ok(())
    }

    /// Stop everything.
    pub async fn stop(&mut self) {
        for task in self.tasks.drain(..) {
            task.abort();
        }
        // Snapshot the live session idcodes BEFORE closing, then emit a
        // SessionDisconnected for each so the frontend's reactive session
        // map drops them. Without this, useSessions on the JS side keeps
        // q1234567 alive across stop→start cycles and the UI shows a
        // ghost session that never reconnects (selectedIdcode points at
        // a session the backend already destroyed).
        let live: Vec<String> = {
            let sessions_r = self.sessions.read().await;
            sessions_r.keys().cloned().collect()
        };
        let mut sessions = self.sessions.write().await;
        for session in sessions.values_mut() {
            session.close();
        }
        sessions.clear();
        drop(sessions);
        for idcode in live {
            let _ = self.event_tx.send(PmuEvent::SessionDisconnected { idcode });
        }
        info!("MasterStation stopped");
    }

    // --- Public command senders (called from tauri commands) ---

    pub async fn connect_to_substation(
        &self,
        host: String,
        mgmt_port: u16,
        data_port: u16,
        version: ProtocolVersion,
    ) -> Result<(), String> {
        self.cmd_tx
            .send(MasterCmd::Connect {
                host,
                port: mgmt_port,
                data_port,
                version,
            })
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn send_command(&self, idcode: String, cmd: String, period: Option<u32>) -> Result<(), String> {
        let mc = match cmd.as_str() {
            "request_cfg1" => MasterCmd::RequestCfg1 { idcode },
            "send_cfg2_cmd" => MasterCmd::SendCfg2Cmd { idcode },
            "send_cfg2" => MasterCmd::SendCfg2 {
                idcode,
                period: period.map(|p| p as u16),
            },
            "request_cfg2" => MasterCmd::RequestCfg2 { idcode },
            "open_data" => MasterCmd::OpenData { idcode },
            "close_data" => MasterCmd::CloseData { idcode },
            "trigger" => MasterCmd::Trigger { idcode },
            other => return Err(format!("Unknown command: {other}")),
        };
        self.cmd_tx.send(mc).await.map_err(|e| e.to_string())
    }

    pub async fn auto_handshake(&self, idcode: String, period: Option<u32>) -> Result<(), String> {
        self.cmd_tx
            .send(MasterCmd::AutoHandshake {
                idcode,
                period: period.map(|p| p as u16),
            })
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn disconnect_substation(&self, idcode: String) -> Result<(), String> {
        self.cmd_tx
            .send(MasterCmd::Disconnect { idcode })
            .await
            .map_err(|e| e.to_string())
    }

    // =========================================================================
    // Internal loops (run as spawned tasks)
    // =========================================================================

    /// Accept incoming data pipe connections from substations.
    async fn data_listener_loop(
        listener: TcpListener,
        sessions: Arc<RwLock<HashMap<String, SubStationSession>>>,
        event_tx: EventSender,
    ) {
        loop {
            let Ok((stream, addr)) = listener.accept().await else {
                break;
            };
            let peer_ip = addr.ip().to_string();
            info!("Data connection from {peer_ip}");

            let sessions = sessions.clone();
            let handle = event_tx.clone();
            tokio::spawn(async move {
                Self::handle_data_connection(stream, peer_ip, sessions, handle).await;
            });
        }
    }

    /// Handle a single inbound data pipe connection.
    async fn handle_data_connection(
        stream: TcpStream,
        peer_ip: String,
        sessions: Arc<RwLock<HashMap<String, SubStationSession>>>,
        event_tx: EventSender,
    ) {
        let (mut reader, writer) = stream.into_split();

        // Read first frame to determine version and idcode.
        let frame_data = match read_frame(&mut reader).await {
            Ok(d) => d,
            Err(e) => {
                warn!("Data connection read error: {e}");
                return;
            }
        };

        if frame_data.len() < 4 {
            return;
        }

        let sync = u16::from_be_bytes([frame_data[0], frame_data[1]]);
        let version = match pmusim_core::protocol::constants::parse_sync(sync) {
            Ok((_, v)) => v,
            Err(e) => {
                warn!("Invalid sync on data pipe: {e}");
                return;
            }
        };

        let session_idcode = if version == ProtocolVersion::V3 {
            // V3 data frames carry IDCODE at offset 4.
            if frame_data.len() < 4 + IDCODE_LEN {
                return;
            }
            String::from_utf8_lossy(&frame_data[4..4 + IDCODE_LEN])
                .trim_end_matches('\0')
                .to_string()
        } else {
            // V2: match by IP.
            let sessions_r = sessions.read().await;
            let found = sessions_r
                .values()
                .find(|s| s.peer_ip == peer_ip)
                .map(|s| s.idcode.clone());
            drop(sessions_r);
            match found {
                Some(id) => id,
                None => {
                    warn!("No mgmt session for V2 data connection from {peer_ip}");
                    return;
                }
            }
        };

        // Attach data writer to session.
        {
            let mut sessions_w = sessions.write().await;
            if let Some(session) = sessions_w.get_mut(&session_idcode) {
                session.data_writer = Some(writer);
            } else {
                // Create a minimal session if not yet known.
                let mut session = SubStationSession::new(session_idcode.clone(), version, peer_ip.clone());
                session.data_writer = Some(writer);
                sessions_w.insert(session_idcode.clone(), session);
                emit_event(
                    &event_tx,
                    PmuEvent::SessionCreated {
                        idcode: session_idcode.clone(),
                        peer_ip: peer_ip.clone(),
                    },
                );
            }
        }

        // Parse first data frame.
        {
            let sessions_r = sessions.read().await;
            if let Some(session) = sessions_r.get(&session_idcode) {
                if let Some(cfg2) = &session.cfg2 {
                    if let Ok(Frame::Data(df)) = parse(&frame_data, cfg2.format_flags, cfg2.phnmr, cfg2.annmr, cfg2.dgnmr) {
                        emit_event(
                            &event_tx,
                            PmuEvent::DataFrame {
                                idcode: session_idcode.clone(),
                                data: data_frame_to_info(&df),
                            },
                        );
                    }
                }
            }
        }

        // Continue reading data frames.
        loop {
            let frame_data = match read_frame(&mut reader).await {
                Ok(d) => d,
                Err(_) => break,
            };

            let sessions_r = sessions.read().await;
            if let Some(session) = sessions_r.get(&session_idcode) {
                if let Some(cfg2) = &session.cfg2 {
                    if let Ok(Frame::Data(df)) = parse(&frame_data, cfg2.format_flags, cfg2.phnmr, cfg2.annmr, cfg2.dgnmr) {
                        emit_event(
                            &event_tx,
                            PmuEvent::DataFrame {
                                idcode: session_idcode.clone(),
                                data: data_frame_to_info(&df),
                            },
                        );
                    }
                }
            }
            drop(sessions_r);

            emit_event(
                &event_tx,
                PmuEvent::RawFrame {
                    idcode: session_idcode.clone(),
                    direction: "recv".into(),
                    hex: hex_encode(&frame_data),
                },
            );
        }

        // Cleanup.
        let mut sessions_w = sessions.write().await;
        if let Some(session) = sessions_w.get_mut(&session_idcode) {
            session.data_writer = None;
            if !session.mgmt_connected() {
                session.state = SessionState::Disconnected;
                emit_event(
                    &event_tx,
                    PmuEvent::SessionDisconnected {
                        idcode: session_idcode.clone(),
                    },
                );
            }
        }
    }

    /// Process commands from the UI thread. Holds a self-loopback
    /// `cmd_tx` clone so spawn-only paths (e.g. data_read_loop_outbound
    /// reacting to STAT bit10) can re-enqueue AutoHandshake without
    /// having to `tokio::spawn` a non-Send future of their own.
    async fn command_loop(
        mut cmd_rx: mpsc::Receiver<MasterCmd>,
        cmd_tx: mpsc::Sender<MasterCmd>,
        sessions: Arc<RwLock<HashMap<String, SubStationSession>>>,
        event_tx: EventSender,
    ) {
        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                MasterCmd::Connect { host, port, data_port, version } => {
                    Self::do_connect(host, port, data_port, version, sessions.clone(), event_tx.clone()).await;
                }
                MasterCmd::RequestCfg1 { idcode } => {
                    Self::do_send_cmd(&sessions, &event_tx, &idcode, Cmd::SendCfg1 as u16).await;
                }
                MasterCmd::SendCfg2Cmd { idcode } => {
                    Self::do_send_cmd(&sessions, &event_tx, &idcode, Cmd::SendCfg2Cmd as u16).await;
                }
                MasterCmd::SendCfg2 { idcode, period } => {
                    Self::do_send_cfg2(&sessions, &event_tx, &idcode, period).await;
                }
                MasterCmd::RequestCfg2 { idcode } => {
                    Self::do_send_cmd(&sessions, &event_tx, &idcode, Cmd::SendCfg2 as u16).await;
                }
                MasterCmd::OpenData { idcode } => {
                    // Spec §6 requires CFG-1 → CFG-2 → OpenData. Without
                    // a CFG, data_read_loop_outbound has no dims; every
                    // frame parse fails silently yet the UI flips to
                    // Streaming and the user thinks it's working. Gate
                    // the command and emit a UI-visible error instead.
                    let state_ok = {
                        let r = sessions.read().await;
                        r.get(&idcode)
                            .map(|s| matches!(s.state, SessionState::Cfg2Sent | SessionState::Streaming))
                            .unwrap_or(false)
                    };
                    if !state_ok {
                        emit_event(&event_tx, PmuEvent::Error {
                            idcode: idcode.clone(),
                            error: "OpenData 拒绝: 必须先完成 CFG-1/CFG-2 握手".into(),
                        });
                        continue;
                    }
                    // For V3 the data pipe runs from master → substation, and the
                    // substation may start streaming the moment it receives
                    // OpenData. Open the pipe FIRST so the first frames are not
                    // lost to a race; do_open_data_v3 is a no-op for V2.
                    // If the V3 data pipe fails to open (timeout / refused),
                    // skip OpenData entirely — otherwise the UI flips to
                    // Streaming on a pipe that will never deliver frames.
                    if !Self::do_open_data_v3(&sessions, &cmd_tx, &event_tx, &idcode).await {
                        continue;
                    }
                    Self::do_send_cmd(&sessions, &event_tx, &idcode, Cmd::OpenData as u16).await;
                    let mut sessions_w = sessions.write().await;
                    if let Some(s) = sessions_w.get_mut(&idcode) {
                        s.state = SessionState::Streaming;
                    }
                    drop(sessions_w);
                    emit_event(&event_tx, PmuEvent::StreamingStarted { idcode });
                }
                MasterCmd::CloseData { idcode } => {
                    Self::do_send_cmd(&sessions, &event_tx, &idcode, Cmd::CloseData as u16).await;
                    let mut sessions_w = sessions.write().await;
                    if let Some(s) = sessions_w.get_mut(&idcode) {
                        s.state = SessionState::Cfg2Sent;
                    }
                    emit_event(&event_tx, PmuEvent::StreamingStopped { idcode });
                }
                MasterCmd::Trigger { idcode } => {
                    Self::do_send_cmd(&sessions, &event_tx, &idcode, Cmd::Trigger as u16).await;
                }
                MasterCmd::AutoHandshake { idcode, period } => {
                    Self::do_auto_handshake(&sessions, &cmd_tx, &event_tx, &idcode, period).await;
                }
                MasterCmd::Disconnect { idcode } => {
                    Self::do_disconnect(&sessions, &event_tx, &idcode).await;
                }
            }
        }
    }

    /// Send heartbeats periodically. The sleep interval is read fresh from
    /// `interval_ms` each iteration so a UI-driven `set_heartbeat_interval`
    /// takes effect on the next tick without restarting the loop.
    async fn heartbeat_loop(
        sessions: Arc<RwLock<HashMap<String, SubStationSession>>>,
        event_tx: EventSender,
        interval_ms: Arc<AtomicU64>,
    ) {
        loop {
            let ms = interval_ms.load(Ordering::Relaxed).max(100);
            tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;

            let idcodes: Vec<String> = {
                let sessions_r = sessions.read().await;
                sessions_r
                    .iter()
                    .filter(|(_, s)| s.mgmt_connected() && s.state != SessionState::Disconnected)
                    .map(|(id, _)| id.clone())
                    .collect()
            };

            // Three-interval grace before declaring timeout. Using elapsed
            // since last echo (instead of incrementing missed_heartbeats
            // unconditionally after send) closes the off-by-one race
            // where a prompt echo+counter-reset between send and the
            // post-send `++` left the steady-state counter at 1 — making
            // the actual disconnect threshold 2 missed beats, not 3.
            let timeout_ms = ms.saturating_mul(3);
            for idcode in idcodes {
                // Stamp the outbound SOC into the session BEFORE send so
                // process_mgmt_frame's echo check has a value to compare
                // (do_send_cmd uses current_soc() too — both call it within
                // the same second 99.99% of the time, but record what we
                // *will* send to keep the contract explicit).
                let outbound_soc = current_soc();
                {
                    let mut sessions_w = sessions.write().await;
                    if let Some(session) = sessions_w.get_mut(&idcode) {
                        session.pending_heartbeat_soc = Some(outbound_soc);
                    }
                }
                Self::do_send_cmd(&sessions, &event_tx, &idcode, Cmd::Heartbeat as u16).await;

                let mut sessions_w = sessions.write().await;
                if let Some(session) = sessions_w.get_mut(&idcode) {
                    let elapsed_ms = session.last_heartbeat.elapsed().as_millis() as u64;
                    // Mirror the counter for any consumer that still reads it.
                    session.missed_heartbeats = (elapsed_ms / ms.max(1)) as u32;
                    if elapsed_ms > timeout_ms {
                        session.state = SessionState::Disconnected;
                        emit_event(
                            &event_tx,
                            PmuEvent::HeartbeatTimeout {
                                idcode: idcode.clone(),
                            },
                        );
                    }
                }
            }
        }
    }

    // =========================================================================
    // Command helpers
    // =========================================================================

    /// Connect to a substation's management port (master = TCP client).
    async fn do_connect(
        host: String,
        port: u16,
        data_port: u16,
        version: ProtocolVersion,
        sessions: Arc<RwLock<HashMap<String, SubStationSession>>>,
        event_tx: EventSender,
    ) {
        let tmp_id = format!("{host}:{port}");

        // Insert a "pending" placeholder atomically so that a second, queued
        // connect to the same target sees it and bails out — *before* the TCP
        // SYN flight settles. Without this, click-spamming "连接" while the
        // first TcpStream::connect is still in progress queues N connect
        // commands; each one passes the duplicate guard (no session exists
        // yet) and tries its own TCP connect. The substation then sees a
        // storm of accept/EOF pairs as each successor session overwrites the
        // previous one. The placeholder also gives the duplicate guard
        // something to find even if the network is hung in SYN_SENT.
        {
            let mut sessions_w = sessions.write().await;
            let existing_id = sessions_w
                .values()
                .find(|s| {
                    s.peer_host == host
                        && s.peer_mgmt_port == port
                        && s.state != SessionState::Disconnected
                })
                .map(|s| s.idcode.clone());
            if let Some(existing_id) = existing_id {
                drop(sessions_w);
                warn!("Refusing duplicate connect to {host}:{port}; already connected/connecting as {existing_id}");
                emit_event(
                    &event_tx,
                    PmuEvent::Error {
                        idcode: existing_id.clone(),
                        error: format!(
                            "Already connected to {host}:{port} (session {existing_id}); disconnect first"
                        ),
                    },
                );
                return;
            }

            let mut placeholder = SubStationSession::new(tmp_id.clone(), version, host.clone());
            placeholder.peer_host = host.clone();
            placeholder.peer_mgmt_port = port;
            placeholder.peer_data_port = if data_port == 0 {
                port.saturating_add(1)
            } else {
                data_port
            };
            // No reader/writer yet — the TCP connect hasn't returned.
            sessions_w.insert(tmp_id.clone(), placeholder);
        }

        // Emit SessionCreated for the pending session so the UI shows "connecting…"
        emit_event(
            &event_tx,
            PmuEvent::SessionCreated {
                idcode: tmp_id.clone(),
                peer_ip: host.clone(),
            },
        );

        // Bounded TCP connect — fail fast instead of waiting for the OS's ~75s default.
        let connect_fut = TcpStream::connect((host.as_str(), port));
        let stream = match tokio::time::timeout(std::time::Duration::from_secs(5), connect_fut).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                error!("Failed to connect to {host}:{port}: {e}");
                sessions.write().await.remove(&tmp_id);
                emit_event(
                    &event_tx,
                    PmuEvent::Error {
                        idcode: tmp_id.clone(),
                        error: format!("Failed to connect {host}:{port}: {e}"),
                    },
                );
                emit_event(
                    &event_tx,
                    PmuEvent::SessionDisconnected { idcode: tmp_id },
                );
                return;
            }
            Err(_) => {
                error!("Timed out connecting to {host}:{port} after 5s");
                sessions.write().await.remove(&tmp_id);
                emit_event(
                    &event_tx,
                    PmuEvent::Error {
                        idcode: tmp_id.clone(),
                        error: format!("Connect to {host}:{port} timed out (5s)"),
                    },
                );
                emit_event(
                    &event_tx,
                    PmuEvent::SessionDisconnected { idcode: tmp_id },
                );
                return;
            }
        };

        let (reader, writer) = stream.into_split();

        // Attach the live socket to the placeholder we already inserted.
        let session_uid = {
            let mut sessions_w = sessions.write().await;
            let Some(session) = sessions_w.get_mut(&tmp_id) else {
                // Placeholder vanished (user clicked 断开 during the connect?). Drop the socket.
                return;
            };
            session.mgmt_reader = Some(reader);
            session.mgmt_writer = Some(Arc::new(tokio::sync::Mutex::new(writer)));
            session.uid
        };

        info!("Management pipe connected to {tmp_id}");

        // Spawn management read loop - needs to take ownership of the reader.
        let sessions2 = sessions.clone();
        let handle2 = event_tx.clone();
        tokio::spawn(async move {
            Self::mgmt_read_loop(tmp_id, session_uid, sessions2, handle2).await;
        });
    }

    /// Cleanly disconnect a session: remove it from the map (which drops
    /// reader/writer halves, closing both TCP pipes) and emit
    /// SessionDisconnected.
    async fn do_disconnect(
        sessions: &Arc<RwLock<HashMap<String, SubStationSession>>>,
        event_tx: &EventSender,
        idcode: &str,
    ) {
        let removed = {
            let mut sessions_w = sessions.write().await;
            sessions_w.remove(idcode)
        };
        if let Some(mut session) = removed {
            session.close();
            info!("Disconnected session {idcode}");
            emit_event(
                event_tx,
                PmuEvent::SessionDisconnected {
                    idcode: idcode.to_string(),
                },
            );
        }
    }

    /// Open the V3 data pipe to a substation. V3 (GB/T 26865.2-2011) inverts
    /// the data-pipe direction from V2: substation acts as a TCP server on
    /// its data port (mgmt_port + 1 by convention), master connects out.
    /// Without this, the substation's Bus queue piles up and we never see
    /// DataFrame events even though mgmt CFG/OpenData round-trips fine.
    ///
    /// Returns `true` if the caller may proceed to send Cmd::OpenData:
    /// `true` for V2 sessions (no-op — substation initiates data inbound),
    /// `true` for V3 sessions whose data pipe was already open or just
    /// connected successfully, and `false` for V3 sessions whose data
    /// connect failed (timeout / refused) so the caller can suppress the
    /// misleading StreamingStarted that would otherwise fire on a pipe
    /// that never opened.
    async fn do_open_data_v3(
        sessions: &Arc<RwLock<HashMap<String, SubStationSession>>>,
        cmd_tx: &mpsc::Sender<MasterCmd>,
        event_tx: &EventSender,
        idcode: &str,
    ) -> bool {
        let (peer_host, data_port, version, already_open) = {
            let sessions_r = sessions.read().await;
            let Some(s) = sessions_r.get(idcode) else {
                return false; // session vanished — don't proceed with OpenData
            };
            // peer_data_port was populated in do_connect; explicit override
            // wins, otherwise it's mgmt_port + 1 by GB/T 26865.2 convention.
            (
                s.peer_host.clone(),
                s.peer_data_port,
                s.version,
                s.data_connected(),
            )
        };

        if version != ProtocolVersion::V3 || already_open {
            return true;
        }

        info!("Opening V3 data pipe to {peer_host}:{data_port} for session {idcode}");

        let connect_fut = TcpStream::connect((peer_host.as_str(), data_port));
        let stream = match tokio::time::timeout(std::time::Duration::from_secs(5), connect_fut).await
        {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                error!("V3 data connect to {peer_host}:{data_port} failed: {e}");
                emit_event(
                    event_tx,
                    PmuEvent::Error {
                        idcode: idcode.to_string(),
                        error: format!("Data connect to {peer_host}:{data_port} failed: {e}"),
                    },
                );
                return false;
            }
            Err(_) => {
                error!("V3 data connect to {peer_host}:{data_port} timed out");
                emit_event(
                    event_tx,
                    PmuEvent::Error {
                        idcode: idcode.to_string(),
                        error: format!("Data connect to {peer_host}:{data_port} timed out (5s)"),
                    },
                );
                return false;
            }
        };

        let (reader, writer) = stream.into_split();
        let session_uid = {
            let mut sessions_w = sessions.write().await;
            let Some(s) = sessions_w.get_mut(idcode) else {
                return false; // session removed during connect
            };
            s.data_reader = Some(reader);
            s.data_writer = Some(writer);
            s.uid
        };

        let sessions2 = sessions.clone();
        let event_tx2 = event_tx.clone();
        let cmd_tx2 = cmd_tx.clone();
        let idcode2 = idcode.to_string();
        tokio::spawn(async move {
            Self::data_read_loop_outbound(idcode2, session_uid, sessions2, cmd_tx2, event_tx2).await;
        });
        true
    }

    /// Read loop for a V3 master-initiated outbound data pipe.
    /// Parses each frame using the session's cached CFG-2 (falling back to
    /// CFG-1) dimensions, emits DataFrame + RawFrame events. Cleanup is
    /// uid-checked so a stale loop never disturbs a successor session.
    async fn data_read_loop_outbound(
        idcode: String,
        my_uid: u64,
        sessions: Arc<RwLock<HashMap<String, SubStationSession>>>,
        cmd_tx: mpsc::Sender<MasterCmd>,
        event_tx: EventSender,
    ) {
        // Take the reader and snapshot the CFG dims under one lock so the
        // per-frame hot path (~97 fps observed in field testing) doesn't pay
        // an async RwLock acquire to recover invariants that don't change
        // after handshake.
        let (mut reader, mut dims) = {
            let mut sessions_w = sessions.write().await;
            let Some(s) = sessions_w.get_mut(&idcode).filter(|s| s.uid == my_uid) else {
                return;
            };
            let Some(reader) = s.data_reader.take() else {
                return;
            };
            let dims = s
                .cfg2
                .as_ref()
                .or(s.cfg1.as_ref())
                .map(|c| (c.format_flags, c.phnmr, c.annmr, c.dgnmr));
            (reader, dims)
        };

        loop {
            let frame_data = match read_frame(&mut reader).await {
                Ok(d) => d,
                Err(_) => break,
            };

            emit_event(
                &event_tx,
                PmuEvent::RawFrame {
                    idcode: idcode.clone(),
                    direction: "recv".into(),
                    hex: hex_encode(&frame_data),
                },
            );

            // If we didn't have CFG dims when the loop started (rare — pipe
            // opened before CFG-2 reply landed), look once and cache.
            if dims.is_none() {
                let sessions_r = sessions.read().await;
                dims = sessions_r
                    .get(&idcode)
                    .and_then(|s| s.cfg2.as_ref().or(s.cfg1.as_ref()))
                    .map(|c| (c.format_flags, c.phnmr, c.annmr, c.dgnmr));
            }
            let (format_flags, phnmr, annmr, dgnmr) = dims.unwrap_or((0, 0, 0, 0));

            if let Ok(Frame::Data(df)) = parse(&frame_data, format_flags, phnmr, annmr, dgnmr) {
                // STAT bit10 = "子站配置在最近 1 min 内发生改变" (§8.11 表
                // 12). Cached dims are now stale — once latched we
                // re-run auto_handshake to pull a fresh CFG-1/CFG-2 and
                // refresh dims. cfg_change_seen avoids spamming the
                // handshake every frame until the flag clears.
                let cfg_changed = (df.stat & 0x0400) != 0;
                if cfg_changed {
                    let should_refresh = {
                        let mut sessions_w = sessions.write().await;
                        match sessions_w.get_mut(&idcode).filter(|s| s.uid == my_uid) {
                            Some(s) if !s.cfg_change_seen => {
                                s.cfg_change_seen = true;
                                true
                            }
                            _ => false,
                        }
                    };
                    if should_refresh {
                        emit_event(&event_tx, PmuEvent::Error {
                            idcode: idcode.clone(),
                            error: "子站宣告配置变更 (STAT bit10),重新召唤 CFG…".into(),
                        });
                        // Re-enqueue handshake through command_loop instead
                        // of spawning here — that path is single-threaded
                        // and Send-safe; spawning do_auto_handshake from a
                        // data-pipe task would require a Send bound on the
                        // RwLock guards it holds across awaits.
                        let _ = cmd_tx.try_send(MasterCmd::AutoHandshake {
                            idcode: idcode.clone(),
                            period: None,
                        });
                        // Force dims re-fetch on the next iteration so the
                        // new CFG-2 takes effect immediately. cfg_change_seen
                        // is reset by the AutoHandshake completion path
                        // (do_auto_handshake clears it at the end).
                        dims = None;
                    }
                }

                emit_event(
                    &event_tx,
                    PmuEvent::DataFrame {
                        idcode: idcode.clone(),
                        data: data_frame_to_info(&df),
                    },
                );
            }
        }

        // Cleanup — only if we still own the slot.
        let mut sessions_w = sessions.write().await;
        if let Some(s) = sessions_w.get_mut(&idcode) {
            if s.uid != my_uid {
                return;
            }
            s.data_writer = None;
            if !s.mgmt_connected() {
                s.state = SessionState::Disconnected;
                emit_event(
                    &event_tx,
                    PmuEvent::SessionDisconnected { idcode },
                );
            }
        }
    }

    /// Read loop for an outbound management connection.
    ///
    /// `my_uid` is the SubStationSession::uid captured at spawn time. Every
    /// mutation cross-checks it so a stale loop (whose session has been
    /// removed or replaced under the same key) never touches the live state.
    async fn mgmt_read_loop(
        initial_id: String,
        my_uid: u64,
        sessions: Arc<RwLock<HashMap<String, SubStationSession>>>,
        event_tx: EventSender,
    ) {
        let mut current_id = initial_id.clone();

        // Take the reader out of the session so we can use it without holding the lock.
        let reader = {
            let mut sessions_w = sessions.write().await;
            sessions_w
                .get_mut(&current_id)
                .filter(|s| s.uid == my_uid)
                .and_then(|s| s.mgmt_reader.take())
        };
        let Some(mut reader) = reader else {
            return;
        };

        loop {
            let frame_data = match read_frame(&mut reader).await {
                Ok(d) => d,
                Err(_) => break,
            };

            let parsed = {
                // For command/config frames, phnmr/annmr/dgnmr are not needed.
                parse(&frame_data, 0, 0, 0, 0).ok()
            };

            // Re-key session on first real IDCODE.
            if let Some(ref frame) = parsed {
                let real_id = match frame {
                    Frame::Command(c) => Some(c.idcode.clone()),
                    Frame::Config(c) => Some(c.idcode.clone()),
                    Frame::Data(d) => Some(d.idcode.clone()),
                };
                if let Some(real_id) = real_id {
                    if !real_id.is_empty() && real_id != current_id {
                        let old_id = current_id.clone();
                        let peer_ip_for_event;
                        let displaced_real_id;
                        {
                            let mut sessions_w = sessions.write().await;
                            // Only re-key if the slot at current_id is still ours.
                            let owned = sessions_w
                                .get(&current_id)
                                .map(|s| s.uid == my_uid)
                                .unwrap_or(false);
                            if !owned {
                                // Slot was taken over by a newer session — abandon this loop.
                                return;
                            }
                            // If a prior session is already living under real_id,
                            // evict it explicitly so the frontend knows it's gone;
                            // silently overwriting would drop its cached cfg1/cfg2
                            // and leave the frontend with a ghost row.
                            displaced_real_id = if sessions_w.contains_key(&real_id) {
                                sessions_w.remove(&real_id).map(|mut old| {
                                    old.close();
                                    real_id.clone()
                                })
                            } else {
                                None
                            };

                            let Some(mut session) = sessions_w.remove(&current_id) else {
                                return;
                            };
                            let frame_version = match frame {
                                Frame::Command(c) => c.version,
                                Frame::Config(c) => c.version,
                                Frame::Data(d) => d.version,
                            };
                            session.version = frame_version;
                            session.idcode = real_id.clone();
                            peer_ip_for_event = session.peer_ip.clone();
                            sessions_w.insert(real_id.clone(), session);
                        }

                        // Tell the frontend the placeholder row is gone, plus any
                        // displaced same-IDCODE record, then announce the re-keyed
                        // session. Without the placeholder-disconnect emit, the UI
                        // accumulates a ghost "host:port" row on every connect.
                        emit_event(
                            &event_tx,
                            PmuEvent::SessionDisconnected { idcode: old_id },
                        );
                        if let Some(displaced) = displaced_real_id {
                            emit_event(
                                &event_tx,
                                PmuEvent::SessionDisconnected { idcode: displaced },
                            );
                        }
                        emit_event(
                            &event_tx,
                            PmuEvent::SessionCreated {
                                idcode: real_id.clone(),
                                peer_ip: peer_ip_for_event,
                            },
                        );
                        current_id = real_id;
                    }
                }
            }

            // Process the frame.
            if let Some(frame) = parsed {
                Self::process_mgmt_frame(&sessions, &event_tx, &current_id, &frame, &frame_data)
                    .await;
            }
        }

        // Cleanup on disconnect — only if the slot is still ours.
        let mut sessions_w = sessions.write().await;
        if let Some(session) = sessions_w.get_mut(&current_id) {
            if session.uid != my_uid {
                return;
            }
            session.mgmt_writer = None;
            if !session.data_connected() {
                session.state = SessionState::Disconnected;
                emit_event(
                    &event_tx,
                    PmuEvent::SessionDisconnected {
                        idcode: current_id,
                    },
                );
            }
        }
    }

    /// Process a frame received on the management pipe.
    async fn process_mgmt_frame(
        sessions: &Arc<RwLock<HashMap<String, SubStationSession>>>,
        event_tx: &EventSender,
        idcode: &str,
        frame: &Frame,
        raw: &[u8],
    ) {
        emit_event(
            event_tx,
            PmuEvent::RawFrame {
                idcode: idcode.to_string(),
                direction: "recv".into(),
                hex: hex_encode(raw),
            },
        );

        match frame {
            Frame::Command(cmd) => {
                if cmd.cmd == Cmd::Heartbeat as u16 {
                    // §8.13: substation echoes the same SOC. If the echo
                    // doesn't match the last outbound heartbeat, do NOT
                    // reset liveness — it's either a replay or a stale
                    // beat from before the last clock change. Log it for
                    // diagnosis but keep counters running.
                    let mut sessions_w = sessions.write().await;
                    if let Some(session) = sessions_w.get_mut(idcode) {
                        let echo_ok = match session.pending_heartbeat_soc.take() {
                            // ±1s tolerance: the loop stamps an outbound
                            // SOC reference, then do_send_cmd later calls
                            // current_soc() inside the actual frame
                            // build. A one-second tick between those two
                            // is normal and harmless.
                            Some(expected) => cmd.soc.abs_diff(expected) <= 1,
                            None => true, // no pending — accept (covers boot race)
                        };
                        if echo_ok {
                            session.last_heartbeat = std::time::Instant::now();
                            session.missed_heartbeats = 0;
                        } else {
                            warn!("{idcode}: heartbeat SOC echo mismatch (got {})", cmd.soc);
                        }
                    }
                } else if cmd.cmd == Cmd::Ack as u16 || cmd.cmd == Cmd::Nack as u16 {
                    // Deliver to whoever is awaiting (do_auto_handshake step).
                    // Without this, NACK on CFG-2 download is silently ignored
                    // and we proceed to OpenData on a half-broken handshake.
                    let tx = {
                        let mut sessions_w = sessions.write().await;
                        sessions_w.get_mut(idcode).and_then(|s| s.pending_ack.take())
                    };
                    if let Some(tx) = tx {
                        let _ = tx.send(cmd.cmd);
                    }
                }
            }
            Frame::Config(cfg) => {
                if cfg.cfg_type == FrameType::Cfg1 as u8 {
                    let info = ConfigInfo::from(cfg);
                    let mut sessions_w = sessions.write().await;
                    if let Some(session) = sessions_w.get_mut(idcode) {
                        session.cfg1 = Some(cfg.clone());
                        session.state = SessionState::Cfg1Received;
                    }
                    drop(sessions_w);
                    emit_event(
                        event_tx,
                        PmuEvent::Cfg1Received {
                            idcode: idcode.to_string(),
                            cfg: info,
                        },
                    );
                } else if cfg.cfg_type == FrameType::Cfg2 as u8 {
                    let info = ConfigInfo::from(cfg);
                    let mut sessions_w = sessions.write().await;
                    if let Some(session) = sessions_w.get_mut(idcode) {
                        session.cfg2 = Some(cfg.clone());
                    }
                    drop(sessions_w);
                    emit_event(
                        event_tx,
                        PmuEvent::Cfg2Received {
                            idcode: idcode.to_string(),
                            cfg: info,
                        },
                    );
                }
            }
            Frame::Data(_) => {
                // Data on management pipe is unusual; ignore.
            }
        }
    }

    /// Send a command frame to a substation.
    async fn do_send_cmd(
        sessions: &Arc<RwLock<HashMap<String, SubStationSession>>>,
        event_tx: &EventSender,
        idcode: &str,
        cmd: u16,
    ) {
        let (version, has_writer) = {
            let sessions_r = sessions.read().await;
            match sessions_r.get(idcode) {
                Some(s) => (s.version, s.mgmt_connected()),
                None => return,
            }
        };

        if !has_writer {
            emit_event(
                event_tx,
                PmuEvent::Error {
                    idcode: idcode.to_string(),
                    error: "Management pipe not connected".into(),
                },
            );
            return;
        }

        let frame = CommandFrame {
            version,
            idcode: idcode.to_string(),
            soc: current_soc(),
            fracsec: 0,
            cmd,
        };

        let raw = match build_command(&frame) {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to build command: {e}");
                return;
            }
        };

        // Clone the writer Arc out of the sessions map under a brief
        // read lock; do the actual TCP write_all+flush on the writer's
        // own mutex, NOT under sessions.write(). That way one slow peer
        // can't block heartbeat_loop / other do_send_cmd invocations
        // for unrelated sessions.
        let writer_arc = {
            let r = sessions.read().await;
            r.get(idcode).and_then(|s| s.mgmt_writer.clone())
        };
        let write_err: Option<String> = match writer_arc {
            Some(arc) => {
                let mut writer = arc.lock().await;
                match writer.write_all(&raw).await {
                    Ok(()) => {
                        let _ = writer.flush().await;
                        None
                    }
                    Err(e) => Some(e.to_string()),
                }
            }
            None => Some("Management pipe vanished mid-send".into()),
        };

        if let Some(err) = write_err {
            // Tear the session down now so heartbeat_loop + the UI stop
            // pretending it's alive — without this we kept the session
            // in Streaming/Cfg2Sent until mgmt_read_loop eventually
            // noticed EOF, leaking false "online" status for seconds.
            error!("write_all to {idcode} failed: {err}");
            let mut sessions_w = sessions.write().await;
            if let Some(session) = sessions_w.get_mut(idcode) {
                session.close();
            }
            drop(sessions_w);
            emit_event(event_tx, PmuEvent::Error {
                idcode: idcode.to_string(),
                error: format!("命令发送失败,管道已关闭: {err}"),
            });
            emit_event(event_tx, PmuEvent::SessionDisconnected {
                idcode: idcode.to_string(),
            });
            return;
        }

        emit_event(
            event_tx,
            PmuEvent::RawFrame {
                idcode: idcode.to_string(),
                direction: "send".into(),
                hex: hex_encode(&raw),
            },
        );
    }

    /// Build and send CFG-2 based on the stored CFG-1 template.
    async fn do_send_cfg2(
        sessions: &Arc<RwLock<HashMap<String, SubStationSession>>>,
        event_tx: &EventSender,
        idcode: &str,
        period: Option<u16>,
    ) {
        // Build cfg2 from cfg1.
        let cfg2 = {
            let sessions_r = sessions.read().await;
            let session = match sessions_r.get(idcode) {
                Some(s) => s,
                None => return,
            };
            if !session.mgmt_connected() {
                emit_event(
                    event_tx,
                    PmuEvent::Error {
                        idcode: idcode.to_string(),
                        error: "Management pipe not connected".into(),
                    },
                );
                return;
            }
            let cfg1 = match &session.cfg1 {
                Some(c) => c,
                None => {
                    emit_event(
                        event_tx,
                        PmuEvent::Error {
                            idcode: idcode.to_string(),
                            error: "No CFG-1 available".into(),
                        },
                    );
                    return;
                }
            };

            // §6 "主站宜具有 CFG1/CFG2 校验机制": every block must have
            // channel_names.len() == phnmr + annmr + 16*dgnmr, otherwise
            // build_config silently produces a misaligned frame the
            // substation will CRC-fail and drop, leaving the handshake
            // deadlocked with no diagnostic.
            let blocks_to_check: Vec<_> = if cfg1.pmu_blocks.is_empty() {
                vec![(cfg1.phnmr, cfg1.annmr, cfg1.dgnmr, cfg1.channel_names.len())]
            } else {
                cfg1.pmu_blocks.iter()
                    .map(|b| (b.phnmr, b.annmr, b.dgnmr, b.channel_names.len()))
                    .collect()
            };
            for (i, (phnmr, annmr, dgnmr, names_len)) in blocks_to_check.iter().enumerate() {
                let expected = *phnmr as usize + *annmr as usize + (*dgnmr as usize) * 16;
                if *names_len != expected {
                    emit_event(event_tx, PmuEvent::Error {
                        idcode: idcode.to_string(),
                        error: format!(
                            "CFG-1 PMU#{i} 通道名个数不符: 期望 {expected}(PHNMR={phnmr}+ANNMR={annmr}+16×DGNMR={dgnmr}),实际 {names_len};拒绝下传 CFG-2"
                        ),
                    });
                    return;
                }
            }

            // Carry forward every PMU block from cfg1; only PMU #0 takes
            // the user-supplied PERIOD override (the others keep their
            // CFG-1 cadence). Without copying pmu_blocks the builder
            // would fall back to the top-level convenience fields and
            // silently downgrade a multi-PMU substation to one PMU.
            let mut blocks = cfg1.pmu_blocks.clone();
            if let Some(p) = period {
                if let Some(first) = blocks.first_mut() {
                    first.period = p;
                }
            }
            ConfigFrame {
                version: cfg1.version,
                cfg_type: FrameType::Cfg2 as u8,
                idcode: cfg1.idcode.clone(),
                soc: current_soc(),
                fracsec: 0,
                d_frame: cfg1.d_frame,
                meas_rate: cfg1.meas_rate,
                num_pmu: cfg1.num_pmu,
                stn: cfg1.stn.clone(),
                pmu_idcode: cfg1.pmu_idcode.clone(),
                format_flags: cfg1.format_flags,
                phnmr: cfg1.phnmr,
                annmr: cfg1.annmr,
                dgnmr: cfg1.dgnmr,
                channel_names: cfg1.channel_names.clone(),
                phunit: cfg1.phunit.clone(),
                anunit: cfg1.anunit.clone(),
                digunit: cfg1.digunit.clone(),
                fnom: cfg1.fnom,
                period: period.unwrap_or(cfg1.period),
                pmu_blocks: blocks,
            }
        };

        let raw = match build_config(&cfg2) {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to build CFG-2: {e}");
                return;
            }
        };

        // Write outside the sessions lock — same rationale as do_send_cmd
        // (don't block other sessions on this peer's TCP buffer drain).
        let writer_arc = {
            let r = sessions.read().await;
            r.get(idcode).and_then(|s| s.mgmt_writer.clone())
        };
        if let Some(arc) = writer_arc {
            let mut writer = arc.lock().await;
            if let Err(e) = writer.write_all(&raw).await {
                error!("Failed to send CFG-2 to {idcode}: {e}");
                return;
            }
            let _ = writer.flush().await;
        }
        // State + cfg2 cache update under a brief write lock.
        {
            let mut sessions_w = sessions.write().await;
            if let Some(session) = sessions_w.get_mut(idcode) {
                session.cfg2 = Some(cfg2);
                // Mid-stream rate change pushes a fresh CFG-2 without
                // tearing down the data pipe — keep Streaming so the UI
                // doesn't bounce back to "已下传 CFG-2".
                if session.state != SessionState::Streaming {
                    session.state = SessionState::Cfg2Sent;
                }
            }
        }

        emit_event(
            event_tx,
            PmuEvent::RawFrame {
                idcode: idcode.to_string(),
                direction: "send".into(),
                hex: hex_encode(&raw),
            },
        );
        emit_event(
            event_tx,
            PmuEvent::Cfg2Sent {
                idcode: idcode.to_string(),
            },
        );
    }

    /// Install a one-shot waiter into the session's `pending_ack` slot.
    /// Returns the receiver; the next ACK/NACK CMD frame received on the
    /// mgmt pipe will fill it (see `process_mgmt_frame`).
    async fn install_ack_waiter(
        sessions: &Arc<RwLock<HashMap<String, SubStationSession>>>,
        idcode: &str,
    ) -> Option<oneshot::Receiver<u16>> {
        let (tx, rx) = oneshot::channel();
        let mut sessions_w = sessions.write().await;
        let s = sessions_w.get_mut(idcode)?;
        s.pending_ack = Some(tx);
        Some(rx)
    }

    /// Await up to 2s for the substation's ACK/NACK reply. `true` on ACK
    /// (0xE000); `false` on NACK (0x2000) or timeout — with a UI-visible
    /// Error event so the user sees why the handshake stalled.
    async fn wait_for_ack(
        event_tx: &EventSender,
        idcode: &str,
        rx: oneshot::Receiver<u16>,
        step: &str,
    ) -> bool {
        match tokio::time::timeout(std::time::Duration::from_secs(2), rx).await {
            Ok(Ok(cmd)) if cmd == Cmd::Ack as u16 => true,
            Ok(Ok(cmd)) if cmd == Cmd::Nack as u16 => {
                emit_event(event_tx, PmuEvent::Error {
                    idcode: idcode.to_string(),
                    error: format!("{step}: 子站 NACK,握手中止"),
                });
                false
            }
            Ok(Ok(other)) => {
                emit_event(event_tx, PmuEvent::Error {
                    idcode: idcode.to_string(),
                    error: format!("{step}: 子站回了非 ACK/NACK CMD={other:#06x}"),
                });
                false
            }
            Ok(Err(_)) => {
                emit_event(event_tx, PmuEvent::Error {
                    idcode: idcode.to_string(),
                    error: format!("{step}: ACK 等待通道关闭"),
                });
                false
            }
            Err(_) => {
                emit_event(event_tx, PmuEvent::Error {
                    idcode: idcode.to_string(),
                    error: format!("{step}: ACK 等待超时 (2s)"),
                });
                false
            }
        }
    }

    /// Install waiter → send command → wait for ACK. Combined helper for
    /// steps that take a single CMD frame (e.g. SendCfg2Cmd).
    async fn do_send_cmd_await_ack(
        sessions: &Arc<RwLock<HashMap<String, SubStationSession>>>,
        event_tx: &EventSender,
        idcode: &str,
        cmd: u16,
        step: &str,
    ) -> bool {
        let Some(rx) = Self::install_ack_waiter(sessions, idcode).await else {
            emit_event(event_tx, PmuEvent::Error {
                idcode: idcode.to_string(),
                error: format!("{step}: session 已消失"),
            });
            return false;
        };
        Self::do_send_cmd(sessions, event_tx, idcode, cmd).await;
        Self::wait_for_ack(event_tx, idcode, rx, step).await
    }

    /// Automated handshake sequence. After SendCfg1 the substation's real
    /// IDCODE arrives and the session is re-keyed, so we resolve the current
    /// idcode via peer_host:peer_mgmt_port before each subsequent step.
    async fn do_auto_handshake(
        sessions: &Arc<RwLock<HashMap<String, SubStationSession>>>,
        cmd_tx: &mpsc::Sender<MasterCmd>,
        event_tx: &EventSender,
        idcode: &str,
        period: Option<u16>,
    ) {
        // Capture peer identity so we can follow the session across re-key.
        let peer = {
            let r = sessions.read().await;
            r.get(idcode)
                .map(|s| (s.peer_host.clone(), s.peer_mgmt_port))
        };
        let Some((peer_host, peer_port)) = peer else {
            emit_event(
                event_tx,
                PmuEvent::Error {
                    idcode: idcode.to_string(),
                    error: "Session not found".into(),
                },
            );
            return;
        };

        // Step 1: Request CFG-1.
        Self::do_send_cmd(sessions, event_tx, idcode, Cmd::SendCfg1 as u16).await;

        // Wait up to 2s for CFG-1 to arrive (idcode may have been re-keyed).
        let current = match wait_for_cfg1(
            sessions,
            &peer_host,
            peer_port,
            std::time::Duration::from_secs(2),
        )
        .await
        {
            Some(id) => id,
            None => {
                emit_event(
                    event_tx,
                    PmuEvent::Error {
                        idcode: idcode.to_string(),
                        error: "CFG-1 not received after request".into(),
                    },
                );
                return;
            }
        };

        // Step 2: 下传 CFG-2 命令 → expect ACK (V3 §8.4).
        if !Self::do_send_cmd_await_ack(
            sessions, event_tx, &current, Cmd::SendCfg2Cmd as u16, "SendCfg2Cmd",
        )
        .await
        {
            return;
        }

        // Step 3: 下传 CFG-2 配置帧 → expect ACK (V3 §8.6).
        let Some(rx3) = Self::install_ack_waiter(sessions, &current).await else {
            emit_event(event_tx, PmuEvent::Error {
                idcode: current.clone(),
                error: "CFG-2 帧: session 已消失".into(),
            });
            return;
        };
        Self::do_send_cfg2(sessions, event_tx, &current, period).await;
        if !Self::wait_for_ack(event_tx, &current, rx3, "CFG-2 帧").await {
            return;
        }

        // Step 4: 召唤 CFG-2 → substation re-uploads CFG-2 frame (not ACK).
        // No ACK to wait on here; the Cfg2Received event signals completion.
        Self::do_send_cmd(sessions, event_tx, &current, Cmd::SendCfg2 as u16).await;
        // Brief settle — wait_for_cfg2 here would be cleaner but we don't
        // currently track it; 500ms is enough for a healthy LAN substation.
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Step 5: Open the V3 data pipe (no-op for V2), then send OpenData.
        // Order matters: substation may stream the moment it receives OpenData,
        // so the pipe needs to exist first or initial frames are lost. If the
        // V3 data pipe fails (timeout/refused), skip OpenData — do_open_data_v3
        // already emitted an Error event and StreamingStarted would lie.
        if !Self::do_open_data_v3(sessions, cmd_tx, event_tx, &current).await {
            return;
        }
        Self::do_send_cmd(sessions, event_tx, &current, Cmd::OpenData as u16).await;
        {
            let mut sessions_w = sessions.write().await;
            if let Some(session) = sessions_w.get_mut(&current) {
                session.state = SessionState::Streaming;
                // Clear the STAT bit10 latch so the next config-change
                // observation can re-trigger a refresh handshake.
                session.cfg_change_seen = false;
            }
        }
        emit_event(
            event_tx,
            PmuEvent::StreamingStarted {
                idcode: current,
            },
        );
    }
}

// =============================================================================
// Free helpers
// =============================================================================

/// Poll `sessions` for a session whose peer matches `(host, port)` and that
/// has received a CFG-1. Returns the session's current idcode (may differ
/// from the original tmp_id after re-key) or `None` on timeout.
async fn wait_for_cfg1(
    sessions: &Arc<RwLock<HashMap<String, SubStationSession>>>,
    peer_host: &str,
    peer_port: u16,
    timeout: std::time::Duration,
) -> Option<String> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        {
            let r = sessions.read().await;
            for (id, s) in r.iter() {
                if s.peer_host == peer_host
                    && s.peer_mgmt_port == peer_port
                    && s.cfg1.is_some()
                {
                    return Some(id.clone());
                }
            }
        }
        if tokio::time::Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

/// Read a complete frame from a TCP stream.
async fn read_frame(reader: &mut OwnedReadHalf) -> Result<Vec<u8>, String> {
    let mut header = [0u8; 4];
    reader
        .read_exact(&mut header)
        .await
        .map_err(|e| format!("read header: {e}"))?;

    if header[0] != SYNC_BYTE {
        return Err(format!("Invalid sync byte: {:#04x}", header[0]));
    }

    let frame_size = u16::from_be_bytes([header[2], header[3]]) as usize;
    if frame_size < 4 {
        return Err(format!("Invalid frame size: {frame_size}"));
    }

    let mut buf = vec![0u8; frame_size];
    buf[..4].copy_from_slice(&header);
    reader
        .read_exact(&mut buf[4..])
        .await
        .map_err(|e| format!("read body: {e}"))?;

    Ok(buf)
}

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

fn data_frame_to_info(df: &pmusim_core::protocol::frame::DataFrame) -> DataInfo {
    DataInfo {
        soc: df.soc,
        fracsec: df.fracsec,
        stat: df.stat,
        format_flags: df.format_flags,
        time_quality: ((df.fracsec >> 24) & 0x0F) as u8,
        freq: df.freq,
        dfreq: df.dfreq,
        analog: df.analog.clone(),
        digital: df.digital.clone(),
        phasors: df.phasors.clone(),
    }
}

fn emit_event(event_tx: &EventSender, event: PmuEvent) {
    if let Err(e) = event_tx.send(event) {
        error!("Failed to emit event: {e}");
    }
}
