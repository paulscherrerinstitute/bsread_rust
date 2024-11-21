use super::*;
use core::result::Result;
use std::collections::HashMap;
use std::convert::TryFrom;
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
            println!("{:?}", channel_data.getValue());
        },
        Err(e) =>{
            println!("{:?}", e);
        }
    }
}


fn on_message(message : &BsMessage)->(){
    //println!("------");
    //println!("{:?}", message.get_htype());
    //println!("{:?}", message.get_dh_compression());
    //println!("{:?}", message.get_hash());
    println!("{:?}", message.get_id());
    //println!("{:?}", message.get_timestamp());

    //println!("Main Header");
    //println!("{:?}", message.get_main_header());
    //println!("Data Header");
    //println!("{:?}", message.get_data_header());
    //println!("{:?}", message.get_data_header().keys());
    let mut channel_names = Vec::new();
    for channel in message.get_channels(){
        let config = channel.getConfig();
        //println!("{:?} {:?} {:?} {:?} {:?}", config.get_name(), config.get_type(), config.get_shape(), config.get_elements(), config.get_compression());
        channel_names.push( config.get_name());
    }
    println!("{} {:?}", channel_names.len(), channel_names);

    let data = message.get_data();
    for (key, value) in data {
        //println!("{}", key);
        //print_channel_data(value);
    }
    //let v = data.get("UInt8Waveform");
    //println!("{:?}", &v.unwrap().unwrap().value);

}




const MESSAGES: u32 = 4;

#[test]
fn single() -> Result<(), Box<dyn std::error::Error>> {
    let bsread = crate::Bsread::new().unwrap();
    let mut rec = bsread.receiver(Some(vec!["tcp://127.0.0.1:9999"]), zmq::SUB)?;
    let handle = rec.listen(on_message, Some(MESSAGES));
    Ok(())
}

#[test]
fn multi() -> Result<(), Box<dyn std::error::Error>> {
    let bsread = crate::Bsread::new().unwrap();
    let mut rec = bsread.receiver(Some(vec!["tcp://localhost:5554", "tcp://127.0.0.1:9999"]), zmq::SUB)?;
    let handle = rec.listen(on_message, Some(MESSAGES));
    Ok(())
}


#[test]
fn dynamic() -> Result<(), Box<dyn std::error::Error>> {
    let bsread = crate::Bsread::new().unwrap();
    let mut rec = bsread.receiver(None, zmq::SUB)?;
    rec.connect("tcp://127.0.0.1:9999");
    rec.connect("tcp://localhost:5554");
    rec.listen(on_message, Some(MESSAGES));
    Ok(())
}

#[test]
fn manual() -> Result<(), Box<dyn std::error::Error>> {
    let bsread = crate::Bsread::new().unwrap();
    let mut rec = bsread.receiver(None, zmq::SUB)?;
    rec.connect("tcp://127.0.0.1:9999");
    let message = rec.receive(None).unwrap();
    on_message(&message);
    Ok(())
}


#[test]
fn threaded() -> Result<(), Box<dyn std::error::Error>> {
    let bsread = crate::Bsread::new().unwrap();
    let mut rec = bsread.receiver(Some(vec!["tcp://127.0.0.1:9999"]), zmq::SUB)?;
    let handle = rec.fork(on_message, Some(MESSAGES));
    Ok(())
}


#[test]
fn interrupting() -> Result<(), Box<dyn std::error::Error>> {
    let bsread = crate::Bsread::new().unwrap();
    let mut rec = bsread.receiver(Some(vec!["tcp://127.0.0.1:9999"]), zmq::SUB)?;
    let handle = rec.fork(on_message, Some(MESSAGES));
    thread::sleep(Duration::from_millis(10));
    bsread.interrupt();
    println! ("{}", bsread.is_interrupted());
    rec.join(handle);
    Ok(())
}
