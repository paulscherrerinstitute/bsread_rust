use crate::IOResult;
use crate::receiver::Receiver;
use crate::pool::Pool;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use zmq::{Context, SocketType};

pub struct Bsread {
    context: Context,
    interrupted: Arc<AtomicBool>,
}

impl Bsread {
    pub fn new() ->IOResult<Self> {
        let context = zmq::Context::new();
        let interrupted = Arc::new(AtomicBool::new(false));
        Ok(Self { context, interrupted })
    }

    pub fn new_forked(interrupted: Arc<AtomicBool>) ->IOResult<Self> {
        let context = zmq::Context::new();
        Ok(Self { context, interrupted })
    }

    pub fn receiver(&self, endpoint: Option<Vec<&str>>, socket_type: SocketType) -> IOResult<Receiver> {
        Receiver::new(&self, endpoint, socket_type)
    }

    pub fn pool_auto(&self, endpoints: Vec<&str>, socket_type: SocketType, threads:usize) -> IOResult<Pool> {
        Pool::new_auto(&self, endpoints, socket_type, threads)
    }

    pub fn pool_manual(&self, endpoints: Vec<Vec<&str>>, socket_type: SocketType) -> IOResult<Pool> {
        Pool::new_manual(&self, endpoints, socket_type)
    }

    pub fn interrupt(&self) {
        self.interrupted.store(true, Ordering::Relaxed);
    }

    pub fn is_interrupted(&self) -> bool {
        self.interrupted.load(Ordering::Relaxed)
    }

    pub fn get_context(&self) -> &Context {
        return &self.context;
    }


    pub fn get_interrupted(&self) -> &Arc<AtomicBool> {
        return &self.interrupted;
    }
}

impl Drop for Bsread {
    fn drop(&mut self) {
    }
}


