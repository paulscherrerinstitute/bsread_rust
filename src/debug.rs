use std::ops::Sub;
use crate::*;
use crate::IOResult;
use crate::receiver::{Receiver};
use crate::pool::{Pool};
use crate::message::{Message, ChannelData};
use crate::bsread::Bsread;
use crate::sender::Sender;
use crate::value::Value;
use indexmap::IndexMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use zmq::SocketType;
use lazy_static::lazy_static;
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub fn vec_to_hex_string(vec: &[u8]) -> String {
    vec.iter()
        .map(|byte| format!("0x{:02X}", byte)) // Format each byte as a two-digit hexadecimal
        .collect::<Vec<String>>()
        .join(", ") // Join all formatted strings with ", "
}


pub struct LimitedDebugVec<T> {
    pub data: Vec<T>,
    pub limit: usize,
}

impl<T: std::fmt::Debug> std::fmt::Debug for LimitedDebugVec<T>  {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let len = self.data.len();
        let display_len = self.limit.min(len);
        let limited_data = &self.data[..display_len];
        write!(f, "{:?}", limited_data)?; // Print the limited vector
        if len > display_len {
            write!(f, " ... ({} more elements)", len - display_len)?;
        }
        Ok(())
    }
}

pub fn print_channel_data(channel_data: &Option<ChannelData>, prefix:&str, max_elements: usize) {
    match &channel_data {
        Some(channel_data) => {
            let value = channel_data.get_value();
            if value.is_array() {
                println!("{}{:?}", prefix, LimitedDebugVec { data: value.as_str_array().unwrap(), limit: max_elements });
            } else {
                println!("{}{}", prefix, channel_data.get_value().as_str());
            }
        }
        None => {
            println!("{}<None>", prefix);
        }
    }
}

static mut MESSAGE_COUNTER: Mutex<i32> = Mutex::new(0);


fn increment_counter() {
    unsafe {
        let mut counter = MESSAGE_COUNTER.lock().unwrap();
        *counter += 1;
    }
}

pub fn print_message(message: &Message, max_size:usize, header:bool, id:bool, attrs:bool, main_header:bool, data_header:bool, meta:bool, data:bool) -> () {
    if header {
        println!("{}", "-".repeat(80));
        let current_thread = thread::current(); // Keep the thread alive
        let thread_name = current_thread.name().unwrap_or("Unnamed Thread");
        unsafe {
            println!("Message: {} \t Thread: {}", *MESSAGE_COUNTER.lock().unwrap(), thread_name);
        }

        println!("{}", "-".repeat(80));
    }
    increment_counter();
    if id {
        println!("ID = {:?}", message.get_id());
    }
    if attrs {
        println!("Attrs:");
        println!("\thtype: {:?}", message.get_htype());
        println!("\tdh_compression: {:?}", message.get_dh_compression());
        println!("\thash: {:?}", message.get_hash());
        println!("\ttimestamp: {:?}", message.get_timestamp());
    }
    if main_header {
        println!("Main Header:");
        println!("\t {:?}", message.get_main_header());
    }
    if data_header{
        println!("Data Header:");
        println!("\t {:?}", message.get_data_header());
    }
    if meta {
        let mut channel_names = Vec::new();
        println!("Channel Metadata:");
        for channel in message.get_channels() {
            let config = channel.get_config();
            let shape : Vec<u32> = config.get_shape().unwrap_or(Vec::new());
            println!("\t{} {} {:?} {} {}", config.get_name(), config.get_type(), shape, config.get_elements(), config.get_compression());
            channel_names.push(config.get_name());
        }
    }
    if data{
        println!("Channel Data:");
        let data = message.get_data();
        for (key, value) in data {
            //println!("{}", key);
            print_channel_data(value, format!("\t{}: ", key).as_str(), max_size);
        }
    }
}

pub fn print_stats_rec(rec: &Receiver) -> () {
    let mode = rec.get_mode();
    let socket_type = rec.get_socket_type();
    println!("Receiver {}  {:?} [{}]", rec.index(), socket_type, mode);
    println!("\tConnections: {}", rec.connections());
    println!("\tAvailable: {}", rec.available());
    println!("\tDropped: {}", rec.dropped());
    println!("\tMessage Count: {}", rec.message_count());
    println!("\tError Count: {}", rec.error_count());
    println!("\tHeader Changes: {}", rec.change_count());
}


pub fn print_stats_pool(pool: &Pool) -> () {
    println!("Pool: {} threads", pool.threads());
    for rec in pool.receivers(){
        print_stats_rec(rec);
    }
}


pub fn create_test_values(value: u64, size:usize) -> Vec<Value>{
    let value = (value % 100) as u8;
    let bvalue = (value % 2) == 1;
    let values = vec!(
        Value::STR(format!("{}", value).to_string()),
        Value::BOOL(bvalue), Value::ABOOL(vec![bvalue;size]),
        Value::U8(value),Value::AU8(vec![value;size]),
        Value::U16(value as u16), Value::AU16(vec![value as u16;size]),
        Value::U32(value as u32), Value::AU32(vec![value as u32;size]),
        Value::U64(value as u64), Value::AU64(vec![value as u64;size]),
        Value::I8(value as i8), Value::AI8(vec![value as i8;size]),
        Value::I16(value as i16), Value::AI16(vec![value as i16;size]),
        Value::I32(value as i32), Value::AI32(vec![value as i32;size]),
        Value::I64(value as i64), Value::AI64(vec![value as i64;size]),
        Value::F32(value as f32), Value::AF32(vec![value as f32;size]),
        Value::F64(value as f64), Value::AF64(vec![value as f64;size]),
    );
    values
}

lazy_static! {
    static ref SENDER_INTERRUPTED: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    static ref SENDER_HANDLES: Mutex<Vec<JoinHandle<IOResult<()>>>> = Mutex::new(Vec::new());

}

fn create_message(v:u64, s:usize, compression:Option<String>) -> IOResult<Message>{
    let comp = compression.unwrap_or("none".to_string());
    let mut channels = Vec::new();
    let mut channel_data = Vec::new();
    let values = create_test_values(v, s);
    for value in values {
        let little_endian = true;
        let shape = if value.is_array() { Some(vec![value.get_size() as u32]) } else { None };
        let ch = channel::new(value.get_name().to_string(), value.get_type().to_string(), shape, little_endian, comp.clone())?;
        let ch_data = Some(ChannelData::new(value, (0, 0)));
        channels.push(ch);
        channel_data.push(ch_data);
    }

    let mut data: IndexMap<String, Option<ChannelData>> = IndexMap::new();
    for i in 0..channels.len() {
        data.insert(channels[i].get_config().get_name().clone(),channel_data[i].take() );
    }
    Message::new_from_channel_map(0,(0,0), channels, data)
}

pub fn start_sender(port:u32, socket_type:SocketType, interval_ms:u64, block:Option<bool>, compression:Option<String>) -> IOResult<()> {
    fn create_sender(port:u32, socket_type:SocketType, interval_ms:u64, block:Option<bool>, compression:Option<String>)  -> IOResult<()>{
        let bsread = Bsread::new().unwrap();
        let mut sender = Sender::new(&bsread,  socket_type, port, Some("127.0.0.1".to_string()), None, block, None, None)?;
        sender.start()?;
        let mut count = 0;
        let mut start_time = Instant::now().sub( Duration::from_secs(1));
        while  !SENDER_INTERRUPTED.load(Ordering::Relaxed){
            if start_time.elapsed() >= Duration::from_millis(interval_ms){
                match create_message(count, 100, compression.clone()){
                    Ok(msg) => {
                        match sender.send_message(&msg, true){
                            Ok(_) => {}
                            Err(e) => {log::warn!("Error in Sender [port={}, socketType={:?}]: {:?}", port, socket_type, e)}
                        }
                    }
                    Err(e) => {log::warn!("Error in Sender [port={}, socketType={:?}]: {:?}", port, socket_type, e)}
                }
                count = count+1;
                start_time = Instant::now();
            }
            thread::sleep(Duration::from_millis(10));
        }
        sender.stop();
        Ok(())
    }
    //let interrupted = Arc::clone(&SENDER_INTERRUPTED);
    let handle = thread::Builder::new()
        .name("Sender".to_string())
        .spawn(move || -> IOResult<()> {
            match create_sender(port, socket_type, interval_ms, block, compression){
                Ok(_) => {}
                Err(e) => {log::warn!("Error in Sender [port={}, socketType={:?}]: {:?}", port, socket_type, e)}
            }
            Ok(())

        })
        .expect("Failed to spawn thread");
    let mut handles = SENDER_HANDLES.lock().unwrap(); // Acquire the lock
    handles.push(handle);
    Ok(())
}

pub fn stop_senders(){
    SENDER_INTERRUPTED.store(true, Ordering::Relaxed);
    let mut handles = SENDER_HANDLES.lock().unwrap(); // Acquire the lock
    for handle in handles.drain(..) {
        if let Err(e) = handle.join().unwrap() {
            log::warn!("Error: {:?}", e);
        }
    }
}