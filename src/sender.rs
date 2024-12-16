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
use serde_json::Number as JsonNumber;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::compression::{decompress_bitshuffle_lz4, decompress_lz4};


pub struct Sender<'a> {
    socket: zmq::Socket,
    socket_type: SocketType,
    main_header: HashMap<String, JsonValue>,
    data_header: HashMap<String, JsonValue>,
    data_header_buffer: Vec<u8>,
    bsread: &'a Bsread,
    port: u32,
    address:String,
    queue_size: usize,
    block:bool,
    pulse_id:u64,
    data_compression:String,
    header_compression:String,
    started: bool
}

impl
<'a> Sender<'a> {
    pub fn new(bsread: &'a Bsread, socket_type: SocketType, port: u32,
               address:Option<String>, queue_size: Option<usize>, block:Option<bool>,
               start_id:Option<u64>, data_compression:Option<String>, header_compression:Option<String>) -> IOResult<Self> {
        let socket = bsread.get_context().socket(socket_type)?;
        let address = address.unwrap_or("tcp://*".to_string());
        let queue_size = queue_size.unwrap_or(10);
        let block = block.unwrap_or(false);
        let start_id = start_id.unwrap_or(0);
        let data_compression = data_compression.unwrap_or("none".to_string());
        let header_compression = header_compression.unwrap_or("none".to_string());
        Ok(Self { socket, socket_type, main_header:HashMap::new(), data_header: HashMap::new(), data_header_buffer: vec![],
            bsread, port, address, queue_size, block,pulse_id:start_id, data_compression, header_compression, started:false})
    }

    pub fn create_data_header(&mut self, channels: &Vec<Box<dyn ChannelTrait>>,)-> IOResult<()> {
        self.data_header = create_data_header(channels)?;
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
        self.data_header_buffer = (*blob).to_vec();
        Ok(())
    }


    pub fn start(&mut self) -> IOResult<()> {
        let url = self.get_url();
        println!("url: {}", url);
        self.socket.bind(url.as_str())?;
        let mut main_header: HashMap<String, JsonValue> = HashMap::new();
        main_header.insert("htype".to_string(), JsonValue::String("bsr_m-1.1".to_string()));
        if self.header_compression != "none" {
            main_header.insert("dh_compression".to_string(), JsonValue::String(self.header_compression.to_string()));
        }
        self.started = true;
        Ok(())
    }

    pub fn stop(&mut self) -> IOResult<()> {
        self.started = false;
        self.socket.unbind(self.get_url().as_str())?;
        Ok(())
    }


    pub fn send(&mut self, channels: &Vec<Box<dyn ChannelTrait>>, channel_data: &Vec<Option<&ChannelData>>) -> IOResult<()> {
        if channel_data.len() ==0 {
            return Err(new_error(ErrorKind::InvalidInput, "Empty channel data list"));
        }
        if channel_data.len() != channels.len(){
            return Err(new_error(ErrorKind::InvalidInput, "Invalid size of channel data list"));
        }

        let valid_channels = channel_data.iter().filter(|item| item.is_some()).count();
        let main_header_json = serde_json::to_string(&self.main_header)?;
        let blob = main_header_json.as_bytes();
        let main_header_buffer = (*blob).to_vec();

        self.socket.send(main_header_buffer, zmq::SNDMORE)?;
        self.socket.send(&self.data_header_buffer, if valid_channels>0 {zmq::SNDMORE} else {0} )?;

        let mut channel_index = 0;
        for i in 0..channels.len(){
            let ch = &channels[i];
            if let Some(channel_data) = &channel_data[i] {
                let last = channel_index >= (valid_channels- 1);
                let (data,tm) =  serialize_channel(&ch, &channel_data)?;
                self.socket.send(data, zmq::SNDMORE)?;
                self.socket.send(tm, if last {0} else { zmq::SNDMORE})?;
                channel_index = channel_index + 1;
            } ;
        }
        Ok(())
    }


    pub fn send_message(&mut self, message: Message, check_channels:bool) -> IOResult<()> {
        if check_channels {
            self.create_data_header(message.get_channels())?;
        }
        self.pulse_id = if message.get_id() > 0 {message.get_id()} else {self.pulse_id + 1};
        let mut tm =message.get_timestamp();
        if tm == (0,0){
            let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards");
            tm = ( now.as_secs(),  now.subsec_nanos() as u64)
        }
        self.main_header.insert("pulse_id".to_string(),  JsonValue::Number(JsonNumber::from(self.pulse_id)));
        self.main_header.insert("global_timestamp".to_string(), JsonValue::Array(vec![JsonValue::Number(JsonNumber::from(tm.0)),JsonValue::Number(JsonNumber::from(tm.1)),]));
        let channel_data = message.get_data();

        let ordered_values: Vec<Option<&ChannelData>> = channel_data.values().map(|result| result.as_ref()).collect();
        self.send(message.get_channels(), &ordered_values)
    }

    pub fn is_started(&self) -> bool{
        self.started
    }

    pub fn get_url(&self) -> String {
        format!("{}:{}", self.address.as_str(), self.port)
    }

}
