use crate::*;
use crate::compression::*;
use std::{cmp, thread};
use std::io::Cursor;
use std::time::Duration;
use byteorder::{BigEndian, WriteBytesExt};
use rand::Rng;


const PRINT_ARRAY_MAX_SIZE: usize = 10;
const PRINT_HEADER: bool = true;
const PRINT_ID: bool = true;
const PRINT_ATTRS: bool = false;
const PRINT_MAIN_HEADER: bool = false;
const PRINT_DATA_HEADER: bool = false;
const PRINT_META_DATA: bool = false;
const PRINT_DATA: bool = false;

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

const MESSAGES: u32 = 10;
const BSREADSENDER: &str = "tcp://127.0.0.1:9999";
const BSREADSENDER_COMPRESSED: &str = "tcp://127.0.0.1:8888";
const PIPELINES: [&str;2] = ["tcp://localhost:5554", "tcp://localhost:5555"];
const CHANNEL_NAMES: [&str;2] = ["SINEG01-DBPM340:X1", "SINEG01-DBPM340:Y1"];
const SOCKET_TYPE:SocketType=  zmq::PULL;

#[test]
fn single() ->  IOResult<()> {
    let bsread = crate::Bsread::new().unwrap();
    let mut rec = bsread.receiver(Some(vec![BSREADSENDER]), SOCKET_TYPE)?;
    rec.listen(on_message, Some(MESSAGES))?;
    print_stats_rec(&rec);
    Ok(())
}

#[test]
fn pipeline() ->  IOResult<()> {
    let bsread = crate::Bsread::new().unwrap();
    let mut rec = bsread.receiver(Some(PIPELINES.to_vec()), SOCKET_TYPE)?;
    rec.listen(on_message, Some(MESSAGES))?;
    print_stats_rec(&rec);
    Ok(())
}

#[test]
fn multi() -> IOResult<()> {
    let bsread = crate::Bsread::new().unwrap();
    let mut rec = bsread.receiver(Some(vec![BSREADSENDER, BSREADSENDER_COMPRESSED]), SOCKET_TYPE)?;
    //rec.set_header_buffer_size(0);
    rec.listen(on_message, Some(MESSAGES))?;
    print_stats_rec(&rec);
    Ok(())
}


#[test]
fn dynamic() ->  IOResult<()> {
    let bsread = crate::Bsread::new().unwrap();
    let mut rec = bsread.receiver(None, SOCKET_TYPE)?;
    rec.connect(BSREADSENDER)?;
    rec.connect(PIPELINES[0])?;
    rec.connect(PIPELINES[1])?;
    rec.listen(on_message, Some(MESSAGES))?;
    print_stats_rec(&rec);
    Ok(())
}

#[test]
fn manual() -> IOResult<()> {
    let bsread = crate::Bsread::new().unwrap();
    let mut rec = bsread.receiver(None, SOCKET_TYPE)?;
    match rec.connect(BSREADSENDER) {
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
    let bsread = crate::Bsread::new().unwrap();
    let mut rec = bsread.receiver(Some(vec![BSREADSENDER]), SOCKET_TYPE)?;
    rec.fork(on_message, Some(MESSAGES));
    let r = rec.join();
    println!("{:?}", r);
    print_stats_rec(&rec);
    Ok(())
}


#[test]
fn interrupting() ->  IOResult<()> {
    let bsread = crate::Bsread::new().unwrap();
    let mut rec = bsread.receiver(Some(vec![BSREADSENDER]), SOCKET_TYPE)?;
    rec.fork(on_message, None);
    thread::sleep(Duration::from_millis(50));
    let ret = rec.stop();
    println!("Stop result: {:?}", ret);
    println!("Receiver is interrupted: {:?}", rec.is_interrupted());
    println!("Context is interrupted: {:?}", bsread.is_interrupted());
    print_stats_rec(&rec);
    Ok(())
}

#[test]
fn joining() ->  IOResult<()> {
    let bsread = crate::Bsread::new().unwrap();
    let mut rec = bsread.receiver(Some(vec![BSREADSENDER]), SOCKET_TYPE)?;
    rec.fork(on_message, None);
    thread::sleep(Duration::from_millis(50));
    bsread.interrupt();
    let ret = rec.join();
    println!("Join result: {:?}", ret);
    println!("Receiver is interrupted: {:?}", rec.is_interrupted());
    println!("Context is interrupted: {:?}", bsread.is_interrupted());
    print_stats_rec(&rec);
    Ok(())
}

#[test]
fn compression() ->  IOResult<()> {
    let bsread = Bsread::new().unwrap();
    let mut rec = bsread.receiver(Some(vec![BSREADSENDER_COMPRESSED]), SOCKET_TYPE)?;
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
    let bsread = Bsread::new().unwrap();
    let mut rec = bsread.receiver(None, SOCKET_TYPE)?;
    match rec.connect(PIPELINES[0]) {
        Ok(_) => {}
        Err(err) => { println!("Connection error: {}", err) }
    }
    let message = rec.receive()?;
    print_message(&message);
    let v = message.get_value("y_fit_gauss_function").unwrap();
    println!("{:?}", v.as_str_array());
    println!("{:?}", v.as_num_array::<i32>());
    println!("{:?}", v.as_num_array::<f32>());
    println!("{:?}", v.as_num_array::<f64>());
    //.unwrap().get_value();
    Ok(())
}


#[test]
fn booleans() -> IOResult<()> {
    let bsread = Bsread::new().unwrap();
    let mut rec = bsread.receiver(None, SOCKET_TYPE)?;
    match rec.connect(BSREADSENDER) {
        Ok(_) => {}
        Err(err) => { println!("Connection error: {}", err) }
    }
    let message = rec.receive()?;
    print_message(&message);
    let v = message.get_value("BoolWaveform").unwrap();
    println!("{:?}", v.as_str_array());
    println!("{:?}", v.as_num_array::<i32>());

    let v = message.get_value("BoolScalar").unwrap();
    println!("{:?}", v.as_str());
    println!("{:?}", v.as_num::<i32>());

    Ok(())
}

#[test]
fn buffered() -> IOResult<()> {
    let bsread = crate::Bsread::new().unwrap();
    let mut rec = bsread.receiver(Some(vec![BSREADSENDER]), SOCKET_TYPE)?;
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
    let bsread = crate::Bsread::new().unwrap();
    let mut pool = bsread.pool_auto(vec![BSREADSENDER, BSREADSENDER_COMPRESSED], SOCKET_TYPE, 2)?;
    pool.start_sync(on_message)?;
    thread::sleep(Duration::from_millis(100));
    pool.stop()?;
    print_stats_pool(&pool);
    Ok(())
}

#[test]
fn pool_manual() -> IOResult<()> {
    let bsread = crate::Bsread::new().unwrap();
    let mut pool = bsread.pool_manual(vec![vec![BSREADSENDER,], vec![BSREADSENDER_COMPRESSED]], SOCKET_TYPE)?;
    pool.start_sync(on_message)?;
    thread::sleep(Duration::from_millis(100));
    pool.stop()?;
    print_stats_pool(&pool);
    Ok(())
}

#[test]
fn pool_buffered() -> IOResult<()> {
    let bsread = crate::Bsread::new().unwrap();
    let mut pool = bsread.pool_auto(vec![BSREADSENDER, BSREADSENDER_COMPRESSED], SOCKET_TYPE, 2)?;
    pool.start_buffered(on_message,100)?;
    thread::sleep(Duration::from_millis(100));
    pool.stop()?;
    print_stats_pool(&pool);
    Ok(())
}

#[test]
fn dispatcher() -> IOResult<()> {
    let bsread = crate::Bsread::new().unwrap();
    let channels = vec![
        dispatcher::ChannelDescription::of(CHANNEL_NAMES[0]),
        dispatcher::ChannelDescription::of(CHANNEL_NAMES[1]),
    ];
    let stream = dispatcher::request_stream(channels, None, None, true, false)?;
    let mut rec = bsread.receiver(Some(vec![stream.get_endpoint()]), zmq::SUB)?;
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
    for i in 0..data.len(){
        (&mut buffer[i*4..(i+1)*4]).write_u32::<BigEndian>(data[i]).unwrap();
    }
    let compressed = compress_bitshuffle_lz4(&buffer, elem_size)?;
    let decompressed = decompress_bitshuffle_lz4(&compressed, elem_size)?;
    assert_eq!(&buffer, &decompressed);
    let mut out = vec![0u32; 128];
    let mut cursor = Cursor::new(&decompressed);
    reader::READER_ABU32(& mut cursor, out.as_mut_slice())?;
    assert_eq!(&data, &out);
    Ok(())
}