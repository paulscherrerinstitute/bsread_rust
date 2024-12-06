# BSREAD

Rust implementation of the BSREAD streaming protocol


## Receiver

The Receiver struct implements parsing of Message structs from BSREAD streams.
Receivers are created specifying a list of endpoints and a ZMQ socket type (SUB or PULL):
```rust
    let bsread = Bsread::new().unwrap();
    let mut rec = bsread.receiver(Some(vec![ENDPOINT_1, ..., ENDPOINT_N]), zmq::PULL)?;
```

Receivers can operate in the different modes: 

### Synchronous
Data is received on a callback in the caller thread.
```rust
    fn on_message(message: Message) -> () {
        println!("Received ID = {}", message.get_id());
    }
    rec.listen(on_message, Some(10))?; //Receices 10 messages
```

### Asynchronous
Data is received on a callback in a separated thread.
```rust
    fn on_message(message: Message) -> () {
        println!("Received ID = {}", message.get_id());
    }
    rec.fork(on_message, None);
    thread::sleep(Duration::from_millis(1000)); //Receives for 1s
    rec.interrupt();
```


### Buffered
Data is produced in separated thread, buffered, and received in the caller thread.
```rust
    rec.start(100)?; //Buffer size = 100
    match rec.wait(1000) { //Wait 1s for a message 
        Ok(msg) => {print_message(&msg)}
        Err(e) => {println!("{}",e)}
    }
    rec.stop();
```

## Pool
Pool structs are compositions of multiple Receivers, each running in a private thread.
Pools can be created
- With automatic allocation of endpoints, providing a vector of endpoints and the number of threads.

```rust
    let bsread = crate::Bsread::new().unwrap();
    let mut pool = bsread.pool_auto(vec![ENDPOINT_1, ..., ENDPOINT_N], zmq::SUB, NUMBER_OF_THREADS)?;
```
- Or else assigning the endpoints manually, with a vector of vectors of endpoints:
```rust
    let bsread = crate::Bsread::new().unwrap();
    let mut pool = bsread.pool_manual(vec![vec![ENDPOINT_1, ..., ENDPOINT_N], vec![ENDPOINT_M, ..., ENDPOINT_Z]], zmq::SUB)?;
```

A Pool can operate in the different modes:

### Synchronous
Message callback is called synchronously in each receiving thread.
```rust
    fn on_message(message: Message) -> () {
        println!("Received ID = {}", message.get_id());
    }
    pool.start(on_message);
    thread::sleep(Duration::from_millis(1000)); //Receives for 1s
    pool.stop();
```


### Buffered
Messages are buffered in the receiving thread and message callback is called asynchronously in another thread .
```rust
    fn on_message(message: Message) -> () {
        println!("Received ID = {}", message.get_id());
    }
    pool.start_buffered(on_message,100); //Size of buffer = 100
    thread::sleep(Duration::from_millis(1000)); //Receives for 1s
    pool.stop();
```

## Message
A BSREAD message is composed by the elements:
- Main Header, which provides the message  ID and timestamp.
- Data header, which generates the metadata for the channels.
- List of channel values and channel timestamps.

This callback prints message contents:
 
```rust
fn on_message(message: Message) -> () {
    println!("ID = {:?}", message.get_id());
    println!("Hash: {:?}", message.get_hash());
    println!("Timestamp: {:?}", message.get_timestamp());

    println!("Channel Metadata:");
    let mut channel_names = Vec::new();
    for channel in message.get_channels() {
        let config = channel.get_config();
        let shape : Vec<u32> = config.get_shape().unwrap_or(Vec::new());
        println!("\t{} {} {:?} {} {}", config.get_name(), config.get_type(), shape, config.get_elements(), config.get_compression());
        channel_names.push(config.get_name());
    }

    println!("Channel Data:");
    let data = message.get_data();
    for (key, data) in data {
        let value = data.as_ref().unwrap().get_value();
        if value.is_array() {
            println!("\t{} : Array of {} elements of type {:?}", key, value.get_size(), value.get_type());
        } else {
            println!("\t{} : {:?}", key, value);
        }
    }
}
```

## Value

The enum Value contained in the channel data above can hold the data types supported by BSREAD. 
It includes many helper methods to identify and convert types.

```rust
pub enum Value {
    STR(String),
    BOOL(bool),
    I8(i8),
    U8(u8),
    I16(i16),
    U16(u16),
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
    F32(f32),
    F64(f64),
    ASTR(Vec<String>),
    ABOOL(Vec<bool>),
    AI8(Vec<i8>),
    AU8(Vec<u8>),
    AI16(Vec<i16>),
    AU16(Vec<u16>),
    AI32(Vec<i32>),
    AU32(Vec<u32>),
    AI64(Vec<i64>),
    AU64(Vec<u64>),
    AF32(Vec<f32>),
    AF64(Vec<f64>),
}
```