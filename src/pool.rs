use std::collections::HashMap;
use std::ops::DerefMut;
use crate::*;
use crate::receiver::{ConnectionMode, Receiver};
use crate::bsread::Bsread;
use std::sync::{Arc, Mutex};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};
use zmq::SocketType;
use crate::sockets::{EndpointDiag, EndpointEvent, EndpointState, Heartbeat, KeepAlive, SocketConfig, SocketMonitor, TrackedSocket};

pub struct Pool {
    socket_type: SocketType,
    threads: usize,
    connected: bool,
    bsread: Arc<Bsread>,
    receivers: Vec<Receiver>,
    socket_monitor: Option<SocketMonitor>,
    tx:crossbeam_channel::Sender<EndpointEvent>,
    rx:crossbeam_channel::Receiver<EndpointEvent>,
}

impl
Pool {
    //Endpoints are automatically distributed to the threads
    pub fn new(bsread: Arc<Bsread>, endpoints: Vec<&str>, socket_type: SocketType, threads: usize, connection_mode: ConnectionMode) -> IOResult<Self> {
        if threads<=0{
            return Err(IOError::new(ErrorKind::InvalidInput, "Invalid number of threads"));
        }
        let mut receivers: Vec<Receiver> = (0..threads).map(|_id| Receiver::new(bsread.clone(), None, socket_type, connection_mode.clone()).unwrap()).collect();
        let mut index = 0;
        for endpoint in endpoints{
            receivers[index].add_endpoint(endpoint);
            index += 1;
            if index >= threads {
                index = 0;
            }
        }
        let (tx, rx) = crossbeam_channel::unbounded();
        Ok(Self { socket_type, threads, connected:false, bsread,  receivers, socket_monitor:None, tx,rx})
    }

    //Endpoints manually set grouped per thread
    pub fn new_grouped(bsread: Arc<Bsread>, endpoints: Vec<Vec<&str>>, socket_type: SocketType, connection_mode: ConnectionMode) -> IOResult<Self> {
        let threads = endpoints.len();
        if threads==0{
            return Err(IOError::new(ErrorKind::InvalidInput, "Invalid configuration"));
        }
        let mut receivers: Vec<Receiver> = (0..threads).map(|_id| Receiver::new(bsread.clone(), None, socket_type, connection_mode.clone()).unwrap()).collect();
        let mut index = 0;
        for group in endpoints {
            for endpoint  in group {
                receivers[index].add_endpoint(endpoint);
            }
            index += 1;
            if index >= threads {
                index = 0;
            }
        }
        let (tx, rx) = crossbeam_channel::unbounded();
        Ok(Self { socket_type, threads, connected: false, bsread,  receivers, socket_monitor:None, tx,rx})
    }

    pub fn connect(&mut self) -> IOResult<()> {
        if !self.connected {
            for receiver in & mut self.receivers {
                receiver.connect()?;
                if let Some(socket_monitor) = &self.socket_monitor {
                    receiver.enable_shared_monitoring(socket_monitor)?;
                }
            }
            self.connected = true;
        }
        Ok(())
    }

    pub fn disconnect(&mut self)  {
        if self.connected {
            self.connected = false;
            for receiver in &mut self.receivers {
                if let Some(socket_monitor) = &self.socket_monitor {
                    receiver.disable_shared_monitoring(socket_monitor);
                }
                receiver.disconnect();
            }
        }
    }

    pub fn add_endpoint(&mut self, endpoint: &str, index: Option<usize>) -> IOResult<()> {
        let index = match(index){
            None => {
                self.receivers
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, receiver)| receiver.connections())
                    .map(|(i, _)| i as i32)
                    .unwrap_or(-1) as usize
            }
            Some(index) => {index}
        };

        if self.has_endpoint(endpoint) {
            if let Some(receiver) = self.endpoint_receiver(endpoint){
                if self.receivers[index].index() == receiver.index() {
                    return Ok(())
                }
            }
            return Err(IOError::new(ErrorKind::InvalidInput, "endpoint already exists"));
        }
        if index >= self.receivers.len(){
            return Err(IOError::new(ErrorKind::Other, format!("Invalid receiver index: {}", index)));
        }

        self.receivers[index].add_endpoint(endpoint)?;
        let socket_monitor = &self.socket_monitor;
        if let Some(socket_monitor) = &socket_monitor {
            self.receivers[index].enable_shared_monitoring_socket(socket_monitor, endpoint);
        }
        Ok(())
    }

    pub fn remove_endpoint(&mut self, endpoint: &str) {
        let socket_monitor = self.socket_monitor.clone();
        if let Some(receiver) = self.endpoint_receiver_mut(endpoint) {
            if let Some(sm) = &socket_monitor {
                receiver.disable_shared_monitoring_socket(&sm, endpoint);
            }
            receiver.remove_endpoint(endpoint);
        }
    }

    pub fn has_endpoint(&self, endpoint: &str) -> bool {
       self.endpoint_receiver(endpoint).is_some()
    }


    pub fn set_raw(&mut self, raw:bool) {
        for receiver in & mut self.receivers{
            receiver.set_raw(raw);
        }
    }
    pub fn is_raw(&self) -> bool{
        self.receivers[0].is_raw()
    }

    pub fn receive(&mut self, index:usize) -> IOResult<ReceivedMessage> {
         self.receivers[index].receive()
    }

    //TODO: this is blocking in each receiver
    //Synchronous Mode: blocking, callback in same thread
    pub fn listen<F>(&mut self, callback: F, num_messages: Option<u32>) -> IOResult<()>
        where
        F: Fn(ReceivedMessage),
        {
        self.reset_counters();
        self.connect()?;

        loop {
            for index in 0..self.num_receivers(){
                if let Ok(rx) = self.receive(index) {
                    callback(rx);
                }
                if let Some(n) = num_messages {
                    if self.message_count() >= n {
                        return Ok(())
                    }
                }
                if self.is_stopped() {
                    return Err(IOError::new(ErrorKind::ConnectionAborted, "Pool stopped"));
                }
            }
        }
    }

    //Threaded Mode: non-blocking, callback in another thread
    pub fn fork<F>(&mut self, callback: F) -> IOResult<()>
    where
        F: Fn(ReceivedMessage) + Send + Sync + 'static,
    {
        let shared_callback = Arc::new(callback);
        for receiver in &mut self.receivers {
            let callback = Arc::clone(&shared_callback);
            receiver.fork(
                move |msg| {
                    callback(msg);
                },
                None,
            );
        }
        Ok(())
    }

    //Buffered mode: non-blocking, messages buffered ibn another thread
    pub fn start(&mut self, buffer_size:usize) -> IOResult<()>
    {
        for receiver in &mut self.receivers{
            receiver.start(buffer_size);
        }
        Ok(())
    }

    pub fn stop(&mut self) -> IOResult<()> {
        for receiver in &mut self.receivers{
            receiver.interrupt();
        }
        for receiver in &mut self.receivers{
            receiver.join()?;
        }
        Ok(())
    }

    pub fn is_stopped(&self) ->bool {
        self.receivers[0].is_interrupted()
    }

    // TODO: Get/wait are inefficient in Pool - based on polling.
    // Potentialy pool could set a common buffer on receivers, but then Receiver.wait fail.
    // To be accessed if buffered mode may me useful for buffered delivery mode.
    pub fn get(&self) -> Option<ReceivedMessage> {
        for receiver in & self.receivers {
            match receiver.get (){
                None => { }
                Some(fifo) => return Some(fifo)
            }
        }
        None
    }

    pub fn wait(&self, timeout_ms: u64) -> IOResult<ReceivedMessage> {
        let timeout_duration = Duration::from_millis(timeout_ms);
        let start_time = Instant::now();
        while start_time.elapsed() < timeout_duration {
            if let Some(msg) = self.get() {
                return Ok(msg);
            }
            thread::sleep(Duration::from_millis(10));
        }
        Err(IOError::new(ErrorKind::TimedOut, "Timeout waiting for message"))
    }

    pub fn wait_messages(&self, count:usize, timeout_ms: u64) -> IOResult<Vec<ReceivedMessage>> {
        let mut ret = Vec::new();
        for _ in 0..count {
            let msg = self.wait(timeout_ms)?;
            ret.push(msg);
        }
        Ok(ret)
    }

    #[cfg(feature = "async")]
    pub fn start_async<F, Fut>(&mut self, callback: F, concurrent:bool, handle: Option<tokio::runtime::Handle>) -> IOResult<()>
    where
        F: Fn(ReceivedMessage) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let shared_callback = Arc::new(Mutex::new(callback));
        let handle  =  match handle{
            None => {tokio::runtime::Handle::current()}
            Some(handle) => {handle}
        };
        for receiver in &mut self.receivers {
            let receiver_handle = handle.clone();
            let callback_clone = Arc::clone(&shared_callback);
            receiver.start_async( move |msg| {
                let callback = callback_clone.lock().unwrap();
                callback(msg)
            }, None, concurrent, Some(receiver_handle));
        }
        Ok(())
    }

    #[cfg(feature = "async")]
    pub async fn stop_async(&mut self) -> IOResult<()> {
        for receiver in &mut self.receivers{
            receiver.interrupt();
        }
        for receiver in &mut self.receivers{
            receiver.join_async().await?;
        }
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        for receiver in & self.receivers{
            if receiver.is_running(){
                return true;
            }
        }
        false
    }

    pub fn socket_type(&self) -> SocketType {
        self.socket_type
    }

    pub fn threads(&self) -> usize {
        self.threads
    }

    pub fn receivers(&self) -> &Vec<Receiver> {
        &self.receivers
    }

    pub fn num_receivers(&self) -> usize {
        self.receivers.len()
    }

    pub fn delivery_mode(&self) -> DeliveryMode {
        self.receivers[0].delivery_mode()
    }

    pub fn connection_mode(&self) -> ConnectionMode {
        self.receivers[0].connection_mode()
    }

    pub fn endpoints(&self) -> impl Iterator<Item = String> {
        self.receivers
            .iter()
            .flat_map(|r| r.endpoints())
    }

    pub fn connections(&self) -> usize {
        self.receivers
            .iter()
            .map(|r| r.connections())
            .sum()
    }

    pub fn available(&self) -> u32 {
        self.receivers
            .iter()
            .map(|r| r.available())
            .sum()
    }

    pub fn dropped(&self) -> u32 {
        self.receivers
            .iter()
            .map(|r| r.dropped())
            .sum()
    }

    pub fn message_count(&self) -> u32 {
        self.receivers
            .iter()
            .map(|r| r.message_count())
            .sum()
    }

    pub fn error_count(&self) -> u32 {
        self.receivers
            .iter()
            .map(|r| r.error_count())
            .sum()
    }

    pub fn reset_counters(& mut self){
        for receiver in &mut self.receivers {
            receiver.reset_counters();
        }
    }
    pub fn diagnostics(&self) -> HashMap<String, HashMap<EndpointDiag, u32>> {
        let mut diagnostics = HashMap::new();
        for receiver in &self.receivers {
            diagnostics.extend(receiver.diagnostics());
        }
        diagnostics
    }

    pub fn diagnostics_endpoints(&self) -> Vec<String> {
        self.receivers
            .iter()
            .flat_map(|receiver| receiver.diagnostics_endpoints())
            .collect()
    }

    pub fn endpoint_receiver(&self, endpoint: &str) -> Option<&Receiver> {
        self.receivers
            .iter()
            .find(|receiver| receiver.endpoints().iter().any(|e| e == endpoint))
    }

    pub fn endpoint_receiver_mut(&mut self, endpoint: &str) -> Option<&mut Receiver> {
        self.receivers
            .iter_mut()
            .find(|receiver| receiver.endpoints().iter().any(|e| e == endpoint))
    }


    pub fn endpoint_diagnostics(& self,  endpoint: &str) -> Option<HashMap<EndpointDiag, u32>> {
        self.endpoint_receiver(endpoint)
            .map_or(None, |receiver| receiver.endpoint_diagnostics(endpoint))
    }

    pub fn endpoint_diagnostic(& self,  endpoint: &str, diag:EndpointDiag) -> Option<u32> {
        self.endpoint_receiver(endpoint)
            .map_or(None, |receiver| receiver.endpoint_diagnostic(endpoint, diag))
    }

    pub fn header_changes(&self, endpoint: &str) -> u32 {
        self.endpoint_receiver(endpoint)
            .map_or(0, |receiver| receiver.header_changes(endpoint))
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
        for receiver in &mut self.receivers {
            receiver.enable_check(check);
        }
    }

    pub fn disable_check(& mut self, check:u64){
        for receiver in &mut self.receivers {
            receiver.disable_check(check);
        }
    }

    pub fn socket(& mut self, endpoint: &str) -> Option<&mut TrackedSocket>{
        self.endpoint_receiver_mut(endpoint)
            .map_or(None, |receiver| receiver.socket(endpoint))
    }

    pub fn sockets(&mut self) -> Vec<&mut TrackedSocket> {
        let mut sockets = Vec::new();
        for receiver in &mut self.receivers {
            sockets.extend(receiver.sockets());
        }
        sockets
    }

    pub fn enable_monitoring(& mut self)-> IOResult< crossbeam_channel::Receiver<EndpointEvent>> {
        if self.socket_monitor.is_none(){
            let  socket_monitor = SocketMonitor::new(self.tx.clone());
            for receiver in &mut self.receivers {
                receiver.enable_shared_monitoring(&socket_monitor);
            }
            self.socket_monitor =Some(socket_monitor);
        }
        Ok(self.rx.clone())
    }

}


impl SocketConfig for Pool {
    fn zmq_sockets(&self) -> Vec<&zmq::Socket> {
        let mut sockets = Vec::new();
        for receiver in &self.receivers {
            sockets.extend(receiver.zmq_sockets());
        }
        sockets
    }

    fn set_linger(&mut self, value: i32) -> IOResult<()> {
        for receiver in &mut self.receivers {
            receiver.set_linger(value)?;
        }
        Ok(())
    }

    fn set_rcvhwm(&mut self, value: i32)-> IOResult<()> {
        for receiver in &mut self.receivers {
            receiver.set_rcvhwm(value)?;
        }
        Ok(())
    }

    fn set_sndhwm(&mut self, value: i32)-> IOResult<()> {
        for receiver in &mut self.receivers {
            receiver.set_sndhwm(value)?;
        }
        Ok(())
    }

    fn set_keepalive(&mut self, idle: i32, intvl: i32, cnt: i32) -> IOResult<()> {
        for receiver in &mut self.receivers {
            receiver.set_keepalive(idle, intvl, cnt)?;
        }
        Ok(())
    }

    fn set_heartbeat(&mut self, ivl: i32, timeout: i32, ttl: i32) -> IOResult<()> {
        for receiver in &mut self.receivers {
            receiver.set_heartbeat(ivl, timeout, ttl)?;
        }
        Ok(())
    }
}


impl Drop for Pool {
    fn drop(&mut self) {
        self.stop();
        if let Some(socket_monitor) = &self.socket_monitor {
            socket_monitor.shutdown();
            self.socket_monitor = None;
        }
    }
}
