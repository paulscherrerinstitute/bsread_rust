use crate::*;
use std::string::String;
use byteorder::{LittleEndian, BigEndian, WriteBytesExt};
use std::io::{Cursor, Write};

pub const WRITER_I8: fn(&mut Cursor<&mut Vec<u8>>, &i8) -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, v: &i8| { cursor.write_i8(*v)};
pub const WRITER_I16: fn(&mut Cursor<&mut Vec<u8>>, &i16)  -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, v:&i16| { cursor.write_i16::<LittleEndian>(*v) };
pub const WRITER_I32: fn(&mut Cursor<&mut Vec<u8>>, &i32)  -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, v:&i32| { cursor.write_i32::<LittleEndian>(*v) };
pub const WRITER_I64: fn(&mut Cursor<&mut Vec<u8>>, &i64)  -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, v:&i64| { cursor.write_i64::<LittleEndian>(*v) };
pub const WRITER_U8: fn(&mut Cursor<&mut Vec<u8>>, &u8)  -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, v:&u8| { cursor.write_u8(*v) };
pub const WRITER_U16: fn(&mut Cursor<&mut Vec<u8>>, &u16)  -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, v:&u16| { cursor.write_u16::<LittleEndian>(*v) };
pub const WRITER_U32: fn(&mut Cursor<&mut Vec<u8>>, &u32)  -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, v:&u32| { cursor.write_u32::<LittleEndian>(*v) };
pub const WRITER_U64: fn(&mut Cursor<&mut Vec<u8>>, &u64)  -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, v:&u64| { cursor.write_u64::<LittleEndian>(*v) };
pub const WRITER_F32: fn(&mut Cursor<&mut Vec<u8>>, &f32)  -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, v:&f32| { cursor.write_f32::<LittleEndian>(*v) };
pub const WRITER_F64: fn(&mut Cursor<&mut Vec<u8>>, &f64)  -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, v:&f64| { cursor.write_f64::<LittleEndian>(*v) };
pub const WRITER_BI16: fn(&mut Cursor<&mut Vec<u8>>, &i16)  -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, v:&i16| { cursor.write_i16::<BigEndian>(*v) };
pub const WRITER_BI32: fn(&mut Cursor<&mut Vec<u8>>, &i32)  -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, v:&i32| { cursor.write_i32::<BigEndian>(*v) };
pub const WRITER_BI64: fn(&mut Cursor<&mut Vec<u8>>, &i64)  -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, v:&i64| { cursor.write_i64::<BigEndian>(*v) };
pub const WRITER_BU16: fn(&mut Cursor<&mut Vec<u8>>, &u16)  -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, v:&u16| { cursor.write_u16::<BigEndian>(*v) };
pub const WRITER_BU32: fn(&mut Cursor<&mut Vec<u8>>, &u32)  -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, v:&u32| { cursor.write_u32::<BigEndian>(*v) };
pub const WRITER_BU64: fn(&mut Cursor<&mut Vec<u8>>, &u64)  -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, v:&u64| { cursor.write_u64::<BigEndian>(*v) };
pub const WRITER_BF32: fn(&mut Cursor<&mut Vec<u8>>, &f32)  -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, v:&f32| { cursor.write_f32::<BigEndian>(*v) };
pub const WRITER_BF64: fn(&mut Cursor<&mut Vec<u8>>, &f64)  -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, v:&f64| { cursor.write_f64::<BigEndian>(*v) };
pub const WRITER_BOOL: fn(&mut Cursor<&mut Vec<u8>>, &bool)  -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, v:&bool| WRITER_U8(cursor,if *v{&1u8} else {&0u8});
pub const WRITER_STRING: fn(&mut Cursor<&mut Vec<u8>>, &String)  -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, v:&String| {cursor.write_all(v.as_bytes())};

pub const WRITER_AI8: fn(&mut Cursor<&mut Vec<u8>>, &[i8]) -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, arr: &[i8]| {
    for i in 0..arr.len() { WRITER_I8(cursor, &arr[i])?;} return Ok(());
};
pub const WRITER_AI16: fn(&mut Cursor<&mut Vec<u8>>, &[i16]) -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, arr: &[i16]| {
    for i in 0..arr.len() { WRITER_I16(cursor, &arr[i])?;} return Ok(());
};
pub const WRITER_AI32: fn(&mut Cursor<&mut Vec<u8>>, &[i32]) -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, arr: &[i32]| {
    for i in 0..arr.len() { WRITER_I32(cursor, &arr[i])?;} return Ok(());
};
pub const WRITER_AI64: fn(&mut Cursor<&mut Vec<u8>>, &[i64]) -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, arr: &[i64]|{
    for i in 0..arr.len() { WRITER_I64(cursor, &arr[i])?;} return Ok(());
};
pub const WRITER_AU8: fn(&mut Cursor<&mut Vec<u8>>, &[u8]) -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, arr: &[u8]| {
    for i in 0..arr.len() { WRITER_U8(cursor, &arr[i])?;} return Ok(());
};
pub const WRITER_AU16: fn(&mut Cursor<&mut Vec<u8>>, &[u16]) -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, arr: &[u16]| {
    for i in 0..arr.len() { WRITER_U16(cursor, &arr[i])?;} return Ok(());
};
pub const WRITER_AU32: fn(&mut Cursor<&mut Vec<u8>>, &[u32]) -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, arr: &[u32]| {
    for i in 0..arr.len() { WRITER_U32(cursor, &arr[i])?;} return Ok(());
};
pub const WRITER_AU64: fn(&mut Cursor<&mut Vec<u8>>, &[u64]) -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, arr: &[u64]| {
    for i in 0..arr.len() { WRITER_U64(cursor, &arr[i])?;} return Ok(());
};
pub const WRITER_AF32: fn(&mut Cursor<&mut Vec<u8>>, &[f32]) -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, arr: &[f32]| {
    for i in 0..arr.len() { WRITER_F32(cursor, &arr[i])?;} return Ok(());
};
pub const WRITER_AF64: fn(&mut Cursor<&mut Vec<u8>>, &[f64]) -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, arr: &[f64]| {
    for i in 0..arr.len() { WRITER_F64(cursor, &arr[i])?;} return Ok(());
};
pub const WRITER_ABI16: fn(&mut Cursor<&mut Vec<u8>>, &[i16]) -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, arr: &[i16]| {
    for i in 0..arr.len() { WRITER_BI16(cursor, &arr[i])?;} return Ok(());
};
pub const WRITER_ABI32: fn(&mut Cursor<&mut Vec<u8>>, &[i32]) -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, arr: &[i32]| {
    for i in 0..arr.len() { WRITER_BI32(cursor, &arr[i])?;} return Ok(());
};
pub const WRITER_ABI64: fn(&mut Cursor<&mut Vec<u8>>, &[i64]) -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, arr: &[i64]| {
    for i in 0..arr.len() { WRITER_BI64(cursor, &arr[i])?;} return Ok(());
};
pub const WRITER_ABU16: fn(&mut Cursor<&mut Vec<u8>>, &[u16]) -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, arr: &[u16]| {
    for i in 0..arr.len() { WRITER_BU16(cursor, &arr[i])?;} return Ok(());
};
pub const WRITER_ABU32: fn(&mut Cursor<&mut Vec<u8>>, &[u32]) -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, arr: &[u32]| {
    for i in 0..arr.len() { WRITER_BU32(cursor, &arr[i])?;} return Ok(());
};
pub const WRITER_ABU64: fn(&mut Cursor<&mut Vec<u8>>, &[u64]) -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, arr: &[u64]|{
    for i in 0..arr.len() { WRITER_BU64(cursor, &arr[i])?;} return Ok(());
};
pub const WRITER_ABF32: fn(&mut Cursor<&mut Vec<u8>>, &[f32]) -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, arr: &[f32]| {
    for i in 0..arr.len() { WRITER_BF32(cursor, &arr[i])?;} return Ok(());
};
pub const WRITER_ABF64: fn(&mut Cursor<&mut Vec<u8>>, &[f64]) -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, arr: &[f64]| {
    for i in 0..arr.len() { WRITER_BF64(cursor, &arr[i])?;} return Ok(());
};
pub const WRITER_ABOOL: fn(&mut Cursor<&mut Vec<u8>>, &[bool]) -> IOResult<()>= |cursor: &mut Cursor<&mut Vec<u8>>, arr: &[bool]| {
    for i in 0..arr.len() { WRITER_BOOL(cursor, &arr[i])?;} return Ok(());
};
pub const WRITER_ASTRING: fn(&mut Cursor<&mut  Vec<u8>>, &[String]) -> IOResult<()> = |cursor: &mut Cursor<&mut Vec<u8>>, arr: &[String]| {
    for i in 0..1 { WRITER_STRING(cursor, &arr[i].clone())?;} return Ok(());
};
