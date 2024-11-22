use super::*;
use core::result::Result;
use std::collections::HashMap;
use std::io;
use std::thread;
use std::time::Duration;


fn print_res_map<T: std::fmt::Debug,U: std::fmt::Debug> (res: & io::Result<HashMap<T,U>>){
    match &res {
        Ok(ok) => {
            println!("{:?}", ok);
        },
        Err(e) =>{
            println!("{:?}", e);
        }
    }

}

fn print_channel_data (channel_data:& io::Result <ChannelData> ){
    match &channel_data {
        Ok(channel_data) => {
            println!("{:?}", channel_data.get_value());
        },
        Err(e) =>{
            println!("{:?}", e);
        }
    }
}


fn on_message(message : &BsMessage)->(){
    println!("------");
    println!("{:?}", message.get_id());
    /*
    println!("{:?}", message.get_htype());
    println!("{:?}", message.get_dh_compression());
    println!("{:?}", message.get_hash());
    println!("{:?}", message.get_timestamp());

    println!("Main Header");
    println!("{:?}", message.get_main_header());
    println!("Data Header");
    println!("{:?}", message.get_data_header());
    println!("{:?}", message.get_data_header().keys());
    let mut channel_names = Vec::new();
    for channel in message.get_channels(){
        let config = channel.get_config();
        println!("{:?} {:?} {:?} {:?} {:?}", config.get_name(), config.get_type(), config.get_shape(), config.get_elements(), config.get_compression());
        channel_names.push( config.get_name());
    }
    println!("{} {:?}", channel_names.len(), channel_names);

    let data = message.get_data();
    for (key, value) in data {
        println!("{}", key);
        //print_channel_data(value);
    }
    */
}




const MESSAGES: u32 = 10;
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
    match rec.connect(BSREADSENDER){
        Ok(_) => {}
        Err(err) => {println!("Connection error: {}", err)}
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
    println! ("{}", bsread.is_interrupted());
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