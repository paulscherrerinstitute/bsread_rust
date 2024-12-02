extern crate core;

use crate::bsread::{Bsread};
use crate::channel::{ChannelConfig, ChannelArray, ChannelScalar, ChannelTrait};
use crate::channel_value::{ChannelValue};
use crate::message::{ChannelData, BsMessage};
use crate::receiver::Receiver;
use core::result::Result;
use zmq::{Context, SocketType};
use std::io::Result as IOResult;
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

