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


pub trait SocketConfig {
    fn socket(&self) -> &zmq::Socket;
    fn transport(&self) -> Transport;
    fn socket_type(&self) -> SocketType;
    fn set_linger(&self, value: i32) -> IOResult<()> {
        self.socket().set_linger(value)?;
        Ok(())
    }

    fn set_rcvhwm(&mut self, value: i32)-> IOResult<()> {
        if let Err(e) =   self.socket().set_rcvhwm(value) {
            return Err(e.into());
        }
        Ok(())
    }

    fn set_sndhwm(&mut self, value: i32)-> IOResult<()> {
        if let Err(e) =   self.socket().set_sndhwm(value) {
            return Err(e.into());
        }
        Ok(())
    }

    fn set_keepalive(&self, idle: i32, intvl: i32, cnt: i32) -> IOResult<()> {
        match self.transport() {
            Transport::Tcp { .. } => {
                self.socket().set_tcp_keepalive(1)?;
                self.socket().set_tcp_keepalive_idle(idle)?;
                self.socket().set_tcp_keepalive_intvl(intvl)?;
                self.socket().set_tcp_keepalive_cnt(cnt)?;
            }
            Transport::Ipc { .. } => {
                log::info!(
                    "Ignoring keepalive on IPC endpoint {}",
                    self.transport().endpoint()
                );
            }
        }
        Ok(())
    }

    fn set_heartbeat(&self, ivl: i32, timeout: i32, ttl: i32) -> IOResult<()> {
        self.socket().set_heartbeat_ivl(ivl)?;
        self.socket().set_heartbeat_timeout(timeout)?;
        self.socket().set_heartbeat_ttl(ttl)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointState {
    Connecting,
    Connected,
    Disconnected,
}

#[derive(Clone, Debug)]
pub enum EndpointEvent {
    Connecting(String),
    Connected(String),
    Disconnected(String),
}


impl EndpointEvent {
    pub fn state(&self) -> EndpointState {
        match self {
            EndpointEvent::Connecting(_) => {EndpointState::Connecting}
            EndpointEvent::Connected(_) => {EndpointState::Connected}
            EndpointEvent::Disconnected(_) => {EndpointState::Disconnected}
        }
    }

    pub fn endpoint(&self) -> String {
        match self {
            EndpointEvent::Connecting(s)
            | EndpointEvent::Connected(s)
            | EndpointEvent::Disconnected(s) => s.clone(),
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
        SocketEvent::CONNECTED => Some(EndpointEvent::Connecting(endpoint)),
        SocketEvent::CONNECT_DELAYED => Some(EndpointEvent::Connecting(endpoint)),
        SocketEvent::CONNECT_RETRIED => Some(EndpointEvent::Connecting(endpoint)),
        SocketEvent::HANDSHAKE_SUCCEEDED  => Some(EndpointEvent::Connected(endpoint)),
        SocketEvent::DISCONNECTED  => Some(EndpointEvent::Disconnected(endpoint)),
        SocketEvent::HANDSHAKE_FAILED_NO_DETAIL => Some(EndpointEvent::Disconnected(endpoint)),
        SocketEvent::HANDSHAKE_FAILED_PROTOCOL => Some(EndpointEvent::Disconnected(endpoint)),
        SocketEvent::HANDSHAKE_FAILED_AUTH  => Some(EndpointEvent::Disconnected(endpoint)),
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

pub fn monitor_loop(monitor: zmq::Socket,states: Arc<Mutex<HashMap<String, EndpointState>>>,tx: crossbeam_channel::Sender<EndpointEvent>, index: u32) {
    loop {
        if let Ok((socket_event, endpoint_event)) = decode_monitor_event(&monitor, index) {
            if let Some(event) = endpoint_event {
                let mut map = states.lock().unwrap();
                let endpoint = event.endpoint().to_string();
                let new_state = event.state();
                let should_send = match map.get(&endpoint) {
                    Some(old_state) => *old_state != new_state,
                    None => true,
                };
                if should_send {
                    log::info!("Endpoint event: {:?} [{:}]", event, index);
                    map.insert(endpoint.clone(), new_state);
                    let _ = tx.send(event);
                }
            }
        }
    }
}

pub struct TrackedSocket {
    socket: zmq::Socket,
    connections: Vec<String>,
    index: u32,
    topics: Vec<String>,
    endpoint_states: Arc<Mutex<HashMap<String, EndpointState>>>,
    event_tx: Option<crossbeam_channel::Sender<EndpointEvent>>,
}

impl TrackedSocket {
    pub fn new(context: &Context, socket_type: zmq::SocketType, index: u32) -> IOResult<TrackedSocket> {
        let socket = context.socket(socket_type)?;
        let mut _self = Self {
            socket,
            index,
            connections: Vec::new(),
            topics: Vec::new(),
            endpoint_states: Arc::new(Mutex::new(HashMap::new())),
            event_tx: None,
        };
        //_self.enable_monitoring(context)?;
        Ok(_self)
    }

    pub fn enable_monitoring(&mut self, context: &Context) -> IOResult<crossbeam_channel::Receiver<EndpointEvent>> {
        let monitor_ep = format!("inproc://monitor-{}", Uuid::new_v4());
        if let Err(e) = self.socket.monitor(&monitor_ep, zmq::SocketEvent::ALL as i32, ) {
            log::error!("Error creating monitor: {}", e);
            return Err(e.into());
        }
        let (tx, rx) = crossbeam_channel::unbounded();

        self.event_tx = Some(tx.clone());
        let mon = context.socket(zmq::PAIR)?;
        mon.connect(&monitor_ep)?;
        let states = Arc::clone(&self.endpoint_states);
        let index = self.index;
        thread::spawn(move || {
            monitor_loop(mon, states, tx, index);
        });
        Ok((rx))
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
        if !self.has_connected_to(endpoint) {
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
            self.connections.push(endpoint.to_string());
        }
        Ok(())
    }

    pub fn has_connected_to(&self, endpoint: &str) -> bool {
        self.connections.contains(&endpoint.to_string())
    }

    pub fn has_any_connection(&self) -> bool {
        !self.connections.is_empty()
    }

    pub fn num_connection(&self) -> usize {
        self.connections.len()
    }

    pub fn connection(&self, index: u32) -> Option<String> {
        if index >= self.connections.len() as u32 {
            return None
        }
        Some(self.connections[index as usize].clone())
    }

    pub fn disconnect(&mut self, endpoint: &str) {
        if self.has_connected_to(endpoint) {
            //self.connections.retain(|x| x != endpoint);
            log::info!("Disonnecting endpoint {}", endpoint);
            if let Err(e) = self.socket.disconnect(endpoint) {
                log::error!("Error disonnecting endpoint {}: {}", endpoint, e);
            }
        }
    }

    pub fn endpoint_state(&self, endpoint: &str) -> Option<EndpointState> {
        let mut map = self.endpoint_states.lock().unwrap();
        map.get(endpoint).copied()
    }
    pub fn endpoint_states(&self) -> HashMap<String, EndpointState> {
        let mut map = self.endpoint_states.lock().unwrap();
        map.clone()
    }
    pub fn disconnect_all(&mut self) {
        for endpoint in self.connections.clone(){
            self.disconnect(endpoint.as_str());
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
}

impl Drop for TrackedSocket {
    fn drop(&mut self) {
        if self.has_any_connection(){
            self.disconnect_all();
        }
    }
}


