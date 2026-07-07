use crate::*;
use crate::message::*;
use crate::utils::*;
use crate::sockets::*;
use std::{io, thread};
use std::collections::HashMap;
use std::error::Error;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use zmq::{Context, SocketType};
use std::time::{Duration, Instant};
use uuid::Uuid;

static RECEIVER_INDEX: Mutex<u32> = Mutex::new(0);
fn index() -> u32{
    unsafe {
        let mut counter = RECEIVER_INDEX.lock().unwrap();
        *counter += 1;
        *counter
    }
}

struct Stats {
    counter_messages: u32,
    counter_error: u32,
    counter_header_changes: u32
}

impl Stats{
    fn increase_messages(& mut self){
        self.counter_messages = self.counter_messages + 1;
    }
    fn increase_errors(& mut self){
        self.counter_error = self.counter_error + 1;
    }
    fn increase_header_changes(& mut self){
        self.counter_header_changes = self.counter_header_changes + 1;
    }

    pub fn reset(& mut self){
        self.counter_messages = 0;
        self.counter_error = 0;
        self.counter_header_changes = 0;
    }

}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryMode {
    Callback,
    Buffered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionMode {
    Shared,     //Single receive socked
    Individual,  //One socket per endpoint
}


enum ConnectionSockets {
    Shared {
        socket: TrackedSocket,
    },
    Individual {
        sockets: HashMap<String, TrackedSocket>,
    },
}


const VALID_ID_RANGE:u64 = 3600 * 24 * 100;

pub const CHECK_ID_POSITIVE:u64 = 1;
pub const CHECK_ID_MONOTONIC:u64 = 2;
pub const CHECK_ID_RANGE:u64 = 3;
pub const CHECK_ID_PAST_RANGE:u64 = 4;

pub const CHECK_ALL:u64 = !0;


pub struct Receiver {
    sockets: ConnectionSockets,
    endpoints: Option<Vec<String>>,
    socket_type: SocketType,
    header_buffer: LimitedHashMap<String, DataHeaderInfo>,
    id_buffer: HashMap<String, u64>,
    check_mask: u64,
    bsread: Arc<Bsread>,
    fifo: Option<Arc<FifoQueue<Message>>>,
    handle: Option<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>>,
    stats: Arc<Mutex<Stats>>,
    index: u32,
    forwarder_config: Option<ForwarderConfig>,
    forwarder: Option<Sender>,
    interrupted: Arc<AtomicBool>,
    delivery_mode: DeliveryMode,
    raw: bool,
    connection_mode: ConnectionMode,
    socket_monitor: Option<SocketMonitor>,
    tx:crossbeam_channel::Sender<EndpointEvent>,
    rx:crossbeam_channel::Receiver<EndpointEvent>,
}


impl
Receiver{
    pub fn new(bsread: Arc<Bsread>, endpoints: Option<Vec<&str>>, socket_type: SocketType, connection_mode: ConnectionMode) -> IOResult<Self> {
        let index =  index();
        let mut sockets:ConnectionSockets = match connection_mode{
            ConnectionMode::Shared => {
                ConnectionSockets::Shared {socket: TrackedSocket::new(&bsread.context(), socket_type, index)?}
            }
            ConnectionMode::Individual => {
                ConnectionSockets::Individual {sockets: HashMap::new()}
            }
        };
        let endpoints = endpoints.map(|vec| vec.into_iter().map(|s| s.to_string()).collect());
        let stats = Arc::new(Mutex::new(Stats{counter_messages:0, counter_error:0, counter_header_changes:0}));
        let delivery_mode = DeliveryMode::Callback;
        let  interrupted = Arc::new(AtomicBool::new(false));
        let (tx, rx) = crossbeam_channel::unbounded();
        let check_mask = CHECK_ALL;

        Ok(Self { sockets, endpoints, socket_type, header_buffer: LimitedHashMap::void(), id_buffer: HashMap::new(), check_mask,
            bsread, fifo:None, handle:None, stats, index,
            forwarder_config:None, forwarder:None,interrupted, delivery_mode , raw: false,connection_mode, socket_monitor:None, tx,rx})
    }

    pub fn to_string(& self,) -> String {
        format!("Receiver {}" , self.index)
    }

    pub fn connect(&mut self) -> IOResult<()> {
        if let Some(endpoints) = self.endpoints.clone() { // Clone to avoid immutable borrow
            for endpoint in endpoints {
                //TODO: Should break if one of the endpoints fail?
                self.connect_endpoint(&endpoint)?;
            }
        }
        Ok(())
    }

    pub fn disconnect(&mut self)  {
        for socket in  self.sockets(){
            socket.disconnect();
        }
    }

    pub fn add_endpoint(&mut self, endpoint: &str) {
        match &mut self.endpoints {
            Some(vec) => {
                let ep = endpoint.to_string();
                if !vec.contains(&ep) {
                    vec.push(ep);
                }
            }
            None => {
                self.endpoints = Some(vec![endpoint.to_string()]);
            }
        }
    }


    pub fn connect_endpoint(&mut self, endpoint: &str) -> IOResult<()> {
        self.add_endpoint(endpoint);
        let context = self.bsread.context();
        let socket_type = self.socket_type();
        let index = self.index;
        match &mut self.sockets {
            ConnectionSockets::Shared { socket } => {
                socket.connect(endpoint)?
            }
            ConnectionSockets::Individual { sockets} => {
                match sockets.get(endpoint){
                    None => {
                        let mut socket = TrackedSocket::new(context, socket_type, index)?;
                        socket.connect(endpoint)?;
                        if let Some(socket_monitor) = &self.socket_monitor {
                            socket.enable_monitoring(self.bsread.context(), &socket_monitor, Some(endpoint.to_string()))?;
                        }
                        sockets.insert(endpoint.to_string(), socket);
                    }
                    Some(_) => {}
                }
            }
        }
        Ok(())
    }

    pub fn disconnect_endpoint(&mut self, endpoint: &str)  {
        match &mut self.sockets {
            ConnectionSockets::Shared { socket } => {
                socket.disconnect_endpoint(endpoint);
            }
            ConnectionSockets::Individual { sockets} => {
                match sockets.get_mut(endpoint){
                    None => {}
                    Some(socket) => {
                        socket.disconnect();
                        sockets.remove(endpoint);
                    }
                }
            }
        }
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

    fn process(&mut self, endpoint: Option<String>, message_parts:Vec<Vec<u8>>) -> IOResult<Message> {
        if let Some(sender) = self.forwarder.as_mut() {
            match sender.forward(&message_parts) {
                Ok(_) => (),
                Err(e) => log::warn!("Error forwarding message to {}: {}", sender.endpoint(), e),
            }
        }
        let message = parse_message(message_parts, &mut self.header_buffer, &mut self.stats.lock().unwrap().counter_header_changes, self.raw)?;
        self.check_message(message, endpoint)
    }


    fn check_message(&mut self, message:Message,  endpoint: Option<String>) -> IOResult<(Message)> {
        let id = message.id();
        if self.check_mask & CHECK_ID_POSITIVE != 0 {
            if id <=0 {
                return Err(IOError::new(ErrorKind::InvalidData,"Non positive ID",));
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
                    return Err(IOError::new(ErrorKind::InvalidData, "Out of range ID", ));
                }
            }
        }

        if self.check_mask & CHECK_ID_MONOTONIC != 0 {
            if let Some(endpoint) = endpoint {
                if let Some(last_id) = self.id_buffer.get(&endpoint){
                    if *last_id > id{
                        return Err(IOError::new(ErrorKind::InvalidData,"Decreasing ID"));
                    } else if *last_id == id{
                        return Err(IOError::new(ErrorKind::InvalidData,"Repeated ID"));
                    }
                }
                self.id_buffer.insert(endpoint, id);
            }
        }
        Ok(message)
    }

    pub fn receive(&mut self) -> IOResult<Message> {
        let (endpoint, message_parts)  =  match &self.sockets {
            ConnectionSockets::Shared { socket } => {
                (None, socket.receive()?)
            }
            ConnectionSockets::Individual { sockets } => {
                let mut items: Vec<_> = sockets
                    .values()
                    .map(|socket| socket.socket().as_poll_item(zmq::POLLIN))
                    .collect();

                zmq::poll(&mut items, -1)?;
                let mut result = None;

                for (item, socket) in items.iter().zip(sockets.values()) {
                    if item.is_readable() {
                        let endpoint = socket.endpoint(0).ok_or_else(|| {
                            IOError::new(ErrorKind::Other,"Individual socket with no endpoint",)
                        })?;
                        let message_parts = socket.receive()?;
                        result = Some((Some(endpoint), message_parts));
                        break;                    }
                }
                result.ok_or_else(|| {
                    IOError::new(ErrorKind::Other,"poll() returned but no socket was readable",)
                })?
            }
        };

        self.process(endpoint, message_parts)
    }


    //Synchronous Mode: blocking, callback in same thread
    pub fn listen<F>(&mut self, mut callback: F, num_messages: Option<u32>) -> IOResult<()>
    where
        F: FnMut(Message),
    {
        self.reset_counters();
        if let Some(cfg) = self.forwarder_config.as_mut() {
            match Sender::new(self.bsread.clone(), cfg.socket_type, cfg.transport.clone(), None, None, None,) {
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
        if self.header_buffer.is_void(){
            self.set_header_buffer_size(self.connections());
        }

        loop {
            let message = self.receive();
            match message {
                Ok(msg) => {
                    match &self.fifo {
                        None => {callback(msg)}
                        Some(fifo) => {fifo.add(msg)}
                    };
                    self.stats.lock().unwrap().increase_messages();
                }
                Err(e) => {
                    //TODO: error callback?
                    log::trace!("Receiver Error: {}", e);
                    self.stats.lock().unwrap().increase_errors();
                }
            }
            if num_messages.map_or(false, |m| self.stats.lock().unwrap().counter_messages >= m) {
                break;
            }
            if self.is_interrupted() {
                break;
            }
        }
        self.stop_forwarder();
        Ok(())
    }

    //Asynchronous Mode: non-bloclong, callback in another thread
    pub fn fork<F>(& mut self, mut callback: F,  num_messages: Option<u32>)
        where
        F: FnMut(Message) + Send + 'static,
        {

        fn listen_process<F>(endpoint: Option<Vec<&str>>, socket_type: SocketType, connection_mode:ConnectionMode,
                             callback: Arc<Mutex<F>>, num_messages: Option<u32>,
                             producer_fifo: Option<Arc<FifoQueue<Message>>>, producer_stats:Arc<Mutex<Stats>>,
                             forwarder_config:Option<ForwarderConfig>,
                             interrupted_context: Arc<AtomicBool>, interrupted_self: Arc<AtomicBool>, raw: bool) -> IOResult<()>
            where
                F: FnMut(Message) + Send + 'static,
            {
            let bsread = crate::Bsread::new_with_interrupted(interrupted_context).unwrap();
            let mut receiver = bsread.receiver(endpoint, socket_type, connection_mode)?;
            receiver.fifo = producer_fifo;
            receiver.stats = producer_stats;
            receiver.interrupted = interrupted_self;
            receiver.forwarder_config = forwarder_config;
            receiver.raw = raw;
            let mut callback = callback.lock().unwrap();
            receiver.listen(&mut callback.deref_mut(), num_messages)
        }
        let endpoints: Option<Vec<String>> = self.endpoints.as_ref().map(|vec| vec.clone());
        let socket_type = self.socket_type.clone();
        let connection_mode = self.connection_mode.clone();
        let interrupted_context = Arc::clone(self.bsread.interrupted());
        let interrupted_self = Arc::clone(&self.interrupted);
        let forwarder_config = self.forwarder_config.clone();

        let producer_fifo = match &self.fifo {
            None => { None }
            Some(f) => { Some(f.clone()) }
        };
        //let producer_stats = Arc::clone(&self.stats);
        //let producer_stats = Arc::new(Mutex::new(&self.stats));
        let producer_stats =self.stats.clone();
        let shared_callback = Arc::new(Mutex::new(callback));
        let raw = self.raw;
        let thread_name = self.to_string(); 
        let handle = thread::Builder::new()
            .name(thread_name.to_string())
            .spawn(move || -> Result<(), Box<dyn std::error::Error + Send + Sync>>{
                let endpoints_as_str: Option<Vec<&str>> = endpoints.as_ref().map(|vec| vec.iter().map(String::as_str).collect());
                listen_process(endpoints_as_str, socket_type, connection_mode, shared_callback, num_messages, producer_fifo, producer_stats, forwarder_config, interrupted_context, interrupted_self, raw).map_err(|e| {
                    // Handle thread panic and convert to an error
                    let error: Box<dyn Error + Send + Sync> = format!("{}|{}",e.kind(), e.to_string()).into();
                    error
                })
                //self.listen(callback, num_messages);
             })
            .expect("Failed to spawn thread");
        self.handle = Some(handle);
        self.delivery_mode = DeliveryMode::Callback;
    }

    pub fn join(& mut self) -> io::Result<()> {
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


    //Buffered mode: non-blocking, messages buffered ibn another thread
    pub fn start(&mut self, buffer_size:usize) -> IOResult<()> {
        if self.fifo.is_some(){
            return Err(IOError::new(ErrorKind::AlreadyExists, "Receiver already started"));
        }
        self.fifo = Some(Arc::new(FifoQueue::new(buffer_size)));

        fn callback(_: Message) -> () {}
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

    pub fn get(&self) -> Option<Message> {
        match &self.fifo{
            None => {None}
            Some(fifo) => {fifo.get()}
        }
    }

    pub fn wait(&self, timeout_ms: u64) -> IOResult<Message> {
        let timeout_duration = Duration::from_millis(timeout_ms);
        let start_time = Instant::now();
        while start_time.elapsed() < timeout_duration {
            if let Some(msg) = self.get() {
                return Ok(msg);
            }
            thread::sleep(Duration::from_millis(10));
        }
        Err(IOError::new(ErrorKind::TimedOut, "Timout waiting for message"))
    }

    pub fn wait_messages(&self, count:usize, timeout_ms: u64) -> IOResult<Vec<Message>> {
        let mut ret = Vec::new();
        for _ in 0..count {
            let msg = self.wait(timeout_ms)?;
            ret.push(msg);
        }
        Ok(ret)
    }

    pub fn fifo(&self) -> Option<Arc<FifoQueue<Message>>> {
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

    pub fn endpoints(&self) ->  & Option<Vec<String>> {
        &self.endpoints
    }

    pub fn connections(&self) -> usize {
        match &self.endpoints{
            None => {0}
            Some(e) => {e.len()}
        }
    }
    pub fn available(&self) -> u32 {
        if let Some(fifo) = &self.fifo {
            fifo.available_count() as u32
        } else {
            0
        }
    }

    pub fn dropped(&self) -> u32 {
        if let Some(fifo) = &self.fifo {
            fifo.dropped_count()
        } else {
            0
        }
    }

    pub fn message_count(&self) -> u32 {
        self.stats.lock().unwrap().counter_messages
    }

    pub fn error_count(&self) -> u32 {
        self.stats.lock().unwrap().counter_error
    }

    pub fn change_count(&self) -> u32 {
        self.stats.lock().unwrap().counter_header_changes
    }
    pub fn reset_counters(& mut self) {
        self.stats.lock().unwrap().reset()
    }

    pub fn set_header_buffer_size(&mut self, size:usize) {
        self.header_buffer = LimitedHashMap::new(size);
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



    pub fn enable_monitoring(& mut self)-> IOResult< crossbeam_channel::Receiver<EndpointEvent>> {
        if self.socket_monitor.is_none(){
            let  socket_monitor = SocketMonitor::new(self.tx.clone());
            match &mut self.sockets {
                ConnectionSockets::Shared { socket } => {
                    //socket.enable_monitoring(self.bsread.context())
                    socket.enable_monitoring(self.bsread.context(), &socket_monitor, None)?;

                }
                ConnectionSockets::Individual { sockets } => {
                    for (endpoint, socket) in sockets.iter_mut() {
                        //socket.enable_monitoring(self.bsread.context(),self.tx.clone(),Some(endpoint.clone()))?;
                        socket.enable_monitoring(self.bsread.context(),  &socket_monitor, Some(endpoint.clone()))?;
                    }
                }
            }
            self.socket_monitor =Some(socket_monitor);
        }
        Ok(self.rx.clone())
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

    pub fn socket(& mut self, endpoint: &str) -> Option<&mut TrackedSocket>{
        match &mut  self.sockets {
            ConnectionSockets::Shared { socket } => {
                Some(socket)
            }
            ConnectionSockets::Individual { sockets} => {
                sockets.get_mut(endpoint)
            }
        }
    }
    fn ref_socket(& self) -> Option <&TrackedSocket>{
        match &self.sockets {
            ConnectionSockets::Shared { socket } => {Some(socket)}
            ConnectionSockets::Individual {sockets} => {
                if (sockets.is_empty()){
                     return None;
                }
                sockets.values().next()
            }
        }
    }

    pub fn sockets(&mut self) -> Vec<&mut TrackedSocket> {
        match &mut self.sockets {
            ConnectionSockets::Shared { socket } => {
                vec![socket]
            }
            ConnectionSockets::Individual { sockets } => {
                sockets.values_mut().collect()
            }
        }
    }

}

impl SocketConfig for Receiver {
    fn transport(&self) -> Transport {
        if let  Some(socket)  = self.ref_socket() {
            if let  Some(transport)  = socket.transport() {
                return transport
            }
        }
        Transport::Tcp { port: 0, host: None}
    }
    fn socket_type(&self) -> SocketType {
        self.socket_type
    }
    fn sockets(&self) -> Vec<&zmq::Socket> {
        match &self.sockets {
            ConnectionSockets::Shared { socket } => {
                vec![socket.socket()]
            }
            ConnectionSockets::Individual { sockets } => {
                sockets
                    .values()
                    .map(|socket| socket.socket())
                    .collect()
            }
        }

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
        self.stop_forwarder();
        if let Some(socket_monitor) = &self.socket_monitor {
            socket_monitor.shutdown();
            self.socket_monitor = None;
        }
    }
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