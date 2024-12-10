use crate::*;
use lz4::block::{decompress as lz4_decompress, CompressionMode};
use lz4::block::compress as lz4_compress;
use std::io::{Cursor};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::cmp;


extern "C" {
    pub fn bshuf_decompress_lz4(
        input: *const u8,
        output: *mut u8,
        size: usize,
        elem_size: usize,
        block_size: usize,
    ) -> i64; // Corresponds to `int64_t` in C

    pub fn bshuf_compress_lz4(
        input: *const u8,
        output: *mut u8,
        size: usize,
        elem_size: usize,
        block_size: usize,
    ) -> i64;

    pub fn  bshuf_compress_lz4_bound(
        size: usize,
        elem_size: usize,
        block_size: usize) ->usize;
}

fn bshuf_untrans_bit_elem(input: &[u8],  elem_size: usize, ) -> Result<Vec<u8>, String> {
    let elem_size = elem_size;
    let mut c =  Cursor::new(input);
    let elements =   c.read_u64::<BigEndian>().unwrap() as usize;
    let block_size =c.read_u32::<BigEndian>().unwrap();
    let block_size = block_size / (elem_size as u32);
    let blob = &input[12..];
    let mut output = vec![0u8; elem_size * elements];

    let ret  = unsafe {
        bshuf_decompress_lz4(
            blob.as_ptr(),
            output.as_mut_ptr(),
            elements,
            elem_size,
            block_size as usize,
        )
    };

    // Check the return value for errors
    if ret < 0 {
        Err(format!("Decompression failed with error code {}", ret))
    } else {
        Ok(output)
    }
}


fn  bshuf_trans_bit_elem(input: &[u8],  elem_size: usize, ) -> Result<Vec<u8>, String> {
    let elem_size = elem_size;
    let blob_in = &input[0..];
    let target_block_size = 8192;
    let minimum_block_size = 128;
    let block_size_multiplier = 8;
    let block_size = target_block_size / elem_size;
    let block_size = (block_size / block_size_multiplier) * block_size_multiplier;
    let block_size = cmp::max(block_size, minimum_block_size);
    let elements= input.len() / elem_size;
    let output_bound = unsafe {
        bshuf_compress_lz4_bound(elements, elem_size, block_size)
    };
    let mut output = vec![0u8; output_bound+12];
    (&mut output[0..8]).write_u64::<BigEndian>(elements as u64).unwrap();
    (&mut output[8..12]).write_u32::<BigEndian>((block_size*elem_size) as u32).unwrap();

    let blob_out = &mut output[12..];
    let ret  = unsafe {
        bshuf_compress_lz4(
            blob_in.as_ptr(),
            blob_out.as_mut_ptr(),
            elements,
            elem_size,
            block_size as usize,
        )
    };
    // Check the return value for errors
    if ret < 0 {
        Err(format!("Compression failed with error code {}", ret))
    } else {
        let size = ret as usize;
        output.truncate(size+12);
        Ok(output)
    }
}



pub fn decompress_bitshuffle_lz4(compressed_data: &[u8], element_size: usize) -> IOResult<Vec<u8>> {
    match bshuf_untrans_bit_elem(&compressed_data, element_size) {
        Ok(out) => {Ok(out)}
        Err(e) => {Err(new_error(ErrorKind::InvalidInput, e.as_str()))}
    }
}

pub fn decompress_lz4(compressed_data: &[u8]) -> IOResult<Vec<u8>> {
    let output = lz4_decompress(compressed_data, None)?;
    Ok(output)
}


pub fn compress_bitshuffle_lz4(data: &[u8], element_size: usize) -> IOResult<Vec<u8>> {
    match bshuf_trans_bit_elem(data, element_size){
        Ok(out) => {Ok(out)}
        Err(e) => {Err(new_error(ErrorKind::InvalidInput, e.as_str()))}
    }
}

pub fn compress_lz4(data: &[u8]) -> IOResult<Vec<u8>> {
    let output = lz4_compress(data, Some(CompressionMode::DEFAULT), true)?;
    Ok(output)
}


