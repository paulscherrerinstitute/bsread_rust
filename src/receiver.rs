use super::message::{BsMessage, parse_message};
use std::{io, thread};
use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread::JoinHandle;
use zmq::{Context, SocketType};
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
            self.socket.set_subscribe(b"")?;
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
}

impl
<'a> Receiver<'a> {
    pub fn new(bsread: &'a Bsread, endpoint: Option<Vec<&str>>, socket_type: SocketType) -> Result<Self, Box<dyn std::error::Error>> {
        let socket = TrackedSocket::new(&bsread.context, socket_type)?;
        let endpoints = endpoint.map(|vec| vec.into_iter().map(|s| s.to_string()).collect());
        Ok(Self { socket, endpoints, socket_type, last: None, bsread })
    }


    pub fn connect(&mut self, endpoint: &str) -> io::Result<()> {
        self.socket.connect(endpoint)?;
        //self.socket.connect(&self.address)?;
        //self.socket.set_subscribe(b"")?;
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

    pub fn receive(&self, last: Option<BsMessage>) -> io::Result<BsMessage> {
        //let x: Option<BsMessage> = *self.last;
        let message_parts = self.socket.socket.recv_multipart(0)?;
        let message = parse_message(message_parts, last);
        message
    }

    pub fn listen(&mut self, callback: fn(msg: &BsMessage) -> (), num_messages: Option<u32>) -> Result<(), Box<dyn std::error::Error>> {
        self.connect_all()?;
        let mut count = 0;
        let mut last = None;
        loop {
            let message = self.receive(last);
            match &message {
                Ok(msg) => {
                    callback(&msg);
                    count = count + 1;
                }
                Err(_) => {
                    //TODO: error callback?
                }
            }
            if num_messages.map_or(false, |m| count >= m) {
                break;
            }
            if self.bsread.is_interrupted() {
                break;
            }
            last = message.ok();
        }
        Result::Ok(())
    }


    pub fn fork(&self, callback: fn(msg: &BsMessage) -> (), num_messages: Option<u32>) -> JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
        fn listen_process(endpoint: Option<Vec<&str>>, socket_type: SocketType, callback: fn(msg: &BsMessage) -> (), num_messages: Option<u32>, interrupted: Arc<AtomicBool>) -> Result<(), Box<dyn std::error::Error>> {
            let bsread = crate::Bsread::new_forked(interrupted).unwrap();
            let mut receiver = bsread.receiver(endpoint, socket_type)?;
            receiver.listen(callback, num_messages)
        }
        let endpoints: Option<Vec<String>> = self.endpoints.as_ref().map(|vec| vec.clone());
        let socket_type = self.socket_type.clone();
        let interrupted = Arc::clone(&self.bsread.interrupted);
        let handle = thread::spawn(move || -> Result<(), Box<dyn std::error::Error + Send + Sync>>{
            let endpoints_as_str: Option<Vec<&str>> = endpoints.as_ref().map(|vec| vec.iter().map(String::as_str).collect());
            listen_process(endpoints_as_str, socket_type, callback, num_messages, interrupted).map_err(|e| {
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
}