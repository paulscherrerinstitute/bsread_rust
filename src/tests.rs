use crate::*;
use crate::compression::*;
use crate::channel::new_channel;
use std::{cmp, thread};
use std::io::Cursor;
use std::time::Duration;
use indexmap::IndexMap;
use byteorder::{BigEndian, WriteBytesExt};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering, AtomicI32};
use rand::Rng;
use crate::debug::*;
use crate::reader::READER_ABOOL;
use crate::sender::Sender;
use crate::writer::WRITER_ABOOL;
use lazy_static::lazy_static;
use crate::dispatcher::ChannelDescription;

const PRINT_ARRAY_MAX_SIZE: usize = 10;
const PRINT_HEADER: bool = true;
const PRINT_ID: bool = true;
const PRINT_ATTRS: bool = false;
const PRINT_MAIN_HEADER: bool = false;
const PRINT_DATA_HEADER: bool = false;
const PRINT_META_DATA: bool = false;
const PRINT_DATA: bool = true;

pub fn print_message(message: &Message){
    debug::print_message( message, PRINT_ARRAY_MAX_SIZE, PRINT_HEADER, PRINT_ID, PRINT_ATTRS,
                         PRINT_MAIN_HEADER, PRINT_DATA_HEADER, PRINT_META_DATA, PRINT_DATA);
}

pub fn print_stats_rec(receiver: &Receiver){
    println!("");
    debug::print_stats_rec(receiver);
}

pub fn print_stats_pool(pool: &Pool){
    println!("");
    debug::print_stats_pool(pool);
}


fn on_message(message: Message) -> () {
    print_message(&message);
}

const MESSAGES: u32 = 1;
const SENDER_PUB: &str = "tcp://127.0.0.1:10300";
const SENDER_COMPRESSED: &str = "tcp://127.0.0.1:10301";
const SENDER_PUSH: &str = "tcp://127.0.0.1:10302";
const DISPATCHER_CHANNEL_NAMES: [&str;0] = []; //[&str;2] = ["SINEG01-DBPM340:X1", "SINEG01-DBPM340:Y1"];


lazy_static! {
    static ref STARTED_SERVERS: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    static ref RUNNING_TESTS: Arc<AtomicI32> = Arc::new(AtomicI32::new(0));
}
struct TestEnvironment {
    bsread:Bsread
}
impl TestEnvironment {
    fn new() -> IOResult<Self> {
        let running_tests = RUNNING_TESTS.fetch_add(1, Ordering::SeqCst);
        println!("Setting up test environment [{}]", running_tests);
        //env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
        if !STARTED_SERVERS.load(Ordering::Relaxed) {
            println!("Starting senders...");
            start_sender(10300, SocketType::PUB, 1000, None, None)?;
            start_sender(10301, SocketType::PUB, 1000, None, Some("bitshuffle_lz4".to_string()))?;
            start_sender(10302, SocketType::PUSH, 1000, Some(false), None)?;
            STARTED_SERVERS.store(true, Ordering::Relaxed);
        }
        let bsread = Bsread::new()?;
        Ok(Self {bsread})
    }
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        let running_tests = RUNNING_TESTS.fetch_sub(1, Ordering::SeqCst);
        println!("Cleaning up test environment [{}]", running_tests);
        if running_tests<=0 {
            println!("Stopping senders...");
            stop_senders();
        }
    }
}


#[test]
fn receiver_sub() ->  IOResult<()> {
    let env = TestEnvironment::new()?;
    let mut rec = env.bsread.receiver(Some(vec![SENDER_PUB]), SocketType::SUB)?;
    rec.listen(on_message, Some(MESSAGES))?;
    print_stats_rec(&rec);
    Ok(())
}


#[test]
fn receiver_pull() ->  IOResult<()> {
    let env = TestEnvironment::new()?;
    let mut rec = env.bsread.receiver(Some(vec![SENDER_PUSH]),  SocketType::PULL)?;
    rec.listen(on_message, Some(MESSAGES))?;
    print_stats_rec(&rec);
    Ok(())
}

#[test]
fn multi() -> IOResult<()> {
    let env = TestEnvironment::new()?;
    let mut rec = env.bsread.receiver(Some(vec![SENDER_PUB, SENDER_COMPRESSED]), SocketType::SUB)?;
    //rec.set_header_buffer_size(0);
    rec.listen(on_message, Some(MESSAGES))?;
    print_stats_rec(&rec);
    Ok(())
}


#[test]
fn dynamic() ->  IOResult<()> {
    let env = TestEnvironment::new()?;
    let mut rec = env.bsread.receiver(None, SocketType::SUB)?;
    rec.connect(SENDER_PUB)?;
    rec.connect(SENDER_COMPRESSED)?;
    rec.listen(on_message, Some(MESSAGES))?;
    print_stats_rec(&rec);
    Ok(())
}

#[test]
fn manual() -> IOResult<()> {
    let env = TestEnvironment::new()?;
    let mut rec = env.bsread.receiver(None, SocketType::SUB)?;
    match rec.connect(SENDER_PUB) {
        Ok(_) => {}
        Err(err) => { println!("Connection error: {}", err) }
    }
    let message = rec.receive()?;
    print_message(&message);
    print_stats_rec(&rec);
    Ok(())
}


#[test]
fn threaded() -> IOResult<()> {
    let env = TestEnvironment::new()?;
    let mut rec = env.bsread.receiver(Some(vec![SENDER_PUB]), SocketType::SUB)?;
    rec.fork(on_message, Some(MESSAGES));
    let r = rec.join();
    println!("{:?}", r);
    print_stats_rec(&rec);
    Ok(())
}


#[test]
fn interrupting() ->  IOResult<()> {
    let env = TestEnvironment::new()?;
    let mut rec = env.bsread.receiver(Some(vec![SENDER_PUB]), SocketType::SUB)?;
    rec.fork(on_message, None);
    thread::sleep(Duration::from_millis(50));
    let ret = rec.stop();
    println!("Stop result: {:?}", ret);
    println!("Receiver is interrupted: {:?}", rec.is_interrupted());
    println!("Context is interrupted: {:?}", env.bsread.is_interrupted());
    print_stats_rec(&rec);
    Ok(())
}

#[test]
fn joining() ->  IOResult<()> {
    let env = TestEnvironment::new()?;
    let mut rec = env.bsread.receiver(Some(vec![SENDER_PUB]), SocketType::SUB)?;
    rec.fork(on_message, None);
    thread::sleep(Duration::from_millis(50));
    env.bsread.interrupt();
    let ret = rec.join();
    println!("Join result: {:?}", ret);
    println!("Receiver is interrupted: {:?}", rec.is_interrupted());
    println!("Context is interrupted: {:?}", env.bsread.is_interrupted());
    print_stats_rec(&rec);
    Ok(())
}

#[test]
fn compressed() ->  IOResult<()> {
    let env = TestEnvironment::new()?;
    let mut rec = env.bsread.receiver(Some(vec![SENDER_COMPRESSED]), SocketType::SUB)?;
    rec.listen(on_message, Some(MESSAGES))?;
    print_stats_rec(&rec);
    Ok(())
}




#[test]
fn bitshuffle() -> IOResult<()> {
    let elem_size = 4; // 4 bytes per element
    let array: [u8; 292] = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x01, 0x14, 0xFF, 0xF4, 0xD3, 0x2F, 0x3F, 0x6B, 0x0F, 0x8C, 0x92, 0x6E, 0xAC, 0x22, 0xBF, 0x51, 0x16, 0x37, 0x8B, 0xCB, 0xC4, 0x0A, 0xB3, 0xA0, 0xA2, 0xC2, 0x14, 0x04, 0x21, 0x12, 0xBF, 0x45, 0x7B, 0x83, 0x82, 0xED, 0x65, 0x5E, 0x6C, 0x76, 0x88, 0x5C, 0xBF, 0x84, 0x6E, 0xA1, 0xB6, 0x36, 0x66, 0x35, 0x94, 0x09, 0x2E, 0xED, 0xED, 0x72, 0x89, 0x05, 0xD9, 0x7F, 0xBE, 0x98, 0x73, 0x38, 0x05, 0xD3, 0xE5, 0x6C, 0x9D, 0x83, 0x64, 0xE6, 0x60, 0xC2, 0xA2, 0x63, 0x8D, 0x48, 0x5E, 0xBC, 0x4F, 0xE5, 0xCF, 0x87, 0xA5, 0xC8, 0xF6, 0x9D, 0x4B, 0x04, 0x98, 0xA8, 0x89, 0x43, 0x3B, 0x23, 0x29, 0x66, 0xA0, 0x77, 0x36, 0x5F, 0xA2, 0xB1, 0xB1, 0x2C, 0xD4, 0xB7, 0xC1, 0xE9, 0x4F, 0xAE, 0x60, 0x4A, 0x02, 0xAC, 0xF3, 0x5A, 0x5D, 0x10, 0x8D, 0x78, 0x52, 0x74, 0x7D, 0xE0, 0xFB, 0x5C, 0x6B, 0xDD, 0x92, 0xCB, 0x13, 0x0A, 0xF3, 0x1F, 0x2F, 0xAB, 0xEC, 0x5E, 0x6E, 0x3D, 0xD7, 0xDF, 0x54, 0xE4, 0x3A, 0xC6, 0xD0, 0x42, 0x5D, 0xAB, 0x76, 0x37, 0x20, 0xF2, 0x02, 0xF0, 0xF7, 0xB0, 0xF1, 0xFD, 0xC7, 0x82, 0x19, 0xD4, 0x6F, 0x13, 0x5E, 0xDD, 0x36, 0x85, 0xB0, 0x34, 0x75, 0x9C, 0x9C, 0x4E, 0xD3, 0x97, 0x8B, 0xFB, 0xE9, 0xD1, 0x5E, 0xBF, 0xCF, 0xFC, 0xB6, 0x68, 0xD3, 0xE1, 0x52, 0x52, 0xD1, 0xA8, 0xFD, 0x52, 0xF2, 0x47, 0x58, 0x60, 0x34, 0x79, 0xB6, 0xAD, 0x7D, 0x09, 0xCA, 0xDA, 0x62, 0x8F, 0x72, 0x90, 0x63, 0xC9, 0xDD, 0xCC, 0x5C, 0xF0, 0x9D, 0x76, 0x75, 0x8D, 0xF3, 0x45, 0x4F, 0x77, 0x75, 0x2C, 0xFD, 0xCC, 0xF2, 0x5D, 0xE0, 0xCA, 0x99, 0x00, 0x3C, 0x2D, 0x95, 0xFA, 0xE0, 0xC6, 0xC8, 0xBF, 0x81, 0xE5, 0x2E, 0x8B, 0xEF, 0x70, 0x5B, 0x69, 0x77, 0x35, 0xBE, 0x31, 0xA2, 0xB3, 0x00, 0x00, 0x00, 0x03, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xEB, 0x50, 0x00, 0x00, 0x00, 0x00, 0x00];
    let outout = decompress_bitshuffle_lz4(&array, elem_size)?;
    println!("{:?}", outout);
    Ok(())
}

#[test]
fn conversion() -> IOResult<()> {
    let env = TestEnvironment::new()?;
    let mut rec = env.bsread.receiver(None, SocketType::SUB)?;
    match rec.connect(SENDER_PUB) {
        Ok(_) => {}
        Err(err) => { println!("Connection error: {}", err) }
    }
    let message = rec.receive()?;
    print_message(&message);
    let v = message.get_value("AF32").unwrap();
    println!("{:?}", v.as_str_array());
    println!("{:?}", v.as_num_array::<i32>());
    println!("{:?}", v.as_num_array::<f32>());
    println!("{:?}", v.as_num_array::<f64>());
    //.unwrap().get_value();
    Ok(())
}


#[test]
fn booleans() -> IOResult<()> {
    let env = TestEnvironment::new()?;
    let mut rec = env.bsread.receiver(None,  SocketType::SUB)?;
    match rec.connect(SENDER_PUB) {
        Ok(_) => {}
        Err(err) => { println!("Connection error: {}", err) }
    }
    let message = rec.receive()?;
    print_message(&message);
    let v = message.get_value("ABOOL").unwrap();
    println!("{:?}", v.as_str_array());
    println!("{:?}", v.as_num_array::<i32>());

    let v = message.get_value("BOOL").unwrap();
    println!("{:?}", v.as_str());
    println!("{:?}", v.as_num::<i32>());

    Ok(())
}

#[test]
fn buffered() -> IOResult<()> {
    let env = TestEnvironment::new()?;
    let mut rec = env.bsread.receiver(Some(vec![SENDER_PUB]),  SocketType::SUB)?;
    rec.start(100)?;
    for _ in 0..MESSAGES {
        match rec.wait(100) {
            Ok(msg) => {print_message(&msg)}
            Err(e) => {println!("{}",e)}
        }
    }
    rec.stop()?;
    print_stats_rec(&rec);
    Ok(())
}

#[test]
fn limited_hashmap() {
    let mut limited_map = crate::utils::LimitedHashMap::new(3);

    limited_map.insert("a", 1);
    limited_map.insert("b", 2);
    limited_map.insert("c", 3);
    limited_map.insert("d", 1);
    limited_map.insert("e", 2);
    limited_map.insert("f", 3);
    limited_map.insert("a", 5);
    limited_map.insert("d", 4); // "a" will be dropped

    println!("{:?}", limited_map.get(&"a")); // None
    println!("{:?}", limited_map.get(&"b")); // Some(2)
    println!("{:?}", limited_map.get(&"c")); // Some(3)
    println!("{:?}", limited_map.get(&"d")); // Some(4)

    println!("{:?}", limited_map.remove(&"a")); // Some(4)
    println!("---"); // Some(4)
    limited_map.insert("a", 6);
    println!("---"); // Some(4)
    println!("{:?}", limited_map.remove(&"a")); // Some(4)

    println!("{:?}", limited_map.is_void()); // Some(4)

    limited_map = crate::utils::LimitedHashMap::new(1);
    println!("{:?}", limited_map.is_void()); // Some(4)
    limited_map = crate::utils::LimitedHashMap::new(0);
    println!("{:?}", limited_map.is_void()); // Some(4)
    limited_map = crate::utils::LimitedHashMap::void();
    println!("{:?}", limited_map.is_void()); // Some(4)
}

#[test]
fn pool_auto() -> IOResult<()> {
    let env = TestEnvironment::new()?;
    let mut pool = env.bsread.pool_auto(vec![SENDER_PUB, SENDER_COMPRESSED],  SocketType::SUB, 2)?;
    pool.start_sync(on_message)?;
    thread::sleep(Duration::from_millis(100));
    pool.stop()?;
    print_stats_pool(&pool);
    Ok(())
}

#[test]
fn pool_manual() -> IOResult<()> {
    let env = TestEnvironment::new()?;
    let mut pool = env.bsread.pool_manual(vec![vec![SENDER_PUB,], vec![SENDER_COMPRESSED]],  SocketType::SUB)?;
    pool.start_sync(on_message)?;
    thread::sleep(Duration::from_millis(100));
    pool.stop()?;
    print_stats_pool(&pool);
    Ok(())
}

#[test]
fn pool_buffered() -> IOResult<()> {
    let env = TestEnvironment::new()?;
    let mut pool = env.bsread.pool_auto(vec![SENDER_PUB, SENDER_COMPRESSED],  SocketType::SUB, 2)?;
    pool.start_buffered(on_message,100)?;
    thread::sleep(Duration::from_millis(100));
    pool.stop()?;
    print_stats_pool(&pool);
    Ok(())
}

#[test]
fn dispatcher() -> IOResult<()> {
    if DISPATCHER_CHANNEL_NAMES.len()==0{
        return Ok(());
    }
    let bsread = Bsread::new().unwrap();
    let mut channels :Vec<ChannelDescription> = Vec::new();
    for channel in DISPATCHER_CHANNEL_NAMES{
        channels.push(ChannelDescription::of(channel));
    }
    let stream = dispatcher::request_stream(channels, None, None, true, false)?;
    let mut rec = bsread.receiver(Some(vec![stream.get_endpoint()]), SocketType::SUB)?;
    rec.listen(on_message, Some(MESSAGES))?;

    /*
    rec.start(100)?;
    for _ in 0..MESSAGES {
        match rec.wait(200) {
            Ok(msg) => {print_message(&msg)}
            Err(e) => {println!("{}",e)}
        }
    }
    */
    print_stats_rec(&rec);

    Ok(())
}

#[test]
fn lz4() ->  IOResult<()> {
    let mut buffer = vec![0u8; 1024]; // Allocate 1024 bytes
    rand::thread_rng().fill(&mut buffer[..]); // Fill with random data
    let compressed = compress_lz4(&buffer)?;
    let decompressed = decompress_lz4(&compressed)?;
    assert_eq!(&buffer, &decompressed);
    Ok(())
}


#[test]
fn bitshuffle_lz4() ->  IOResult<()> {
    let elem_size = 1;
    let mut buffer = vec![0u8; 1024]; // Allocate 1024 bytes
    rand::thread_rng().fill(&mut buffer[..]); // Fill with random data
    let compressed = compress_bitshuffle_lz4(&buffer, elem_size)?;
    let decompressed = decompress_bitshuffle_lz4(&compressed, elem_size)?;
    assert_eq!(&buffer, &decompressed);


    let elem_size = 4;
    let mut data = vec![0u32; 128];
    rand::thread_rng().fill(&mut data[..]); // Fill with random data
    let mut buffer = vec![0u8; elem_size*data.len()];
    let mut cursor = Cursor::new(&mut buffer);
    writer::WRITER_ABU32(& mut cursor, data.as_slice())?;
    //for i in 0..data.len(){
    //    (&mut buffer[i*4..(i+1)*4]).write_u32::<BigEndian>(data[i]).unwrap();
    //}
    let compressed = compress_bitshuffle_lz4(&buffer, elem_size)?;
    let decompressed = decompress_bitshuffle_lz4(&compressed, elem_size)?;
    assert_eq!(&buffer, &decompressed);
    let mut out = vec![0u32; 128];
    let mut cursor = Cursor::new(&decompressed);
    reader::READER_ABU32(& mut cursor, out.as_mut_slice())?;
    assert_eq!(&data, &out);
    Ok(())
}

#[test]
fn serializer() ->  IOResult<()> {
    let mut buf = vec![0u8; 2000];
    let values = create_test_values(100, 100);
    for value in values {
        for little_endian in  vec!(true, false) {
            let shape= if value.is_array() {Some(vec![value.get_size()as u32])} else {None};
            let ch = new_channel(value.get_type().to_string(), value.get_type().to_string(), shape, little_endian, "none".to_string())?;
            let mut cursor = Cursor::new(&mut buf);
            ch.write(&mut cursor, &value)?;
            let mut cursor = Cursor::new(&buf);
            let ret = ch.read(&mut cursor)?;
            assert_eq!(&value, &ret);
        }
    }
    Ok(())

}

#[test]
fn sender() ->  IOResult<()> {
    let bsread = Bsread::new().unwrap();
    let mut sender = Sender::new(&bsread,  SocketType::PUB, 10300, None, None, None, None, None)?;
    let value = Value::U8(100);
    let little_endian = true;
    let shape= if value.is_array() {Some(vec![value.get_size()as u32])} else {None};
    let ch = new_channel(value.get_type().to_string(), value.get_type().to_string(), shape, little_endian, "none".to_string())?;

    let channels = vec![ch];
    let channel_data = ChannelData::new(value,(0,0));
    let data = vec![Some(&channel_data)];
    sender.start()?;
    sender.create_data_header(&channels)?;
    sender.send(&channels, &data)?;
    sender.stop();
    Ok(())
}

#[test]
fn sender_receiver_pub() ->  IOResult<()> {
    let env = TestEnvironment::new()?;
    let mut rec = env.bsread.receiver(Some(vec![SENDER_PUB]), SocketType::SUB)?;
    rec.listen(on_message, Some(1))?;
    //thread::sleep(Duration::from_millis(1000));
    print_stats_rec(&rec);
    Ok(())
}

#[test]
fn sender_receiver_push() ->  IOResult<()> {
    let env = TestEnvironment::new()?;
    let mut rec = env.bsread.receiver(Some(vec![SENDER_PUSH]), SocketType::PULL)?;
    rec.listen(on_message, Some(1))?;
    print_stats_rec(&rec);
    Ok(())
}

#[test]
fn sender_receiver_compressed() ->  IOResult<()> {
    let env = TestEnvironment::new()?;
    let mut rec = env.bsread.receiver(Some(vec![SENDER_COMPRESSED]), SocketType::SUB)?;
    rec.listen(on_message, Some(1))?;
    print_stats_rec(&rec);
    Ok(())
}


#[test]
fn logs() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::error!("This is an error message.");
    log::warn!("This is a warning.");
    log::info!("This is an info message.");
    log::debug!("This is a debug message.");
    log::trace!("This is a trace message.");
}