use super::*;
use super::compression::decompress_bitshuffle_lz4;
use super::utils::LimitedDebugVec;
use core::result::Result;
use std::collections::HashMap;
use std::io;
use std::thread;
use std::time::Duration;


fn print_res_map<T: std::fmt::Debug, U: std::fmt::Debug>(res: &io::Result<HashMap<T, U>>) {
    match &res {
        Ok(ok) => {
            println!("{:?}", ok);
        }
        Err(e) => {
            println!("{:?}", e);
        }
    }
}

fn print_channel_data(channel_data: &io::Result<ChannelData>, prefix:&str, max_elements: usize) {
    match &channel_data {
        Ok(channel_data) => {
            //println!("{}{:?}", prefix, channel_data.get_value());
            //println!("{}{:?}", prefix, LimitedDebug { data: channel_data.get_value().as_slice(), limit: 5});
            let value = channel_data.get_value();
            if value.is_array() {
                println!("{}{:?}", prefix, LimitedDebugVec { data: value.as_str_array().unwrap(), limit: max_elements });
            } else {
                println!("{}{:?}", prefix, channel_data.get_value());
            }
        }
        Err(e) => {
            println!("{}{:?}", prefix, e);
        }
    }
}



const PRINT_HEADER: bool = true;
const PRINT_ID: bool = true;
const PRINT_ATTRS: bool = true;
const PRINT_MAIN_HEADER: bool = true;
const PRINT_DATA_HEADER: bool = true;
const PRINT_META_DATA: bool = true;
const PRINT_DATA: bool = true;

const PRINT_ARRAY_MAX_SIZE: usize = 10;


fn on_message(message: &BsMessage) -> () {
    if PRINT_ID {
        println!("{}", "-".repeat(80));
    }
    if PRINT_ID {
        println!("ID = {:?}", message.get_id());
    }
    if (PRINT_ATTRS) {
        println!("Attrs:");
        println!("\thtype: {:?}", message.get_htype());
        println!("\tdh_compression: {:?}", message.get_dh_compression());
        println!("\thash: {:?}", message.get_hash());
        println!("\ttimestamp: {:?}", message.get_timestamp());
    }
    if (PRINT_MAIN_HEADER) {
        println!("Main Header:");
        println!("\t {:?}", message.get_main_header());
    }
    if (PRINT_DATA_HEADER) {
        println!("Data Header:");
        println!("\t {:?}", message.get_data_header());
    }
    if (PRINT_META_DATA) {
        let mut channel_names = Vec::new();
        println!("Channel Metadata:");
        for channel in message.get_channels() {
            let config = channel.get_config();
            let shape : Vec<i32> = config.get_shape().unwrap_or(Vec::new());
            println!("\t{} {} {:?} {} {}", config.get_name(), config.get_type(), shape, config.get_elements(), config.get_compression());
            channel_names.push(config.get_name());
        }
    }
    if (PRINT_DATA) {
        println!("Channel Data:");
        let data = message.get_data();
        for (key, value) in data {
            //println!("{}", key);
            print_channel_data(value, format!("\t{}: ", key).as_str(), PRINT_ARRAY_MAX_SIZE);
        }
    }
}


const MESSAGES: u32 = 1;
const BSREADSENDER: &str = "tcp://127.0.0.1:9999";
const BSREADSENDER_COMPRESSED: &str = "tcp://127.0.0.1:9999";
const PIPELINE: &str = "tcp://localhost:5554";

#[test]
fn single() -> Result<(), Box<dyn std::error::Error>> {
    let bsread = crate::Bsread::new().unwrap();
    let mut rec = bsread.receiver(Some(vec![BSREADSENDER]), zmq::SUB)?;
    rec.listen(on_message, Some(MESSAGES))?;
    Ok(())
}

#[test]
fn pipeline() -> Result<(), Box<dyn std::error::Error>> {
    let bsread = crate::Bsread::new().unwrap();
    let mut rec = bsread.receiver(Some(vec![PIPELINE]), zmq::SUB)?;
    rec.listen(on_message, Some(MESSAGES))?;
    Ok(())
}

#[test]
fn multi() -> Result<(), Box<dyn std::error::Error>> {
    let bsread = crate::Bsread::new().unwrap();
    let mut rec = bsread.receiver(Some(vec![PIPELINE, BSREADSENDER]), zmq::SUB)?;
    rec.listen(on_message, Some(MESSAGES))?;
    Ok(())
}


#[test]
fn dynamic() -> Result<(), Box<dyn std::error::Error>> {
    let bsread = crate::Bsread::new().unwrap();
    let mut rec = bsread.receiver(None, zmq::SUB)?;
    rec.connect(BSREADSENDER);
    rec.connect(PIPELINE);
    rec.listen(on_message, Some(MESSAGES))?;
    Ok(())
}

#[test]
fn manual() -> Result<(), Box<dyn std::error::Error>> {
    let bsread = crate::Bsread::new().unwrap();
    let mut rec = bsread.receiver(None, zmq::SUB)?;
    match rec.connect(BSREADSENDER) {
        Ok(_) => {}
        Err(err) => { println!("Connection error: {}", err) }
    }
    let message = rec.receive(None)?;
    on_message(&message);
    Ok(())
}


#[test]
fn threaded() -> Result<(), Box<dyn std::error::Error>> {
    let bsread = crate::Bsread::new().unwrap();
    let rec = bsread.receiver(Some(vec![BSREADSENDER]), zmq::SUB)?;
    let handle = rec.fork(on_message, Some(MESSAGES));
    let r = rec.join(handle);
    println!("{:?}", r);
    Ok(())
}


#[test]
fn interrupting() -> Result<(), Box<dyn std::error::Error>> {
    let bsread = crate::Bsread::new().unwrap();
    let rec = bsread.receiver(Some(vec![BSREADSENDER]), zmq::SUB)?;
    let handle = rec.fork(on_message, Some(MESSAGES));
    thread::sleep(Duration::from_millis(10));
    bsread.interrupt();
    println!("{}", bsread.is_interrupted());
    rec.join(handle);
    Ok(())
}
#[test]
fn compression() -> Result<(), Box<dyn std::error::Error>> {
    let bsread = crate::Bsread::new().unwrap();
    let mut rec = bsread.receiver(Some(vec![BSREADSENDER_COMPRESSED]), zmq::SUB)?;
    rec.listen(on_message, Some(MESSAGES))?;
    Ok(())
}


#[test]
fn bitshuffle() -> io::Result<()> {
    let size = 2048; // Number of elements
    let elem_size = 4; // 4 bytes per element
    let array: [u8; 292] = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x01, 0x14, 0xFF, 0xF4, 0xD3, 0x2F, 0x3F, 0x6B, 0x0F, 0x8C, 0x92, 0x6E, 0xAC, 0x22, 0xBF, 0x51, 0x16, 0x37, 0x8B, 0xCB, 0xC4, 0x0A, 0xB3, 0xA0, 0xA2, 0xC2, 0x14, 0x04, 0x21, 0x12, 0xBF, 0x45, 0x7B, 0x83, 0x82, 0xED, 0x65, 0x5E, 0x6C, 0x76, 0x88, 0x5C, 0xBF, 0x84, 0x6E, 0xA1, 0xB6, 0x36, 0x66, 0x35, 0x94, 0x09, 0x2E, 0xED, 0xED, 0x72, 0x89, 0x05, 0xD9, 0x7F, 0xBE, 0x98, 0x73, 0x38, 0x05, 0xD3, 0xE5, 0x6C, 0x9D, 0x83, 0x64, 0xE6, 0x60, 0xC2, 0xA2, 0x63, 0x8D, 0x48, 0x5E, 0xBC, 0x4F, 0xE5, 0xCF, 0x87, 0xA5, 0xC8, 0xF6, 0x9D, 0x4B, 0x04, 0x98, 0xA8, 0x89, 0x43, 0x3B, 0x23, 0x29, 0x66, 0xA0, 0x77, 0x36, 0x5F, 0xA2, 0xB1, 0xB1, 0x2C, 0xD4, 0xB7, 0xC1, 0xE9, 0x4F, 0xAE, 0x60, 0x4A, 0x02, 0xAC, 0xF3, 0x5A, 0x5D, 0x10, 0x8D, 0x78, 0x52, 0x74, 0x7D, 0xE0, 0xFB, 0x5C, 0x6B, 0xDD, 0x92, 0xCB, 0x13, 0x0A, 0xF3, 0x1F, 0x2F, 0xAB, 0xEC, 0x5E, 0x6E, 0x3D, 0xD7, 0xDF, 0x54, 0xE4, 0x3A, 0xC6, 0xD0, 0x42, 0x5D, 0xAB, 0x76, 0x37, 0x20, 0xF2, 0x02, 0xF0, 0xF7, 0xB0, 0xF1, 0xFD, 0xC7, 0x82, 0x19, 0xD4, 0x6F, 0x13, 0x5E, 0xDD, 0x36, 0x85, 0xB0, 0x34, 0x75, 0x9C, 0x9C, 0x4E, 0xD3, 0x97, 0x8B, 0xFB, 0xE9, 0xD1, 0x5E, 0xBF, 0xCF, 0xFC, 0xB6, 0x68, 0xD3, 0xE1, 0x52, 0x52, 0xD1, 0xA8, 0xFD, 0x52, 0xF2, 0x47, 0x58, 0x60, 0x34, 0x79, 0xB6, 0xAD, 0x7D, 0x09, 0xCA, 0xDA, 0x62, 0x8F, 0x72, 0x90, 0x63, 0xC9, 0xDD, 0xCC, 0x5C, 0xF0, 0x9D, 0x76, 0x75, 0x8D, 0xF3, 0x45, 0x4F, 0x77, 0x75, 0x2C, 0xFD, 0xCC, 0xF2, 0x5D, 0xE0, 0xCA, 0x99, 0x00, 0x3C, 0x2D, 0x95, 0xFA, 0xE0, 0xC6, 0xC8, 0xBF, 0x81, 0xE5, 0x2E, 0x8B, 0xEF, 0x70, 0x5B, 0x69, 0x77, 0x35, 0xBE, 0x31, 0xA2, 0xB3, 0x00, 0x00, 0x00, 0x03, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xEB, 0x50, 0x00, 0x00, 0x00, 0x00, 0x00];

    let outout = decompress_bitshuffle_lz4(&array, elem_size)?;
    println!("{:?}", outout);
    Ok(())
}

#[test]
fn conversion() -> Result<(), Box<dyn std::error::Error>> {
    let bsread = crate::Bsread::new().unwrap();
    let mut rec = bsread.receiver(None, zmq::SUB)?;
    match rec.connect(PIPELINE) {
        Ok(_) => {}
        Err(err) => { println!("Connection error: {}", err) }
    }
    let message = rec.receive(None)?;
    on_message(&message);
    let v = message.get_value("y_fit_gauss_function").unwrap();
    println!("{:?}", v.as_str_array());
    println!("{:?}", v.as_array::<i32>());
    println!("{:?}", v.as_array::<f32>());
    println!("{:?}", v.as_array::<f64>());
    //.unwrap().get_value();

    Ok(())
}


#[test]
fn booleans() -> Result<(), Box<dyn std::error::Error>> {
    let bsread = crate::Bsread::new().unwrap();
    let mut rec = bsread.receiver(None, zmq::SUB)?;
    match rec.connect(BSREADSENDER) {
        Ok(_) => {}
        Err(err) => { println!("Connection error: {}", err) }
    }
    let message = rec.receive(None)?;
    on_message(&message);
    let v = message.get_value("BoolWaveform").unwrap();
    println!("{:?}", v.as_str_array());
    println!("{:?}", v.as_array::<i32>());

    Ok(())
}
