use crate::*;
use crate::message::*;
use crate::utils::*;
use crate::sockets::*;
use std::{io, thread};
use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread::JoinHandle;
use zmq::{Context, PollItem, SocketEvent, SocketType};
use std::time::{Duration, Instant};
use uuid::Uuid;
#[cfg(feature = "async")]
use tokio::runtime::Handle;


static RECEIVER_INDEX: AtomicU32 = AtomicU32::new(0);
fn index() -> u32{
    RECEIVER_INDEX.fetch_add(1, Ordering::Relaxed) + 1
}

struct Stats {
    counter_messages: u32,
    counter_error: u32,
    counter_drop: u32,
    diagnostics: HashMap<String, HashMap<EndpointDiag, u32>>
}

pub struct ReceivedMessage{
    pub endpoint: Option<String>,
    pub message: Message,
}

impl Stats{
    fn increase_messages(& mut self){
        self.counter_messages = self.counter_messages + 1;
    }

    fn increase_errors(& mut self){
        self.counter_error = self.counter_error + 1;
    }
    fn increase_drops(& mut self){
        self.counter_drop = self.counter_drop + 1;
    }

    fn reset(& mut self){
        self.counter_messages = 0;
        self.counter_error = 0;
        self.counter_drop = 0;
        self.diagnostics = HashMap::new();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryMode {
    Inline,
    Threaded,
    Buffered,
    Async
}

impl DeliveryMode {
    fn thraded(&self) -> bool{
        *self != DeliveryMode::Inline
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionMode {
    Shared,     //Single receive socked
    Individual,  //One socket per endpoint
}

#[cfg(feature = "async")]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AsyncExecution {
    Concurrent,
    Ordered {capacity: usize,blocking: bool},
}



enum ConnectionSockets {
    Shared {
        socket: TrackedSocket,
    },
    Individual {
        sockets: HashMap<String, TrackedSocket>,
        poll_endpoints: Vec<String>,
        poll_ready_list: VecDeque<String>,
    },
}

impl ConnectionSockets {
    fn update_poll_items(&mut self) {
        match self {
            ConnectionSockets::Shared { socket } => {}
            ConnectionSockets::Individual { sockets,  poll_endpoints, .. } => {
                //poll_endpoints.clear();
                // poll_endpoints.extend(sockets.keys().cloned());
                *poll_endpoints =  sockets.keys().cloned().collect(); //This has extra allocation comparing to the above, but avoid the inconsistent state.
            }
        }
    }
    fn clear(&mut self) {
        match self {
            ConnectionSockets::Shared { socket } => {}
            ConnectionSockets::Individual { sockets, poll_endpoints, .. } => {
                sockets.clear();
                poll_endpoints.clear();
            }
        }
    }
}

const VALID_ID_RANGE:u64 = 3600 * 24 * 100;

pub const CHECK_ID_POSITIVE:u64 = 1;
pub const CHECK_ID_MONOTONIC:u64 = 2;
pub const CHECK_ID_RANGE:u64 = 3;
pub const CHECK_ID_PAST_RANGE:u64 = 4;

pub const CHECK_ALL:u64 = !0;

enum ReceiverCommand {
    Connect {response: Option<crossbeam_channel::Sender<IOResult<()>>>,},
    Disconnect {response: Option<crossbeam_channel::Sender<IOResult<()>>>,},
    AddEndpoint {endpoint: String,response: Option<crossbeam_channel::Sender<IOResult<()>>>,},
    RemoveEndpoint {endpoint: String, response: Option<crossbeam_channel::Sender<IOResult<()>>>,},
    EnableMonitoring {monitor: SocketMonitor, response: Option<crossbeam_channel::Sender<IOResult<()>>>,},
    SocketOptions {endpoint: String, response: Option<crossbeam_channel::Sender<IOResult<SocketOptions>>>,},
}

#[derive(Debug, Clone)]
pub struct ForwarderConfig {
    socket_type: SocketType,
    transport: Transport,
    sndhwm: Option<i32>
}

impl ForwarderConfig {
    pub fn new(socket_type: SocketType, transport: Transport, sndhwm: Option<i32>) -> Self {
        Self { socket_type, transport, sndhwm }
    }
}

struct Worker {
    index: u32,
    bsread: Arc<Bsread>,
    connected: bool,
    sockets: ConnectionSockets,
    endpoints: Arc<RwLock<Vec<String>>>,
    socket_type: SocketType,
    connection_mode: ConnectionMode,
    socket_options: Arc<Mutex<SocketOptions>>,
    header_buffer: LimitedHashMap<String, DataHeaderInfo>,
    id_buffer: HashMap<String, u64>,
    socket_monitor: Option<SocketMonitor>,
    rx_cmd:crossbeam_channel::Receiver<ReceiverCommand>,
    interrupted: Arc<AtomicBool>,
    forwarder_config: Option<ForwarderConfig>,
    forwarder: Option<Sender>,
    fifo: Option<Arc<FifoQueue<ReceivedMessage>>>,
    stats: Arc<RwLock<Stats>>,
    raw: bool,
    check_mask: u64,
    threaded: bool,
}

impl Worker {
    pub fn new(index: u32, bsread: Arc<Bsread>, endpoints: Arc<RwLock<Vec<String>>>, socket_type: SocketType, connection_mode: ConnectionMode,
               socket_options: Arc<Mutex<SocketOptions>>, socket_monitor: Option<SocketMonitor>, rx_cmd: crossbeam_channel::Receiver<ReceiverCommand>,
               forwarder_config: Option<ForwarderConfig>, forwarder: Option<Sender>,
               fifo: Option<Arc<FifoQueue<ReceivedMessage>>>, stats: Arc<RwLock<Stats>>,
               interrupted: Arc<AtomicBool>, raw: bool, check_mask: u64, threaded: bool,
    ) -> Self {
        let sockets: ConnectionSockets = match connection_mode {
            ConnectionMode::Shared => {
                ConnectionSockets::Shared { socket: TrackedSocket::new(&bsread.context(), socket_type, index).unwrap() }
            }
            ConnectionMode::Individual => {
                ConnectionSockets::Individual { sockets: HashMap::new(), poll_endpoints: Vec::new(), poll_ready_list: VecDeque::new() }
            }
        };

        let mut worker = Self {
            index, bsread, connected: false, sockets, endpoints, socket_type, connection_mode,
            header_buffer: LimitedHashMap::void(), id_buffer: HashMap::new(),
            socket_options, socket_monitor:None, rx_cmd, interrupted, forwarder_config, forwarder,
            fifo, stats, raw, check_mask, threaded
        };
        if let Some (socket_monitor) = socket_monitor{
            worker.enable_monitoring(socket_monitor);
        }
        worker
    }

    pub fn from_receiver(receiver: &mut Receiver) -> Self {
        let index = receiver.index;
        let bsread = receiver.bsread.clone();
        let endpoints = receiver.endpoints.clone();
        let socket_type = receiver.socket_type.clone();
        let connection_mode = receiver.connection_mode.clone();
        let interrupted = Arc::clone(&receiver.interrupted);
        let forwarder_config = receiver.forwarder_config.clone();
        let forwarder = receiver.forwarder.take();
        let fifo = None;
        let stats = receiver.stats.clone();
        let raw = receiver.raw;
        let socket_options = receiver.socket_options.clone();
        let socket_monitor = receiver.socket_monitor.clone();
        let rx_cmd = receiver.rx_cmd.clone();
        let check_mask = receiver.check_mask;
        let threaded = false;
        Worker::new(
            index, bsread, endpoints, socket_type, connection_mode, socket_options, socket_monitor,
            rx_cmd, forwarder_config, forwarder, fifo, stats, interrupted, raw, check_mask, threaded
        )
    }

    pub fn endpoints(&self) ->  Vec<String> {
        self.endpoints.read().unwrap().clone()
    }

    pub fn has_endpoint(&self, endpoint: &str) -> bool {
        self.endpoints
            .read()
            .unwrap()
            .iter()
            .any(|e| e == endpoint)
    }

    fn connect_endpoint(&mut self, endpoint: &str) -> IOResult<()> {
        let context = self.bsread.context();

        match &mut self.sockets {
            ConnectionSockets::Shared { socket } => {
                socket.connect(endpoint)?
            }
            ConnectionSockets::Individual { sockets, .. } => {
                match sockets.get(endpoint) {
                    None => {
                        let mut socket = TrackedSocket::new(context, self.socket_type, self.index)?;
                        self.socket_options.lock().unwrap().set(socket.socket())?;
                        socket.connect(endpoint)?;
                        if let Some(socket_monitor) = &self.socket_monitor {
                            socket.enable_monitoring(self.bsread.context(), &socket_monitor, Some(endpoint.to_string()))?;
                        }
                        sockets.insert(endpoint.to_string(), socket);
                        self.sockets.update_poll_items();
                    }
                    Some(_) => {}
                }
            }
        }
        self.set_header_buffer_size(self.connections());

        Ok(())
    }

    fn disconnect_endpoint(&mut self, endpoint: &str) {
        match &mut self.sockets {
            ConnectionSockets::Shared { socket } => {
                socket.disconnect_endpoint(endpoint);
            }
            ConnectionSockets::Individual { sockets, .. } => {
                match sockets.get_mut(endpoint) {
                    None => {}
                    Some(socket) => {
                        socket.disconnect();
                        if let Some(socket_monitor) = &self.socket_monitor {
                            socket.disable_monitoring(&socket_monitor);
                        }
                        sockets.remove(endpoint);
                        self.sockets.update_poll_items();
                        self.header_buffer.remove(&endpoint.to_string());
                    }
                }
            }
        }
    }

    fn set_header_buffer_size(&mut self, size: usize) {
        if self.header_buffer.is_void() {
            self.header_buffer = LimitedHashMap::new(size);
        } else {
            self.header_buffer.set_max_size(size);
        }
    }

    //Synchronous Mode: blocking, callback in same thread
    pub fn listen<F>(&mut self, callback: F, num_messages: Option<u32>) -> IOResult<()>
    where
        F: Fn(ReceivedMessage),
    {
        self.reset_counters();
        if let Some(cfg) = self.forwarder_config.as_mut() {
            match Sender::new(self.bsread.clone(), cfg.socket_type, cfg.transport.clone(), None, None, None, ) {
                Ok(mut sender) => {
                    if let Err(e) = sender.start() {
                        log::warn!("Error binding forwarder endpoint {}: {}",cfg.transport.endpoint(), e);
                    } else {
                        if let Some(hwm) = cfg.sndhwm {
                            if let Err(e) = sender.set_sndhwm(hwm) {
                                log::warn!("Error setting forwarder sndhwm to {}: {}", hwm, e);
                            }
                        }
                        thread::sleep(Duration::from_millis(100));
                        self.forwarder = Some(sender);
                    }
                }
                Err(e) => {
                    log::warn!("Error creating forwarder endpoint {}: {}",cfg.transport.endpoint(),e);
                }
            }
        }
        self.connect()?;
        loop {
            let message = self.receive();
            if let Ok(msg) = message {
                match &self.fifo {
                    None => {
                        callback(msg)
                    }
                    Some(fifo) => {
                        if let Some(dropped) =  fifo.add(msg) {
                            log::debug!("Dropping message {} from {:?}: endpoint queue is full", dropped.message.id(), &dropped.endpoint);
                            self.send_diag(&dropped.endpoint, EndpointDiag::Dropped, Some(dropped.message.id()));
                            self.stats.write().unwrap().increase_drops();
                        }
                    }
                }
            };
            if num_messages.map_or(false, |m| self.message_count() >= m) {
                break;
            }
            if self.is_interrupted() {
                break;
            }
            while let Ok(command) = self.rx_cmd.try_recv() {
                match command {
                    ReceiverCommand::Connect { response } => {
                        let ret = self.connect();
                        if let Some(response) = response {
                            let _ = response.send(ret);
                        }
                    }
                    ReceiverCommand::Disconnect { response } => {
                        self.disconnect();
                        if let Some(response) = response {
                            let _ = response.send(Ok(()));
                        }
                    }
                    ReceiverCommand::AddEndpoint { endpoint, response } => {
                        let ret = self.add_endpoint(&endpoint);
                        if let Some(response) = response {
                            let _ = response.send(ret);
                        }
                    }
                    ReceiverCommand::RemoveEndpoint { endpoint, response } => {
                        if let Some(response) = response {
                            let _ = response.send(Ok(()));
                        }
                    }
                    ReceiverCommand::EnableMonitoring { monitor, response } => {
                        let ret = self.enable_monitoring(monitor);
                        if let Some(response) = response {
                            let _ = response.send(ret);
                        }
                    }

                    ReceiverCommand::SocketOptions { endpoint, response } => {
                        let options = if let Some(socket) = self.socket(endpoint.as_str()){
                            Ok(socket.options())
                        } else {
                            Err(IOError::new(ErrorKind::InvalidData, "Invalid endpoint", ))
                        };
                        if let Some(response) = response {
                            let _ = response.send(options);
                        }
                    }
                }
            }
        }
        self.stop_forwarder();
        Ok(())
    }

    fn process(&mut self, endpoint: &Option<String>, message_parts: Vec<Vec<u8>>) -> IOResult<Message> {
        if let Some(sender) = self.forwarder.as_mut() {
            match sender.forward(&message_parts) {
                Ok(_) => (),
                Err(e) => log::warn!("Error forwarding message to {}: {}", sender.endpoint(), e),
            }
        }
        let message = parse_message(message_parts, endpoint, &mut self.header_buffer, self.raw);
        match message {
            Ok(message) => {
                self.check_message(message, endpoint)
            },
            Err(e) => {
                if (e.kind() == DECOMPRESSION_ERROR) {
                    self.send_diag(&endpoint, EndpointDiag::DecompressionError, None);
                } else {
                    self.send_diag(&endpoint, EndpointDiag::ParsingError, None);
                }
                return Err(e)
            }
        }
    }

    fn check_message(&mut self, message: Message, endpoint: &Option<String>) -> IOResult<(Message)> {
        let id = message.id();
        if self.check_mask & CHECK_ID_POSITIVE != 0 {
            if id <= 0 {
                self.send_diag(&endpoint, EndpointDiag::NonPositiveId, Some(id));
                return Err(IOError::new(ErrorKind::InvalidData, "Non positive ID", ));
            }
        }

        if self.check_mask & CHECK_ID_RANGE != 0 {
            if let Ok(simulated_id) = current_id() {
                let out_of_range = if self.check_mask & CHECK_ID_PAST_RANGE != 0 {
                    id.abs_diff(simulated_id) > VALID_ID_RANGE
                } else {
                    id > simulated_id && (id - simulated_id) > VALID_ID_RANGE
                };
                if out_of_range {
                    self.send_diag(&endpoint, EndpointDiag::OutOfRangeId, Some(id));
                    return Err(IOError::new(ErrorKind::InvalidData, "Out of range ID", ));
                }
            }
        }

        if self.check_mask & CHECK_ID_MONOTONIC != 0 {
            if let Some(ep) = endpoint.clone() {
                if let Some(last_id) = self.id_buffer.get(&ep) {
                    if *last_id > id {
                        self.send_diag(&endpoint, EndpointDiag::DecreasingId, Some(id));
                        return Err(IOError::new(ErrorKind::InvalidData, "Decreasing ID"));
                    } else if *last_id == id {
                        self.send_diag(&endpoint, EndpointDiag::RepeatedId, Some(id));
                        return Err(IOError::new(ErrorKind::InvalidData, "Repeated ID"));
                    }
                }
                self.id_buffer.insert(ep, id);
            }
        }
        if message.header_changed() {
            self.send_diag(&endpoint, EndpointDiag::HeaderChange, Some(id));
        }
        Ok(message)
    }

    fn send_diag(&mut self, endpoint: &Option<String>, diag: EndpointDiag, id:Option<u64>) {
        self.increse_stats(endpoint, diag);
        if let Some(socket_monitor) = &self.socket_monitor {
            if let Some(ep) = endpoint {
                socket_monitor.send_diag(ep.clone(), diag, id);
            }
        }
    }

    fn _receive(&mut self) -> (Option<String>, IOResult<Vec<Vec<u8>>>) {
        match &mut self.sockets {
            ConnectionSockets::Shared { socket } => {
                (None, socket.receive())
            }
            ConnectionSockets::Individual { sockets, poll_endpoints, poll_ready_list } => {
                if poll_ready_list.is_empty() {
                    let mut poll_items = Vec::with_capacity(poll_endpoints.len());
                    for endpoint in poll_endpoints.iter() {
                        if let Some(socket) = sockets.get(endpoint) {
                            poll_items.push(socket.socket().as_poll_item(zmq::POLLIN));
                        } else {
                            log::error!("Poll endpoint not found in sockets, updating: {}", endpoint);
                            self.sockets.update_poll_items();
                            return (None, Err(IOError::new(ErrorKind::Other, "Poll endpoint not found")),);
                        }
                    }
                    //In same thread receive is blocking.When forked, must check commands.
                    let timeout = if self.threaded { 10 } else { -1 };
                    if let Err(e) = zmq::poll(&mut poll_items, timeout) {
                        return (None, Err(e.into()));
                    }
                    for (idx, item) in poll_items.iter().enumerate() {
                        if item.is_readable() {
                            poll_ready_list.push_back(poll_endpoints[idx].clone());
                        }
                    }
                }

                if let Some(endpoint) = poll_ready_list.pop_front() {
                    if let Some(socket) = sockets.get(&endpoint) {
                        return (Some(endpoint), socket.receive());
                    };
                }

                (None, Err(IOError::new(ErrorKind::TimedOut, "No socket was readable")),)
            }
        }
    }

    fn receive(&mut self) -> IOResult<ReceivedMessage> {

        if self.connections() == 0 {
            return Err(IOError::new(ErrorKind::NotConnected, "No connected endpoint"));
        }
        let (endpoint, message_parts) = self._receive();


        let message_parts = message_parts.map_err(|e| {
            if e.kind() != ErrorKind::TimedOut {
                //TODO: Should we count socket errors?
                self.send_diag(&endpoint, EndpointDiag::SocketError, None);
            }
            e
        })?;

        let message = self.process(&endpoint, message_parts);
        match message {
            Ok(msg) => {
                //if let Some(socket_monitor) = &self.socket_monitor {
                //    socket_monitor.on_message(&endpoint);
                //}
                self.increse_stats(&endpoint, EndpointDiag::Message);
                Ok(ReceivedMessage { endpoint, message: msg })
            }
            Err(e) => {
                log::trace!("Receiver Error: {}", e);
                self.increse_stats(&endpoint, EndpointDiag::Error);
                Err(IOError::new(e.kind(), e))
            }
        }
    }

    fn connect(&mut self) -> IOResult<()> {
        log::info!("Connecting");
        if !self.connected {
            for endpoint in self.endpoints() {
                //TODO: Should break if one of the endpoints fail?
                self.connect_endpoint(&endpoint)?;
            }
            self.connected = true;
        }
        Ok(())
    }

    fn disconnect(&mut self) {
        log::info!("Disconecting");
        if self.connected {
            self.connected = false;
            //for socket in  self.sockets(){
            //    socket.disconnect();
            //}
            for endpoint in self.endpoints() {
                //TODO: Should break if one of the endpoints fail?
                self.disconnect_endpoint(&endpoint);
            }
        }
    }

    fn add_endpoint(&mut self, endpoint: &str) -> IOResult<()> {
        log::info!("Adding endpoint: {}", endpoint);
        {
            let mut endpoints = self.endpoints.write().unwrap();
            let ep = endpoint.to_string();
            if !endpoints.contains(&ep) {
                endpoints.push(ep);
            }
        }
        if (self.connected) {
            self.connect_endpoint(endpoint)?;
        }
        Ok(())
    }

    fn remove_endpoint(&mut self, endpoint: &str) {
        log::info!("Removing endpoint: {}", endpoint);
        if (self.connected) {
            self.disconnect_endpoint(endpoint);
        }
        {
            let mut endpoints = self.endpoints.write().unwrap();
            endpoints.retain(|e| e != endpoint);
        }
        self.remove_stats(endpoint);
    }

    fn connections(&self) -> usize {
        if let Ok(mut endpoints) =self.endpoints.read() {
            endpoints.len()
        } else {
            0
        }
    }

    fn reset_counters(& mut self) {
        if let Ok(mut stats) = self.stats.write() {
            stats.reset();
        }
        if let Some(fifo) = &self.fifo {
            fifo.reset_dropped()
        }
    }

    fn message_count(&self) -> u32 {
        if let Ok(stats) = self.stats.read() {
            stats.counter_messages
        } else {
            0
        }
    }

    fn is_interrupted(&self) ->bool {
        self.interrupted.load(Ordering::Relaxed) || self.bsread.is_interrupted()
    }

    pub fn stop_forwarder(&mut self) -> IOResult<()> {
        //Only handle lifecycle of forwarder created with forward_config
        if let Some(forwarder_config) = self.forwarder_config.as_mut() {
            if let Some(sender) = self.forwarder.as_mut() {
                sender.stop()
            }
        }
        Ok(())
    }


    pub fn socket(& mut self, endpoint: &str) -> Option<&mut TrackedSocket>{
        match &mut  self.sockets {
            ConnectionSockets::Shared { socket } => {
                Some(socket)
            }
            ConnectionSockets::Individual { sockets, ..} => {
                sockets.get_mut(endpoint)
            }
        }
    }

    pub fn sockets(&mut self) -> Vec<&mut TrackedSocket> {
        match &mut self.sockets {
            ConnectionSockets::Shared { socket } => {
                vec![socket]
            }
            ConnectionSockets::Individual { sockets, .. } => {
                sockets.values_mut().collect()
            }
        }
    }
    fn increse_stats(& mut self, endpoint: &Option<String>, diag:EndpointDiag){
        let ep: &str = endpoint.as_deref().unwrap_or("");
        //Only clone endpoint if entry is absent
        let mut stats = self.stats.write().unwrap();

        if diag == EndpointDiag::Message{
            stats.increase_messages();
        } else if diag == EndpointDiag::Error {
            stats.increase_errors();
        }

        let map = if let Some(map) = stats.diagnostics.get_mut(ep) {
            map
        } else {
            stats.diagnostics.entry(ep.to_string()).or_insert_with(HashMap::new)
        };
        *map.entry(diag).or_insert(0) += 1;
    }

    fn remove_stats(& mut self, endpoint: &str){
        let mut stats = self.stats.write().unwrap();
        stats.diagnostics.remove(endpoint);
    }

    pub fn enable_monitoring(& mut self, socket_monitor: SocketMonitor) -> IOResult<()> {
        if self.socket_monitor.is_none(){ ;
            match &mut self.sockets {
                ConnectionSockets::Shared { socket } => {
                    //socket.enable_monitoring(self.bsread.context())
                    socket.enable_monitoring(self.bsread.context(), &socket_monitor, None)?;

                }
                ConnectionSockets::Individual { sockets, ..} => {
                    for (endpoint, socket) in sockets.iter_mut() {
                        socket.enable_monitoring(self.bsread.context(),  &socket_monitor, Some(endpoint.clone()))?;
                    }
                }
            }
            self.socket_monitor = Some(socket_monitor)
        }
        Ok(())
    }

    fn launch<F>(
        bsread: Arc<Bsread>,
        index: u32,
        endpoints: Arc<RwLock<Vec<String>>>,
        socket_type: SocketType,
        connection_mode: ConnectionMode,
        callback: F,
        num_messages: Option<u32>,
        fifo: Option<Arc<FifoQueue<ReceivedMessage>>>,
        stats: Arc<RwLock<Stats>>,
        forwarder_config: Option<ForwarderConfig>,
        interrupted: Arc<AtomicBool>,
        raw: bool,
        socket_options: Arc<Mutex<SocketOptions>>,
        socket_monitor: Option<SocketMonitor>,
        rx_cmd: crossbeam_channel::Receiver<ReceiverCommand>,
        check_mask: u64
    ) -> IOResult<()>
    where
        F: Fn(ReceivedMessage) + Send + 'static,
    {
        let mut worker = Worker::new(
            index, bsread, endpoints, socket_type, connection_mode, socket_options, socket_monitor,
            rx_cmd, forwarder_config, None, fifo, stats, interrupted, raw, check_mask, true
        );
        worker
            .listen(callback, num_messages)
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stop_forwarder();
        self.sockets.clear();
        self.socket_monitor = None;

    }
}


pub struct Receiver {
    endpoints: Arc<RwLock<Vec<String>>>,
    socket_type: SocketType,
    check_mask: u64,
    bsread: Arc<Bsread>,
    fifo: Option<Arc<FifoQueue<ReceivedMessage>>>,
    handle: Option<JoinHandle<IOResult<()>>>,
    #[cfg(feature = "async")]
    async_handle: Option<tokio::task::JoinHandle<IOResult<()>>>,
    stats: Arc<RwLock<Stats>>,
    index: u32,
    forwarder_config: Option<ForwarderConfig>,
    forwarder: Option<Sender>,
    interrupted: Arc<AtomicBool>,
    delivery_mode: DeliveryMode,
    raw: bool,
    connection_mode: ConnectionMode,
    socket_monitor: Option<SocketMonitor>,
    tx_cmd:crossbeam_channel::Sender<ReceiverCommand>,
    rx_cmd:crossbeam_channel::Receiver<ReceiverCommand>,
    forked:bool,
    blocking_config: bool,
    socket_options: Arc<Mutex<SocketOptions>>,
    worker: Option<Worker>,
}


impl Receiver{
    pub fn new(bsread: Arc<Bsread>, endpoints: Option<Vec<&str>>, socket_type: SocketType, connection_mode: ConnectionMode) -> IOResult<Self> {
        let index =  index();
        let endpoints = Arc::new(RwLock::new(endpoints
            .unwrap_or_default()
            .into_iter()
            .map(str::to_string)
            .collect()));
        let stats = Arc::new(RwLock::new(Stats{counter_messages:0, counter_error:0, counter_drop:0, diagnostics:HashMap::new()}));
        let delivery_mode = DeliveryMode::Inline;
        let  interrupted = Arc::new(AtomicBool::new(false));
        let (tx_cmd, rx_cmd) = crossbeam_channel::unbounded();
        let check_mask = CHECK_ALL;
        let socket_options =Arc::new( Mutex::new(SocketOptions::new()));

        Ok(Self { endpoints, socket_type, check_mask,
            bsread, fifo:None, handle:None,
            stats, index,
            forwarder_config:None, forwarder:None,interrupted, delivery_mode , raw: false,connection_mode,
            socket_monitor:None, tx_cmd, rx_cmd, forked: false, socket_options, blocking_config:true,
            #[cfg(feature = "async")]
            async_handle:None, worker:None
        })
    }

    pub fn to_string(& self,) -> String {
        format!("Receiver {}" , self.index)
    }


    pub fn blocking_config(&self) -> bool{
        self.blocking_config
    }

    pub fn set_blocking_config(&mut self, value: bool){
        self.blocking_config = value;
    }

    fn send_command<T>(&self,command: impl FnOnce(Option<crossbeam_channel::Sender<IOResult<T>>>) -> ReceiverCommand,) -> IOResult<T> {
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.tx_cmd.send(command(Some(tx)))
            .map_err(|_| {IOError::new(std::io::ErrorKind::BrokenPipe,"Receiver thread is not running",)})?;
        rx.recv().map_err(|_| {IOError::new(std::io::ErrorKind::BrokenPipe,"Receiver thread terminated",)})?
    }

    fn send_command_no_wait(&self,command: impl FnOnce(Option<crossbeam_channel::Sender<IOResult<()>>>) -> ReceiverCommand,) -> IOResult<()> {
        self.tx_cmd.send(command(None))
            .map_err(|_| {IOError::new(std::io::ErrorKind::BrokenPipe,"Receiver thread is not running",)})
    }

    pub fn connect(&mut self) -> IOResult<()> {
        if self.delivery_mode.thraded(){
            if self.blocking_config {
                self.send_command(|response| { ReceiverCommand::Connect { response } })
            } else {
                self.send_command_no_wait(|_| { ReceiverCommand::Connect { response:None } })
            }
        } else {
            self.create_worker().connect()
        }
    }

    pub fn disconnect(&mut self)  {
        if let Some(mut worker) = self.worker.as_mut() {
            worker.disconnect();
        } else if self.delivery_mode.thraded(){
            if self.blocking_config {
                self.send_command(|response| { ReceiverCommand::Disconnect { response } });
            } else {
                self.send_command_no_wait(|_| { ReceiverCommand::Disconnect { response:None } });
            }
        }
    }

    pub fn add_endpoint(&mut self, endpoint: &str) -> IOResult<()> {
        if let Some(mut worker) = self.worker.as_mut() {
            worker.add_endpoint(endpoint)
        } else if self.delivery_mode.thraded(){
            let endpoint = endpoint.to_string();
            if self.blocking_config {
                self.send_command(|response| { ReceiverCommand::AddEndpoint { endpoint, response } })
            } else {
                self.send_command_no_wait(|_| { ReceiverCommand::AddEndpoint { endpoint, response:None } })
            }
        } else {
            let mut endpoints = self.endpoints.write().unwrap();
            let ep = endpoint.to_string();
            if !endpoints.contains(&ep) {
                endpoints.push(ep);
            }
            Ok(())
        }
    }

    pub fn remove_endpoint(&mut self, endpoint: &str) {
        if let Some(mut worker) = self.worker.as_mut() {
            worker.remove_endpoint(endpoint)
        } else if self.delivery_mode.thraded(){
            let endpoint = endpoint.to_string();
            if self.blocking_config {
                self.send_command(|response| { ReceiverCommand::RemoveEndpoint { endpoint, response } });
            } else {
                self.send_command_no_wait(|_| { ReceiverCommand::RemoveEndpoint { endpoint, response:None } });
            }
        } else {
            let mut endpoints = self.endpoints.write().unwrap();
            endpoints.retain(|e| e != endpoint);
        }
    }

    pub fn enable_monitoring(& mut self)-> IOResult< crossbeam_channel::Receiver<EndpointEvent>> {
        if self.socket_monitor.is_none() {
            let socket_monitor = SocketMonitor::new();
            self.enable_shared_monitoring(&socket_monitor);
        }
        match self.socket_monitor.as_ref() {
            None => {Err(IOError::new(std::io::ErrorKind::BrokenPipe,"Socket monitor is none",))},
            Some(socket_monitor) => {Ok(socket_monitor.diag_rx())}
        }
    }

    pub fn enable_shared_monitoring(&mut self, socket_monitor: &SocketMonitor)-> IOResult<()> {
        if self.socket_monitor.is_none() {
            let mut monitor = socket_monitor.clone();
            let socket_options = self.socket_options.lock().unwrap();
            if socket_options.handshake_ivl == Some(0) {
                monitor.disable_handshake_check();
            }
            self.socket_monitor = Some(monitor.clone());
            if let Some(mut worker) = self.worker.as_mut() {
                worker.enable_monitoring(monitor);
            } else if self.delivery_mode.thraded() {
                self.send_command(|response| { ReceiverCommand::EnableMonitoring { monitor, response } });
            }
        }
        Ok(())
    }

    pub fn socket_options(& mut self, endpoint: &str) -> IOResult<SocketOptions>{
        if let Some(mut worker) = self.worker.as_mut() {
            Ok(worker.socket(endpoint).ok_or(IOError::new(std::io::ErrorKind::InvalidData, "Invalid endpoint"))?.options())
        } else if self.delivery_mode.thraded() {
            let endpoint = endpoint.to_string();
            self.send_command(|response| { ReceiverCommand::SocketOptions { endpoint, response } })
        } else {
            Err(IOError::new(std::io::ErrorKind::BrokenPipe,"Receiver thread is not running",))
        }
    }


    pub fn endpoints(&self) ->  Vec<String> {
        self.endpoints.read().unwrap().clone()
    }

    pub fn has_endpoint(&self, endpoint: &str) -> bool {
        self.endpoints
            .read()
            .unwrap()
            .iter()
            .any(|e| e == endpoint)
    }

    pub fn forwarder(& self) -> &Option<Sender>{
        &self.forwarder
    }

    pub fn set_forwarder(&mut self, forwarder_sender: sender::Sender) {
        self.forwarder = Some(forwarder_sender);
    }

    pub fn set_forwarder_config(&mut self, forwarder_config: ForwarderConfig) {
        self.forwarder_config = Some(forwarder_config);
    }

    pub fn set_raw(&mut self, raw:bool) {
        self.raw = raw;
    }
    pub fn is_raw(&self) -> bool{
        self.raw
    }

    fn create_worker(& mut self) -> &mut Worker {
        self.delivery_mode = DeliveryMode::Inline;
        if self.worker.is_none() {
            let worker = Worker::from_receiver(self);
            self.worker = Some(worker);
        }
        self.worker.as_mut().expect("Worker was just initialized")
    }

    //Synchronous Mode: blocking, callback in same thread
    pub fn listen<F>(&mut self, callback: F, num_messages: Option<u32>) -> IOResult<()>
    where
        F: Fn(ReceivedMessage),
    {
        self.create_worker().listen(callback, num_messages)
    }

    pub fn receive(&mut self) -> IOResult<ReceivedMessage> {
        let worker= self.create_worker();
        worker.connect();
        worker.receive()
    }

    //Threaded Mode: non-blocking, callback in another thread
    pub fn fork<F>(&mut self, callback: F, num_messages: Option<u32>)
    where
        F: Fn(ReceivedMessage) + Send + 'static,
    {
        self.worker = None;

        let index = self.index;
        let bsread = Arc::clone(&self.bsread);
        let endpoints = Arc::clone(&self.endpoints);
        let socket_type = self.socket_type.clone();
        let connection_mode = self.connection_mode.clone();
        let interrupted = Arc::clone(&self.interrupted);
        let forwarder_config = self.forwarder_config.clone();
        let fifo = self.fifo.clone();
        let stats = Arc::clone(&self.stats);
        let raw = self.raw;
        let thread_name = self.to_string();
        let socket_options = Arc::clone(&self.socket_options);
        let rx_cmd = self.rx_cmd.clone();
        let check_mask = self.check_mask;
        let socket_monitor = self.socket_monitor.clone();

        let handle = thread::Builder::new()
            .name(thread_name)
            .spawn(move || {
                Worker::launch(bsread, index, endpoints, socket_type, connection_mode, callback, num_messages, fifo, stats,
                               forwarder_config, interrupted, raw, socket_options, socket_monitor, rx_cmd, check_mask)
            })
            .expect("Failed to spawn thread");

        self.handle = Some(handle);
        self.delivery_mode = DeliveryMode::Threaded;
    }

    pub fn join(& mut self) -> IOResult<()> {
        if let Some(handle) = self.handle.take() { // Take ownership of the handle
            self.handle = None;
            handle
                .join()
                .map_err(|e| {
                    log::error!("Listener thread error: {:?}", e);
                    // Handle thread panic and convert to a std::io::Error
                    let error_message = format!("Thread error: {:?}", e);
                    IOError::new(ErrorKind::Other, error_message.as_str())
                })?
                .map_err(|e| {
                    let desc = e.to_string();
                    let parts: Vec<&str> = desc.split('|').collect();
                    log::error!("Listener thread join error: {:?}", parts);
                    IOError::new(error_kind_from_str(parts[0]), parts[1])
                })?;
        }
        Ok(())
    }

    #[cfg(feature = "async")]
    pub fn start_async<F, Fut>(
        &mut self,
        callback: F,
        num_messages: Option<u32>,
        execution: AsyncExecution,
        handle: Option<tokio::runtime::Handle>,
    )
    where
        F: Fn(ReceivedMessage) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.worker = None;

        let index = self.index;
        let bsread = Arc::clone(&self.bsread);
        let endpoints = Arc::clone(&self.endpoints);
        let socket_type = self.socket_type.clone();
        let connection_mode = self.connection_mode.clone();
        let interrupted = Arc::clone(&self.interrupted);
        let forwarder_config = self.forwarder_config.clone();
        let stats = Arc::clone(&self.stats);
        let raw = self.raw;
        let socket_options = Arc::clone(&self.socket_options);
        let socket_monitor = self.socket_monitor.clone();
        let rx_cmd = self.rx_cmd.clone();
        let check_mask = self.check_mask;

        let handle = handle.unwrap_or_else(tokio::runtime::Handle::current);

        let join_handle = match execution {
            AsyncExecution::Concurrent => {
                let callback_handle = handle.clone();

                handle.spawn_blocking(move || {
                    let cb = move |msg: ReceivedMessage| {
                        callback_handle.spawn(callback(msg));
                    };
                    Worker::launch(bsread, index, endpoints, socket_type, connection_mode, cb,
                                   num_messages, None, stats,
                                   forwarder_config, interrupted, raw,
                                   socket_options, socket_monitor, rx_cmd, check_mask)
                })
            }

            AsyncExecution::Ordered { capacity, blocking } => {
                let callback = Arc::new(callback);
                let callback_handle = handle.clone();
                let st = Arc::clone(&self.stats);

                handle.spawn_blocking(move || {
                    let senders =
                        Arc::new(Mutex::new(
                            HashMap::<
                                String,
                                tokio::sync::mpsc::Sender<ReceivedMessage>,
                            >::new(),
                        ));

                    let monitor = socket_monitor.clone();
                    let drop_stats = Arc::clone(&stats);
                    let cb = move |msg: ReceivedMessage| {
                        let endpoint = msg.endpoint.clone().unwrap_or_default();

                        let sender = {
                            let mut senders = senders.lock().unwrap();
                            senders
                                .entry(endpoint)
                                .or_insert_with(|| {
                                    let (tx, mut rx) =tokio::sync::mpsc::channel(capacity);
                                    let callback = Arc::clone(&callback);
                                    callback_handle.spawn(async move {
                                        while let Some(msg) = rx.recv().await {
                                            callback(msg).await;
                                        }
                                    });
                                    tx
                                })
                                .clone()
                        };
                        if blocking {
                            if let Err(err) = sender.blocking_send(msg) {
                                log::error!(
                                "Error sending blocking message: {:?}",
                                err
                            );
                            }
                        } else {
                            match sender.try_send(msg) {
                                Ok(()) => {}
                                Err(tokio::sync::mpsc::error::TrySendError::Full(msg)) => {
                                    log::debug!("Dropping message {} from {:?}: endpoint queue is full",msg.message.id(),msg.endpoint);
                                    if let Some(endpoint) = msg.endpoint {
                                        let mut st = st.write().unwrap();
                                        *st.diagnostics.entry(endpoint.to_string()).or_default().entry(EndpointDiag::Dropped).or_default() += 1;
                                        if let Some(sm) = &monitor {
                                            sm.send_diag(endpoint, EndpointDiag::Dropped, Some(msg.message.id()),);
                                        }
                                    }
                                    drop_stats.write().unwrap().increase_drops();
                                }
                                Err(err) => {
                                    log::error!("Error trying sending message: {:?}", err);
                                }
                            }
                        }
                    };
                    Worker::launch(bsread, index, endpoints, socket_type, connection_mode, cb,
                                   num_messages, None, stats,
                                   forwarder_config, interrupted, raw,
                                   socket_options, socket_monitor, rx_cmd, check_mask)
                })
            }
        };

        self.delivery_mode = DeliveryMode::Async;
        self.async_handle = Some(join_handle);
    }
    #[cfg(feature = "async")]
    pub async fn join_async(&mut self) -> IOResult<()> {
        if let Some(handle) = self.async_handle.take() {
            match handle.await {
                Ok(result) => result.map_err(|e| {
                    io::Error::new(io::ErrorKind::Other, e.to_string())
                }),
                Err(e) => Err(io::Error::new(
                    io::ErrorKind::Other,format!("Tokio join error: {}", e),
                )),
            }
        } else {
            Ok(())
        }
    }

    pub fn is_running(&self) -> bool {
        let running = self.handle.as_ref().is_some_and(|h| !h.is_finished());
        #[cfg(feature = "async")]
        let running = running || self.async_handle.as_ref().is_some_and(|h| !h.is_finished());
        running
    }

    //Buffered mode: non-blocking, messages buffered ibn another thread
    pub fn start(&mut self, buffer_size:usize) -> IOResult<()> {
        if self.fifo.is_some(){
            return Err(IOError::new(ErrorKind::AlreadyExists, "Receiver already started"));
        }
        self.fifo = Some(Arc::new(FifoQueue::new(buffer_size)));
        fn callback(_: ReceivedMessage) -> () {}
        self.fork(callback, None);
        self.delivery_mode = DeliveryMode::Buffered;
        Ok(())
    }

    pub fn interrupt(&self) {
        self.interrupted.store(true, Ordering::Relaxed);
    }

    pub fn is_interrupted(&self) ->bool {
        self.interrupted.load(Ordering::Relaxed) || self.bsread.is_interrupted()
    }

    pub fn stop(&mut self) -> IOResult<()> {
        self.interrupt();
        self.join()?;
        self.fifo = None;
        Ok(())
    }

    pub fn get(&self) -> Option<ReceivedMessage> {
        match &self.fifo{
            None => {None}
            Some(fifo) => {fifo.get()}
        }
    }

    pub fn wait(&self, timeout_ms: u64) -> IOResult<ReceivedMessage> {
        match &self.fifo{
            None => {
                Err(IOError::new(ErrorKind::Unsupported, "Operation only valid for buffered delivery mode"))
            }
            Some(fifo) => {
                match fifo.wait(timeout_ms){
                    None => {
                        Err(IOError::new(ErrorKind::TimedOut, "Timeout waiting for message"))
                    }
                    Some(rx) => {
                        Ok(rx)
                    }
                }
            }
        }
    }

    pub fn wait_messages(&self, count:usize, timeout_ms: u64) -> IOResult<Vec<ReceivedMessage>> {
        let mut ret = Vec::new();
        for _ in 0..count {
            let msg = self.wait(timeout_ms)?;
            ret.push(msg);
        }
        Ok(ret)
    }

    pub fn fifo(&self) -> Option<Arc<FifoQueue<ReceivedMessage>>> {
        match &self.fifo{
            None => {None}
            Some(fifo) => {Some(fifo.clone())}
        }
    }
    pub fn index(&self) -> u32 {
        self.index
    }

    pub fn delivery_mode(&self) -> DeliveryMode {
        self.delivery_mode.clone()
    }

    pub fn connection_mode(&self) -> ConnectionMode {
        self.connection_mode.clone()
    }

    pub fn connections(&self) -> usize {
        self.endpoints.read().unwrap().len()
    }

    pub fn available(&self) -> u32 {
        if let Some(fifo) = &self.fifo {
            fifo.available_count() as u32
        } else {
            0
        }
    }

    pub fn reset_counters(& mut self) {
        self.stats.write().unwrap().reset()
    }
    pub fn diagnostics(&self) -> HashMap<String, HashMap<EndpointDiag, u32>>{
        self.stats.read().unwrap().diagnostics.clone()
    }
    pub fn diagnostics_endpoints(&self) -> Vec<String> {
        self.stats.read().unwrap().diagnostics.keys().cloned().collect()
    }

    pub fn endpoint_diagnostics(& self,  endpoint: &str) -> Option<HashMap<EndpointDiag, u32>> {
        self.stats.read().unwrap().diagnostics.get(endpoint).cloned()
    }

    pub fn endpoint_diagnostic(& self,  endpoint: &str, diag:EndpointDiag) -> Option<u32> {
        self.stats.read().unwrap().diagnostics.get(endpoint)?.get(&diag).copied()
    }

    pub fn header_changes(& self,  endpoint:  &str) -> u32 {
        self.endpoint_diagnostic(endpoint, EndpointDiag::HeaderChange).unwrap_or(
            if let Some (x) =  self.endpoint_diagnostic(endpoint, EndpointDiag::Message) {
                1
            } else {
                0
            }
        )
    }

    pub fn messages(&self) -> u32 {
        if let Ok(stats) = self.stats.read() {
            stats.counter_messages
        } else {
            0
        }
    }

    pub fn errors(&self) -> u32 {
        if let Ok(stats) = self.stats.read() {
            stats.counter_error
        } else {
            0
        }
    }

    pub fn dropped(&self) -> u32 {
        if let Some(fifo) = &self.fifo {
            fifo.dropped_count()
        } else {
            if let Ok(stats) = self.stats.read() {
                stats.counter_drop
            } else {
                0
            }
        }
    }

    pub fn endpoint_state(&self, endpoint: &str) -> Option<EndpointState> {
        match &self.socket_monitor{
            None => {None}
            Some(socket_monitor) => {socket_monitor.endpoint_state(endpoint)}
        }
    }
    pub fn endpoint_states(&self) -> HashMap<String, EndpointState> {
        match &self.socket_monitor{
            None => {HashMap::new()}
            Some(socket_monitor) => {socket_monitor.endpoint_states()}
        }
    }

    pub fn enable_check(& mut self, check:u64){
        self.check_mask = self.check_mask | check;
    }

    pub fn disable_check(& mut self, check:u64){
        self.check_mask = self.check_mask & !check;
    }

    pub fn socket_type(&self) -> SocketType {
        self.socket_type
    }

}

impl SocketConfig for Receiver {
    fn socket(&self) -> Option<&zmq::Socket> {
        match self.worker.as_ref(){
            None => {None}
            Some(worker) => {
                match &worker.sockets {
                    ConnectionSockets::Shared { socket } => {
                        Some(&socket.socket())
                    }
                    ConnectionSockets::Individual { sockets, .. } => {
                        None
                    }
                }
            }
        }

    }

    fn set_linger(&mut self, value: i32) -> IOResult<()> {
        let mut socket_options = self.socket_options.lock().unwrap();
        socket_options.linger = Some(value);
        self.set_options(&socket_options)?;
        Ok(())
    }

    fn set_rcvhwm(&mut self, value: i32)-> IOResult<()> {
        let mut socket_options = self.socket_options.lock().unwrap();
        socket_options.rcvhwm = Some(value);
        self.set_options(&socket_options)?;
        Ok(())
    }

    fn set_sndhwm(&mut self, value: i32)-> IOResult<()> {
        let mut socket_options = self.socket_options.lock().unwrap();
        socket_options.sndhwm = Some(value);
        self.set_options(&socket_options)?;
        Ok(())
    }
    fn set_handshake_ivl(&mut self, value: i32)-> IOResult<()> {
        let mut socket_options = self.socket_options.lock().unwrap();
        socket_options.handshake_ivl = Some(value);
        self.set_options(&socket_options)?;
        if value == 0 {
            if let Some(mut socket_monitor) = self.socket_monitor.as_mut() {
                socket_monitor.disable_handshake_check();
            }
        }
        Ok(())
    }

    fn set_keepalive(&mut self, idle: i32, intvl: i32, cnt: i32) -> IOResult<()> {
        let mut socket_options = self.socket_options.lock().unwrap();
        socket_options.keepalive = Some(KeepAlive { idle, intvl, cnt});
        self.set_options(&socket_options)?;
        Ok(())
    }

    fn set_heartbeat(&mut self, ivl: i32, timeout: i32, ttl: i32) -> IOResult<()> {
        let mut socket_options = self.socket_options.lock().unwrap();
        socket_options.heartbeat = Some(Heartbeat { ivl, timeout, ttl});
        self.set_options(&socket_options)?;
        Ok(())
    }
}


fn error_kind_from_str(s: &str) -> ErrorKind {
    let str = s.replace(" ", "").to_lowercase();
    match str.as_str() {
        "notfound" => ErrorKind::NotFound,
        "permissiondenied" => ErrorKind::PermissionDenied,
        "connectionrefused" => ErrorKind::ConnectionRefused,
        "connectionreset" => ErrorKind::ConnectionReset,
        "connectionaborted" => ErrorKind::ConnectionAborted,
        "notconnected" => ErrorKind::NotConnected,
        "addrinuse" => ErrorKind::AddrInUse,
        "addrnotavailable" => ErrorKind::AddrNotAvailable,
        "brokenpipe" => ErrorKind::BrokenPipe,
        "alreadyexists" => ErrorKind::AlreadyExists,
        "wouldblock" => ErrorKind::WouldBlock,
        "invalidinput" => ErrorKind::InvalidInput,
        "invaliddata" => ErrorKind::InvalidData,
        "timedout" => ErrorKind::TimedOut,
        "interrupted" => ErrorKind::Interrupted,
        "unsupported" => ErrorKind::Unsupported,
        "unexpectedeof" => ErrorKind::UnexpectedEof,
        "outofmemory" => ErrorKind::OutOfMemory,
        _ => ErrorKind::Other,  // Return Other for unknown variants
    }
}


impl Drop for Receiver {
    fn drop(&mut self) {
        self.socket_monitor = None;
    }
}

