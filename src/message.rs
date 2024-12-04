use crate::*;
use crate::value::*;
use crate::reader::*;
use crate::compression::*;
use crate::utils::LimitedHashMap;
use std::collections::HashMap;
use std::io::{Cursor};
use indexmap::IndexMap;
use serde_json::Error as JSonError;
use serde_json::Value as JsonValue;
use serde_json::Map as JsonMap;

fn decode_json(bytes: &Vec<u8>) -> Result<HashMap<String, JsonValue>, JSonError> {
    serde_json::from_slice(&bytes)
}

fn convert_shape_val_to_vec(opt_val: Option<&JsonValue>) -> Option<Vec<u32>> {
    opt_val.and_then(|val| {
        if let JsonValue::Array(arr) = val {
            // Try converting all elements to integers
            let vec: Option<Vec<u32>> = arr
                .into_iter()
                .map(|item| item.as_i64().and_then(|n| Some(n as u32)))
                .collect();

            // Return None if vec is empty, otherwise Some(vec)
            vec.filter(|v| !v.is_empty())
        } else {
            None
        }
    })
}

fn get_channel(channel_data: &JsonMap<String, JsonValue>) -> Result<Box<dyn ChannelTrait>, &'static str> {
    let name = channel_data.get("name")
        .and_then(|v| v.as_str())
        .ok_or("Invalid format: 'name' missing or not a string")?
        .to_string();

    let typ = channel_data.get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("float64")
        .to_string();

    //let shape = channel_data.get("shape")
    //    .and_then(|v| v.as_str())
    //    .unwrap_or("")
    //    .to_string();
    let shape = convert_shape_val_to_vec(channel_data.get("shape"));
    let encoding = channel_data.get("encoding")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let little_endian = if encoding == ">" { false } else { true };

    //"none" or "bitshuffle_lz4"
    let compression = channel_data.get("compression")
        .and_then(|v| v.as_str())
        .unwrap_or("none")
        .to_string();


    if shape.clone().unwrap_or(vec![]).len() > 0 {
        match typ.as_str() {
            "bool" => return Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, READER_ABOOL))),
            //"string" => return Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, READER_ASTRING))),
            "string" => return Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, READER_STRING))),
            "int8" => return Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, READER_AI8))),
            "uint8" => return Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, READER_AU8))),
            "int16" => return Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, if little_endian { READER_AI16 } else { READER_ABI16 }))),
            "uint16" => return Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, if little_endian { READER_AU16 } else { READER_ABU16 }))),
            "int32" => return Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, if little_endian { READER_AI32 } else { READER_ABI32 }))),
            "uint32" => return Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, if little_endian { READER_AU32 } else { READER_ABU32 }))),
            "int64" => return Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, if little_endian { READER_AI64 } else { READER_ABI64 }))),
            "uint64" => return Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, if little_endian { READER_AU64 } else { READER_ABU64 }))),
            "float32" => return Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, if little_endian { READER_AF32 } else { READER_ABF32 }))),
            "float64" => return Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, if little_endian { READER_AF64 } else { READER_ABF64 }))),
            _ => return Err("Unsupported type in 'data'"),
        };
    } else {
        match typ.as_str() {
            "bool" => return Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, READER_BOOL))),
            "string" => return Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, READER_STRING))),
            "int8" => return Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, READER_I8))),
            "uint8" => return Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, READER_U8))),
            "int16" => return Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, if little_endian { READER_I16 } else { READER_BI16 }))),
            "uint16" => return Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, if little_endian { READER_U16 } else { READER_BU16 }))),
            "int32" => return Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, if little_endian { READER_I32 } else { READER_BI32 }))),
            "uint32" => return Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, if little_endian { READER_U32 } else { READER_BU32 }))),
            "int64" => return Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, if little_endian { READER_I64 } else { READER_BI64 }))),
            "uint64" => return Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, if little_endian { READER_U64 } else { READER_BU64 }))),
            "float32" => return Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, if little_endian { READER_F32 } else { READER_BF32 }))),
            "float64" => return Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, if little_endian { READER_F64 } else { READER_BF64 }))),
            _ => return Err("Unsupported type in 'data'"),
        };
    }
}

//fn get_channels(data_header: &HashMap<String, Value>) -> Result<HashMap<String, Channel>, &'static str> {
//fn get_channels(data_header: &HashMap<String, Value>) -> Result<Vec<Channel>, &'static str> {
fn get_channels(data_header: &HashMap<String, JsonValue>) -> Result<Vec<Box<dyn ChannelTrait>>, &'static str> {
    // Attempt to get the "channels" key and ensure it is an array
    let items = data_header
        .get("channels")
        .and_then(|v| v.as_array())
        .ok_or("Invalid format: 'channels' missing or not an array")?;

    // Initialize the resulting HashMap
    //let mut channels = HashMap::new();
    let mut channels = Vec::new();

    // Iterate over each item in the array
    for item in items {
        // Ensure each item is a map with string keys and string values
        let channel_data = item.as_object().ok_or("Invalid format: channel is not an object")?;

        let channel = get_channel(channel_data).unwrap();
        //channels.insert(name, channel);
        channels.push(channel);
    }
    Ok(channels)
}

#[derive(Debug)]
pub struct ChannelData {
    value: Value,
    timestamp: (i64, i64),
}

impl ChannelData {
    pub fn get_value(&self) -> &Value {
        &self.value
    }
    pub fn get_timestamp(&self) -> &(i64, i64) {
        &self.timestamp
    }
}

fn parse_channel(channel: &Box<dyn ChannelTrait>, v: &Vec<u8>, t: &Vec<u8>) -> IOResult<ChannelData> {
    if t.len() != 16 {
        return Err(new_error(ErrorKind::InvalidData, "Invalid channel timestamp"));
    }

    let data = match channel.get_config().get_compression().as_str() {
        "bitshuffle_lz4" => {
            &decompress_bitshuffle_lz4(v, channel.get_config().get_element_size())?
        }
        "lz4" => {
            &decompress_lz4(v)?
        }
        &_ => { v }
    };
    // Create a Cursor to read from the vector
    let mut cursor = Cursor::new(data);
    let value = channel.read(&mut cursor);
    let mut cursor = Cursor::new(t);
    let timestamp_secs = READER_I64(&mut cursor)?;
    let timestamp_nanos = READER_I64(&mut cursor)?;
    let timestamp = (timestamp_secs, timestamp_nanos);
    Ok(ChannelData { value: value.unwrap(), timestamp: timestamp })
}

pub struct Message {
    main_header: HashMap<String, JsonValue>,
    data_header: HashMap<String, JsonValue>,
    channels: Vec<Box<dyn ChannelTrait>>,
    data: IndexMap<String, IOResult<ChannelData>>,
    id: u64,
    hash: String,
    htype: String,
    dh_compression: String,
    timestamp: (u64, u64),
}

pub struct DataHeaderInfo {
    pub data_header: HashMap<String, JsonValue>,
    pub channels: Vec<Box<dyn ChannelTrait>>,
}

fn get_hash(main_header: &HashMap<String, JsonValue>) -> String {
    main_header.get("hash").unwrap().as_str().unwrap().to_string()
}

fn get_dh_compression(main_header: &HashMap<String, JsonValue>) -> String {
    match main_header.get("dh_compression") {
        None => { "none" }
        Some(v) => { v.as_str().unwrap() }
    }.to_string()
}

impl Message {
    fn new(main_header: HashMap<String, JsonValue>,
           data_header: HashMap<String, JsonValue>,
           channels: Vec<Box<dyn ChannelTrait>>,
           data: IndexMap<String, IOResult<ChannelData>>) -> IOResult<Self> {
        let hash = get_hash(&main_header);
        let dh_compression = get_dh_compression(&main_header);
        let id = main_header.get("pulse_id").unwrap().as_u64().unwrap();
        let htype = main_header.get("htype").unwrap().as_str().unwrap().to_string();
        let timestamp = match main_header.get("global_timestamp") {
            None => { (0, 0) }
            Some(v) => {
                let m = v.as_object();
                let ns = m.unwrap().get("ns").unwrap().as_u64().unwrap();
                let sec = m.unwrap().get("sec").unwrap().as_u64().unwrap();
                (sec, ns)
            }
        };
        Ok(Self { main_header, data_header, channels, data, id, hash, htype, dh_compression, timestamp })
    }

    pub fn get_main_header(&self) -> &HashMap<String, JsonValue> {
        &self.main_header
    }

    pub fn get_data_header(&self) -> &HashMap<String, JsonValue> {
        &self.data_header
    }

    pub fn get_channels(&self) -> &Vec<Box<dyn ChannelTrait>> {
        &self.channels
    }

    pub fn get_data(&self) -> &IndexMap<String, IOResult<ChannelData>> {
        &self.data
    }

    pub fn get_hash(&self) -> String {
        self.hash.clone()
    }
    pub fn get_id(&self) -> u64 {
        self.id.clone()
    }

    pub fn get_timestamp(&self) -> (u64, u64) {
        self.timestamp.clone()
    }
    pub fn get_htype(&self) -> String {
        self.htype.clone()
    }

    pub fn get_dh_compression(&self) -> String {
        self.dh_compression.clone()
    }

    pub fn get_value(&self, channel_name: &str) -> Option<&Value> {
        self.get_data().get(channel_name)
            .and_then(|result| result.as_ref().ok())
            .map(|channel_data| channel_data.get_value())
    }
    fn clone_data_header_info(&self) -> Option<DataHeaderInfo> {
        let data_header = self.data_header.clone();
        //TODO: is there a better way to clone channels?
        let channels;
        match get_channels(&data_header){
            Ok(ch) => {channels = ch;}
            Err(_) => {return None}
        }
        Some(DataHeaderInfo {data_header, channels})
    }
}

pub fn parse_message(message_parts: Vec<Vec<u8>>, last_headers:& mut LimitedHashMap<String, DataHeaderInfo> , counter_header_changes:& mut u32) -> IOResult<Message> {
    let mut data = IndexMap::new();
    if message_parts.len() < 2 {
        return Err(new_error(ErrorKind::InvalidData, "Invalid message format"));
    }
    let main_header = decode_json(&message_parts[0])?;
    let hash = get_hash(&main_header);


    // Determine whether to reuse or reparse data
    let (data_header, channels) = if let Some(last_msg) = last_headers.remove(&hash) {
        // Reuse the previous data header and channels
        (last_msg.data_header, last_msg.channels)
    } else {
        *counter_header_changes = *counter_header_changes +1;
        let blob = &message_parts[1];
        let compression = get_dh_compression(&main_header);

        let json = match compression.as_str() {
            "bitshuffle_lz4" => {
                &decompress_bitshuffle_lz4(blob, 1)?
            }
            "lz4" => {
                &decompress_lz4(blob)?
            }
            &_ => { &blob }
        };
        let data_header = decode_json(json)?;
        let channels = get_channels(&data_header).unwrap();
        (data_header, channels)
    };

    if message_parts.len() - 2 != channels.len() * 2 {
        return Err(new_error(ErrorKind::InvalidData, "Invalid number of messages"));
    }
    for i in 0..channels.len() {
        let channel = &channels[i];
        let v = &message_parts[2 * i + 2];
        let t = &message_parts[2 * i + 3];

        let channel_data = parse_channel(channel, v, t);
        data.insert(channel.get_config().get_name(), channel_data);
    }
    let msg = Message::new(main_header, data_header, channels, data);

    if let Ok(m) = &msg {
        if let Some(l) = m.clone_data_header_info() {
            last_headers.insert(hash, l);
        }
    }
    msg
}