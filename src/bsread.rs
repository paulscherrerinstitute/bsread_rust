use crate::IOResult;
use crate::receiver::Receiver;
use crate::sockets::{Transport};
use crate::sender::{Sender};
use crate::pool::Pool;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use zmq::{Context, SocketType};

/// Bsread context. If interrupted all linked Receiver instances will be interrupted.
pub struct Bsread {
    context: Context,
    interrupted: Arc<AtomicBool>,
}

impl Bsread {
    ///
    pub fn new() ->IOResult<Arc<Self>> {
        let interrupted = Arc::new(AtomicBool::new(false));
        Bsread::new_with_interrupted(interrupted)
    }

    pub fn new_with_interrupted(interrupted: Arc<AtomicBool>) ->IOResult<Arc<Self>> {
        let context = Context::new();
        Ok(Arc::new(Self { context,interrupted }))
    }

    pub fn receiver(self: &Arc<Self>, endpoint: Option<Vec<&str>>, socket_type: SocketType) -> IOResult<Receiver> {
        Receiver::new(self.clone(), endpoint, socket_type)
    }

    pub fn pool(self: &Arc<Self>, endpoints: Vec<&str>, socket_type: SocketType, threads:usize) -> IOResult<Pool> {
        Pool::new(self.clone(), endpoints, socket_type, threads)
    }

    pub fn pool_grouped(self: &Arc<Self>, endpoints: Vec<Vec<&str>>, socket_type: SocketType) -> IOResult<Pool> {
        Pool::new_grouped(self.clone(), endpoints, socket_type)
    }

    pub fn sender(self: &Arc<Self>, socket_type: SocketType, transport: Transport,
                  block:Option<bool>, start_id:Option<u64>, header_compression:Option<String>) -> IOResult<Sender> {
        Sender::new(self.clone(), socket_type, transport, block, start_id, header_compression)
    }

    pub fn interrupt(&self) {
        self.interrupted.store(true, Ordering::Relaxed);
    }

    pub fn is_interrupted(&self) -> bool {
        self.interrupted.load(Ordering::Relaxed)
    }

    pub fn context(&self) -> &Context {
        &self.context
    }


    pub fn interrupted(&self) -> &Arc<AtomicBool> {
        &self.interrupted
    }
}

impl Drop for Bsread {
    fn drop(&mut self) {
    }
}


