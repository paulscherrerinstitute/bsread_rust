use crate::*;
use crate::message::*;
use crate::utils::LimitedHashMap;
use std::{io, thread};
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;
use std::thread::JoinHandle;
use zmq::{Context, SocketType};
use std::collections::{VecDeque};
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



pub struct Receiver<'a> {
    socket: TrackedSocket,
    endpoints: Option<Vec<String>>,
    socket_type: SocketType,
    header_buffer: LimitedHashMap<String, DataHeaderInfo>,
    bsread: &'a Bsread,
    fifo: Option<Arc<FifoQueue>>,
    handle: Option<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>>,
    counter_messages: u32,
    counter_error: u32,
    counter_header_changes: u32
}

impl
<'a> Receiver<'a> {
    pub fn new(bsread: &'a Bsread, endpoint: Option<Vec<&str>>, socket_type: SocketType) -> IOResult<Self> {
        let socket = TrackedSocket::new(&bsread.get_context(), socket_type)?;
        let endpoints = endpoint.map(|vec| vec.into_iter().map(|s| s.to_string()).collect());
        Ok(Self { socket, endpoints, socket_type, header_buffer: LimitedHashMap::void(), bsread, fifo:None, handle:None,
            counter_messages:0, counter_error:0, counter_header_changes:0 })
    }

    pub fn connect(&mut self, endpoint: &str) -> IOResult<()> {
        self.socket.connect(endpoint)?;
        Ok(())
    }

    fn connect_all(&mut self) -> IOResult<()> {
        if let Some(endpoints) = self.endpoints.clone() { // Clone to avoid immutable borrow
            for endpoint in endpoints {
                //TODO: Should break if one of the endpoints fail?
                self.connect(&endpoint)?;
            }
        }
        Ok(())
    }

    //Asynchronous API
    pub fn receive(& mut self) -> IOResult<BsMessage> {
        let message_parts = self.socket.socket.recv_multipart(0)?;
        let message = parse_message(message_parts, &mut self.header_buffer, &mut self.counter_header_changes);
        message
    }

    pub fn listen(&mut self, callback: fn(msg: BsMessage) -> (), num_messages: Option<u32>) -> IOResult<()> {
        self.connect_all()?;
        if self.header_buffer.is_void(){
            self.set_header_buffer_size(self.connections());
        }

        loop {
            let message = self.receive();
            match message {
                Ok(msg) => {
                    match (&self.fifo) {
                        None => {callback(msg)}
                        Some(fifo) => {fifo.add(msg)}
                    };
                    self.counter_messages = self.counter_messages + 1;
                }
                Err(e) => {
                    //TODO: error callback?
                    println!("Socket Listen Error: {}", e);
                    self.counter_error = self.counter_error + 1;
                }
            }
            if num_messages.map_or(false, |m| self.counter_messages >= m) {
                break;
            }
            if self.bsread.is_interrupted() {
                break;
            }
        }
        Result::Ok(())
    }


    pub fn fork(&self, callback: fn(msg: BsMessage) -> (), num_messages: Option<u32>) -> JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
        fn listen_process(endpoint: Option<Vec<&str>>, socket_type: SocketType, callback: fn(msg: BsMessage) -> (), num_messages: Option<u32>,  producer_fifo: Option<Arc<FifoQueue>> , interrupted: Arc<AtomicBool>) -> IOResult<()> {
            let bsread = crate::Bsread::new_forked(interrupted).unwrap();
            let mut receiver = bsread.receiver(endpoint, socket_type)?;
            receiver.fifo = producer_fifo;
            receiver.listen(callback, num_messages)
        }
        let endpoints: Option<Vec<String>> = self.endpoints.as_ref().map(|vec| vec.clone());
        let socket_type = self.socket_type.clone();
        let interrupted = Arc::clone(self.bsread.get_interrupted());
        let producer_fifo = match &self.fifo {
            None => { None }
            Some(f) => { Some(f.clone()) }
        };
        let handle = thread::spawn(move || -> Result<(), Box<dyn std::error::Error + Send + Sync>>{
            let endpoints_as_str: Option<Vec<&str>> = endpoints.as_ref().map(|vec| vec.iter().map(String::as_str).collect());
            listen_process(endpoints_as_str, socket_type, callback, num_messages, producer_fifo, interrupted).map_err(|e| {
                // Handle thread panic and convert to an error
                let error: Box<dyn Error + Send + Sync> = format!("{}|{}",e.kind(), e.to_string()).into();
                error
            })
            //self.listen(callback, num_messages);
        });
        handle
    }

    pub fn join(&self, handle: JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>) -> io::Result<()> {
        handle
            .join()
            .map_err(|e| {
                // Handle thread panic and convert to a std::io::Error
                let error_message = format!("Thread error: {:?}", e);
                new_error(ErrorKind::Other, error_message.as_str())
            })?
            .map_err(|e| {
                let desc = e.to_string();
                let parts:Vec<&str>  = desc.split('|').collect();
                println!("{:?}", parts);
                new_error(error_kind_from_str(parts[0]), parts[1])
            })?;

        Ok(())
    }


    //Synchronous API
    pub fn start(&mut self, buffer_size:usize) -> IOResult<()> {
        if self.fifo.is_some(){
            return Err(new_error(ErrorKind::AlreadyExists, "Receiver listener already started"));
        }
        self.fifo = Some(Arc::new(FifoQueue::new(buffer_size)));

        fn callback(message: BsMessage) -> () {
        }
        self.handle = Some(self.fork(callback, None));
        Ok(())
    }

    pub fn get(&self) -> Option<BsMessage> {
        match (&self.fifo){
            None => {None}
            Some(fifo) => {fifo.get()}
        }
    }

    pub fn wait(&self, timeout_ms: u64) -> IOResult<BsMessage> {
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

    pub fn connections(&self) -> usize {
        self.socket.connections.len()
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
        self.counter_messages
    }

    pub fn error_count(&self) -> u32 {
        self.counter_error
    }

    pub fn change_count(&self) -> u32 {
        self.counter_header_changes
    }
    pub fn reset_counters(& mut self) {
        self.counter_messages = 0;
        self.counter_error = 0;
        self.counter_header_changes = 0;
    }

    pub fn set_header_buffer_size(&mut self, size:usize) {
        self.header_buffer = LimitedHashMap::new(size);
    }
}

struct FifoQueue {
    queue: Mutex<VecDeque<BsMessage>>, // Thread-safe FIFO
    dropped_count: Mutex<u32>,        // Counter for dropped items
    max_size: usize,                  // Maximum size of the FIFO
}

impl FifoQueue {
    fn new(max_size: usize) -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            dropped_count: Mutex::new(0),
            max_size,
        }
    }

    /// Adds a message to the FIFO. Drops the oldest if the FIFO is full.
    fn add(&self, message: BsMessage) {
        let mut queue = self.queue.lock().unwrap();
        let mut dropped_count = self.dropped_count.lock().unwrap();

        if queue.len() >= self.max_size {
            queue.pop_front(); // Drop the oldest element
            *dropped_count += 1; // Increment the dropped counter
        }
        queue.push_back(message);
    }

    /// Retrieves the next message from the FIFO, or `None` if empty.
    fn get(&self) -> Option<BsMessage> {
        let mut queue = self.queue.lock().unwrap();
        queue.pop_front()
    }

    /// Retrieves the total count of dropped messages.
    fn get_dropped_count(&self) -> u32 {
        *self.dropped_count.lock().unwrap()
    }

    /// Retrieves the count of available messages.
    fn get_available_count(&self) -> usize {
        let mut queue = self.queue.lock().unwrap();
        queue.len()
    }
}