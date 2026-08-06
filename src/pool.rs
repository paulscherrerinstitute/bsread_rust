use std::ops::DerefMut;
use crate::*;
use crate::receiver::{ConnectionMode, Receiver};
use crate::bsread::Bsread;
use std::sync::{Arc, Mutex};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use zmq::SocketType;

pub struct Pool {
    socket_type: SocketType,
    threads: usize,
    bsread: Arc<Bsread>,
    receivers: Vec<Receiver>
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
        Ok(Self { socket_type, threads, bsread,  receivers})
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
        Ok(Self { socket_type, threads, bsread,  receivers})
    }


    //Callback called in each receiver thread
    pub fn start<F>(&mut self, callback: F) -> IOResult<()>
    where
        F: Fn(Message) + Send + 'static,
    {
        let shared_callback = Arc::new(Mutex::new(callback));
        for receiver in &mut self.receivers {
            let callback_clone = Arc::clone(&shared_callback);
            receiver.fork(move |msg| {
                let mut callback = callback_clone.lock().unwrap();
                callback(msg);
            }, None);

        }
        Ok(())
    }

    //Callback called in a private thread for each receiver using a message buffer.
    pub fn start_buffered<F>(&mut self, mut callback: F, buffer_size:usize) -> IOResult<()>
    where
        F: Fn(Message) + Send + 'static,
    {
        let shared_callback = Arc::new(Mutex::new(callback));
        for receiver in & mut self.receivers {
            let callback_clone = Arc::clone(&shared_callback);
            let thread_name = format!("Pool {}", receiver.to_string());
            let interrupted = Arc::clone(self.bsread.interrupted());
            receiver.start(buffer_size)?;
            let fifo = receiver.fifo().unwrap();

            thread::Builder::new()
                .name(thread_name.to_string())
                .spawn(move || -> IOResult<()>{
                    while !interrupted.load(Ordering::Relaxed){
                        match fifo.get(){
                            None => {
                                thread::sleep(Duration::from_millis(10));
                            }
                            Some(msg) => {
                                // Lock the callback and extend the lifetime of the mutable reference
                                let mut callback = callback_clone.lock().unwrap();
                                let callback_ref = callback.deref_mut(); // Create a long-lived reference
                                callback_ref(msg); // Call the callback using the long-lived reference
                            }
                        }
                    }
                    Ok(())
                })
                .expect("Failed to spawn thread");
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
    
    #[cfg(feature = "async")]
    pub fn start_async<F, Fut>(&mut self, callback: F, handle: Option<tokio::runtime::Handle>) -> IOResult<()>
    where
        F: Fn(Message) -> Fut + Send + Sync + 'static,
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
            }, None, Some(receiver_handle));
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
    pub fn socket_type(&self) -> SocketType {
        self.socket_type
    }

    pub fn threads(&self) -> usize {
        self.threads
    }

    pub fn receivers(&self) -> &Vec<Receiver> {
        &self.receivers
    }

}
