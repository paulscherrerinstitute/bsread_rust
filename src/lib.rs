extern crate core;

use crate::bsread::{Bsread};
use crate::channel::{ChannelConfig, ChannelArray, ChannelScalar, ChannelTrait};
use crate::channel_value::{ChannelValue};
use crate::message::{ChannelData, BsMessage};
use crate::receiver::Receiver;
use core::result::Result;
use std::io;
use zmq::{Context, SocketType};
use std::io::Result as IOResult;
use std::io::Error as IOError;
use std::io::ErrorKind as ErrorKind;

fn error_kind_from_str(s: &str) -> ErrorKind {
    let str = s.replace(" ", "").to_lowercase();
    match str.as_str() {
        "notfound" => ErrorKind::NotFound,
        "permissiondenied" => ErrorKind::PermissionDenied,
        "connectionrefused" => ErrorKind::ConnectionRefused,
        "connectionreset" => ErrorKind::ConnectionReset,
        "connectionaborted" => ErrorKind::ConnectionAborted,
        "notconnected" => ErrorKind::NotConnected,
        "addrinuse" => ErrorKind::AddrInUse,
        "addrnotavailable" => ErrorKind::AddrNotAvailable,
        "brokenpipe" => ErrorKind::BrokenPipe,
        "alreadyexists" => ErrorKind::AlreadyExists,
        "wouldblock" => ErrorKind::WouldBlock,
        "invalidinput" => ErrorKind::InvalidInput,
        "invaliddata" => ErrorKind::InvalidData,
        "timedout" => ErrorKind::TimedOut,
        "interrupted" => ErrorKind::Interrupted,
        "unsupported" => ErrorKind::Unsupported,
        "unexpectedeof" => ErrorKind::UnexpectedEof,
        "outofmemory" => ErrorKind::OutOfMemory,
        _ => ErrorKind::Other,  // Return Other for unknown variants
    }
}


fn new_error(kind: ErrorKind, desc: &str) -> IOError {
    IOError::new(kind, desc)
}

//Result<(), Box<dyn std::error::Error>>
#[cfg(test)]
mod tests;
mod bsread;
mod channel;
mod message;
mod reader;
mod receiver;
mod compression;
mod utils;
mod convert;
mod channel_value;

