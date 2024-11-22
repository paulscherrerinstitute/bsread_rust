use std::io;
use std::io::Cursor;

#[derive(Debug)]
pub struct ChannelConfig {
    name: String,
    typ: String,
    shape: Option<Vec<i32>>,
    elements: usize,
    element_size: usize,
    le: bool,
    compression: String,
}

impl ChannelConfig {
    pub fn get_name(&self) -> String {
        self.name.clone()
    }
    pub fn get_type(&self) -> String {
        self.typ.clone()
    }
    pub fn get_shape(&self) -> Option<Vec<i32>> {
        self.shape.clone()
    }
    pub fn get_elements(&self) -> usize {
        self.elements.clone()
    }
    pub fn get_compression(&self) -> String {
        self.compression.clone()
    }
    pub fn get_element_size(&self) -> usize {
        self.element_size.clone()
    }
}

pub struct ChannelScalar<T> {
    config: ChannelConfig,
    reader: fn(&mut Cursor<&Vec<u8>>) -> io::Result<T>,
}

pub struct ChannelArray<T> {
    config: ChannelConfig,
    reader: fn(&mut Cursor<&Vec<u8>>, &mut [T]) -> io::Result<()>,
    buffer: Vec<T>,
}

pub fn get_elements(shape: &Option<Vec<i32>>) -> usize {
    let nelm = shape.clone()
        .filter(|v| !v.is_empty()) // Ensure it's not empty
        .map(|v| v.into_iter().product()) // Compute product of elements
        .unwrap_or(1); //Default to 1 if None or empty
    let elements = nelm as usize;
    elements
}

fn get_element_size(typ: &str) -> usize {
    match typ {
        "bool" => 4,
        "string" => 1,
        "int8" => 1,
        "uint8" => 1,
        "int16" => 2,
        "uint16" => 2,
        "int32" => 4,
        "uint32" => 4,
        "int64" => 8,
        "uint64" => 8,
        "float32" => 4,
        "float64" => 8,
        _ => 4,
    }
}
impl<T: Default + Clone> ChannelScalar<T> {
    pub fn new(name: String, typ: String, shape: Option<Vec<i32>>, le: bool, compression: String, reader: fn(&mut Cursor<&Vec<u8>>) -> io::Result<T>) -> Self {
        let elements = get_elements(&shape);
        let element_size = get_element_size(&typ);
        let config = ChannelConfig { name, typ, shape, elements, element_size, le, compression };
        Self { config, reader }
    }
}


impl<T: Default + Clone> ChannelArray<T> {
    pub fn new(name: String, typ: String, shape: Option<Vec<i32>>, le: bool, compression: String, reader: fn(&mut Cursor<&Vec<u8>>, &mut [T]) -> io::Result<()>) -> Self {
        let elements = get_elements(&shape);
        let element_size = get_element_size(&typ);
        let config = ChannelConfig { name, typ, shape, elements, element_size, le, compression };
        let buffer = vec![T::default(); elements];
        Self { config, reader, buffer }
    }

    fn update_cache(&mut self, index: usize, value: T) {
        if let Some(elem) = self.buffer.get_mut(index) {
            *elem = value; // Update the value at the specified index
        }
    }
}

#[derive(Debug)]
pub enum ChannelValue {
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

static EMPTY_CONFIG: ChannelConfig = ChannelConfig { name: String::new(), typ: String::new(), shape: None, elements: 0, element_size: 0, le: false, compression: String::new() };
pub trait ChannelTrait: Send {
    fn get_config(&self) -> &ChannelConfig {
        &EMPTY_CONFIG
    }
    fn read(&self, cursor: &mut Cursor<&Vec<u8>>) -> io::Result<ChannelValue> {
        Err(io::Error::new(io::ErrorKind::Other, "Unsupported channel type"))
    }
}

/*
impl ChannelTrait for Channel<i32> {
    fn read(&self, cursor: &mut Cursor<&Vec<u8>>) -> io::Result<ChannelValue> {
        let result = (self.reader)(cursor)?;
        Ok(ChannelValue::I32(result))
    }
}
 */

macro_rules! impl_channel_scalar_trait {
    ($t:ty, $variant:ident) => {
        impl ChannelTrait for ChannelScalar<$t> {
            fn read(&self, cursor: &mut Cursor<&Vec<u8>>) -> io::Result<ChannelValue> {
                    let result = (self.reader)(cursor)?;
                    Ok(ChannelValue::$variant(result))
            }
            fn get_config(&self) -> &ChannelConfig{
                return &self.config
            }
        }
    };
}


macro_rules! impl_channel_array_trait {
    ($t:ty, $variant:ident) => {
        impl ChannelTrait for ChannelArray<$t> {
           fn read(&self, cursor: &mut Cursor<&Vec<u8>>) -> io::Result<ChannelValue> {
                    //let mut buffer: Vec<$t>  = Vec::new();
                    //buffer.resize(self.config.elements, <$t>::default());
                    let mut buffer: Vec<$t> = Vec::with_capacity(self.config.elements);
                    unsafe {
                        buffer.set_len(self.config.elements); // Initialize the buffer without default values
                    }
                    (self.reader)(cursor, & mut buffer)?;
                    Ok(ChannelValue::$variant(buffer))
            }
            fn get_config(&self) -> &ChannelConfig{
                return &self.config
            }
         }
    };
}

impl_channel_scalar_trait!(String, STR);
impl_channel_scalar_trait!(bool, BOOL);
impl_channel_scalar_trait!(i8,  I8);
impl_channel_scalar_trait!(i16, I16);
impl_channel_scalar_trait!(i32, I32);
impl_channel_scalar_trait!(i64, I64);
impl_channel_scalar_trait!(u8,  U8);
impl_channel_scalar_trait!(u16, U16);
impl_channel_scalar_trait!(u32, U32);
impl_channel_scalar_trait!(u64, U64);
impl_channel_scalar_trait!(f32, F32);
impl_channel_scalar_trait!(f64, F64);

impl_channel_array_trait!(bool, ABOOL);
impl_channel_array_trait!(i8,  AI8);
impl_channel_array_trait!(i16, AI16);
impl_channel_array_trait!(i32, AI32);
impl_channel_array_trait!(i64, AI64);
impl_channel_array_trait!(u8,  AU8);
impl_channel_array_trait!(u16, AU16);
impl_channel_array_trait!(u32, AU32);
impl_channel_array_trait!(u64, AU64);
impl_channel_array_trait!(f32, AF32);
impl_channel_array_trait!(f64, AF64);

impl ChannelTrait for ChannelArray<String> {
    fn read(&self, cursor: &mut Cursor<&Vec<u8>>) -> io::Result<ChannelValue> {
        Err(io::Error::new(io::ErrorKind::Other, "String array not supported"))
    }
}
