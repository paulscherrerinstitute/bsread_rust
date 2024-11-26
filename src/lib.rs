use crate::channel::{ChannelConfig, ChannelArray, ChannelScalar, ChannelValue, ChannelTrait};
use crate::message::{ChannelData, BsMessage};
use crate::receiver::Receiver;

use core::result::Result;
use zmq::{Context, SocketType};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc};

struct Bsread {
    context: Context,
    interrupted: Arc<AtomicBool>,
}

impl Bsread {
    pub fn new() -> io::Result<Self> {
        let context = zmq::Context::new();
        let interrupted = Arc::new(AtomicBool::new(false));
        Ok(Self { context, interrupted })
    }

    pub fn new_forked(interrupted: Arc<AtomicBool>) -> io::Result<Self> {
        let context = zmq::Context::new();
        Ok(Self { context, interrupted })
    }

    fn receiver(&self, endpoint: Option<Vec<&str>>, socket_type: SocketType) -> Result<Receiver, Box<dyn std::error::Error>> {
        Receiver::new(&self, endpoint, socket_type)
    }

    fn interrupt(&self) {
        self.interrupted.store(true, Ordering::Relaxed);
    }

    fn is_interrupted(&self) -> bool {
        let ret = self.interrupted.load(Ordering::Relaxed);
        ret
    }
}


#[cfg(test)]
mod tests;
mod channel;
mod message;
mod reader;
mod receiver;
mod compression;
mod utils;
mod convert;

