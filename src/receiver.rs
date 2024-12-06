use crate::*;
use crate::message::*;
use crate::utils::*;
use std::{io, thread};
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;
use std::thread::JoinHandle;
use zmq::{Context, SocketType};
use std::time::{Duration, Instant};

struct TrackedSocket {
    socket: zmq::Socket,
    connections: Vec<String>,
}

impl TrackedSocket {
    fn new(context: &Context, socket_type: zmq::SocketType) -> IOResult<TrackedSocket> {
        let socket = context.socket(socket_type)?;
        Ok(Self {
            socket,
            connections: Vec::new(),
        })
    }

    fn connect(&mut self, endpoint: &str) -> IOResult<()> {
        if !self.has_connected_to(endpoint) {
            self.socket.connect(endpoint)?;
            if self.socket.get_socket_type().unwrap() == SocketType::SUB {
                self.socket.set_subscribe(b"")?;
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
}

static mut receiver_index: Mutex<u32> = Mutex::new(0);
fn get_index() -> u32{
    unsafe {
        let mut counter = receiver_index.lock().unwrap();
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

    fn reset(& mut self){
        self.counter_messages = 0;
        self.counter_error = 0;
        self.counter_header_changes = 0;
    }

}

pub struct Receiver<'a> {
    socket: TrackedSocket,
    endpoints: Option<Vec<String>>,
    socket_type: SocketType,
    header_buffer: LimitedHashMap<String, DataHeaderInfo>,
    bsread: &'a Bsread,
    fifo: Option<Arc<FifoQueue<Message>>>,
    handle: Option<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>>,
    stats: Arc<Mutex<Stats>>,
    index: u32
}

impl
<'a> Receiver<'a> {
    pub fn new(bsread: &'a Bsread, endpoint: Option<Vec<&str>>, socket_type: SocketType) -> IOResult<Self> {
        let socket = TrackedSocket::new(&bsread.get_context(), socket_type)?;
        let endpoints = endpoint.map(|vec| vec.into_iter().map(|s| s.to_string()).collect());
        let index =  get_index();
        let stats = Arc::new(Mutex::new(Stats{counter_messages:0, counter_error:0, counter_header_changes:0}));
        Ok(Self { socket, endpoints, socket_type, header_buffer: LimitedHashMap::void(), bsread, fifo:None, handle:None, stats, index })
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

    //Asynchronous API
    pub fn receive(& mut self) -> IOResult<Message> {
        let message_parts = self.socket.socket.recv_multipart(0)?;
        let message = parse_message(message_parts, &mut self.header_buffer, &mut self.stats.lock().unwrap().counter_header_changes);
        message
    }

    pub fn listen(&mut self, callback: fn(msg: Message) -> (), num_messages: Option<u32>) -> IOResult<()> {
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
                    //println!("Socket Listen Error: {}", e);
                    self.stats.lock().unwrap().increase_errors();
                }
            }
            if num_messages.map_or(false, |m| self.stats.lock().unwrap().counter_messages >= m) {
                break;
            }
            if self.bsread.is_interrupted() {
                break;
            }
        }
        Result::Ok(())
    }

    pub fn fork(& mut self, callback: fn(msg: Message) -> (), num_messages: Option<u32>) {
        fn listen_process(endpoint: Option<Vec<&str>>, socket_type: SocketType, callback: fn(msg: Message) -> (), num_messages: Option<u32>,
                          producer_fifo: Option<Arc<FifoQueue<Message>>>, producer_stats:Arc<Mutex<Stats>>,interrupted: Arc<AtomicBool>) -> IOResult<()> {
            let bsread = crate::Bsread::new_forked(interrupted).unwrap();
            let mut receiver = bsread.receiver(endpoint, socket_type)?;
            receiver.fifo = producer_fifo;
            receiver.stats = producer_stats;
            receiver.listen(callback, num_messages)
        }
        let endpoints: Option<Vec<String>> = self.endpoints.as_ref().map(|vec| vec.clone());
        let socket_type = self.socket_type.clone();
        let interrupted = Arc::clone(self.bsread.get_interrupted());

        let producer_fifo = match &self.fifo {
            None => { None }
            Some(f) => { Some(f.clone()) }
        };
        //let producer_stats = Arc::clone(&self.stats);
        //let producer_stats = Arc::new(Mutex::new(&self.stats));
        let producer_stats =self.stats.clone();

        let thread_name = self.to_string();
        let handle = thread::Builder::new()
            .name(thread_name.to_string())
            .spawn(move || -> Result<(), Box<dyn std::error::Error + Send + Sync>>{
                let endpoints_as_str: Option<Vec<&str>> = endpoints.as_ref().map(|vec| vec.iter().map(String::as_str).collect());
                listen_process(endpoints_as_str, socket_type, callback, num_messages, producer_fifo, producer_stats, interrupted).map_err(|e| {
                    // Handle thread panic and convert to an error
                    let error: Box<dyn Error + Send + Sync> = format!("{}|{}",e.kind(), e.to_string()).into();
                    error
                })
                //self.listen(callback, num_messages);
             })
            .expect("Failed to spawn thread");
        self.handle = Some(handle);
    }

    pub fn join(& mut self) -> io::Result<()> {
        if let Some(handle) = self.handle.take() { // Take ownership of the handle
            self.handle = None;
            handle
                .join()
                .map_err(|e| {
                    // Handle thread panic and convert to a std::io::Error
                    let error_message = format!("Thread error: {:?}", e);
                    new_error(ErrorKind::Other, error_message.as_str())
                })?
                .map_err(|e| {
                    let desc = e.to_string();
                    let parts: Vec<&str> = desc.split('|').collect();
                    println!("{:?}", parts);
                    new_error(error_kind_from_str(parts[0]), parts[1])
                })?;
        }
        Ok(())
    }





    //Synchronous API
    pub fn start(&mut self, buffer_size:usize) -> IOResult<()> {
        if self.fifo.is_some(){
            return Err(new_error(ErrorKind::AlreadyExists, "Receiver listener already started"));
        }
        self.fifo = Some(Arc::new(FifoQueue::new(buffer_size)));

        fn callback(_: Message) -> () {}
        self.fork(callback, None);
        Ok(())
    }

    pub fn interrupt(&mut self)  {
        self.bsread.interrupt();
    }

    pub fn stop(&mut self) -> IOResult<()> {
        self.bsread.interrupt();
        self.join();
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

    pub fn get_fifo(&self) -> Option<Arc<FifoQueue<Message>>> {
        match &self.fifo{
            None => {None}
            Some(fifo) => {Some(fifo.clone())}
        }
    }
    pub fn index(&self) -> u32 {
        self.index
    }

    pub fn get_mode(&self) -> &str {
        if self.fifo.is_some(){
            return "sync"
        }
        "async"
    }
    pub fn get_socket_type(&self) -> SocketType {
        self.socket_type
    }

    pub fn get_endpoints(&self) ->  & Option<Vec<String>> {
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
            fifo.get_available_count() as u32
        } else {
            0
        }
    }

    pub fn dropped(&self) -> u32 {
        if let Some(fifo) = &self.fifo {
            fifo.get_dropped_count()
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
