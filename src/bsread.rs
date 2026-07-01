use crate::{Compression, IOError, IOResult};
use crate::receiver::{ConnectionMode, Receiver};
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

    pub fn receiver(self: &Arc<Self>, endpoints: Option<Vec<&str>>, socket_type: SocketType, connection_mode: ConnectionMode ) -> IOResult<Receiver> {
        Receiver::new(self.clone(), endpoints, socket_type, connection_mode)
    }


    pub fn pool(self: &Arc<Self>, endpoints: Vec<&str>, socket_type: SocketType, connection_mode: ConnectionMode, threads:usize) -> IOResult<Pool> {
        Pool::new(self.clone(), endpoints, socket_type, threads, connection_mode)
    }

    pub fn pool_grouped(self: &Arc<Self>, endpoints: Vec<Vec<&str>>, socket_type: SocketType, connection_mode: ConnectionMode) -> IOResult<Pool> {
        Pool::new_grouped(self.clone(), endpoints, socket_type, connection_mode)
    }

    pub fn sender(self: &Arc<Self>, socket_type: SocketType, transport: Transport,
                  block:Option<bool>, start_id:Option<u64>, header_compression:Option<Compression>) -> IOResult<Sender> {
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


