use std::io::Cursor;
use byteorder::BigEndian;
use std::mem;
use crate::convert::*;


#[derive(Debug, PartialEq, Clone)]
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

macro_rules! impl_as{
    ($name_num:ident, $variant_num:ident, $name_slice:ident, $variant_slice:ident, $ty:ty) => {
        pub fn $name_num(&self) -> Option<$ty> {
            match self {
                Value::$variant_num(v) => Some(*v),
                _ => None,
            }
        }

        pub fn $name_slice(&self) -> Option<&[$ty]> {
            if let Value::$variant_slice(v) = self {
                Some(v.as_slice())
            } else {
                None
            }
        }
    };
}


macro_rules! impl_to {
    ($name_num:ident, $name_array:ident, $target:ty) => {
        pub fn $name_num(&self) -> Option<$target> {
            match self {
                Value::BOOL(data) => try_convert_bool(data),
                Value::I8(data) => try_convert_num(data),
                Value::U8(data) => try_convert_num(data),
                Value::I16(data) => try_convert_num(data),
                Value::U16(data) => try_convert_num(data),
                Value::I32(data) => try_convert_num(data),
                Value::U32(data) => try_convert_num(data),
                Value::I64(data) => try_convert_num(data),
                Value::U64(data) => try_convert_num(data),
                Value::F32(data) => try_convert_num(data),
                Value::F64(data) => try_convert_num(data),

                /*
                Value::BOOL(data) => num_traits::cast::<_, $target>(*data as u8),
                Value::I8(data) => num_traits::cast::<_, $target>(*data),
                Value::U8(data) => num_traits::cast::<_, $target>(*data),
                Value::I16(data) => num_traits::cast::<_, $target>(*data),
                Value::U16(data) => num_traits::cast::<_, $target>(*data),
                Value::I32(data) => num_traits::cast::<_, $target>(*data),
                Value::U32(data) => num_traits::cast::<_, $target>(*data),
                Value::I64(data) => num_traits::cast::<_, $target>(*data),
                Value::U64(data) => num_traits::cast::<_, $target>(*data),
                Value::F32(data) => num_traits::cast::<_, $target>(*data),
                Value::F64(data) => num_traits::cast::<_, $target>(*data),
                */
                _ => None,
            }
        }
        pub fn $name_array(&self) -> Option<Vec<$target>> {
            match self {
                Value::BOOL(data) => try_convert_bool_arr::<$target>(&vec![*data;1]),
                Value::I8(data) => try_convert_num_arr::<_, $target>(&vec![*data;1]),
                Value::U8(data) => try_convert_num_arr::<_, $target>(&vec![*data;1]),
                Value::I16(data) => try_convert_num_arr::<_, $target>(&vec![*data;1]),
                Value::U16(data) => try_convert_num_arr::<_, $target>(&vec![*data;1]),
                Value::I32(data) => try_convert_num_arr::<_, $target>(&vec![*data;1]),
                Value::U32(data) => try_convert_num_arr::<_, $target>(&vec![*data;1]),
                Value::I64(data) => try_convert_num_arr::<_, $target>(&vec![*data;1]),
                Value::U64(data) => try_convert_num_arr::<_, $target>(&vec![*data;1]),
                Value::F32(data) => try_convert_num_arr::<_, $target>(&vec![*data;1]),
                Value::F64(data) => try_convert_num_arr::<_, $target>(&vec![*data;1]),
                Value::ABOOL(data) => try_convert_bool_arr::<$target>(&data),
                Value::AI8(data) => try_convert_num_arr::<_, $target>(&data),
                Value::AU8(data) => try_convert_num_arr::<_, $target>(&data),
                Value::AI16(data) => try_convert_num_arr::<_, $target>(&data),
                Value::AU16(data) => try_convert_num_arr::<_, $target>(&data),
                Value::AI32(data) => try_convert_num_arr::<_, $target>(&data),
                Value::AU32(data) => try_convert_num_arr::<_, $target>(&data),
                Value::AI64(data) => try_convert_num_arr::<_, $target>(&data),
                Value::AU64(data) => try_convert_num_arr::<_, $target>(&data),
                Value::AF32(data) => try_convert_num_arr::<_, $target>(&data),
                Value::AF64(data) => try_convert_num_arr::<_, $target>(&data),
                _ => None,
            }
        }
    };
}

impl Value {
    //Properties
    pub fn is_array(&self) -> bool {
        match self {
            Value::ASTR(_) => true,
            Value::ABOOL(_) => true,
            Value::AI8(_) => true,
            Value::AU8(_) => true,
            Value::AI16(_) => true,
            Value::AU16(_) => true,
            Value::AI32(_) => true,
            Value::AU32(_) => true,
            Value::AI64(_) => true,
            Value::AU64(_) => true,
            Value::AF32(_) => true,
            Value::AF64(_) => true,
            _ => false, // Non-array types return None
        }
    }
    pub fn is_scalar(&self) -> bool {
        !self.is_array()
    }
    pub fn get_element_size(&self) -> u32 {
        match self {
            Value::STR(_) | Value::ASTR(_) => { 1 }
            Value::BOOL(_) | Value::ABOOL(_) => { 4 }
            Value::I8(_) | Value::U8(_) | Value::AI8(_) | Value::AU8(_) => { 8 }
            Value::I16(_) | Value::U16(_) | Value::AI16(_) | Value::AU16(_) => { 16 }
            Value::I32(_) | Value::U32(_) | Value::AI32(_) | Value::AU32(_) => { 32 }
            Value::I64(_) | Value::U64(_) | Value::AI64(_) | Value::AU64(_) => { 64 }
            Value::F32(_) | Value::AF32(_) => { 32 }
            Value::F64(_) | Value::AF64(_) => { 64 }
        }
    }

    pub fn is_float(&self) -> bool {
        match self {
            Value::F32(_) | Value::AF32(_) => { true }
            Value::F64(_) | Value::AF64(_) => { true }
            _ => false, // Non-array types return None
        }
    }
    pub fn is_bool(&self) -> bool {
        match self {
            Value::BOOL(_) | Value::ABOOL(_) => { true }
            _ => false, // Non-array types return None
        }
    }

    pub fn is_str(&self) -> bool {
        match self {
            Value::STR(_) | Value::ASTR(_) => { true }
            _ => false, // Non-array types return None
        }
    }
    pub fn is_int(&self) -> bool {
        match self {
            Value::I8(_) | Value::U8(_) | Value::AI8(_) | Value::AU8(_) => { true }
            Value::I16(_) | Value::U16(_) | Value::AI16(_) | Value::AU16(_) => { true }
            Value::I32(_) | Value::U32(_) | Value::AI32(_) | Value::AU32(_) => { true }
            Value::I64(_) | Value::U64(_) | Value::AI64(_) | Value::AU64(_) => { true }
            _ => false, // Non-array types return None
        }
    }
    pub fn get_size(&self) -> usize {
        match self {
            Value::ASTR(data) => data.len(),
            Value::ABOOL(data) => data.len(),
            Value::AI8(data) => data.len(),
            Value::AU8(data) => data.len(),
            Value::AI16(data) => data.len(),
            Value::AU16(data) => data.len(),
            Value::AI32(data) => data.len(),
            Value::AU32(data) => data.len(),
            Value::AI64(data) => data.len(),
            Value::AU64(data) => data.len(),
            Value::AF32(data) => data.len(),
            Value::AF64(data) => data.len(),
            _ => 1, // Non-array types return 1
        }
    }

    impl_as!(as_u8, U8, as_au8, AU8, u8);
    impl_as!(as_u16, U16, as_au16, AU16, u16);
    impl_as!(as_u32, U32, as_au32, AU32, u32);
    impl_as!(as_u64, U64, as_au64, AU64, u64);
    impl_as!(as_i8, I8, as_ai8, AI8, i8);
    impl_as!(as_i16, I16, as_ai16, AI16, i16);
    impl_as!(as_i32, I32, as_ai32, AI32, i32);
    impl_as!(as_i64, I64, as_ai64, AI64, i64);
    impl_as!(as_f32, F32, as_af32, AF32,f32);
    impl_as!(as_f64, F64, as_af64, AF64, f64);
    impl_as!(as_bool, BOOL, as_abool, ABOOL, bool);

    impl_to!(to_u8, to_au8, u8);
    impl_to!(to_u16, to_au16, u16);
    impl_to!(to_u32, to_au32, u32);
    impl_to!(to_u64, to_au64, u64);
    impl_to!(to_i8, to_ai8, i8);
    impl_to!(to_i16, to_ai16, i16);
    impl_to!(to_i32, to_ai32, i32);
    impl_to!(to_i64, to_ai64, i64);
    impl_to!(to_f32, to_af32, f32);
    impl_to!(to_f64, to_af64, f64);

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::STR(v) => Some(v.as_str()), _ => None,
        }
    }
    pub fn as_astr(&self) -> Option<&[String]> {
        match self {
            Value::ASTR(v) => Some(v.as_slice()), _ => None,
        }
    }

    pub fn to_bool(&self) -> Option<bool>
    {
        match self {
            Value::BOOL(data) => Some(*data),
            Value::I8(data) => Some(*data!=0),
            Value::U8(data) => Some(*data!=0),
            Value::I16(data) => Some(*data!=0),
            Value::U16(data) => Some(*data!=0),
            Value::I32(data) => Some(*data!=0),
            Value::U32(data) => Some(*data!=0),
            Value::I64(data) => Some(*data!=0),
            Value::U64(data) => Some(*data!=0),
            Value::F32(data) => Some(*data != 0.0 && !data.is_nan()),
            Value::F64(data) => Some(*data != 0.0 && !data.is_nan()),
            Value::STR(data) => Some(!data.as_str().trim().is_empty()),
            _ => None,
        }
    }

    pub fn to_str(&self) -> String
    {
        match self {
            Value::STR(data) => { data.to_string() }
            Value::BOOL(data) => { data.to_string() }
            Value::I8(data) => { data.to_string() }
            Value::U8(data) => { data.to_string() }
            Value::I16(data) => { data.to_string() }
            Value::U16(data) => { data.to_string() }
            Value::I32(data) => { data.to_string() }
            Value::U32(data) => { data.to_string() }
            Value::I64(data) => { data.to_string() }
            Value::U64(data) => { data.to_string() }
            Value::F32(data) => { data.to_string() }
            Value::F64(data) => { data.to_string() }
            Value::ASTR(data) => { format!("{:?}", data) }
            Value::ABOOL(data) => { format!("{:?}", data) }
            Value::AI8(data) => { format!("{:?}", data) }
            Value::AU8(data) => { format!("{:?}", data) }
            Value::AI16(data) => { format!("{:?}", data) }
            Value::AU16(data) => { format!("{:?}", data) }
            Value::AI32(data) => { format!("{:?}", data) }
            Value::AU32(data) => { format!("{:?}", data) }
            Value::AI64(data) => { format!("{:?}", data) }
            Value::AU64(data) => { format!("{:?}", data) }
            Value::AF32(data) => { format!("{:?}", data) }
            Value::AF64(data) => { format!("{:?}", data) }
        }
    }

    pub fn to_abool(&self) -> Option<Vec<bool>>
    {
        match self.to_au64() {
            Some(aux) =>  Some(aux.iter()
                .map(|&x| x != 0)
                .collect()),
            None => None
        }
    }

    pub fn to_astr(&self) -> Option<Vec<String>>
    {
        match self {
            Value::ASTR(data) => try_convert_str_arr::<_>(&data),
            Value::ABOOL(data) => try_convert_str_arr::<_>(&data),
            Value::AI8(data) => try_convert_str_arr::<_>(&data),
            Value::AU8(data) => try_convert_str_arr::<_>(&data),
            Value::AI16(data) => try_convert_str_arr::<_>(&data),
            Value::AU16(data) => try_convert_str_arr::<_>(&data),
            Value::AI32(data) => try_convert_str_arr::<_>(&data),
            Value::AU32(data) => try_convert_str_arr::<_>(&data),
            Value::AI64(data) => try_convert_str_arr::<_>(&data),
            Value::AU64(data) => try_convert_str_arr::<_>(&data),
            Value::AF32(data) => try_convert_str_arr::<_>(&data),
            Value::AF64(data) => try_convert_str_arr::<_>(&data),
            _ => None, // Handle scalar values or non-array types as needed
        }
    }

    pub fn get_name(&self) -> &str
    {
        match self {
            Value::STR(_) => {"STR"}
            Value::BOOL(_) => {"BOOL"}
            Value::I8(_) => {"I8"}
            Value::U8(_) => {"U8"}
            Value::I16(_) =>{"I16"}
            Value::U16(_) => {"U16"}
            Value::I32(_) => {"I32"}
            Value::U32(_) => {"U32"}
            Value::I64(_) => {"I64"}
            Value::U64(_) => {"U64"}
            Value::F32(_) => {"F32"}
            Value::F64(_) => {"F64"}
            Value::ASTR(_) => {"ASTR"}
            Value::ABOOL(_) => {"ABOOL"}
            Value::AI8(_) =>{"AI8"}
            Value::AU8(_) => {"AU8"}
            Value::AI16(_) => {"AI16"}
            Value::AU16(_) => {"AU16"}
            Value::AI32(_) => {"AI32"}
            Value::AU32(_) => {"AU32"}
            Value::AI64(_) => {"AI64"}
            Value::AU64(_) => {"AU64"}
            Value::AF32(_) => {"AF32"}
            Value::AF64(_) => {"AF64"}
        }
    }

    pub fn get_type(&self) -> &str {
        match self {
            Value::STR(_) | Value::ASTR(_) => { "string" }
            Value::BOOL(_) | Value::ABOOL(_) => { "bool" }
            Value::I8(_) | Value::AI8(_) => { "int8" }
            Value::U8(_) | Value::AU8(_) => { "uint8" }
            Value::I16(_) | Value::AI16(_) => { "int16" }
            Value::U16(_) | Value::AU16(_) => { "uint16" }
            Value::I32(_) | Value::AI32(_) => { "int32" }
            Value::U32(_) | Value::AU32(_) => { "uint32" }
            Value::I64(_) | Value::AI64(_) => { "int64" }
            Value::U64(_) | Value::AU64(_) => { "uint64" }
            Value::F32(_) | Value::AF32(_) => { "float32" }
            Value::F64(_) | Value::AF64 (_) => {"float64"}
        }
    }

    //Byte array representation
    pub fn as_bytes(&self) -> Option<&[u8]> {
        self.as_au8()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            // Scalars
            Value::STR(s) => s.as_bytes().to_vec(),
            Value::BOOL(b) => vec![*b as u8],
            Value::I8(v) => vec![*v as u8],
            Value::U8(v) => vec![*v],
            Value::I16(v) => v.to_ne_bytes().to_vec(),
            Value::U16(v) => v.to_ne_bytes().to_vec(),
            Value::I32(v) => v.to_ne_bytes().to_vec(),
            Value::U32(v) => v.to_ne_bytes().to_vec(),
            Value::I64(v) => v.to_ne_bytes().to_vec(),
            Value::U64(v) => v.to_ne_bytes().to_vec(),
            Value::F32(v) => v.to_ne_bytes().to_vec(),
            Value::F64(v) => v.to_ne_bytes().to_vec(),

            // Arrays
            Value::ASTR(arr) => arr.iter().flat_map(|s| s.as_bytes()).copied().collect(),
            Value::ABOOL(arr) => bool_arr_to_bytes(arr),
            Value::AI8(arr) => arr.iter().map(|&v| v as u8).collect(),
            Value::AU8(arr) => arr.clone(),
            Value::AI16(arr) => array_to_bytes(arr),
            Value::AU16(arr) => array_to_bytes(arr),
            Value::AI32(arr) => array_to_bytes(arr),
            Value::AU32(arr) => array_to_bytes(arr),
            Value::AI64(arr) => array_to_bytes(arr),
            Value::AU64(arr) => array_to_bytes(arr),
            Value::AF32(arr) => array_to_bytes(arr),
            Value::AF64(arr) => array_to_bytes(arr),
        }
    }

}