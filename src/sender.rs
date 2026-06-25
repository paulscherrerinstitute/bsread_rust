use crate::*;
use crate::message::*;
use crate::utils::*;
use crate::compression::*;
use crate::sockets::*;
use std::error::Error;
use std::{fs, thread};
use std::collections::HashMap;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use env_logger::fmt::Timestamp;
use zmq::SocketType;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use serde_json::Map as JsonMap;
use serde_json::Number as JsonNumber;


pub struct Sender {
    socket: zmq::Socket,
    socket_type: SocketType,
    main_header: HashMap<String, JsonValue>,
    data_header: HashMap<String, JsonValue>,
    data_header_buffer: Vec<u8>,
    bsread: Arc<Bsread>,
    transport: Transport,
    block: bool,
    pulse_id: u64,
    header_compression: String,
    started: bool,
}

impl Sender {
        pub fn new(
            bsread: Arc<Bsread>,
            socket_type: SocketType,
            transport: Transport,
            block: Option<bool>,
            start_id: Option<u64>,
            header_compression: Option<String>,
        ) -> IOResult<Self> {
        let socket = bsread.get_context().socket(socket_type)?;
        let block = block.unwrap_or(false);
        let start_id = start_id.unwrap_or(0);
        let header_compression = header_compression.unwrap_or("none".to_string());

        let mut main_header = HashMap::new();

        //Initialize main header
        main_header.insert("htype".to_string(), JsonValue::String("bsr_m-1.1".to_string()));
        if header_compression != "none" {
            main_header.insert("dh_compression".to_string(), JsonValue::String(header_compression.to_string()));
        }
        let mut _self = Self { socket, socket_type, main_header:main_header, data_header: HashMap::new(), data_header_buffer: vec![],
                bsread, transport, block,pulse_id:start_id, header_compression, started:false};
        _self.set_sndhwm(10); //By default only 10 messages queued
        Ok(_self)
    }

    pub fn create_data_header(&mut self, channels: &Vec<Box<dyn ChannelTrait>>,)-> IOResult<()> {
        self.data_header = create_data_header(channels)?;
        // Convert the HashMap to a BTreeMap to enforce key order
        let ordered_data_header: BTreeMap<_, _> = self.data_header.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let data_header_json = serde_json::to_string(&ordered_data_header)?;

        let blob = match self.header_compression.as_str() {
            "bitshuffle_lz4" => {
                &compress_bitshuffle_lz4(data_header_json.as_bytes(), 1)?
            }
            "lz4" => {
                &compress_lz4(data_header_json.as_bytes(), false)?
            }
            &_ => { data_header_json.as_bytes() }
        };
        let hash = get_hash(blob);
        self.main_header.insert("hash".to_string(),  JsonValue::String(hash));
        self.data_header_buffer = (*blob).to_vec();
        Ok(())
    }

    pub fn get_last_id(& self) -> u64{
        self.pulse_id
    }

    pub fn start(&mut self) -> IOResult<()> {
        let endpoint = self.endpoint();
        log::info!("Binding endpoint: {}", endpoint);
        self.socket.bind(endpoint.as_str())?;
        self.started = true;
        Ok(())
    }

    pub fn stop(&mut self){
        if self.started{
            self.started = false;
            let endpoint = self.endpoint();
            log::info!("Unbinding endpoint: {}", endpoint);
            match self.socket.unbind(endpoint.as_str()) {
                Ok(_) => (),
                Err(e) =>  log::warn!("Error unbinding {}: {}", endpoint, e)
            };
        }
    }


    pub fn send(&mut self,  id:u64, timestamp: (u64,u64), channels: &Vec<Box<dyn ChannelTrait>>, channel_data: &Vec<Option<&ChannelData>>) -> IOResult<()> {
        if channel_data.len() ==0 {
            return Err(new_error(ErrorKind::InvalidInput, "Empty channel data list"));
        }
        if channel_data.len() != channels.len(){
            return Err(new_error(ErrorKind::InvalidInput, "Invalid size of channel data list"));
        }

        self.update_main_header(id, timestamp);

        let flags_last = if self.block {0} else {zmq::DONTWAIT};
        let flags_more = flags_last | zmq::SNDMORE;


        let valid_channels = channel_data.iter().filter(|item| item.is_some()).count();
        let main_header_json = serde_json::to_string(&self.main_header)?;
        let blob = main_header_json.as_bytes();
        let main_header_buffer = (*blob).to_vec();

        self.socket.send(main_header_buffer, flags_more)?;
        self.socket.send(&self.data_header_buffer, if valid_channels>0 {flags_more} else {flags_last} )?;

        let mut channel_index = 0;
        for i in 0..channels.len(){
            let ch = &channels[i];
            if let Some(channel_data) = &channel_data[i] {
                let last = channel_index >= (valid_channels- 1);
                let (data,tm) =  serialize_channel(&ch, &channel_data)?;
                self.socket.send(data, flags_more)?;
                self.socket.send(tm, if last {flags_last} else {flags_more})?;
                channel_index = channel_index + 1;
            } ;
        }
        Ok(())
    }

    pub fn forward (&mut self,  message_parts:&Vec<Vec<u8>>) -> IOResult<()> {
        let flags_last = if self.block {0} else {zmq::DONTWAIT};
        let flags_more = flags_last | zmq::SNDMORE;
        for (index, msg) in message_parts.iter().enumerate() {
            let is_last = index == message_parts.len() - 1;
            self.socket.send(msg, if is_last {flags_last} else {flags_more})?;
        }
        Ok(())
    }


    pub fn send_message(&mut self,  message: &Message, create_data_header:bool) -> IOResult<()> {
        let empty_data_header = self.data_header_buffer.len() == 0;
        if create_data_header || empty_data_header {
            self.create_data_header(message.get_channels())?;
        }
        let id = message.get_id();
        let timestamp = message.get_timestamp();
        let channel_data = message.get_data();
        let ordered_values: Vec<Option<&ChannelData>> = channel_data.values().map(|result| result.as_ref()).collect();
        self.send(id, timestamp, message.get_channels(), &ordered_values)
    }

    pub fn update_main_header(& mut self, id:u64, timestamp: (u64,u64)) {
        let id = if id == ID_SIMULATED {
            let ret = self.pulse_id;
            self.pulse_id = self.pulse_id+1;
            ret
        } else {
            self.pulse_id = id;
            id
        };
        self.main_header.insert("pulse_id".to_string(),  JsonValue::Number(JsonNumber::from(id)));

        let tm =  if timestamp == TIMESTAMP_NOW {
            get_cur_timestamp()
        } else {
            timestamp
        };
        let mut global_timestamp = JsonMap::new();
        global_timestamp.insert("sec".to_string(), JsonValue::Number(tm.0.into()));
        global_timestamp.insert("ns".to_string(), JsonValue::Number(tm.1.into()));
        self.main_header.insert("global_timestamp".to_string(), JsonValue::Object(global_timestamp));

    }
    pub fn is_started(&self) -> bool{
        self.started
    }

    pub fn endpoint(&self) -> String {
        self.transport.endpoint()
    }
}

impl SocketConfig for Sender {
    fn transport(&self) -> Transport {
        self.transport.clone()
    }
    fn socket_type(&self) -> SocketType {
        self.socket_type
    }
    fn socket(&self) -> &zmq::Socket {
        &self.socket
    }
}

