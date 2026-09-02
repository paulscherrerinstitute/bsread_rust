use std::sync::{Arc, Mutex, RwLock};
use zmq::{SocketType, SocketEvent, Context};
use std::collections::HashMap;
use std::io::SeekFrom::End;
use std::thread;
use md5::digest::consts::P1;
use serde::Serialize;
use uuid::Uuid;
use crate::{IOResult, Receiver};
use std::sync::atomic::{AtomicU32, Ordering};
use crate::utils::app_name;

#[derive(Clone, Debug)]
pub enum Transport {
    Tcp { port: u32, host: Option<String> },
    Ipc { name: Option<String> },
}
pub const IPC_FILE_PREFIX:&str = "/bsread_icp_";

//pub fn local_address() -> String { "127.0.0.1".to_string()}
// pub fn local_address() -> &'static str {"*"}
pub fn local_address() -> &'static str {"0.0.0.0"}

pub fn ipc_feeds_folder() -> &'static str {"/tmp"}
impl Transport {
    pub fn endpoint(&self) -> String {
        match self {
            Transport::Tcp {port, host} => {
                let host = host.clone().unwrap_or(local_address().to_string());
                if host.contains("://") {
                    format!("{}:{}", host, port)
                } else {
                    format!("tcp://{}:{}", host, port)
                }
            },
            Transport::Ipc { name } => {
                let folder = ipc_feeds_folder();
                let suffix = match name{
                    None => {
                        match app_name(){
                            None => {"test".to_string()},
                            Some(app) => {app}
                        }
                    }
                    Some(str) => {str.clone()}
                };
                //let path = Path::new(folder.as_str());
                //fs::create_dir_all(path).expect("Failed to create ipc feeds folder");
                format!("ipc://{}{}{}", folder, IPC_FILE_PREFIX, suffix)
            },
        }
    }

    pub fn from_endpoint(endpoint: &str) -> Result<Self, String> {
        if let Some(rest) = endpoint.strip_prefix("tcp://") {
            let (host, port_str) = rest
                .rsplit_once(':')
                .ok_or_else(|| format!("Invalid TCP endpoint: {endpoint}"))?;

            let port = port_str
                .parse::<u32>()
                .map_err(|_| format!("Invalid TCP port: {port_str}"))?;

            let host = if host == local_address() {
                None
            } else {
                Some(host.to_string())
            };

            Ok(Transport::Tcp {port, host})
        } else if let Some(rest) = endpoint.strip_prefix("ipc://") {
            let (_, name) = rest
                .rsplit_once(IPC_FILE_PREFIX)
                .ok_or_else(|| format!("Invalid IPC endpoint: {endpoint}"))?;

            Ok(Transport::Ipc { name : Some(name.to_string())})
        } else {
            Err(format!("Unsupported endpoint: {endpoint}"))
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeepAlive {
    pub idle: i32,
    pub intvl: i32,
    pub cnt: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Heartbeat {
    pub ivl: i32,
    pub timeout: i32,
    pub ttl: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SocketOptions{
    pub linger : Option<i32>,
    pub rcvhwm : Option<i32>,
    pub sndhwm : Option<i32>,
    pub handshake_ivl: Option<i32>,
    pub keepalive: Option<KeepAlive>,
    pub heartbeat: Option<Heartbeat>,
}

impl SocketOptions {
    pub fn new() -> Self {
        Self{linger:None, rcvhwm:None, sndhwm:None, handshake_ivl:None, keepalive:None, heartbeat:None}
    }

    pub fn get(socket: &zmq::Socket) -> Self {
        let linger = socket.get_linger().ok();
        let rcvhwm = socket.get_rcvhwm().ok();
        let sndhwm = socket.get_sndhwm().ok();
        let handshake_ivl = socket.get_handshake_ivl().ok();

        let keepalive = if let Ok(ka) = socket.get_tcp_keepalive() && ka>0{
            Some(KeepAlive{ idle:socket.get_tcp_keepalive_idle().ok().unwrap_or(0),
                            intvl:socket.get_tcp_keepalive_intvl().ok().unwrap_or(0),
                            cnt: socket.get_tcp_keepalive_cnt().ok().unwrap_or(0)})
        } else {
            None
        };

        let hb  = socket.get_heartbeat_ivl().unwrap_or(0);
        let heartbeat = if hb>0{
            Some (Heartbeat { ivl: hb,
                              timeout: socket.get_heartbeat_timeout().ok().unwrap_or(0),
                              ttl:socket.get_heartbeat_ttl().ok().unwrap_or(0),
            })
        } else {
            None
        };
        Self{linger, rcvhwm, sndhwm, handshake_ivl, keepalive, heartbeat}
    }

    pub fn set(self:& SocketOptions, socket: &zmq::Socket) -> IOResult<()>{
        if let Some(linger) = self.linger {
            socket.set_linger(linger)?;
        }
        if let Some(rcvhwm) = self.rcvhwm {
            socket.set_rcvhwm(rcvhwm)?;
        }
        if let Some(sndhwm) = self.sndhwm {
            socket.set_sndhwm(sndhwm)?;
        }
        if let Some(handshake_ivl) = self.handshake_ivl {
            socket.set_handshake_ivl(handshake_ivl)?;
        }
        if let Some(keepalive) = &self.keepalive {
            set_socket_keepalive(socket, keepalive.idle, keepalive.intvl, keepalive.cnt)?;
        }
        if let Some(heartbeat) = &self.heartbeat {
            set_socket_heartbeat(socket, heartbeat.ivl, heartbeat.timeout, heartbeat.ttl)?;
        }
        Ok(())
     }

}

pub fn set_socket_keepalive(socket: &zmq::Socket, idle: i32, intvl: i32, cnt: i32) -> IOResult<()> {
    if !is_socket_ipc(socket) {
        socket.set_tcp_keepalive(1)?;
        socket.set_tcp_keepalive_idle(idle)?;
        socket.set_tcp_keepalive_intvl(intvl)?;
        socket.set_tcp_keepalive_cnt(cnt)?;
    }
    Ok(())
}

pub fn set_socket_heartbeat(socket: &zmq::Socket, ivl: i32, timeout: i32, ttl: i32) -> IOResult<()> {
    socket.set_heartbeat_ivl(ivl)?;
    socket.set_heartbeat_timeout(timeout)?;
    socket.set_heartbeat_ttl(ttl)?;
    Ok(())
}

pub fn set_socket_linger(socket: &zmq::Socket, value:i32) -> IOResult<()> {
    socket.set_linger(value)?;
    Ok(())
}

pub fn set_socket_rcvhwm(socket: &zmq::Socket, value:i32) -> IOResult<()> {
    socket.set_rcvhwm(value)?;
    Ok(())
}

pub fn set_socket_sndhwm(socket: &zmq::Socket, value:i32) -> IOResult<()> {
    socket.set_sndhwm(value)?;
    Ok(())
}

pub fn set_handshake_ivl(socket: &zmq::Socket, value:i32) -> IOResult<()> {
    socket.set_handshake_ivl(value)?;
    Ok(())
}

pub fn is_socket_ipc(socket: &zmq::Socket) -> bool {
    if let Ok(last_endpoint) = socket.get_last_endpoint() {
        if let Ok((endpoint)) = last_endpoint {
            return endpoint.starts_with("ipc://");
        };
    };
    false
}

pub trait SocketConfig {
    fn socket(&self) ->  Option<&zmq::Socket>;
    fn set_options(&self, options: &SocketOptions) -> IOResult<()>{
        if let Some(socket) = self.socket() {
            options.set(socket)?;
        }
        Ok(())
    }
    fn set_linger(&mut self, value: i32) -> IOResult<()> {
        if let Some(socket) = self.socket() {
            set_socket_linger(socket, value)?;
        }
        Ok(())
    }

    fn set_rcvhwm(&mut self, value: i32)-> IOResult<()> {
        if let Some(socket) = self.socket() {
            set_socket_rcvhwm(socket, value)?;
        }
        Ok(())
    }

    fn set_sndhwm(&mut self, value: i32)-> IOResult<()> {
        if let Some(socket) = self.socket() {
            set_socket_sndhwm(socket, value)?;
        }
        Ok(())
    }
    fn set_handshake_ivl(&mut self, value: i32) -> IOResult<()> {
        if let Some(socket) = self.socket() {
            set_handshake_ivl(socket, value)?;
        }
        Ok(())
    }

    fn set_keepalive(& mut self, idle: i32, intvl: i32, cnt: i32) -> IOResult<()> {
        if let Some(socket) = self.socket() {
            set_socket_keepalive(socket, idle, intvl, cnt)?;
        }
        Ok(())
    }

    fn set_heartbeat(& mut self, ivl: i32, timeout: i32, ttl: i32) -> IOResult<()> {
        if let Some(socket) = self.socket() {
            set_socket_heartbeat(socket, ivl, timeout, ttl)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum EndpointState {
    Connecting,
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum EndpointDiag {
    Message,
    Error,
    Dropped,
    RepeatedId,
    NonPositiveId,
    DecreasingId,
    OutOfRangeId,
    SocketError,
    ParsingError,
    DecompressionError,
    HeaderChange,
}

impl EndpointDiag {
    pub const ALL: &'static [EndpointDiag] = &[
        EndpointDiag::Message,
        EndpointDiag::Error,
        EndpointDiag::Dropped,
        EndpointDiag::RepeatedId,
        EndpointDiag::NonPositiveId,
        EndpointDiag::DecreasingId,
        EndpointDiag::OutOfRangeId,
        EndpointDiag::SocketError,
        EndpointDiag::ParsingError,
        EndpointDiag::DecompressionError,
        EndpointDiag::HeaderChange
    ];
}

#[derive(Clone, Debug)]
pub enum EndpointEvent {
    State(String, EndpointState),
    Diagnostic(String, EndpointDiag, Option<u64>),
}


impl EndpointEvent {
    pub fn endpoint(&self) -> String {
        match self {
            EndpointEvent::State(endpoint, _)
            | EndpointEvent::Diagnostic(endpoint,..) => endpoint.clone()
        }
    }
}

fn decode_monitor_event(monitor: &zmq::Socket, rec_index: u32, index: u32, handshake_check: bool) -> Result<(SocketEvent, Option<EndpointEvent>), zmq::Error> {

    // First frame: event info (binary struct)
    let msg = monitor.recv_msg(0)?;
    let data = msg.as_ref();
    if data.len() < 2 {
        return Err(zmq::Error::EINVAL);
    }
    // event id is first 2 bytes (u16 native endian)
    let socket_event = SocketEvent::from_raw(u16::from_ne_bytes([data[0], data[1]]));
    let value = u32::from_ne_bytes([data[2], data[3], data[4], data[5],]);

    // Second frame: endpoint string
    let endpoint_msg = monitor.recv_msg(0)?;
    let endpoint = endpoint_msg.as_str().unwrap_or("").to_string();

    log::debug!("Socket event:{:?} ({:}) [{:}/{:}]", socket_event, endpoint, rec_index, index);

    let endpoint_event = match socket_event {
        SocketEvent::CONNECTED => Some(EndpointEvent::State(endpoint, if handshake_check{EndpointState::Connecting} else {EndpointState::Connected})),
        SocketEvent::CONNECT_DELAYED => Some(EndpointEvent::State(endpoint, EndpointState::Connecting)),
        SocketEvent::CONNECT_RETRIED => Some(EndpointEvent::State(endpoint, EndpointState::Connecting)),
        SocketEvent::HANDSHAKE_SUCCEEDED  => Some(EndpointEvent::State(endpoint, EndpointState::Connected)),
        SocketEvent::DISCONNECTED  => Some(EndpointEvent::State(endpoint, EndpointState::Disconnected)),
        SocketEvent::HANDSHAKE_FAILED_NO_DETAIL => Some(EndpointEvent::State(endpoint, EndpointState::Disconnected)),
        SocketEvent::HANDSHAKE_FAILED_PROTOCOL => Some(EndpointEvent::State(endpoint, EndpointState::Disconnected)),
        SocketEvent::HANDSHAKE_FAILED_AUTH  => Some(EndpointEvent::State(endpoint, EndpointState::Disconnected)),
        //Disregard server and debug events
        SocketEvent::LISTENING => None,
        SocketEvent:: BIND_FAILED  => None,
        SocketEvent::ACCEPTED  => None,
        SocketEvent:: ACCEPT_FAILED => None,
        SocketEvent:: CLOSED  => None,
        SocketEvent::CLOSE_FAILED  => None,
        SocketEvent::MONITOR_STOPPED  => None,
        SocketEvent::ALL => None,
    };
    Ok((socket_event, endpoint_event))
}

#[derive(Clone)]
pub struct SocketMonitor {
    diag_tx:crossbeam_channel::Sender<EndpointEvent>,
    diag_rx:crossbeam_channel::Receiver<EndpointEvent>,
    cmd_tx: crossbeam_channel::Sender<MonitorCommand>,
    endpoint_states: Arc<RwLock<HashMap<String, EndpointState>>>,
    lifetime: Arc<()>,
}

struct MonitorEntry {
    socket: zmq::Socket,
    endpoint: Option<String>,
    rec_index: u32,
    index: u32,
    monitor_ep: String,
}

enum MonitorCommand {
    Add(MonitorEntry),
    Remove(u32),
    DisableHandshakeCheck,
    Shutdown
}

impl SocketMonitor {
    pub fn new( ) -> Self {
        let (diag_tx, diag_rx) = crossbeam_channel::unbounded();
        let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
        let endpoint_states = Arc::new(RwLock::new(HashMap::new()));
        let states = endpoint_states.clone();
        let tx  = diag_tx.clone();
        thread::spawn(move || {
            let mut monitors: Vec<MonitorEntry> = Vec::new();
            let mut handshake_check = true;
            loop {
                // Add newly registered monitors
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        MonitorCommand::Add(entry) => monitors.push(entry),
                        MonitorCommand::Remove(index) => {
                            //monitors.retain(|m| m.index != index);
                            if let Some(pos) = monitors.iter().position(|m| m.index == index) {
                                let entry = monitors.remove(pos);
                                if let Some(endpoint) = entry.endpoint {
                                     {
                                        states.write().unwrap().remove(&endpoint);
                                     }
                                     if let Err(err) = (entry.socket.disconnect(entry.monitor_ep.as_str())){
                                         log::error!("Error disconnecting monitor socket for {}: {:?}", endpoint, err);
                                     } else {
                                         log::info!("Success disconnecting monitor socket for {}", endpoint);
                                     }
                                }
                            }
                        },
                        MonitorCommand::DisableHandshakeCheck => {
                            handshake_check = false;
                        },
                        MonitorCommand::Shutdown => {
                            log::info!("Finishing socket monitor");
                            return;
                        },
                    }
                }
                if monitors.is_empty() {
                    thread::sleep(std::time::Duration::from_millis(10));
                    continue;
                }

                let mut items: Vec<zmq::PollItem> = monitors
                    .iter()
                    .map(|m| m.socket.as_poll_item(zmq::POLLIN))
                    .collect();
                if let Err(err) = zmq::poll(&mut items, 100) {
                    log::error!("Error polling socket monitor channels: {:?}", err);
                    return;
                } else {
                    for (idx, item) in items.iter().enumerate() {
                        if item.is_readable() {
                            let monitor = &monitors[idx];
                            if let Ok((_event, endpoint_event)) = decode_monitor_event(&monitor.socket, monitor.rec_index, monitor.index, handshake_check) {
                                if let Some(event) = endpoint_event {
                                    if let EndpointEvent::State(ep, state) = &event {
                                        let endpoint = monitor.endpoint.clone().unwrap_or_else(|| ep.to_string());
                                        let mut states = states.write().unwrap();
                                        if states.get(&endpoint) != Some(state) {
                                            states.insert(endpoint, state.clone());
                                            let _ = tx.send(event);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
    Self {endpoint_states, cmd_tx, diag_tx, diag_rx, lifetime: Arc::new(())}
    }
    pub fn shutdown(&self) {
        if let Err(err) = self.cmd_tx.send(MonitorCommand::Shutdown){
            log::error!("Error shutting down socket monitoring: {}", err);
        }
    }

    pub fn add(&self,socket: zmq::Socket,endpoint: Option<String>, rec_index: u32,index: u32, monitor_ep: String) {
        if let Err(err) =  self.cmd_tx.send(MonitorCommand::Add(MonitorEntry {socket,endpoint,rec_index,index, monitor_ep})){
            log::error!("Error adding socket monitoring: {}", err);
        }
    }

    pub fn remove(&self,index: u32) {
        if let Err(err) = self.cmd_tx.send(MonitorCommand::Remove(index)){
            log::error!("Error removing socket monitoring: {}", err);
        }
    }

    pub fn endpoint_state(&self, endpoint: &str) -> Option<EndpointState> {
        let mut map = self.endpoint_states.read().ok()?;
        map.get(endpoint).copied()
    }
    pub fn endpoint_states(&self) -> HashMap<String, EndpointState> {
        let mut map = self.endpoint_states.read().unwrap();
        map.clone()
    }

    pub fn send_diag (&self, endpoint: String, diag:EndpointDiag, id:Option<u64>) {
        if let Err(err) = self.diag_tx.send(EndpointEvent::Diagnostic(endpoint, diag, id)) {
            log::error!("Error sending event: {}", err);
        }
    }
    pub fn diag_rx(&self) -> crossbeam_channel::Receiver<EndpointEvent> {
        self.diag_rx.clone()
    }
    pub fn diag_tx(&self) -> crossbeam_channel::Sender<EndpointEvent> {
        self.diag_tx.clone()
    }

    pub fn disable_handshake_check(&mut self) {
        if let Err(err) = self.cmd_tx.send(MonitorCommand::DisableHandshakeCheck){
            log::error!("Error disabling handshake check: {}", err);
        }
    }
    pub fn check_connected(&self, endpoint: &Option<String>) {
        if let Some(ep) = &endpoint {
            //This undesirable check can be done because older ZMQ never sends HANDSHAKE_SUCCEEDED,
            //snd connection state never gets to Connected
            //Only paying the price if handshake_check is disabled -> assuming legacy sources.
            {
                //Cheaper than write
                if self.endpoint_states.read().unwrap().get(ep) != Some(&EndpointState::Connecting) {
                    return;
                }
            }
            {
                let mut states = self.endpoint_states.write().unwrap();
                if let Some(state) = states.get_mut(ep) {
                    log::warn!("Received messge from {}, endpoint didn't send HANDSHAKE_SUCCEEDED - setting Connected", ep);
                    *state = EndpointState::Connected;
                }
            }
        }
    }
}


impl Drop for SocketMonitor {
    fn drop(&mut self) {
        let clones = Arc::strong_count(&self.lifetime);
        if clones == 1 {
            self.shutdown();
        }
    }
}


static SOCKET_INDEX: AtomicU32 = AtomicU32::new(0);
fn index() -> u32{
    SOCKET_INDEX.fetch_add(1, Ordering::Relaxed) + 1
}

pub struct TrackedSocket {
    socket: zmq::Socket,
    endpoints: Vec<String>,
    rec_index: u32,
    topics: Vec<String>,
    monitoring: bool,
    index: u32,
}

impl TrackedSocket {
    pub fn new(context: &Context, socket_type: zmq::SocketType, rec_index: u32) -> IOResult<TrackedSocket> {
        let socket = context.socket(socket_type)?;
        let index =  index();
        Ok (Self {socket, rec_index, endpoints: Vec::new(),topics: Vec::new(),monitoring: false, index })
    }

    pub fn enable_monitoring(&mut self, context: &Context, monitor: &SocketMonitor, endpoint: Option<String>) -> IOResult<()> {
        if self.monitoring {
            return Ok(());
        }
        let monitor_ep = format!("inproc://monitor-{}", Uuid::new_v4());
        self.socket.monitor(&monitor_ep,zmq::SocketEvent::ALL as i32,)?;
        let mon = context.socket(zmq::PAIR)?;
        mon.connect(&monitor_ep)?;
        monitor.add(mon, endpoint, self.rec_index, self.index, monitor_ep);
        self.monitoring = true;
        Ok(())
    }

    pub fn disable_monitoring(&mut self, monitor: &SocketMonitor) -> IOResult<()> {
        if !self.monitoring {
            return Ok(());
        }
        //This is not working on zmq 0.10, as not treanslated into zmq_socket_monitor(socket, NULL, 0),
        /*
            match self.socket.monitor("", 0){
            Ok(_) => {}
            Err(e) => {
                log::error!("Error disabling monitoring: {}", e);
            }
        }
        */
        monitor.remove(self.index);
        self.monitoring = false;

        Ok(())
    }

    pub fn add_topic(&mut self, topic: String) {
        self.topics.push(topic);
    }

    pub fn subscribe(&mut self, topic: &str, endpoint: &str) -> IOResult<()> {
        if let Err(e) = self.socket.set_subscribe(topic.as_bytes()) {
            log::error!("Error subscribing topic {} in endpoint {}: {}", topic, endpoint, e);
            log::error!("Error subscribing topic {} in endpoint {}: {}", topic, endpoint, e);
            return Err(e.into());
        }
        Ok(())
    }

    pub fn connect(&mut self, endpoint: &str) -> IOResult<()> {
        if !self.has_endpoint(endpoint) {
            let socket_type = self.socket.get_socket_type()?;
            log::info!("Connecting to endpoint {}  socket type:{:?}", endpoint, socket_type);
            if let Err(e) = self.socket.connect(endpoint) {
                log::error!("Error connecting to endpoint {}: {}", endpoint, e);
                return Err(e.into());
            }
            if socket_type == SocketType::SUB {
                if self.topics.is_empty() {
                    self.subscribe("", endpoint)?;
                } else {
                    for topic in &self.topics.clone() {
                        self.subscribe(topic, endpoint)?;
                    }
                }
            }
            self.endpoints.push(endpoint.to_string());
        }
        Ok(())
    }

    pub fn has_endpoint(&self, endpoint: &str) -> bool {
        self.endpoints.contains(&endpoint.to_string())
    }

    pub fn has_any_endpoint(&self) -> bool {
        !self.endpoints.is_empty()
    }

    pub fn num_endpoints(&self) -> usize {
        self.endpoints.len()
    }

    pub fn endpoint(&self, index: u32) -> Option<String> {
        if index >= self.endpoints.len() as u32 {
            return None
        }
        Some(self.endpoints[index as usize].clone())
    }

    pub fn disconnect_endpoint(&mut self, endpoint: &str) {
        if self.has_endpoint(endpoint) {
            //self.connections.retain(|x| x != endpoint);
            log::info!("Disonnecting endpoint {}", endpoint);
            if let Err(e) = self.socket.disconnect(endpoint) {
                log::error!("Error disonnecting endpoint {}: {}", endpoint, e);
            }
            self.endpoints.retain(|e| e != endpoint);
        }
    }

    pub fn disconnect(&mut self) {
        for endpoint in self.endpoints.clone(){
            self.disconnect_endpoint(endpoint.as_str());
        }

        self.endpoints.clear();
    }
    pub fn receive(&self) -> IOResult<Vec<Vec<u8>>> {
        match self.socket.recv_multipart(0){
            Ok(msg) => {Ok(msg)}
            Err(e) => { Err(e.into())}
        }
    }

    pub fn socket(&self) -> &zmq::Socket{
        &self.socket
    }

    pub fn transport(&self) -> Option<Transport> {
        if let Some(endpoint) = self.endpoint(0) {
            Transport::from_endpoint(endpoint.as_str()).ok()
        } else {
            None
        }
    }

    pub fn index(&self) -> u32{
        self.index
    }

    pub fn options(&self) -> SocketOptions {SocketOptions::get(self.socket())}

}

impl Drop for TrackedSocket {
    fn drop(&mut self) {
        if self.has_any_endpoint(){
            self.disconnect();
        }
    }
}


