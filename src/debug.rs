use crate::IOResult;
use crate::receiver::{Receiver};
use crate::pool::{Pool};
use crate::message::{Message, ChannelData};
use std::thread;
use std::time::Duration;
use indexmap::IndexMap;
use crate::bsread::Bsread;
use crate::channel::new_channel;
use crate::sender::Sender;
use crate::value::Value;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use zmq::SocketType;
use lazy_static::lazy_static;

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
            //println!("{}{:?}", prefix, channel_data.get_value());
            //println!("{}{:?}", prefix, LimitedDebug { data: channel_data.get_value().as_slice(), limit: 5});
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

lazy_static! {
    static ref SENDER_INTERRUPTED: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
}

pub fn start_sender(port:u32, socketType:SocketType) -> IOResult<()>{
    let interrupted = Arc::clone(&SENDER_INTERRUPTED);
    let handle = thread::Builder::new()
        .name("Sender".to_string())
        .spawn(move || -> IOResult<()> {
            let bsread = Bsread::new().unwrap();
            let mut sender = Sender::new(&bsread,  socketType, port, Some("127.0.0.1".to_string()), None, None, None, None, None)?;
            let value = Value::U8(100);
            let little_endian = true;
            let shape= if value.is_array() {Some(vec![value.get_size()as u32])} else {None};
            let ch = new_channel(value.get_type().to_string(), value.get_type().to_string(), shape, little_endian, "none".to_string())?;
            let channels = vec![ch];
            let mut channel_data =  vec![Some(ChannelData::new(value,(0,0)))];
            let mut data: IndexMap<String, Option<ChannelData>> = IndexMap::new();
            for i in 0..channels.len() {
                data.insert(channels[i].get_config().get_name().clone(),channel_data[i].take() );
            }
            sender.start()?;

            let msg = Message::new_from_ch(0,(0,0), channels, data)?;
            while  !interrupted.load(Ordering::Relaxed){
                sender.send_message(&msg, true)?;
                thread::sleep(Duration::from_millis(1000));
            }
            sender.stop();
            Ok(())

        })
        .expect("Failed to spawn thread");

    Ok(())
}

pub fn stop_senders(){
    SENDER_INTERRUPTED.store(true, Ordering::Relaxed);
}