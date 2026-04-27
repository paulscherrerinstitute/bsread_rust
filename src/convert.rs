use std::mem;
use num_traits::{NumCast, ToPrimitive};

pub fn cast_arr<T, U>(input: &Vec<T>) -> Option<Vec<U>>
where
    T: Copy + NumCast,
    U: NumCast,
{
    input.iter().map(|&item| num_traits::cast(item)).collect()
}

pub fn to_str_array<T: ToString>(input: &Vec<T>) -> Option<Vec<String>> {
    // Map each item to its string representation and collect the results into a Vec<String>
    Some(input.iter().map(|item| item.to_string()).collect())
}

pub fn cast_bool_arr<U>(input: &Vec<bool>) -> Option<Vec<U>>
where
    U: NumCast,       // U must implement NumCast
{
    let au32: Vec<u32> =  input.iter().map(|b| if *b { 1 } else { 0 }).collect();
    cast_arr::<u32, U>(&au32)
}


pub fn cast<T, U>(input: &T) -> Option<U>
where
    T: NumCast + Copy,
    U: NumCast,
{
    num_traits::cast(*input)
}


pub fn cast_bool<U>(input: &bool) -> Option<U>
where
    U: NumCast,       // U must implement NumCast
{
    let v = if *input { 1 } else { 0 };
    U::from(v)
}

pub fn array_to_bytes<T: Copy>(input: &Vec<T>) -> Vec<u8> {
    let len_bytes = input.len() * std::mem::size_of::<T>();
    let mut out = Vec::<u8>::with_capacity(len_bytes);

    unsafe {
        let ptr = input.as_ptr() as *const u8;
        let slice = std::slice::from_raw_parts(ptr, len_bytes);
        out.extend_from_slice(slice);
    }
    out
}

pub fn array_as_bytes<T: Copy>(input: &Vec<T>) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            input.as_ptr() as *const u8,
            input.len() * std::mem::size_of::<T>(),
        )
    }
}

pub fn scalar_to_bytes<T: Copy>(input: &T) -> Vec<u8> {
    let size = std::mem::size_of::<T>();
    let mut out = Vec::with_capacity(size);
    unsafe {
        let ptr = (input as *const T) as *const u8;
        let slice = std::slice::from_raw_parts(ptr, size);
        out.extend_from_slice(slice);
    }

    out
}

pub fn scalar_as_bytes<T: Copy>(input: &T) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            (input as *const T) as *const u8,
            std::mem::size_of::<T>(),
        )
    }
}

pub fn bool_to_bytes(input: &bool) -> Vec<u8> {
    vec![*input as u8]
}
pub fn bool_as_bytes(input: &bool) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(input as *const bool as *const u8, 1)
    }
}
pub fn str_as_bytes(input: &str) -> &[u8] {
    input.as_bytes()
}

pub fn str_to_bytes(input: &str) -> Vec<u8> {
    input.as_bytes().to_vec()
}

pub fn bool_arr_to_bytes(input: &Vec<bool>) -> Vec<u8> {
    input.iter().map(|&b| b as u8).collect()
}