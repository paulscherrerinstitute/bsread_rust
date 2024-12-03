use crate::IOResult;
use crate::message::{BsMessage, ChannelData};

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

pub fn print_channel_data(channel_data: &IOResult<ChannelData>, prefix:&str, max_elements: usize) {
    match &channel_data {
        Ok(channel_data) => {
            //println!("{}{:?}", prefix, channel_data.get_value());
            //println!("{}{:?}", prefix, LimitedDebug { data: channel_data.get_value().as_slice(), limit: 5});
            let value = channel_data.get_value();
            if value.is_array() {
                println!("{}{:?}", prefix, LimitedDebugVec { data: value.as_str_array().unwrap(), limit: max_elements });
            } else {
                println!("{}{}", prefix, channel_data.get_value().as_str());
            }
        }
        Err(e) => {
            println!("{}{:?}", prefix, e);
        }
    }
}

pub fn print_message(message: &BsMessage, max_size:usize, header:bool, id:bool, attrs:bool, main_header:bool, data_header:bool, meta:bool, data:bool) -> () {
    if header {
        println!("{}", "-".repeat(80));
    }
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
