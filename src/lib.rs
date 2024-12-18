extern crate core;

pub use crate::bsread::{Bsread};
pub use crate::channel::{ChannelConfig, ChannelArray, ChannelScalar, ChannelTrait};
pub use crate::value::{Value};
pub use crate::message::{ChannelData, Message, DataHeaderInfo};
pub use crate::receiver::Receiver;
pub use crate::pool::Pool;
pub use crate::sender::Sender;
pub use zmq::SocketType;
pub use std::io::Result as IOResult;
pub use std::io::Error as IOError;
pub use std::io::ErrorKind as ErrorKind;
use zmq::Context;
use log;
use core::result::Result;
use std::io;

fn new_error(kind: ErrorKind, desc: &str) -> IOError {
    IOError::new(kind, desc)
}

//Result<(), Box<dyn std::error::Error>>
#[cfg(test)]
mod tests;
pub mod bsread;
pub mod channel;
pub mod message;
pub mod reader;
pub mod writer;
pub mod receiver;
pub mod compression;
pub mod utils;
pub mod convert;
pub mod value;
pub mod debug;
pub mod pool;
pub mod dispatcher;
pub mod sender;

