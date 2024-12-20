use crate::*;
use crate::receiver::Receiver;
use crate::bsread::Bsread;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use zmq::SocketType;

pub struct Pool {
    socket_type: SocketType,
    threads: usize,
    bsread: Arc<Bsread>,
    receivers: Vec<Receiver>
}

impl
Pool {
    pub fn new_auto(bsread: Arc<Bsread>, endpoints: Vec<&str>, socket_type: SocketType, threads: usize) -> IOResult<Self> {
        if threads<=0{
            return Err(new_error(ErrorKind::InvalidInput, "Invalid number of threads"));
        }
        let mut receivers: Vec<Receiver> = (0..threads).map(|_id| Receiver::new(bsread.clone(), None, socket_type).unwrap()).collect();
        let mut index = 0;
        for endpoint in endpoints{
            receivers[index].add_endpoint(endpoint.to_string());
            index += 1;
            if index >= threads {
                index = 0;
            }
        }
        Ok(Self { socket_type, threads, bsread,  receivers})
    }

    pub fn new_manual(bsread: Arc<Bsread>, endpoints: Vec<Vec<&str>>, socket_type: SocketType) -> IOResult<Self> {
        let threads = endpoints.len();
        if threads==0{
            return Err(new_error(ErrorKind::InvalidInput, "Invalid configuration"));
        }
        let mut receivers: Vec<Receiver> = (0..threads).map(|_id| Receiver::new(bsread.clone(), None, socket_type).unwrap()).collect();
        let mut index = 0;
        for group in endpoints {
            for endpoint  in group {
                receivers[index].add_endpoint(endpoint.to_string());
            }
            index += 1;
            if index >= threads {
                index = 0;
            }
        }
        Ok(Self { socket_type, threads, bsread,  receivers})
    }



    pub fn start_sync(&mut self, callback: fn(msg: Message) -> ()) -> IOResult<()> {
        for receiver in &mut self.receivers{
            receiver.fork(callback, None);
        }
        Ok(())
    }

    pub fn start_buffered(&mut self, callback: fn(msg: Message) -> (), buffer_size:usize) -> IOResult<()> {
        for receiver in & mut self.receivers {
            let thread_name = format!("Pool {}", receiver.to_string());
            let interrupted = Arc::clone(self.bsread.get_interrupted());
            receiver.start(buffer_size)?;
            let fifo = receiver.get_fifo().unwrap();
            thread::Builder::new()
                .name(thread_name.to_string())
                .spawn(move || -> IOResult<()>{
                    while !interrupted.load(Ordering::Relaxed){
                        match fifo.get(){
                            None => {}
                            Some(msg) => {callback(msg)}
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

    pub fn get_socket_type(&self) -> SocketType {
        self.socket_type
    }

    pub fn threads(&self) -> usize {
        self.threads
    }

    pub fn receivers(&self) -> &Vec<Receiver> {
        &self.receivers
    }

}
