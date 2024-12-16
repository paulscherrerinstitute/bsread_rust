use std::collections::HashMap;
use crate::*;
use crate::value::Value;
use std::io::Cursor;
use serde_json::Value as JsonValue;
use crate::reader::{READER_ABF32, READER_ABF64, READER_ABI16, READER_ABI32, READER_ABI64, READER_ABOOL, READER_ABU16, READER_ABU32, READER_ABU64, READER_AF32, READER_AF64, READER_AI16, READER_AI32, READER_AI64, READER_AI8, READER_AU16, READER_AU32, READER_AU64, READER_AU8, READER_BF32, READER_BF64, READER_BI16, READER_BI32, READER_BI64, READER_BOOL, READER_BU16, READER_BU32, READER_BU64, READER_F32, READER_F64, READER_I16, READER_I32, READER_I64, READER_I8, READER_STRING, READER_U16, READER_U32, READER_U64, READER_U8};
use crate::writer::{WRITER_ABF32, WRITER_ABF64, WRITER_ABI16, WRITER_ABI32, WRITER_ABI64, WRITER_ABOOL, WRITER_ABU16, WRITER_ABU32, WRITER_ABU64, WRITER_AF32, WRITER_AF64, WRITER_AI16, WRITER_AI32, WRITER_AI64, WRITER_AI8, WRITER_AU16, WRITER_AU32, WRITER_AU64, WRITER_AU8, WRITER_BF32, WRITER_BF64, WRITER_BI16, WRITER_BI32, WRITER_BI64, WRITER_BOOL, WRITER_BU16, WRITER_BU32, WRITER_BU64, WRITER_F32, WRITER_F64, WRITER_I16, WRITER_I32, WRITER_I64, WRITER_I8, WRITER_STRING, WRITER_U16, WRITER_U32, WRITER_U64, WRITER_U8};

#[derive(Debug)]
pub struct ChannelConfig {
    name: String,
    typ: String,
    shape: Option<Vec<u32>>,
    elements: usize,
    element_size: usize,
    little_endian: bool,
    compression: String,
}

impl ChannelConfig {
    pub fn get_name(&self) -> String {
        self.name.clone()
    }
    pub fn get_type(&self) -> String {
        self.typ.clone()
    }
    pub fn get_shape(&self) -> Option<Vec<u32>> {
        self.shape.clone()
    }
    pub fn get_elements(&self) -> usize {
        self.elements.clone()
    }
    pub fn get_element_size(&self) -> usize {
        self.element_size.clone()
    }
    pub fn get_size(&self) -> usize {
        self.element_size * self.elements
    }
    pub fn is_little_endian(&self) -> bool {
        self.little_endian
    }
    pub fn get_compression(&self) -> String {
        self.compression.clone()
    }

    pub fn get_metadata(&self) -> HashMap<String, JsonValue> {
        let mut metadata: HashMap<String, JsonValue> = HashMap::new();
        metadata.insert("name".to_string(), JsonValue::String(self.get_name()));
        let shape = self.get_shape().unwrap_or(Vec::new());
        let shape_json = JsonValue::Array(shape.into_iter().map(|num| JsonValue::Number(num.into())).collect());
        metadata.insert("shape".to_string(),shape_json);
        metadata.insert("type".to_string(), JsonValue::String(self.get_type()));
        metadata.insert("encoding".to_string(), JsonValue::String((if self.is_little_endian(){"little"} else {"big"}).to_string()));
        if self.get_compression() != "none" {
            metadata.insert("compression".to_string(), JsonValue::String(self.get_compression()));
        }
        metadata
    }
}

pub struct ChannelScalar<T> {
    config: ChannelConfig,
    reader: fn(&mut Cursor<&Vec<u8>>) -> IOResult<T>,
    writer: fn(&mut Cursor<&mut Vec<u8>>, &T) -> IOResult<()>
}

pub struct ChannelArray<T> {
    config: ChannelConfig,
    reader: fn(&mut Cursor<&Vec<u8>>, &mut [T]) -> IOResult<()>,
    writer: fn(&mut Cursor<&mut Vec<u8>>, &[T]) -> IOResult<()>
}

pub fn get_elements(shape: &Option<Vec<u32>>) -> usize {
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
    pub fn new(name: String, typ: String, shape: Option<Vec<u32>>, little_endian: bool, compression: String,
               reader: fn(&mut Cursor<&Vec<u8>>) -> IOResult<T>, writer: fn(&mut Cursor<&mut Vec<u8>>, &T) -> IOResult<()>) -> Self {
        let elements = get_elements(&shape);
        let element_size = get_element_size(&typ);
        let config = ChannelConfig { name, typ, shape, elements, element_size, little_endian, compression };
        Self { config, reader, writer }
    }
}


impl<T: Default + Clone> ChannelArray<T> {
    pub fn new(name: String, typ: String, shape: Option<Vec<u32>>, little_endian: bool, compression: String,
               reader: fn(&mut Cursor<&Vec<u8>>, &mut [T]) -> IOResult<()>,  writer: fn(&mut Cursor<&mut Vec<u8>>, &[T]) -> IOResult<()>) -> Self {
        let elements = get_elements(&shape);
        let element_size = get_element_size(&typ);
        let config = ChannelConfig { name, typ, shape, elements, element_size, little_endian, compression };
        Self { config, reader, writer }
    }
}

static EMPTY_CONFIG: ChannelConfig = ChannelConfig { name: String::new(), typ: String::new(), shape: None, elements: 0, element_size: 0, little_endian: false, compression: String::new() };
pub trait ChannelTrait: Send {
    fn get_config(&self) -> &ChannelConfig {
        &EMPTY_CONFIG
    }
    fn read(&self, _: &mut Cursor<&Vec<u8>>) -> IOResult<Value> {
        Err(new_error(ErrorKind::Unsupported, "Unsupported channel type"))
    }

    fn write(&self, _: &mut Cursor<&mut Vec<u8>>, _:&Value) -> IOResult<()> {
        Err(new_error(ErrorKind::Unsupported, "Unsupported channel type"))
    }

}
/*
impl ChannelTrait for Channel<i32> {
    fn read(&self, cursor: &mut Cursor<&Vec<u8>>) -> IOResult<ChannelValue> {
        let result = (self.reader)(cursor)?;
        Ok(Value::I32(result))
    }
}
 */

macro_rules! impl_channel_scalar_trait {
    ($t:ty, $variant:ident) => {
        impl ChannelTrait for ChannelScalar<$t> {
            fn read(&self, cursor: &mut Cursor<&Vec<u8>>) -> IOResult<Value> {
                    let result = (self.reader)(cursor)?;
                    Ok(Value::$variant(result))
            }

            fn write(&self, cursor: &mut Cursor<&mut Vec<u8>>, value:&Value) -> IOResult<()> {
                if let Value::$variant(data) = value {
                    (self.writer)(cursor, data)?;
                    Ok(())
                } else {
                    Err(new_error(ErrorKind::InvalidInput, "Channel write with invalid variant"))
                }
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
           fn read(&self, cursor: &mut Cursor<&Vec<u8>>) -> IOResult<Value> {
                    let mut buffer: Vec<$t> = Vec::with_capacity(self.config.elements);
                    unsafe {
                        buffer.set_len(self.config.elements); // Initialize the buffer without default values
                    }
                    (self.reader)(cursor, & mut buffer)?;
                    Ok(Value::$variant(buffer))
            }

            fn write(&self, cursor: &mut Cursor<&mut Vec<u8>>, value:&Value) -> IOResult<()> {
                if let Value::$variant(data) = value {
                    (self.writer)(cursor, data)?;
                    Ok(())
                } else {
                    Err(new_error(ErrorKind::InvalidInput, "Channel write with invalid variant"))
                }
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
    fn read(&self, _: &mut Cursor<&Vec<u8>>) -> IOResult<Value> {
        Err(new_error(ErrorKind::Unsupported, "String array not supported"))
    }
}

pub fn new_channel(name: String, typ:String, shape:Option<Vec<u32>>, little_endian:bool, compression:String) -> IOResult<Box<dyn ChannelTrait>> {
    let array = shape.clone().unwrap_or(vec![]).len() > 0;
    if  array {
        match typ.as_str() {
            "bool" => Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, READER_ABOOL, WRITER_ABOOL))),
            //"string" => Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, READER_ASTRING, WRITER_ASTRING))),
            "string" => Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, READER_STRING, WRITER_STRING))),
            "int8" => Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, READER_AI8, WRITER_AI8))),
            "uint8" => Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, READER_AU8, WRITER_AU8))),
            "int16" => Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, if little_endian { READER_AI16 } else { READER_ABI16 }, if little_endian { WRITER_AI16 } else { WRITER_ABI16 }))),
            "uint16" => Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, if little_endian { READER_AU16 } else { READER_ABU16 }, if little_endian { WRITER_AU16 } else { WRITER_ABU16 }))),
            "int32" => Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, if little_endian { READER_AI32 } else { READER_ABI32 }, if little_endian { WRITER_AI32 } else { WRITER_ABI32 }))),
            "uint32" => Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, if little_endian { READER_AU32 } else { READER_ABU32 }, if little_endian { WRITER_AU32 } else { WRITER_ABU32 }))),
            "int64" => Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, if little_endian { READER_AI64 } else { READER_ABI64 },if little_endian { WRITER_AI64 } else { WRITER_ABI64 }))),
            "uint64" => Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, if little_endian { READER_AU64 } else { READER_ABU64 }, if little_endian { WRITER_AU64 } else { WRITER_ABU64 }))),
            "float32" => Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, if little_endian { READER_AF32 } else { READER_ABF32 }, if little_endian { WRITER_AF32 } else { WRITER_ABF32 }))),
            "float64" => Ok(Box::new(ChannelArray::new(name, typ, shape, little_endian, compression, if little_endian { READER_AF64 } else { READER_ABF64 }, if little_endian { WRITER_AF64 } else { WRITER_ABF64 }))),
            _ => Err(new_error(ErrorKind::Unsupported,"Unsupported data type"))
        }
    } else {
        match typ.as_str() {
            "bool" => Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, READER_BOOL, WRITER_BOOL))),
            "string" => Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, READER_STRING, WRITER_STRING))),
            "int8" => Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, READER_I8, WRITER_I8))),
            "uint8" => Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, READER_U8, WRITER_U8))),
            "int16" => Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, if little_endian { READER_I16 } else { READER_BI16 }, if little_endian { WRITER_I16 } else { WRITER_BI16 }))),
            "uint16" => Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, if little_endian { READER_U16 } else { READER_BU16 }, if little_endian { WRITER_U16 } else { WRITER_BU16 }))),
            "int32" => Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, if little_endian { READER_I32 } else { READER_BI32 }, if little_endian { WRITER_I32 } else { WRITER_BI32 }))),
            "uint32" => Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, if little_endian { READER_U32 } else { READER_BU32 }, if little_endian { WRITER_U32 } else { WRITER_BU32 }))),
            "int64" => Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, if little_endian { READER_I64 } else { READER_BI64 }, if little_endian { WRITER_I64 } else { WRITER_BI64 }))),
            "uint64" => Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, if little_endian { READER_U64 } else { READER_BU64 }, if little_endian { WRITER_U64 } else { WRITER_BU64 }))),
            "float32" => Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, if little_endian { READER_F32 } else { READER_BF32 }, if little_endian { WRITER_F32 } else { WRITER_BF32 }))),
            "float64" => Ok(Box::new(ChannelScalar::new(name, typ, shape, little_endian, compression, if little_endian { READER_F64 } else { READER_BF64 }, if little_endian { WRITER_F64 } else { WRITER_BF64 }))),
            _ => Err(new_error(ErrorKind::Unsupported,"Unsupported data type"))
        }
    }
}
