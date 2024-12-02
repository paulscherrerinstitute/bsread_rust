use super::message::{BsMessage, parse_message};
use std::{io, thread};
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;
use std::thread::JoinHandle;
use zmq::{Context, SocketType};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use serde_json::Value;
use crate::Bsread;

struct TrackedSocket {
    socket: zmq::Socket,
    connections: Vec<String>,
}

impl TrackedSocket {
    fn new(context: &Context, socket_type: zmq::SocketType) -> Result<TrackedSocket, Box<dyn std::error::Error>> {
        let socket = context.socket(socket_type)?;
        Ok(Self {
            socket,
            connections: Vec::new(),
        })
    }

    fn connect(&mut self, endpoint: &str) -> zmq::Result<()> {
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
    last: Option<BsMessage>,
    bsread: &'a Bsread,
    fifo: Option<Arc<FifoQueue>>,
    handle: Option<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>>
}


impl
<'a> Receiver<'a> {
    pub fn new(bsread: &'a Bsread, endpoint: Option<Vec<&str>>, socket_type: SocketType) -> Result<Self, Box<dyn std::error::Error>> {
        let socket = TrackedSocket::new(&bsread.context, socket_type)?;
        let endpoints = endpoint.map(|vec| vec.into_iter().map(|s| s.to_string()).collect());
        Ok(Self { socket, endpoints, socket_type, last: None, bsread, fifo:None, handle:None })
    }

    pub fn connect(&mut self, endpoint: &str) -> io::Result<()> {
        self.socket.connect(endpoint)?;
        Ok(())
    }

    fn connect_all(&mut self) -> io::Result<()> {
        if let Some(endpoints) = self.endpoints.clone() { // Clone to avoid immutable borrow
            for endpoint in endpoints {
                //TODO: Should break if one of the endpoints fail?
                self.connect(&endpoint)?;
            }
        }
        Ok(())
    }

    //Asynchronous API
    pub fn receive(&self, last: Option<BsMessage>) -> io::Result<BsMessage> {
        //let x: Option<BsMessage> = *self.last;
        let message_parts = self.socket.socket.recv_multipart(0)?;
        let message = parse_message(message_parts, last);
        message
    }

    pub fn listen(&mut self, callback: fn(msg: BsMessage) -> (), num_messages: Option<u32>) -> Result<(), Box<dyn std::error::Error>> {
        self.connect_all()?;
        let mut count = 0;
        let mut last: Option<BsMessage> = None;
        loop {
            let message = self.receive(last);
            match message {
                Ok(msg) => {
                    last = msg.clone_data_header_info();
                    match (&self.fifo) {
                        None => {callback(msg)}
                        Some(fifo) => {fifo.add(msg)}
                    };
                    count = count + 1;
                }
                Err(e) => {
                    //TODO: error callback?
                    println!("Socket Listen Error: {}", e);
                    last = None
                }
            }
            if num_messages.map_or(false, |m| count >= m) {
                break;
            }
            if self.bsread.is_interrupted() {
                break;
            }
        }
        Result::Ok(())
    }


    pub fn fork(&self, callback: fn(msg: BsMessage) -> (), num_messages: Option<u32>) -> JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
        fn listen_process(endpoint: Option<Vec<&str>>, socket_type: SocketType, callback: fn(msg: BsMessage) -> (), num_messages: Option<u32>,  producer_fifo: Option<Arc<FifoQueue>> , interrupted: Arc<AtomicBool>) -> Result<(), Box<dyn std::error::Error>> {
            let bsread = crate::Bsread::new_forked(interrupted).unwrap();
            let mut receiver = bsread.receiver(endpoint, socket_type)?;
            receiver.fifo = producer_fifo;
            receiver.listen(callback, num_messages)
        }
        let endpoints: Option<Vec<String>> = self.endpoints.as_ref().map(|vec| vec.clone());
        let socket_type = self.socket_type.clone();
        let interrupted = Arc::clone(&self.bsread.interrupted);
        let producer_fifo = match &self.fifo {
            None => { None }
            Some(f) => { Some(f.clone()) }
        };
        let handle = thread::spawn(move || -> Result<(), Box<dyn std::error::Error + Send + Sync>>{
            let endpoints_as_str: Option<Vec<&str>> = endpoints.as_ref().map(|vec| vec.iter().map(String::as_str).collect());
            listen_process(endpoints_as_str, socket_type, callback, num_messages, producer_fifo, interrupted).map_err(|e| {
                // Handle thread panic and convert to an error
                let panic_error: Box<dyn Error + Send + Sync> = format!("Thread panicked: {:?}", e).into();
                panic_error
            })
            //self.listen(callback, num_messages);
        });
        handle
    }

    pub fn join(&self, handle: JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>) -> Result<(), Box<dyn std::error::Error>> {
        handle
            .join()
            .map_err(|e| {
                // Handle thread panic and convert to an error
                let panic_error: Box<dyn Error> = format!("Thread error: {:?}", e).into();
                panic_error
            })?
            .map_err(|e| e as Box<dyn Error>) // Convert Box<dyn Error + Send + 'static> to Box<dyn Error>
    }

    //Synchronous API


    pub fn start(&mut self, buffer_size:usize) -> io::Result<()> {
        if self.fifo.is_some(){
            return Err(io::Error::new(io::ErrorKind::Other, "Receiver listener already started"));
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

    pub fn wait(&self, timeout_ms: u64) -> io::Result<BsMessage> {
        let timeout_duration = Duration::from_millis(timeout_ms);
        let start_time = Instant::now();
        while start_time.elapsed() < timeout_duration {
            if let Some(msg) = self.get() {
                return Ok(msg);
            }
            thread::sleep(Duration::from_millis(10));
        }

        Err(io::Error::new(io::ErrorKind::Other, "Timout waiting for message"))
    }

    pub fn count(&self) -> usize {
        match (&self.fifo){
            None => {0}
            Some(fifo) => {fifo.get_available_count()}
        }
    }
}

struct FifoQueue {
    queue: Mutex<VecDeque<BsMessage>>, // Thread-safe FIFO
    dropped_count: Mutex<u64>,        // Counter for dropped items
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
    fn get_dropped_count(&self) -> u64 {
        *self.dropped_count.lock().unwrap()
    }

    /// Retrieves the count of available messages.
    fn get_available_count(&self) -> usize {
        let mut queue = self.queue.lock().unwrap();
        queue.len()
    }
}