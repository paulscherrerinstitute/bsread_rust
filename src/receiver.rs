use crate::*;
use crate::message::*;
use crate::utils::*;
use crate::sockets::{SocketConfig, Transport};
use std::{io, thread};
use std::collections::HashMap;
use std::error::Error;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use zmq::{Context, SocketType};
use std::time::{Duration, Instant};

struct TrackedSocket {
    socket: zmq::Socket,
    connections: Vec<String>,
    topics: Vec<String>,
}

impl TrackedSocket {
    fn new(context: &Context, socket_type: zmq::SocketType) -> IOResult<TrackedSocket> {
        let socket = context.socket(socket_type)?;
        Ok(Self {
            socket,
            connections: Vec::new(),
            topics: Vec::new(),
        })
    }

    fn add_topic(&mut self, topic: String) {
        self.topics.push(topic);
    }

    fn subscribe(&mut self, topic:&str, endpoint: &str) -> IOResult<()> {
        if let Err(e) =  self.socket.set_subscribe(topic.as_bytes()) {
            log::error!("Error subscribing topic {} in endpoint {}: {}", topic, endpoint, e);
            return Err(e.into());
        }
        Ok(())
    }

    fn connect(&mut self, endpoint: &str) -> IOResult<()> {
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

    fn has_connected_to(&self, endpoint: &str) -> bool {
        self.connections.contains(&endpoint.to_string())
    }

    fn has_any_connection(&self) -> bool {
        !self.connections.is_empty()
    }

    fn disconnect(&mut self, endpoint: &str){
        if self.has_connected_to(endpoint) {
            //self.connections.retain(|x| x != endpoint);
            log::info!("Disonnecting endpoint {}", endpoint);
            if let Err(e) =  self.socket.disconnect(endpoint) {
                log::error!("Error disonnecting endpoint {}: {}", endpoint, e);
            }
        }
    }

    fn disconnect_all(&mut self) {
        for endpoint in self.connections.clone(){
            self.disconnect(endpoint.as_str());
        }
    }
}

impl Drop for TrackedSocket {
    fn drop(&mut self) {
        if self.has_any_connection(){
            self.disconnect_all();
        }
    }
}

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

pub struct Receiver {
    socket: TrackedSocket,
    endpoints: Option<Vec<String>>,
    socket_type: SocketType,
    header_buffer: LimitedHashMap<String, DataHeaderInfo>,
    bsread: Arc<Bsread>,
    fifo: Option<Arc<FifoQueue<Message>>>,
    handle: Option<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>>,
    stats: Arc<Mutex<Stats>>,
    index: u32,
    forwarder_config: Option<ForwarderConfig>,
    forwarder: Option<Sender>,
    interrupted: Arc<AtomicBool>,
    mode: String,
    raw: bool
}

impl
Receiver{
    pub fn new(bsread: Arc<Bsread>, endpoint: Option<Vec<&str>>, socket_type: SocketType) -> IOResult<Self> {
        let interrupted = Arc::new(AtomicBool::new(false));
        Receiver::new_with_interrupted(bsread, endpoint, socket_type, interrupted)
    }

    pub fn new_with_interrupted(bsread: Arc<Bsread>, endpoint: Option<Vec<&str>>, socket_type: SocketType, interrupted: Arc<AtomicBool>) -> IOResult<Self> {
        let socket = TrackedSocket::new(&bsread.context(), socket_type)?;
        let endpoints = endpoint.map(|vec| vec.into_iter().map(|s| s.to_string()).collect());
        let index =  index();
        let stats = Arc::new(Mutex::new(Stats{counter_messages:0, counter_error:0, counter_header_changes:0}));
        let mode = "sync".to_string();
        Ok(Self { socket, endpoints, socket_type, header_buffer: LimitedHashMap::void(), bsread, fifo:None, handle:None, stats, index,
                  forwarder_config:None, forwarder:None,interrupted, mode , raw: false})
    }

    pub fn to_string(& self,) -> String {
        format!("Receiver {}" , self.index)
    }

    pub fn connect(&mut self, endpoint: &str) -> IOResult<()> {
        self.socket.connect(endpoint)?;
        Ok(())
    }

    pub fn connect_all(&mut self) -> IOResult<()> {
        if let Some(endpoints) = self.endpoints.clone() { // Clone to avoid immutable borrow
            for endpoint in endpoints {
                //TODO: Should break if one of the endpoints fail?
                self.connect(&endpoint)?;
            }
        }
        Ok(())
    }

    pub fn disconnect(&mut self, endpoint: &str)  {
        self.socket.disconnect(endpoint);
    }

    pub fn disconnect_all(&mut self)  {
        self.socket.disconnect_all();
    }

    pub fn add_endpoint(&mut self, endpoint: String) {
        match &mut self.endpoints {
            Some(vec) => {
                vec.push(endpoint);
            }
            None => {
                self.endpoints = Some(vec![endpoint]);
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

    pub fn receive(&mut self) -> IOResult<Message> {
        let message_parts = self.socket.socket.recv_multipart(0)?;
        if let Some(sender) = self.forwarder.as_mut() {
            match sender.forward(&message_parts) {
                Ok(_) => (),
                Err(e) => log::warn!("Error forwarding message to {}: {}", sender.endpoint(), e),
            }
        }
        let message = parse_message(message_parts, &mut self.header_buffer, &mut self.stats.lock().unwrap().counter_header_changes, self.raw);
        message
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
        self.connect_all()?;
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
                    log::error!("Socket Listen Error: {}", e);
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

        fn listen_process<F>(endpoint: Option<Vec<&str>>, socket_type: SocketType, callback: Arc<Mutex<F>>, num_messages: Option<u32>,
                             producer_fifo: Option<Arc<FifoQueue<Message>>>, producer_stats:Arc<Mutex<Stats>>, forwarder_config:Option<ForwarderConfig>,
                             interrupted_context: Arc<AtomicBool>, interrupted_self: Arc<AtomicBool>, raw: bool) -> IOResult<()>
            where
                F: FnMut(Message) + Send + 'static,
            {
            let bsread = crate::Bsread::new_with_interrupted(interrupted_context).unwrap();
            let mut receiver = bsread.receiver(endpoint, socket_type)?;
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
                listen_process(endpoints_as_str, socket_type, shared_callback, num_messages, producer_fifo, producer_stats, forwarder_config, interrupted_context, interrupted_self, raw).map_err(|e| {
                    // Handle thread panic and convert to an error
                    let error: Box<dyn Error + Send + Sync> = format!("{}|{}",e.kind(), e.to_string()).into();
                    error
                })
                //self.listen(callback, num_messages);
             })
            .expect("Failed to spawn thread");
        self.handle = Some(handle);
        self.mode = "async".to_string();
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
                    new_error(ErrorKind::Other, error_message.as_str())
                })?
                .map_err(|e| {
                    let desc = e.to_string();
                    let parts: Vec<&str> = desc.split('|').collect();
                    log::error!("Listener thread join error: {:?}", parts);
                    new_error(error_kind_from_str(parts[0]), parts[1])
                })?;
        }
        Ok(())
    }


    //Buffered mode: non-blocking, messages buffered ibn another thread
    pub fn start(&mut self, buffer_size:usize) -> IOResult<()> {
        if self.fifo.is_some(){
            return Err(new_error(ErrorKind::AlreadyExists, "Receiver already started"));
        }
        self.fifo = Some(Arc::new(FifoQueue::new(buffer_size)));

        fn callback(_: Message) -> () {}
        self.fork(callback, None);
        self.mode = "buffered".to_string();
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

        Err(new_error(ErrorKind::TimedOut, "Timout waiting for message"))
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

    pub fn mode(&self) -> &str {
        self.mode.as_str()
    }

    pub fn endpoints(&self) ->  & Option<Vec<String>> {
        &self.endpoints
    }

    pub fn connections(&self) -> usize {
        match &self.endpoints{
            None => {self.socket.connections.len()}
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
}

impl SocketConfig for Receiver {
    fn transport(&self) -> Transport {
        if self.socket.has_any_connection() {
            Transport::from_endpoint(self.socket.connections[0].as_str()).unwrap()
        } else {
            Transport::Tcp { port: 0, host: None }
        }
    }
    fn socket_type(&self) -> SocketType {
        self.socket_type
    }
    fn socket(&self) -> &zmq::Socket {
        &self.socket.socket
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