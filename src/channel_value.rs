use super::convert::*;


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


impl ChannelValue {
    pub fn is_array(&self) -> bool {
        match self {
            ChannelValue::ASTR(data) => true,
            ChannelValue::ABOOL(data) => true,
            ChannelValue::AI8(data) => true,
            ChannelValue::AU8(data) => true,
            ChannelValue::AI16(data) => true,
            ChannelValue::AU16(data) => true,
            ChannelValue::AI32(data) => true,
            ChannelValue::AU32(data) => true,
            ChannelValue::AI64(data) => true,
            ChannelValue::AU64(data) => true,
            ChannelValue::AF32(data) => true,
            ChannelValue::AF64(data) => true,
            _ => false, // Non-array types return None
        }
    }
    pub fn is_scalar(&self) -> bool {
        !self.is_array()
    }
    pub fn get_element_size(&self) -> usize {
        match self {
            ChannelValue::STR(_)|ChannelValue::ASTR(_)  => {1}
            ChannelValue::BOOL(_)|ChannelValue::ABOOL(_) => {4}
            ChannelValue::I8(_) | ChannelValue::U8(_) | ChannelValue::AI8(_) | ChannelValue::AU8(_) => {8}
            ChannelValue::I16(_) | ChannelValue::U16(_) | ChannelValue::AI16(_) | ChannelValue::AU16(_) => {16}
            ChannelValue::I32(_) | ChannelValue::U32(_) | ChannelValue::AI32(_) | ChannelValue::AU32(_) => {32}
            ChannelValue::I64(_) | ChannelValue::U64(_) | ChannelValue::AI64(_) | ChannelValue::AU64(_) => {64}
            ChannelValue::F32(_) | ChannelValue::AF32(_) => {32}
            ChannelValue::F64(_) | ChannelValue::AF64(_) => {64}
        }
    }

    pub fn is_float(&self) -> bool {
        match self {
            ChannelValue::F32(_) | ChannelValue::AF32(_) => {true}
            ChannelValue::F64(_) | ChannelValue::AF64(_) => {true}
            _ => false, // Non-array types return None
        }
    }
    pub fn is_bool(&self) -> bool {
        match self {
            ChannelValue::BOOL(_) | ChannelValue::ABOOL(_) => {true}
            _ => false, // Non-array types return None
        }
    }

    pub fn is_str(&self) -> bool {
        match self {
            ChannelValue::STR(_) | ChannelValue::ASTR(_) => {true}
            _ => false, // Non-array types return None
        }
    }
    pub fn is_int(&self) -> bool {
        match self {
            ChannelValue::I8(_) | ChannelValue::U8(_) | ChannelValue::AI8(_) | ChannelValue::AU8(_) => {true}
            ChannelValue::I16(_) | ChannelValue::U16(_) | ChannelValue::AI16(_) | ChannelValue::AU16(_) => {true}
            ChannelValue::I32(_) | ChannelValue::U32(_) | ChannelValue::AI32(_) | ChannelValue::AU32(_) => {true}
            ChannelValue::I64(_) | ChannelValue::U64(_) | ChannelValue::AI64(_) | ChannelValue::AU64(_) => {true}
            _ => false, // Non-array types return None
        }
    }
    pub fn get_size(&self) -> usize{
        match self {
            ChannelValue::ASTR(data) => data.len(),
            ChannelValue::ABOOL(data) => data.len(),
            ChannelValue::AI8(data) => data.len(),
            ChannelValue::AU8(data) => data.len(),
            ChannelValue::AI16(data) => data.len(),
            ChannelValue::AU16(data) => data.len(),
            ChannelValue::AI32(data) => data.len(),
            ChannelValue::AU32(data) => data.len(),
            ChannelValue::AI64(data) => data.len(),
            ChannelValue::AU64(data) => data.len(),
            ChannelValue::AF32(data) => data.len(),
            ChannelValue::AF64(data) => data.len(),
            _ => 1, // Non-array types return 1
        }
    }

    pub fn as_num_array<U: num_traits::NumCast>(&self) -> Option<Vec<U>>
    where
        U: num_traits::NumCast,

    {
        match self {
            ChannelValue::ABOOL(data) =>  try_convert_bool_arr::<U>(&data),
            ChannelValue::AI8(data) => try_convert_num_arr::<_, U>(&data),
            ChannelValue::AU8(data) => try_convert_num_arr::<_, U>(&data),
            ChannelValue::AI16(data) => try_convert_num_arr::<_, U>(&data),
            ChannelValue::AU16(data) => try_convert_num_arr::<_, U>(&data),
            ChannelValue::AI32(data) => try_convert_num_arr::<_, U>(&data),
            ChannelValue::AU32(data) => try_convert_num_arr::<_, U>(&data),
            ChannelValue::AI64(data) => try_convert_num_arr::<_, U>(&data),
            ChannelValue::AU64(data) => try_convert_num_arr::<_, U>(&data),
            ChannelValue::AF32(data) => try_convert_num_arr::<_, U>(&data),
            ChannelValue::AF64(data) => try_convert_num_arr::<_, U>(&data),
            _ => None, // Handle scalar values or non-array types as needed
        }
    }

    pub fn as_str_array(&self) -> Option<Vec<String>>
    {
        match self {
            ChannelValue::ASTR(data) =>  try_convert_str_arr::<_>(&data),
            ChannelValue::ABOOL(data) =>  try_convert_str_arr::<_>(&data),
            ChannelValue::AI8(data) =>  try_convert_str_arr::<_>(&data),
            ChannelValue::AU8(data) =>  try_convert_str_arr::<_>(&data),
            ChannelValue::AI16(data) =>  try_convert_str_arr::<_>(&data),
            ChannelValue::AU16(data) =>  try_convert_str_arr::<_>(&data),
            ChannelValue::AI32(data) =>  try_convert_str_arr::<_>(&data),
            ChannelValue::AU32(data) =>  try_convert_str_arr::<_>(&data),
            ChannelValue::AI64(data) =>  try_convert_str_arr::<_>(&data),
            ChannelValue::AU64(data) => try_convert_str_arr::<_>(&data),
            ChannelValue::AF32(data) =>  try_convert_str_arr::<_>(&data),
            ChannelValue::AF64(data) =>  try_convert_str_arr::<_>(&data),
            _ => None, // Handle scalar values or non-array types as needed
        }
    }

    pub fn as_num<U: num_traits::NumCast>(&self) -> Option<U>
    where
        U: num_traits::NumCast,

    {
        match self {
            ChannelValue::BOOL(data) =>  try_convert_bool(data),
            ChannelValue::I8(data) => try_convert_num(data),
            ChannelValue::U8(data) => try_convert_num(data),
            ChannelValue::I16(data) => try_convert_num(data),
            ChannelValue::U16(data) => try_convert_num(data),
            ChannelValue::I32(data) => try_convert_num(data),
            ChannelValue::U32(data) => try_convert_num(data),
            ChannelValue::I64(data) => try_convert_num(data),
            ChannelValue::U64(data) => try_convert_num(data),
            ChannelValue::F32(data) => try_convert_num(data),
            ChannelValue::F64(data) => try_convert_num(data),
            _ => None, // Handle scalar values or non-array types as needed
        }
    }

    pub fn as_str(&self) -> String
    {
        match self{
            ChannelValue::STR(data) => {data.to_string()}
            ChannelValue::BOOL(data) => {data.to_string()}
            ChannelValue::I8(data) => {data.to_string()}
            ChannelValue::U8(data) => {data.to_string()}
            ChannelValue::I16(data) => {data.to_string()}
            ChannelValue::U16(data) => {data.to_string()}
            ChannelValue::I32(data) => {data.to_string()}
            ChannelValue::U32(data) => {data.to_string()}
            ChannelValue::I64(data) => {data.to_string()}
            ChannelValue::U64(data) => {data.to_string()}
            ChannelValue::F32(data) => {data.to_string()}
            ChannelValue::F64(data) => {data.to_string()}
            ChannelValue::ASTR(data) => {format!("{:?}", data)}
            ChannelValue::ABOOL(data) => {format!("{:?}", data)}
            ChannelValue::AI8(data) => {format!("{:?}", data)}
            ChannelValue::AU8(data) => {format!("{:?}", data)}
            ChannelValue::AI16(data) => {format!("{:?}", data)}
            ChannelValue::AU16(data) => {format!("{:?}", data)}
            ChannelValue::AI32(data) => {format!("{:?}", data)}
            ChannelValue::AU32(data) => {format!("{:?}", data)}
            ChannelValue::AI64(data) => {format!("{:?}", data)}
            ChannelValue::AU64(data) => {format!("{:?}", data)}
            ChannelValue::AF32(data) => {format!("{:?}", data)}
            ChannelValue::AF64(data) => {format!("{:?}", data)}
        }
    }

}

