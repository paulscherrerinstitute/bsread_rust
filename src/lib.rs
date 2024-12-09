extern crate core;

use crate::bsread::{Bsread};
use crate::channel::{ChannelConfig, ChannelArray, ChannelScalar, ChannelTrait};
use crate::value::{Value};
use crate::message::{ChannelData, Message};
use crate::receiver::Receiver;
use crate::pool::Pool;
use core::result::Result;
use std::io;
use zmq::{Context, SocketType};
use std::io::Result as IOResult;
use std::io::Error as IOError;
use std::io::ErrorKind as ErrorKind;

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
mod value;
mod debug;
mod pool;
mod dispatcher;

