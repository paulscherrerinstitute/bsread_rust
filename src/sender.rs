use crate::*;
use crate::message::*;
use crate::utils::*;
use std::{io, thread};
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::collections::HashMap;
use zmq::{Context, SocketType};
use std::time::{Duration, Instant};
use serde_json::Error as JSonError;
use serde_json::Value as JsonValue;
use crate::compression::*;
use serde_json::Map as JsonMap;
use crate::compression::{decompress_bitshuffle_lz4, decompress_lz4};


pub struct Sender<'a> {
    socket: zmq::Socket,
    socket_type: SocketType,
    main_header: HashMap<String, JsonValue>,
    data_header: HashMap<String, JsonValue>,
    data_header_buffer: Option<Vec<u8>>,
    bsread: &'a Bsread,
    port: u32,
    address:String,
    queue_size: usize,
    block:bool,
    pulse_id:u32,
    data_compression:String,
    header_compression:String,
    channels: Vec<Box<dyn ChannelTrait>>,
    started: bool
}

impl
<'a> Sender<'a> {
    pub fn new(bsread: &'a Bsread, socket_type: SocketType, port: u32,
               address:Option<String>, queue_size: Option<usize>, block:Option<bool>,
               start_id:Option<u32>, data_compression:Option<String>, header_compression:Option<String>) -> IOResult<Self> {
        let socket = bsread.get_context().socket(socket_type)?;
        let address = address.unwrap_or("tcp://*".to_string());
        let queue_size = queue_size.unwrap_or(10);
        let block = block.unwrap_or(false);
        let start_id = start_id.unwrap_or(1);
        let data_compression = data_compression.unwrap_or("none".to_string());
        let header_compression = header_compression.unwrap_or("none".to_string());
        Ok(Self { socket, socket_type, main_header:HashMap::new(), data_header: HashMap::new(), data_header_buffer: None,
            bsread, port, address, queue_size, block,pulse_id:start_id, data_compression, header_compression, channels: vec![], started:false})
    }

    fn create_data_header(&mut self, channels: &Vec<Box<dyn ChannelTrait>>,)-> IOResult<()> {
        self.data_header = HashMap::new();
        self.data_header.insert("htype".to_string(), JsonValue::String("bsr_d-1.1".to_string()));


        let mut channel_metadata = Vec::new();
        for channel in channels{
            channel_metadata.push(channel.get_config().get_metadata());
        }

        let channel_metadata_json: JsonValue = JsonValue::Array(
            channel_metadata
                .into_iter()
                .map(|map| {
                    JsonValue::Object(
                        map.into_iter()
                            .map(|(k, v)| (k, v))
                            .collect::<JsonMap<String, JsonValue>>(),
                    )
                })
                .collect(),
        );

        self.data_header.insert("channels".to_string(), channel_metadata_json);

        let data_header_json = serde_json::to_string(&self.data_header)?;
        let blob = match self.data_compression.as_str() {
            "bitshuffle_lz4" => {
                &compress_bitshuffle_lz4(data_header_json.as_bytes(), 1)?
            }
            "lz4" => {
                &compress_lz4(data_header_json.as_bytes())?
            }
            &_ => { data_header_json.as_bytes() }
        };
        self.main_header.insert("hash".to_string(),  JsonValue::String(get_hash(blob)));
        self.data_header_buffer = Some((*blob).to_vec());
        Ok(())
    }


    pub fn start(&mut self, channels: Vec<Box<dyn ChannelTrait>>) -> IOResult<()> {
        self.socket.bind(self.get_url().as_str())?;
        let mut main_header: HashMap<String, JsonValue> = HashMap::new();
        main_header.insert("htype".to_string(), JsonValue::String("bsr_m-1.1".to_string()));
        if self.header_compression != "none" {
            main_header.insert("dh_compression".to_string(), JsonValue::String(self.header_compression.to_string()));
        }
        self.create_data_header(&channels)?;
        self.channels = channels;
        self.started = true;
        Ok(())
    }

    pub fn stop(&mut self) -> IOResult<()> {
        self.started = false;
        self.socket.unbind(self.get_url().as_str())?;
        Ok(())
    }

    pub fn send(&mut self, values: Vec<Value>) -> IOResult<()> {
        if values.len() != self.channels.len() {
            return Err(new_error(ErrorKind::InvalidInput, "Invalid size of channel value list"));
        }

        Ok(())
    }

    pub fn is_started(&self) -> bool{
        self.started
    }

    pub fn get_url(&self) -> String {
        format!("{}:{}", self.address.as_str(), self.port)
    }

}
