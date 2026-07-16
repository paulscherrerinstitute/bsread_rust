use crate::*;
use crate::value::*;
use crate::reader::*;
use crate::writer::*;
use crate::compression::*;
use crate::utils::LimitedHashMap;
use std::collections::HashMap;
use std::io::{Cursor};
use std::thread;
use indexmap::IndexMap;
use serde_json::Error as JSonError;
use serde_json::Value as JsonValue;
use serde_json::Map as JsonMap;
use serde_json::Number as JsonNumber;

pub const ID_SIMULATED:u64 = 0;
pub const TIMESTAMP_NOW:(u64,u64) = (0,0);

// ErrorKind::NotSeekable is used internally as a marker for decompression failures
pub const DECOMPRESSION_ERROR:ErrorKind = ErrorKind::NotSeekable;

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

fn parse_channel(channel_data: &JsonMap<String, JsonValue>, raw: bool) -> IOResult<Box<dyn ChannelTrait>> {
    let name = channel_data.get("name")
        .and_then(|v| v.as_str())
        .ok_or(IOError::new(ErrorKind::InvalidInput,"Invalid format: 'name' missing or not a string"))?
        .to_string();

    let typ = channel_data.get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("float64")
        .to_string();

    let shape = convert_shape_val_to_vec(channel_data.get("shape"));
    let encoding = channel_data.get("encoding")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let little_endian = if encoding == ">" || encoding.to_lowercase()=="big" { false } else { true };

    //"none" or "bitshuffle_lz4"
    let compression = Compression::from_str(channel_data.get("compression")
        .and_then(|v| v.as_str())
        .unwrap_or("none"))?;

    channel::new(name, typ, shape, little_endian, compression, raw)
}

fn parse_channels(data_header: &HashMap<String, JsonValue>, raw: bool) -> IOResult<Vec<Box<dyn ChannelTrait>>> {
    // Attempt to get the "channels" key and ensure it is an array
    let items = data_header
        .get("channels")
        .and_then(|v| v.as_array())
        .ok_or(IOError::new(ErrorKind::InvalidInput,"Invalid format: 'channels' missing or not an array"))?;

    // Initialize the resulting HashMap
    //let mut channels = HashMap::new();
    let mut channels = Vec::new();

    // Iterate over each item in the array
    for item in items {
        // Ensure each item is a map with string keys and string values
        let channel_data = item.as_object().
            ok_or(IOError::new(ErrorKind::InvalidInput,"Invalid format: is not an object"))?;
        let channel = parse_channel(channel_data, raw).unwrap();
        //channels.insert(name, channel);
        channels.push(channel);
    }
    Ok(channels)
}

#[derive(Debug, Clone)]
pub struct ChannelData {
    value: Value,
    timestamp: (u64, u64),
}

impl ChannelData {
    pub fn new(value: Value, timestamp: (u64, u64)) -> Self {
        Self { value, timestamp }
    }
    pub fn value(&self) -> &Value {
        &self.value
    }
    pub fn timestamp(&self) -> &(u64, u64) {
        &self.timestamp
    }
}

fn parse_channel_data(global_timestamp:&(u64, u64), channel: &Box<dyn ChannelTrait>, v: &Vec<u8>, t: &Vec<u8>, raw:bool) -> IOResult<ChannelData> {
    //if t.len() != 16 {
    //    return Err(IOError::new(ErrorKind::InvalidData, format!("Invalid channel timestamp: {:?}", t).as_str()));
    //}
    let timestamp = if (t.len() == 16){
        let mut cursor = Cursor::new(t);
        let timestamp_secs = READER_U64(&mut cursor)?;
        let timestamp_nanos = READER_U64(&mut cursor)?;
        (timestamp_secs, timestamp_nanos)
    } else {
        global_timestamp.clone()
    };

    let data = match channel.config().compression() {
        Compression::BitshuffleLz4 => {
            &decompress_bitshuffle_lz4(v, channel.config().element_size())
                .map_err(|e| IOError::new(DECOMPRESSION_ERROR, e))?
        }
        Compression::Lz4 => {
            &decompress_lz4(v,  channel.config().is_little_endian())
                .map_err(|e| IOError::new(DECOMPRESSION_ERROR, e))?
        }
        Compression::None => { v }
    };
    // Create a Cursor to read from the vector
    if raw {
        Ok(ChannelData { value: Value::AU8(data.clone()), timestamp: timestamp })
    } else {
        let mut cursor = Cursor::new(data);
        let value = channel.read(&mut cursor);
        Ok(ChannelData { value: value.unwrap(), timestamp: timestamp })
    }
}

pub fn serialize_channel(channel: &Box<dyn ChannelTrait>, channel_data: & ChannelData) -> IOResult<(Vec<u8>,Vec<u8>)> {
    let  value = channel_data.value();
    let  timestamp = channel_data.timestamp();
    let size = channel.config().size();
    let mut buf = vec![0u8; size];
    let mut cursor = Cursor::new(&mut buf);
    channel.write(& mut cursor, &value)?;
    let data = match channel.config().compression() {
        Compression::BitshuffleLz4 => {
            compress_bitshuffle_lz4(&buf, channel.config().element_size())?
        }
        Compression::Lz4 => {
            compress_lz4(&buf, channel.config().is_little_endian())?
        }
        Compression::None => { buf }
    };
    let mut tm = vec![0u8; 16];
    let mut cursor = Cursor::new(&mut tm);
    let timestamp_secs = timestamp.0;
    let timestamp_nanos = timestamp.1;
    WRITER_U64(& mut cursor, &timestamp_secs)?;
    WRITER_U64(& mut cursor, &timestamp_nanos)?;
    Ok((data, tm))
}


pub struct Message {
    main_header: HashMap<String, JsonValue>,
    data_header: HashMap<String, JsonValue>,
    channels: Vec<Box<dyn ChannelTrait>>,
    data: IndexMap<String, Option<ChannelData>>,
    id: u64,
    hash: String,
    htype: String,
    dh_compression: Compression,
    timestamp: (u64, u64),
    header_changed: Option<bool>,
    raw: bool,
}

pub struct DataHeaderInfo {
    pub data_header: HashMap<String, JsonValue>,
    pub channels: Vec<Box<dyn ChannelTrait>>,
}

fn id(main_header: &HashMap<String, JsonValue>) -> IOResult<u64> {
    let v = main_header.get("pulse_id").ok_or_else(|| {
            IOError::new(ErrorKind::InvalidInput, "Missing 'pulse_id'")
        })?;

    match v.as_i64() {
        Some(id) if id >= 0 => Ok(id as u64),
        Some(_) => Err(IOError::new(ErrorKind::InvalidInput,"'pulse_id' cannot be negative",)),
        None => Err(IOError::new(ErrorKind::InvalidInput,"'pulse_id' is not a valid integer",)),
    }
}

fn hash(main_header: &HashMap<String, JsonValue>) -> IOResult<String> {
    main_header.get("hash").and_then(|v| v.as_str()).map(|s| s.to_string()).ok_or_else( ||
        IOError::new(ErrorKind::InvalidInput,"Invalid format: 'hash' missing or not a string")
    )
}
fn htype(main_header: &HashMap<String, JsonValue>) -> IOResult<String> {
    let h = main_header.get("htype").and_then(|v| v.as_str()).map(|s| s.to_string()).ok_or_else( ||
        IOError::new(ErrorKind::InvalidInput,"Invalid format: 'htype' missing or not a string")
    )?;
    if h != HTYPE {
        return Err(IOError::new(ErrorKind::InvalidInput,"Invalid field: 'htype'"));
    }
    Ok(h)
}

fn dh_compression(main_header: &HashMap<String, JsonValue>) -> IOResult<Compression> {
    match main_header.get("dh_compression") {
        None => Ok(Compression::None),
        Some(v) => {
            let s = v.as_str().ok_or_else(|| {
                IOError::new(
                    ErrorKind::InvalidInput,
                    "Invalid format: 'dh_compression' is not a string",
                )
            })?;
            Compression::from_str(s)
        }
    }
}

fn timestamp(main_header: &HashMap<String, JsonValue>) -> (u64, u64) {
    match main_header.get("global_timestamp") {
        None => { (0, 0) }
        Some(v) => {
            let m = v.as_object();
            let ns = m.unwrap().get("ns").unwrap().as_u64().unwrap();
            let sec = m.unwrap().get("sec").unwrap().as_u64().unwrap();
            (sec, ns)
        }
    }
}

impl Message {
    pub fn new(main_header: HashMap<String, JsonValue>,
           data_header: HashMap<String, JsonValue>,
           channels: Vec<Box<dyn ChannelTrait>>,
           data: IndexMap<String, Option<ChannelData>>,
           header_changed: Option<bool>,
           raw: bool) -> IOResult<Self> {
        let hash = hash(&main_header)?;
        let id = id(&main_header)?;
        let htype =htype(&main_header)?;
        let dh_compression = dh_compression(&main_header)?;
        let timestamp = timestamp(&main_header);

        Ok(Self { main_header, data_header, channels, data, id, hash, htype, dh_compression, timestamp, header_changed, raw })
    }
    pub fn new_from_channel_map(id:u64, timestamp: (u64, u64),  channels: Vec<Box<dyn ChannelTrait>>, channel_data:IndexMap<String, Option<ChannelData>>) -> IOResult<Self> {
        let mut main_header: HashMap<String, JsonValue> = HashMap::new();
        main_header.insert("htype".to_string(), JsonValue::String(HTYPE.to_string()));
        main_header.insert("pulse_id".to_string(),  JsonValue::Number(JsonNumber::from(id)));
        let mut global_timestamp = JsonMap::new();
        global_timestamp.insert("sec".to_string(), JsonValue::Number(timestamp.0.into()));
        global_timestamp.insert("ns".to_string(), JsonValue::Number(timestamp.1.into()));
        main_header.insert("global_timestamp".to_string(), JsonValue::Object(global_timestamp));
        let data_header = create_data_header(&channels)?;

        let data_header_json = serde_json::to_string(&data_header)?;
        let blob = data_header_json.as_bytes();
        main_header.insert("hash".to_string(),  JsonValue::String(crate::utils::hash_md5(blob)));
        Message::new(main_header, data_header, channels, channel_data, None, false)
    }

    pub fn new_from_channel_vec(id:u64, timestamp: (u64, u64),  channels: &Vec<Box<dyn ChannelTrait>>, mut channel_data:Vec<Option<ChannelData>>) -> IOResult<Self> {
        let mut data: IndexMap<String, Option<ChannelData>> = IndexMap::new();
        for i in 0..channels.len() {
            //data.insert(channels[i].config().get_name().clone(),channel_data[i].clone());
            data.insert(channels[i].config().name().clone(), channel_data[i].take());
        }
        let mut cloned_channels = Vec::new();
        for channel in channels {
            cloned_channels.push(channel::copy(&channel)?);
        }
        Message::new_from_channel_map(id, timestamp, cloned_channels, data )
    }

    pub fn main_header(&self) -> &HashMap<String, JsonValue> {
        &self.main_header
    }

    pub fn data_header(&self) -> &HashMap<String, JsonValue> {
        &self.data_header
    }

    pub fn channels(&self) -> &Vec<Box<dyn ChannelTrait>> {
        &self.channels
    }

    pub fn data(&self) -> &IndexMap<String, Option<ChannelData>> {
        &self.data
    }

    pub fn hash(&self) -> String {
        self.hash.clone()
    }
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn timestamp(&self) -> (u64, u64) {
        self.timestamp
    }

    pub fn htype(&self) -> String {
        self.htype.clone()
    }

    pub fn dh_compression(&self) -> Compression {
        self.dh_compression.clone()
    }

    pub fn header_changed(&self) -> bool {
        self.header_changed.unwrap_or_else(|| false)
    }

    pub fn is_raw(&self) -> bool {
        self.raw
    }

    pub fn channel_data(&self, channel_name: &str) -> Option<&ChannelData> {
        self.data().get(channel_name)?.as_ref()
    }
    pub fn channel_value(&self, channel_name: &str) -> Option<&Value> {
        self.channel_data(channel_name).map(ChannelData::value)
    }

    fn clone_data_header_info(&self) -> Option<DataHeaderInfo> {
        let data_header = self.data_header.clone();
        //TODO: is there a better way to clone channels?
        let channels;
        match parse_channels(&data_header, self.raw){
            Ok(ch) => {channels = ch;}
            Err(_) => {return None}
        }
        Some(DataHeaderInfo {data_header, channels})
    }
}

pub fn create_data_header(channels: &Vec<Box<dyn ChannelTrait>>,)-> IOResult<HashMap<String,JsonValue>> {
    let mut data_header = HashMap::new();
    data_header.insert("htype".to_string(), JsonValue::String("bsr_d-1.1".to_string()));

    let mut channel_metadata = Vec::new();
    for channel in channels {
        channel_metadata.push(channel.config().metadata());
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
    data_header.insert("channels".to_string(), channel_metadata_json);
    Ok(data_header)
}

pub fn parse_message(message_parts: Vec<Vec<u8>>, last_headers:& mut LimitedHashMap<String, DataHeaderInfo> , counter_header_changes:& mut u32, raw:bool) -> IOResult<Message> {
    let mut data = IndexMap::new();
    if message_parts.len() < 2 {
        return Err(IOError::new(ErrorKind::InvalidData, "Invalid message format"));
    }
    let main_header = decode_json(&message_parts[0])?;
    let hash = hash(&main_header)?;
    let global_timestamp = timestamp(&main_header);

    // Determine whether to reuse or reparse data
    let (data_header, channels, changed) = if let Some(last_msg) = last_headers.remove(&hash) {
        // Reuse the previous data header and channels
        (last_msg.data_header, last_msg.channels, false)
    } else {
        *counter_header_changes = *counter_header_changes +1;
        let blob = &message_parts[1];
        let compression = dh_compression(&main_header)?;

        let json = match compression {
            Compression::BitshuffleLz4 => {
                &decompress_bitshuffle_lz4(blob, 1)
                    .map_err(|e| IOError::new(DECOMPRESSION_ERROR, e))?
            }
            Compression::Lz4 => {
                &decompress_lz4(blob, false)
                    .map_err(|e| IOError::new(DECOMPRESSION_ERROR, e))?
            }
            Compression::None => { &blob }
        };
        let data_header = decode_json(json)?;
        let channels = parse_channels(&data_header, raw).unwrap();
        (data_header, channels, true)
    };

    if message_parts.len() - 2 != channels.len() * 2 {
        return Err(IOError::new(ErrorKind::InvalidData, "Invalid number of messages"));
    }
    for i in 0..channels.len() {
        let channel = &channels[i];
        let v = &message_parts[2 * i + 2];
        let t = &message_parts[2 * i + 3];

        let channel_data = parse_channel_data(&global_timestamp, channel, v, t, raw).ok();
        data.insert(channel.config().name(), channel_data);
    }
    let msg = Message::new(main_header, data_header, channels, data, Some(changed), raw);

    if let Ok(m) = &msg {
        if let Some(l) = m.clone_data_header_info() {
            last_headers.insert(hash, l);
        }
    }
    msg
}