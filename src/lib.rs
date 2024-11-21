use crate::channel::{ChannelConfig, ChannelArray, ChannelScalar, ChannelValue, ChannelTrait};
use crate::message::{ChannelData, BsMessage};
use crate::receiver::Receiver;

use core::result::Result;
use std::collections::HashMap;
use zmq::{Context, Message, SocketType};
use std::convert::TryFrom;
use byteorder::{LittleEndian, BigEndian, ReadBytesExt, ByteOrder};
use std::io::{Cursor, Read};
use std::io;
use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

struct Bsread {
    context: Context,
    interrupted: Arc<AtomicBool>
}

impl Bsread {
    pub fn new() -> io::Result<Self>{
        let context = zmq::Context::new();
        let interrupted = Arc::new(AtomicBool::new(false));
        Ok(Self { context, interrupted })
    }

    fn receiver(&self, endpoint: Option<Vec<&str>>,  socket_type: SocketType) -> Result<Receiver, Box<dyn std::error::Error>> {
        Receiver::new(&self, endpoint, socket_type)
    }

    fn interrupt(&self){
        self.interrupted.store(true, Ordering::Relaxed);
    }

    fn is_interrupted(&self) ->bool {
        let ret = self.interrupted.load(Ordering::Relaxed);
        println!("{ret}");
        ret
    }
}


#[cfg(test)]
mod tests;
mod channel;
mod message;
mod reader;
mod receiver;

