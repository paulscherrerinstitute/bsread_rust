use std::sync::{Arc, Mutex};
use zmq::{SocketType, SocketEvent, Context};
use std::collections::HashMap;
use std::thread;
use uuid::Uuid;
use crate::IOResult;
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

pub struct KeepAlive {
    pub idle: i32,
    pub intvl: i32,
    pub cnt: i32,
}

pub struct Heartbeat {
    pub ivl: i32,
    pub timeout: i32,
    pub ttl: i32,
}
pub struct SocketOptions{
    pub linger : Option<i32>,
    pub rcvhwm : Option<i32>,
    pub sndhwm : Option<i32>,
    pub keepalive: Option<KeepAlive>,
    pub heartbeat: Option<Heartbeat>,

}

impl SocketOptions {
    pub fn new() -> Self {
        Self{linger:None, rcvhwm:None, sndhwm:None, keepalive:None, heartbeat:None}
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
        if let Some(keepalive) = &self.keepalive {
            set_socket_keepalive(socket, keepalive.idle, keepalive.intvl, keepalive.cnt)?;
        }
        if let Some(heartbeat) = &self.heartbeat {
            set_socket_heartbeat(socket, heartbeat.ivl, heartbeat.timeout, heartbeat.ttl)?;
        }
        Ok(())
    }

}

fn set_socket_keepalive(socket: &zmq::Socket, idle: i32, intvl: i32, cnt: i32) -> IOResult<()> {
    if !is_socket_ipc(socket) {
        socket.set_tcp_keepalive(1)?;
        socket.set_tcp_keepalive_idle(idle)?;
        socket.set_tcp_keepalive_intvl(intvl)?;
        socket.set_tcp_keepalive_cnt(cnt)?;
    }
    Ok(())
}

fn set_socket_heartbeat(socket: &zmq::Socket, ivl: i32, timeout: i32, ttl: i32) -> IOResult<()> {
    socket.set_heartbeat_ivl(ivl)?;
    socket.set_heartbeat_timeout(timeout)?;
    socket.set_heartbeat_ttl(ttl)?;
    Ok(())
}

fn set_socket_linger(socket: &zmq::Socket, value:i32) -> IOResult<()> {
    socket.set_linger(value)?;
    Ok(())
}

fn set_socket_rcvhwm(socket: &zmq::Socket, value:i32) -> IOResult<()> {
    socket.set_rcvhwm(value)?;
    Ok(())
}

fn set_socket_sndhwm(socket: &zmq::Socket, value:i32) -> IOResult<()> {
    socket.set_sndhwm(value)?;
    Ok(())
}

fn is_socket_ipc(socket: &zmq::Socket) -> bool {
    if let Ok(last_endpoint) = socket.get_last_endpoint() {
        if let Ok((endpoint)) = last_endpoint {
            return endpoint.starts_with("ipc://");
        };
    };
    false
}

pub trait SocketConfig {
    fn sockets(&self) -> Vec<&zmq::Socket>;
    fn set_options(&self, options: &SocketOptions) -> IOResult<()>{
        for socket in self.sockets() {
            options.set(socket)?;
        }
        Ok(())
    }
    fn set_linger(&mut self, value: i32) -> IOResult<()> {
        for socket in self.sockets() {
            set_socket_linger(socket, value)?;
        }
        Ok(())
    }

    fn set_rcvhwm(&mut self, value: i32)-> IOResult<()> {
        for socket in self.sockets() {
            set_socket_rcvhwm(socket, value)?;
        }
        Ok(())
    }

    fn set_sndhwm(&mut self, value: i32)-> IOResult<()> {
        for socket in self.sockets() {
            set_socket_sndhwm(socket, value)?;
        }
        Ok(())
    }

    fn set_keepalive(& mut self, idle: i32, intvl: i32, cnt: i32) -> IOResult<()> {
        for socket in self.sockets() {
            set_socket_keepalive(socket, idle, intvl, cnt)?;
        }
        Ok(())
    }

    fn set_heartbeat(& mut self, ivl: i32, timeout: i32, ttl: i32) -> IOResult<()> {
        for socket in self.sockets() {
            set_socket_heartbeat(socket, ivl, timeout, ttl)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointState {
    Connecting,
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EndpointDiag {
    RepeatedId,
    NonPositiveId,
    DecreasingId,
    OutOfRangeId,
    SocketError,
    ParsingError,
    DecompressionError,
    HeaderChange
}

impl EndpointDiag {
    pub const ALL: &'static [EndpointDiag] = &[
        EndpointDiag::RepeatedId,
        EndpointDiag::NonPositiveId,
        EndpointDiag::DecreasingId,
        EndpointDiag::OutOfRangeId,
        EndpointDiag::ParsingError,
        EndpointDiag::DecompressionError,
    ];
}

#[derive(Clone, Debug)]
pub enum EndpointEvent {
    State(String, EndpointState),
    Diagnostic(String, EndpointDiag)
}


impl EndpointEvent {
    pub fn endpoint(&self) -> String {
        match self {
            EndpointEvent::State(endpoint, _)
            | EndpointEvent::Diagnostic(endpoint, _) => endpoint.clone()
        }
    }
}

fn decode_monitor_event(monitor: &zmq::Socket, index: u32) -> Result<(SocketEvent, Option<EndpointEvent>), zmq::Error> {

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

    log::debug!("Socket event:{:?} ({:}) [{:}]", socket_event, endpoint, index);

    let endpoint_event = match socket_event {
        SocketEvent::CONNECTED => Some(EndpointEvent::State(endpoint, EndpointState::Connecting)),
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

pub fn monitor_loop(monitor: zmq::Socket,states: Arc<Mutex<HashMap<String, EndpointState>>>,tx: crossbeam_channel::Sender<EndpointEvent>, endpoint: Option<String>, index: u32) {
    loop {
        if let Ok((socket_event, endpoint_event)) = decode_monitor_event(&monitor, index) {
            if let Some(event) = endpoint_event {
                let mut map = states.lock().unwrap();
                let endpoint = endpoint.clone().unwrap_or_else(|| event.endpoint().to_string());
                if let EndpointEvent::State(ep, new_state) = &event {
                    let should_send = match map.get(&endpoint) {
                        Some(old_state) => *old_state != *new_state,
                        None => true,
                    };
                    if should_send {
                        log::info!("Endpoint event: {:?} [{:}]", event, index);
                        map.insert(endpoint.clone(), *new_state);
                        let _ = tx.send(event);
                    }
                }
            }
        }
    }
}

pub struct SocketMonitor {
    cmd_tx: crossbeam_channel::Sender<MonitorCommand>,
    endpoint_states: Arc<Mutex<HashMap<String, EndpointState>>>,
}

struct MonitorEntry {
    socket: zmq::Socket,
    endpoint: Option<String>,
    index: u32,
}

enum MonitorCommand {
    Add(MonitorEntry),
    Shutdown
}

impl SocketMonitor {
    pub fn new( tx: crossbeam_channel::Sender<EndpointEvent>) -> Self {
        let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
        let endpoint_states = Arc::new(Mutex::new(HashMap::new()));
        let states = endpoint_states.clone();
        thread::spawn(move || {
            let mut monitors: Vec<MonitorEntry> = Vec::new();
            loop {
                // Add newly registered monitors
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        MonitorCommand::Add(entry) => monitors.push(entry),
                        MonitorCommand::Shutdown => return,
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
                zmq::poll(&mut items, 100).unwrap();
                let mut states = states.lock().unwrap();
                for (idx, item) in items.iter().enumerate() {
                    if item.is_readable() {
                        let monitor = &monitors[idx];
                        if let Ok((_event, endpoint_event)) =decode_monitor_event(&monitor.socket, monitor.index) {
                            if let Some(event) = endpoint_event {
                                if let EndpointEvent::State(ep, state) = &event {
                                    let endpoint = monitor.endpoint.clone().unwrap_or_else(|| ep.to_string());
                                    if states.get(&endpoint) != Some(state){
                                        states.insert(endpoint, state.clone());
                                        let _ = tx.send(event);
                                    }

                                }
                            }
                        }
                    }
                }
            }
        });
        Self {endpoint_states, cmd_tx,}
    }
    pub fn shutdown(&self) {
        self.cmd_tx.send(MonitorCommand::Shutdown).unwrap();
    }

    pub fn add(&self,socket: zmq::Socket,endpoint: Option<String>,index: u32) {
        self.cmd_tx.send(MonitorCommand::Add(MonitorEntry {socket,endpoint,index,})).unwrap();
    }

    pub fn endpoint_state(&self, endpoint: &str) -> Option<EndpointState> {
        let mut map = self.endpoint_states.lock().unwrap();
        map.get(endpoint).copied()
    }
    pub fn endpoint_states(&self) -> HashMap<String, EndpointState> {
        let mut map = self.endpoint_states.lock().unwrap();
        map.clone()
    }
}


pub struct TrackedSocket {
    socket: zmq::Socket,
    endpoints: Vec<String>,
    index: u32,
    topics: Vec<String>,
    monitoring: bool
}

impl TrackedSocket {
    pub fn new(context: &Context, socket_type: zmq::SocketType, index: u32) -> IOResult<TrackedSocket> {
        let socket = context.socket(socket_type)?;
        Ok (Self {socket, index, endpoints: Vec::new(),topics: Vec::new(),monitoring: false })
    }

    pub fn enable_monitoring(&mut self, context: &Context, monitor: &SocketMonitor, endpoint: Option<String>) -> IOResult<()> {
        if self.monitoring {
            return Ok(());
        }
        let monitor_ep = format!("inproc://monitor-{}", Uuid::new_v4());
        self.socket.monitor(&monitor_ep,zmq::SocketEvent::ALL as i32,)?;
        let mon = context.socket(zmq::PAIR)?;
        mon.connect(&monitor_ep)?;
        monitor.add(mon,endpoint,self.index,);
        self.monitoring = true;
        Ok(())
    }

    pub fn add_topic(&mut self, topic: String) {
        self.topics.push(topic);
    }

    pub fn subscribe(&mut self, topic: &str, endpoint: &str) -> IOResult<()> {
        if let Err(e) = self.socket.set_subscribe(topic.as_bytes()) {
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
                    self.subscribe("", endpoint).unwrap();
                } else {
                    for topic in &self.topics.clone() {
                        self.subscribe(topic, endpoint).unwrap();
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
        }
    }

    pub fn disconnect(&mut self) {
        for endpoint in self.endpoints.clone(){
            self.disconnect_endpoint(endpoint.as_str());
        }
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
            return Some(Transport::from_endpoint(endpoint.as_str()).unwrap())
        }
        None
    }

}

impl Drop for TrackedSocket {
    fn drop(&mut self) {
        if self.has_any_endpoint(){
            self.disconnect();
        }
    }
}


