extern crate core;

pub use crate::bsread::{Bsread};
pub use crate::channel::{ChannelConfig, ChannelArray, ChannelScalar, ChannelTrait};
pub use crate::value::{Value};
pub use crate::message::{ChannelData, Message, DataHeaderInfo};
pub use crate::utils::{init_id_t0, init_sf_id_t0}; 
pub use crate::receiver::Receiver;
pub use crate::pool::Pool;
pub use crate::sender::Sender;
pub use zmq::SocketType;
pub use std::io::Result as IOResult;
pub use std::io::Error as IOError;
pub use std::io::ErrorKind as ErrorKind;
use std::str::FromStr;
use std::fmt::Display;
use zmq::Context;
use log;
use core::result::Result;
use std::io;

//Constants
pub const HTYPE:&str = "bsr_m-1.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    None,
    BitshuffleLz4,
    Lz4,
}

impl FromStr for Compression {
    type Err = IOError;
    fn from_str(s: &str) -> IOResult<Self> {
        match s {
            "none" => Ok(Compression::None),
            "bitshuffle_lz4" => Ok(Compression::BitshuffleLz4),
            "lz4" => Ok(Compression::Lz4),
            _ => Err(IOError::new(std::io::ErrorKind::InvalidInput, "invalid compression")),
        }
    }
}

impl Display for Compression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Compression::None => "none",
            Compression::BitshuffleLz4 => "bitshuffle_lz4",
            Compression::Lz4 => "lz4",
        };
        write!(f, "{}", s)
    }
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
#[cfg(feature = "dispatcher")]
pub mod dispatcher;
pub mod sender;

pub mod sockets;

