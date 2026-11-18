use crate::*;
use lz4::block::{decompress as lz4_decompress, CompressionMode};
use lz4::block::compress as lz4_compress;
use std::io::{Cursor};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::cmp;

const MAX_DECOMPRESSED_SIZE: usize = 1024 * 1024 * 100; // 100MB

unsafe extern "C" {
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
    if elem_size == 0 {
        return Err("Element size cannot be zero".to_string());
    }
    let mut c =  Cursor::new(input);
    let size_out = c
        .read_u64::<BigEndian>()
        .map_err(|e| format!("Failed to read output size: {}", e))? as usize;
    if size_out % elem_size !=0 {
        return Err("Output size is not a multiple of element size".to_string());
    }
    if size_out > MAX_DECOMPRESSED_SIZE {
        return Err(format!("Output size {} exceeds maximum {}", size_out, MAX_DECOMPRESSED_SIZE));
    }

    let elements = size_out/elem_size;
    let block_size = c
        .read_u32::<BigEndian>()
        .map_err(|e| format!("Failed to read block size: {}", e))?;
    if block_size == 0 {
        return Err("Block size cannot be zero".to_string());
    }
    if block_size % elem_size as u32 != 0 {
        return Err("Block size is not a multiple of element size".to_string());
    }
    let block_size = block_size / (elem_size as u32);
    if input.len() <12 {
        return Err("Input buffer is too small".to_string());
    }
    let blob = &input[12..];
    let mut output = vec![0u8; size_out];

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
    if elem_size == 0 {
        return Err("Element size cannot be zero".to_string());
    }
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
    (&mut output[0..8])
        .write_u64::<BigEndian>((elements * elem_size) as u64)
        .map_err(|e| e.to_string())?;

    (&mut output[8..12])
        .write_u32::<BigEndian>((block_size * elem_size) as u32)
        .map_err(|e| e.to_string())?;

    let blob_out = &mut output[12..];
    let ret  = unsafe {
        bshuf_compress_lz4(
            blob_in.as_ptr(),
            blob_out.as_mut_ptr(),
            elements ,
            elem_size,
            block_size,
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
        Err(e) => {Err(IOError::new(ErrorKind::InvalidData, e.as_str()))}
    }
}

pub fn decompress_lz4(compressed_data: &[u8], little_endian:bool) -> IOResult<Vec<u8>> {
    if compressed_data.len() < 4 {
        return Err(IOError::new(ErrorKind::UnexpectedEof, "LZ4 payload too short"));
    }
    let size = if little_endian {
        u32::from_le_bytes(compressed_data[0..4].try_into().unwrap())
    } else {
        u32::from_be_bytes(compressed_data[0..4].try_into().unwrap())
    };
    let size = i32::try_from(size)
        .map_err(|_| IOError::new(ErrorKind::InvalidData, "LZ4 size exceeds i32::MAX"))?;

    let blob: &[u8] = &compressed_data[4..];
    let output = lz4_decompress(blob, Some(size))?;
    Ok(output)
}


pub fn compress_bitshuffle_lz4(data: &[u8], element_size: usize) -> IOResult<Vec<u8>> {
    match bshuf_trans_bit_elem(data, element_size){
        Ok(out) => {Ok(out)}
        Err(e) => {Err(IOError::new(ErrorKind::InvalidData, e.as_str()))}
    }
}

pub fn compress_lz4(data: &[u8], little_endian:bool) -> IOResult<Vec<u8>> {
    let size = data.len();
    let header  = if little_endian {
        [size as u8,  (size >> 8) as u8, (size >> 16) as u8, (size >> 24) as u8]
    } else {
        [(size >> 24)  as u8,  (size >> 16) as u8, (size >> 8) as u8, size as u8]
    };

    let blob = lz4_compress(data, Some(CompressionMode::DEFAULT), false)?;

    let mut output = Vec::with_capacity(4 + blob.len());
    output.extend_from_slice(&header);
    output.extend_from_slice(&blob);
    Ok(output)
}


