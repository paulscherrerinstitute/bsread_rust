use crate::*;
use zmq::SocketType;
use crate::receiver::Receiver;
use crate::bsread::Bsread;

pub struct Pool<'a> {
    socket_type: SocketType,
    threads: usize,
    bsread: &'a Bsread,
    receivers: Vec<Receiver<'a>>
}

impl
<'a> Pool<'a> {
    pub fn new_auto(bsread: &'a Bsread, endpoints: Vec<&str>, socket_type: SocketType, threads: usize) -> IOResult<Self> {
        if threads<=0{
            return Err(new_error(ErrorKind::InvalidInput, "Invalid number of threads"));
        }
        let mut receivers: Vec<Receiver> = (0..threads).map(|id| Receiver::new(bsread, None, socket_type).unwrap()).collect();
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

    pub fn new_manual(bsread: &'a Bsread, endpoints: Vec<Vec<&str>>, socket_type: SocketType) -> IOResult<Self> {
        let threads = endpoints.len();
        if threads==0{
            return Err(new_error(ErrorKind::InvalidInput, "Invalid configuration"));
        }
        let mut receivers: Vec<Receiver> = (0..threads).map(|id| Receiver::new(bsread, None, socket_type).unwrap()).collect();
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



    pub fn start(&mut self, callback: fn(msg: BsMessage) -> ()) -> IOResult<()> {
        for receiver in &mut self.receivers{
            receiver.fork(callback, None);
        }
        Ok(())
    }

    pub fn stop(&mut self) -> IOResult<()> {
        self.bsread.interrupt();
        for receiver in &mut self.receivers{
            receiver.join();
        }
        Ok(())
    }
}
