use crate::*;
use std::string::String;
use byteorder::{LittleEndian, BigEndian, ReadBytesExt};
use std::io::{Cursor, Read};
use std::io;

trait ReadU8Into {
    fn read_u8_into(&mut self, buf: &mut [u8]) -> IOResult<()>;
}

impl<T: AsRef<[u8]>> ReadU8Into for Cursor<T> {
    fn read_u8_into(&mut self, buf: &mut [u8]) -> IOResult<()> {
        self.read_exact(buf)
    }
}

pub const READER_I8: fn(&mut Cursor<&Vec<u8>>) -> IOResult<i8> = |cursor: &mut Cursor<&Vec<u8>>| { cursor.read_i8() };
pub const READER_I16: fn(&mut Cursor<&Vec<u8>>) -> IOResult<i16> = |cursor: &mut Cursor<&Vec<u8>>| { cursor.read_i16::<LittleEndian>() };
pub const READER_I32: fn(&mut Cursor<&Vec<u8>>) -> IOResult<i32> = |cursor: &mut Cursor<&Vec<u8>>| { cursor.read_i32::<LittleEndian>() };
pub const READER_I64: fn(&mut Cursor<&Vec<u8>>) -> IOResult<i64> = |cursor: &mut Cursor<&Vec<u8>>| { cursor.read_i64::<LittleEndian>() };
pub const READER_U8: fn(&mut Cursor<&Vec<u8>>) -> IOResult<u8> = |cursor: &mut Cursor<&Vec<u8>>| { cursor.read_u8() };
pub const READER_U16: fn(&mut Cursor<&Vec<u8>>) -> IOResult<u16> = |cursor: &mut Cursor<&Vec<u8>>| { cursor.read_u16::<LittleEndian>() };
pub const READER_U32: fn(&mut Cursor<&Vec<u8>>) -> IOResult<u32> = |cursor: &mut Cursor<&Vec<u8>>| { cursor.read_u32::<LittleEndian>() };
pub const READER_U64: fn(&mut Cursor<&Vec<u8>>) -> IOResult<u64> = |cursor: &mut Cursor<&Vec<u8>>| { cursor.read_u64::<LittleEndian>() };
pub const READER_F32: fn(&mut Cursor<&Vec<u8>>) -> IOResult<f32> = |cursor: &mut Cursor<&Vec<u8>>| { cursor.read_f32::<LittleEndian>() };
pub const READER_F64: fn(&mut Cursor<&Vec<u8>>) -> IOResult<f64> = |cursor: &mut Cursor<&Vec<u8>>| { cursor.read_f64::<LittleEndian>() };
pub const READER_BI16: fn(&mut Cursor<&Vec<u8>>) -> IOResult<i16> = |cursor: &mut Cursor<&Vec<u8>>| { cursor.read_i16::<BigEndian>() };
pub const READER_BI32: fn(&mut Cursor<&Vec<u8>>) -> IOResult<i32> = |cursor: &mut Cursor<&Vec<u8>>| { cursor.read_i32::<BigEndian>() };
pub const READER_BI64: fn(&mut Cursor<&Vec<u8>>) -> IOResult<i64> = |cursor: &mut Cursor<&Vec<u8>>| { cursor.read_i64::<BigEndian>() };
pub const READER_BU16: fn(&mut Cursor<&Vec<u8>>) -> IOResult<u16> = |cursor: &mut Cursor<&Vec<u8>>| { cursor.read_u16::<BigEndian>() };
pub const READER_BU32: fn(&mut Cursor<&Vec<u8>>) -> IOResult<u32> = |cursor: &mut Cursor<&Vec<u8>>| { cursor.read_u32::<BigEndian>() };
pub const READER_BU64: fn(&mut Cursor<&Vec<u8>>) -> IOResult<u64> = |cursor: &mut Cursor<&Vec<u8>>| { cursor.read_u64::<BigEndian>() };
pub const READER_BF32: fn(&mut Cursor<&Vec<u8>>) -> IOResult<f32> = |cursor: &mut Cursor<&Vec<u8>>| { cursor.read_f32::<BigEndian>() };
pub const READER_BF64: fn(&mut Cursor<&Vec<u8>>) -> IOResult<f64> = |cursor: &mut Cursor<&Vec<u8>>| { cursor.read_f64::<BigEndian>() };
pub const READER_BOOL: fn(&mut Cursor<&Vec<u8>>) -> IOResult<bool> = |cursor: &mut Cursor<&Vec<u8>>| READER_U8(cursor).map(|value| value != 0);
pub const READER_STRING: fn(&mut Cursor<&Vec<u8>>) -> IOResult<String> = |cursor: &mut Cursor<&Vec<u8>>| {
    let mut buffer = Vec::new();
    cursor.read_to_end(&mut buffer)?; // Read the remaining bytes into the buffer
    String::from_utf8(buffer).map_err(|e| new_error(ErrorKind::InvalidData, e.to_string().as_str()))
};

pub const READER_AI8: fn(&mut Cursor<&Vec<u8>>, &mut [i8]) -> IOResult<()> = |cursor: &mut Cursor<&Vec<u8>>, arr: &mut [i8]| { cursor.read_i8_into(arr) };
pub const READER_AI16: fn(&mut Cursor<&Vec<u8>>, &mut [i16]) -> IOResult<()> = |cursor: &mut Cursor<&Vec<u8>>, arr: &mut [i16]| { cursor.read_i16_into::<LittleEndian>(arr) };
pub const READER_AI32: fn(&mut Cursor<&Vec<u8>>, &mut [i32]) -> IOResult<()> = |cursor: &mut Cursor<&Vec<u8>>, arr: &mut [i32]| { cursor.read_i32_into::<LittleEndian>(arr) };
pub const READER_AI64: fn(&mut Cursor<&Vec<u8>>, &mut [i64]) -> IOResult<()> = |cursor: &mut Cursor<&Vec<u8>>, arr: &mut [i64]| { cursor.read_i64_into::<LittleEndian>(arr) };
pub const READER_AU8: fn(&mut Cursor<&Vec<u8>>, &mut [u8]) -> IOResult<()> = |cursor: &mut Cursor<&Vec<u8>>, arr: &mut [u8]| { cursor.read_u8_into(arr) };
pub const READER_AU16: fn(&mut Cursor<&Vec<u8>>, &mut [u16]) -> IOResult<()> = |cursor: &mut Cursor<&Vec<u8>>, arr: &mut [u16]| { cursor.read_u16_into::<LittleEndian>(arr) };
pub const READER_AU32: fn(&mut Cursor<&Vec<u8>>, &mut [u32]) -> IOResult<()> = |cursor: &mut Cursor<&Vec<u8>>, arr: &mut [u32]| { cursor.read_u32_into::<LittleEndian>(arr) };
pub const READER_AU64: fn(&mut Cursor<&Vec<u8>>, &mut [u64]) -> IOResult<()> = |cursor: &mut Cursor<&Vec<u8>>, arr: &mut [u64]| { cursor.read_u64_into::<LittleEndian>(arr) };
pub const READER_AF32: fn(&mut Cursor<&Vec<u8>>, &mut [f32]) -> IOResult<()> = |cursor: &mut Cursor<&Vec<u8>>, arr: &mut [f32]| { cursor.read_f32_into::<LittleEndian>(arr) };
pub const READER_AF64: fn(&mut Cursor<&Vec<u8>>, &mut [f64]) -> IOResult<()> = |cursor: &mut Cursor<&Vec<u8>>, arr: &mut [f64]| { cursor.read_f64_into::<LittleEndian>(arr) };
pub const READER_ABI16: fn(&mut Cursor<&Vec<u8>>, &mut [i16]) -> IOResult<()> = |cursor: &mut Cursor<&Vec<u8>>, arr: &mut [i16]| { cursor.read_i16_into::<BigEndian>(arr) };
pub const READER_ABI32: fn(&mut Cursor<&Vec<u8>>, &mut [i32]) -> IOResult<()> = |cursor: &mut Cursor<&Vec<u8>>, arr: &mut [i32]| { cursor.read_i32_into::<BigEndian>(arr) };
pub const READER_ABI64: fn(&mut Cursor<&Vec<u8>>, &mut [i64]) -> IOResult<()> = |cursor: &mut Cursor<&Vec<u8>>, arr: &mut [i64]| { cursor.read_i64_into::<BigEndian>(arr) };
pub const READER_ABU16: fn(&mut Cursor<&Vec<u8>>, &mut [u16]) -> IOResult<()> = |cursor: &mut Cursor<&Vec<u8>>, arr: &mut [u16]| { cursor.read_u16_into::<BigEndian>(arr) };
pub const READER_ABU32: fn(&mut Cursor<&Vec<u8>>, &mut [u32]) -> IOResult<()> = |cursor: &mut Cursor<&Vec<u8>>, arr: &mut [u32]| { cursor.read_u32_into::<BigEndian>(arr) };
pub const READER_ABU64: fn(&mut Cursor<&Vec<u8>>, &mut [u64]) -> IOResult<()> = |cursor: &mut Cursor<&Vec<u8>>, arr: &mut [u64]| { cursor.read_u64_into::<BigEndian>(arr) };
pub const READER_ABF32: fn(&mut Cursor<&Vec<u8>>, &mut [f32]) -> IOResult<()> = |cursor: &mut Cursor<&Vec<u8>>, arr: &mut [f32]| { cursor.read_f32_into::<BigEndian>(arr) };
pub const READER_ABF64: fn(&mut Cursor<&Vec<u8>>, &mut [f64]) -> IOResult<()> = |cursor: &mut Cursor<&Vec<u8>>, arr: &mut [f64]| { cursor.read_f64_into::<BigEndian>(arr) };
pub const READER_ABOOL: fn(&mut Cursor<&Vec<u8>>, &mut [bool]) -> IOResult<()> = |cursor: &mut Cursor<&Vec<u8>>, arr: &mut [bool]| {
    for i in 0..arr.len() {
        arr[i] = READER_BOOL(cursor).unwrap();
    }
    return Ok(());
};
pub const READER_ASTRING: fn(&mut Cursor<&Vec<u8>>, &mut [String]) -> IOResult<()> = |cursor: &mut Cursor<&Vec<u8>>, arr: &mut [String]| {
    for i in 0..1 { //arr.len() {
        arr[i] = READER_STRING(cursor).unwrap();
    }
    return Ok(());
};
