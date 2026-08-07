use crate::*;
use crate::message::*;
use crate::utils::*;
use crate::sockets::*;
use std::{io, thread};
use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use zmq::{Context, PollItem, SocketEvent, SocketType};
use std::time::{Duration, Instant};
use uuid::Uuid;
#[cfg(feature = "async")]
use tokio::runtime::Handle;


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

    fn reset(& mut self){
        self.counter_messages = 0;
        self.counter_error = 0;
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


pub struct Receiver {
    sockets: ConnectionSockets,
    endpoints: Option<Vec<String>>,
    socket_type: SocketType,
    header_buffer: LimitedHashMap<String, DataHeaderInfo>,
    id_buffer: HashMap<String, u64>,
    check_mask: u64,
    bsread: Arc<Bsread>,
    fifo: Option<Arc<FifoQueue<ReceivedMessage>>>,
    handle: Option<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>>,
    #[cfg(feature = "async")]
    async_handle: Option<tokio::task::JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>>,
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
    socket_options: SocketOptions
}


impl Receiver{
    pub fn new(bsread: Arc<Bsread>, endpoints: Option<Vec<&str>>, socket_type: SocketType, connection_mode: ConnectionMode) -> IOResult<Self> {
        let index =  index();
        let mut sockets:ConnectionSockets = match connection_mode{
            ConnectionMode::Shared => {
                ConnectionSockets::Shared {socket: TrackedSocket::new(&bsread.context(), socket_type, index)?}
            }
            ConnectionMode::Individual => {
                ConnectionSockets::Individual {sockets: HashMap::new(),  poll_endpoints: Vec::new(), poll_ready_list: VecDeque::new()}
            }
        };
        let endpoints = endpoints.map(|vec| vec.into_iter().map(|s| s.to_string()).collect());
        let stats = Arc::new(Mutex::new(Stats{counter_messages:0, counter_error:0, diagnostics:HashMap::new()}));
        let delivery_mode = DeliveryMode::Inline;
        let  interrupted = Arc::new(AtomicBool::new(false));
        let (tx, rx) = crossbeam_channel::unbounded();
        let check_mask = CHECK_ALL;
        let socket_options = SocketOptions::new();

        Ok(Self { sockets, endpoints, socket_type, header_buffer: LimitedHashMap::void(), id_buffer: HashMap::new(), check_mask,
            bsread, fifo:None, handle:None,
            stats, index,
            forwarder_config:None, forwarder:None,interrupted, delivery_mode , raw: false,connection_mode,
            socket_monitor:None, tx,rx, socket_options,
            #[cfg(feature = "async")]
            async_handle:None,
        })
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
        if self.header_buffer.is_void(){
            self.set_header_buffer_size(self.connections());
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
            ConnectionSockets::Individual { sockets, ..} => {
                match sockets.get(endpoint){
                    None => {
                        let mut socket = TrackedSocket::new(context, socket_type, index)?;
                        socket.connect(endpoint)?;
                        if let Some(socket_monitor) = &self.socket_monitor {
                            socket.enable_monitoring(self.bsread.context(), &socket_monitor, Some(endpoint.to_string()))?;
                        }
                        self.socket_options.set(socket.socket())?;
                        sockets.insert(endpoint.to_string(), socket);
                        self.sockets.update_poll_items();
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
            ConnectionSockets::Individual { sockets, .. } => {
                match sockets.get_mut(endpoint) {
                    None => {}
                    Some(socket) => {
                        socket.disconnect();
                        sockets.remove(endpoint);
                        self.sockets.update_poll_items();
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

    fn process(&mut self, endpoint: &Option<String>, message_parts:Vec<Vec<u8>>) -> IOResult<Message> {
        if let Some(sender) = self.forwarder.as_mut() {
            match sender.forward(&message_parts) {
                Ok(_) => (),
                Err(e) => log::warn!("Error forwarding message to {}: {}", sender.endpoint(), e),
            }
        }
        let message =parse_message(message_parts, &mut self.header_buffer, self.raw);
        match message {
            Ok(message) => {
                self.check_message(message, endpoint)
            },
            Err(e) => {
                if (e.kind() == DECOMPRESSION_ERROR){
                    self.send_diag(&endpoint, EndpointDiag::DecompressionError);
                } else {
                    self.send_diag(&endpoint, EndpointDiag::ParsingError);
                }
                return Err(e)
            }
        }
    }
    //self.send_diag(endpoint, EndpointDiag::NonPositiveId);

    fn check_message(&mut self, message:Message,  endpoint: &Option<String>) -> IOResult<(Message)> {
        let id = message.id();
        if self.check_mask & CHECK_ID_POSITIVE != 0 {
            if id <=0 {
                self.send_diag(&endpoint, EndpointDiag::NonPositiveId);
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
                    self.send_diag(&endpoint, EndpointDiag::OutOfRangeId);
                    return Err(IOError::new(ErrorKind::InvalidData, "Out of range ID", ));
                }
            }
        }

        if self.check_mask & CHECK_ID_MONOTONIC != 0 {
            if let Some(ep) = endpoint.clone() {
                if let Some(last_id) = self.id_buffer.get(&ep){
                    if *last_id > id{
                        self.send_diag(&endpoint, EndpointDiag::DecreasingId);
                        return Err(IOError::new(ErrorKind::InvalidData,"Decreasing ID"));
                    } else if *last_id == id{
                        self.send_diag(&endpoint, EndpointDiag::RepeatedId);
                        return Err(IOError::new(ErrorKind::InvalidData,"Repeated ID"));
                    }
                }
                self.id_buffer.insert(ep, id);
            }
        }
        if message.header_changed() {
            self.send_diag(&endpoint, EndpointDiag::HeaderChange);
        }
        Ok(message)
    }


    fn send_diag(&mut self, endpoint: &Option<String>, diag:EndpointDiag){
        self.increse_stats(endpoint, diag);
        if self.socket_monitor.is_some() {
            if let Some(ep) = endpoint {
                self.tx.send(EndpointEvent::Diagnostic(ep.clone(), diag));
            }
        }
    }


    fn _receive(&mut self) -> (Option<String>, IOResult<Vec<Vec<u8>>>) {
        match &mut self.sockets {
            ConnectionSockets::Shared { socket } => {
                (None, socket.receive())
            }
            ConnectionSockets::Individual { sockets, poll_endpoints, poll_ready_list }  => {
                if poll_ready_list.is_empty(){
                    let mut poll_items = Vec::with_capacity(poll_endpoints.len());
                    for endpoint in poll_endpoints.iter() {
                        if let Some(socket) = sockets.get(endpoint) {
                            poll_items.push(socket.socket().as_poll_item(zmq::POLLIN));
                        } else {
                            log::error!("Poll endpoint not found in sockets, updating: {}", endpoint);
                            self.sockets.update_poll_items();
                            return (None,Err(IOError::new(ErrorKind::Other,"Poll endpoint not found")),);
                        }
                    }
                    if let Err(e) = zmq::poll(& mut poll_items, -1) {
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

                (None,Err(IOError::new(ErrorKind::Other,"No socket was readable")),)
            }
        }
    }

    pub fn receive(&mut self) -> IOResult<ReceivedMessage> {
        let (endpoint, message_parts) = self._receive();


        let message_parts = message_parts.map_err(|e| {
            //TODO: Should we count socket errors?
            //self.stats.lock().unwrap().increase_errors();
            self.send_diag(&endpoint, EndpointDiag::SocketError);
            e
        })?;

        let message = self.process(&endpoint, message_parts);
        match message {
            Ok(msg) => {
                self.stats.lock().unwrap().increase_messages();
                self.increse_stats(&endpoint,  EndpointDiag::Messages);
                Ok(ReceivedMessage{endpoint, message:msg})
            }
            Err(e) => {
                log::trace!("Receiver Error: {}", e);
                self.stats.lock().unwrap().increase_errors();
                self.increse_stats(&endpoint,  EndpointDiag::Errors);
                Err(IOError::new(e.kind(), e))
            }
        }

    }

    //Synchronous Mode: blocking, callback in same thread
    pub fn listen<F>(&mut self, callback: F, num_messages: Option<u32>) -> IOResult<()>
    where
        F: Fn(ReceivedMessage),
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
        loop {
            let message= self.receive();
            if let Ok(msg) = message {
                match &self.fifo {
                    None => {
                        callback(msg)
                    }
                    Some(fifo) => {
                        fifo.add(msg)
                    }
                }
            };
            if num_messages.map_or(false, |m| self.message_count() >= m) {
                break;
            }
            if self.is_interrupted() {
                break;
            }
        }
        self.stop_forwarder();
        Ok(())
    }

    //Threaded Mode: non-blocking, callback in another thread
    pub fn fork<F>(&mut self, callback: F, num_messages: Option<u32>)
    where
        F: Fn(ReceivedMessage) + Send + 'static,
    {
        let endpoints = self.endpoints.clone();
        let socket_type = self.socket_type.clone();
        let connection_mode = self.connection_mode.clone();
        let interrupted_context = Arc::clone(self.bsread.interrupted());
        let interrupted_self = Arc::clone(&self.interrupted);
        let forwarder_config = self.forwarder_config.clone();
        let producer_fifo = self.fifo.clone();
        let producer_stats = Arc::clone(&self.stats);
        let raw = self.raw;
        let thread_name = self.to_string();
        let socket_monitor = self.socket_monitor.take();
        let tx = self.tx.clone();

        let handle = thread::Builder::new()
            .name(thread_name)
            .spawn(move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                listen_task(endpoints, socket_type, connection_mode, callback, num_messages, producer_fifo, producer_stats,
                            forwarder_config, interrupted_context, interrupted_self, raw, socket_monitor, tx)
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
    pub fn start_async<F, Fut>(&mut self, callback: F, num_messages: Option<u32>, concurrent:bool, handle: Option<tokio::runtime::Handle>)
    where
        F: Fn(ReceivedMessage) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.reset_counters();
        let endpoints: Option<Vec<String>> = self.endpoints.as_ref().map(|vec| vec.clone());
        let socket_type = self.socket_type.clone();
        let connection_mode = self.connection_mode.clone();
        let interrupted_context = Arc::clone(self.bsread.interrupted());
        let interrupted_self = Arc::clone(&self.interrupted);
        let forwarder_config = self.forwarder_config.clone();
        let producer_fifo =None;
        let producer_stats =self.stats.clone();
        let raw = self.raw;
        let socket_monitor = self.socket_monitor.take();
        let tx = self.tx.clone();

        let handle  =  match handle{
            None => {tokio::runtime::Handle::current()}
            Some(handle) => {handle}
        };
        let callback_handle = handle.clone();

        let join_handle = if concurrent {
             handle.spawn_blocking(move || {
                let cb = move |msg: ReceivedMessage| {
                    let callback = callback(msg);
                    callback_handle.spawn(callback);
                };

                listen_task(endpoints, socket_type, connection_mode, cb,
                            num_messages, producer_fifo, producer_stats,
                            forwarder_config, interrupted_context, interrupted_self, raw, socket_monitor, tx)
            })
        } else {
                //let shared_callback = Arc::new(Mutex::new(callback));
                handle.spawn_blocking(move || {
                    let senders:Arc<Mutex<HashMap<String, tokio::sync::mpsc::Sender<ReceivedMessage>>>>
                        = Arc::new(Mutex::new(HashMap::new()));
                    let senders_cb = senders.clone();
                    let runtime = callback_handle.clone();
                    let callback = Arc::new(callback);
                    let cb = move |msg: ReceivedMessage| {
                        let endpoint = msg.endpoint.clone().unwrap_or_default();
                        let sender = {
                            let mut senders = senders_cb.lock().unwrap();
                            senders.entry(endpoint).or_insert_with(|| {
                                    let (tx, mut rx) =
                                        tokio::sync::mpsc::channel::<ReceivedMessage>(1000);
                                    let callback = callback.clone();
                                    runtime.spawn(async move {
                                        while let Some(msg) = rx.recv().await {
                                            callback(msg).await;
                                        }
                                    });
                                    tx
                                })
                                .clone()
                        };
                        // ZMQ receiver thread is blocking, so use blocking_send
                        sender.blocking_send(msg).unwrap();
                    };
                    listen_task(endpoints, socket_type, connection_mode, cb,
                                num_messages, producer_fifo, producer_stats,
                                forwarder_config, interrupted_context, interrupted_self, raw, socket_monitor, tx)
            })
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
        self.reset_counters();

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
                Err(IOError::new(ErrorKind::Other, "Operation only valid for buffered delivery mode"))
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

    fn increse_stats(& mut self, endpoint: &Option<String>, diag:EndpointDiag){
        let ep: &str = endpoint.as_deref().unwrap_or("");
        //*self.stats.lock().unwrap().diagnostics.entry(ep.clone()).or_insert( HashMap::new()).entry(diag).or_insert(0) += 1;
        //Only clone endpoint if entry is absent
        let mut stats = self.stats.lock().unwrap();
        let map = if let Some(map) = stats.diagnostics.get_mut(ep) {
            map
        } else {
            stats.diagnostics.entry(ep.to_string()).or_insert_with(HashMap::new)
        };
        *map.entry(diag).or_insert(0) += 1;
    }

    pub fn diagnostics(&self) -> HashMap<String, HashMap<EndpointDiag, u32>>{
        self.stats.lock().unwrap().diagnostics.clone()
    }
    pub fn diagnostics_endpoints(&self) -> Vec<String> {
        self.stats.lock().unwrap().diagnostics.keys().cloned().collect()
    }

    pub fn endpoint_diagnostics(& self,  endpoint: &str) -> Option<HashMap<EndpointDiag, u32>> {
        self.stats.lock().unwrap().diagnostics.get(endpoint).cloned()
    }

    pub fn endpoint_diagnostic(& self,  endpoint: &str, diag:EndpointDiag) -> Option<u32> {
        self.stats.lock().unwrap().diagnostics.get(endpoint)?.get(&diag).copied()
    }

    pub fn header_changes(& self,  endpoint:  &str) -> u32 {
        self.endpoint_diagnostic(endpoint, EndpointDiag::HeaderChange).unwrap_or(
            if let Some (x) =  self.endpoint_diagnostic(endpoint, EndpointDiag::Messages) {
                1
            } else {
                0
            }
        )
    }

    pub fn message_count(&self) -> u32 {
        self.stats.lock().unwrap().counter_messages
    }

    pub fn error_count(&self) -> u32 {
        self.stats.lock().unwrap().counter_error
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
                ConnectionSockets::Individual { sockets, ..} => {
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
            ConnectionSockets::Individual { sockets, ..} => {
                sockets.get_mut(endpoint)
            }
        }
    }
    fn ref_socket(& self) -> Option <&TrackedSocket>{
        match &self.sockets {
            ConnectionSockets::Shared { socket } => {Some(socket)}
            ConnectionSockets::Individual {sockets, ..} => {
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
            ConnectionSockets::Individual { sockets, .. } => {
                sockets.values_mut().collect()
            }
        }
    }
    pub fn socket_type(&self) -> SocketType {
        self.socket_type
    }

    pub fn transport(&self) -> Option<Transport> {
        match (self.ref_socket()){
            None => {
                None
            }
            Some(socket) => {
                socket.transport()
            }
        }
    }
}

fn listen_task<F>(
    endpoints: Option<Vec<String>>,
    socket_type: SocketType,
    connection_mode: ConnectionMode,
    callback: F,
    num_messages: Option<u32>,
    producer_fifo: Option<Arc<FifoQueue<ReceivedMessage>>>,
    producer_stats: Arc<Mutex<Stats>>,
    forwarder_config: Option<ForwarderConfig>,
    interrupted_context: Arc<AtomicBool>,
    interrupted_self: Arc<AtomicBool>,
    raw: bool,
    socket_monitor: Option<SocketMonitor>,
    tx: crossbeam_channel::Sender<EndpointEvent>,
) -> Result<(), Box<dyn Error + Send + Sync>>
where
    F: Fn(ReceivedMessage) + Send + 'static,
{
    let endpoints = endpoints
        .as_ref()
        .map(|v| v.iter().map(String::as_str).collect::<Vec<_>>());

    let bsread = crate::Bsread::new_with_interrupted(interrupted_context).unwrap();
    let mut receiver = bsread.receiver(endpoints, socket_type, connection_mode)?;
    receiver.fifo = producer_fifo;
    receiver.stats = producer_stats;
    receiver.interrupted = interrupted_self;
    receiver.forwarder_config = forwarder_config;
    receiver.raw = raw;
    receiver.socket_monitor = socket_monitor;
    receiver.tx = tx;
    receiver
        .listen(callback, num_messages)
        .map_err(|e| format!("{}|{}", e.kind(), e).into())
}

impl SocketConfig for Receiver {
    fn sockets(&self) -> Vec<&zmq::Socket> {
        match &self.sockets {
            ConnectionSockets::Shared { socket } => {
                vec![socket.socket()]
            }
            ConnectionSockets::Individual { sockets, .. } => {
                sockets
                    .values()
                    .map(|socket| socket.socket())
                    .collect()
            }
        }
    }

    fn set_linger(&mut self, value: i32) -> IOResult<()> {
        self.socket_options.linger = Some(value);
        self.set_options(&self.socket_options)?;
        Ok(())
    }

    fn set_rcvhwm(&mut self, value: i32)-> IOResult<()> {
        self.socket_options.rcvhwm = Some(value);
        self.set_options(&self.socket_options)?;
        Ok(())
    }

    fn set_sndhwm(&mut self, value: i32)-> IOResult<()> {
        self.socket_options.sndhwm = Some(value);
        self.set_options(&self.socket_options)?;
        Ok(())
    }

    fn set_keepalive(&mut self, idle: i32, intvl: i32, cnt: i32) -> IOResult<()> {
        self.socket_options.keepalive = Some(KeepAlive { idle, intvl, cnt});
        self.set_options(&self.socket_options)?;
        Ok(())
    }

    fn set_heartbeat(&mut self, ivl: i32, timeout: i32, ttl: i32) -> IOResult<()> {
        self.socket_options.heartbeat = Some(Heartbeat { ivl, timeout, ttl});
        self.set_options(&self.socket_options)?;
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
        self.stop_forwarder();
        self.sockets.clear();
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