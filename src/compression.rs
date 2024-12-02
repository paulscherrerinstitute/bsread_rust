use crate::IOResult;
use std::io;
use lz4::block::decompress as lz4_decompress;
use std::io::{Cursor, Write};
use byteorder::{LittleEndian, BigEndian, ReadBytesExt};


extern "C" {
    pub fn bshuf_decompress_lz4(
        input: *const u8,
        output: *mut u8,
        size: usize,
        elem_size: usize,
        block_size: usize,
    ) -> i64; // Corresponds to `int64_t` in C
}

fn bshuf_untrans_bit_elem(input: &[u8],  elem_size: usize, ) -> Result<Vec<u8>, String> {

    let mut c =  Cursor::new(input);
    let elements =   c.read_u64::<BigEndian>().unwrap() as usize;
    let block_size =c.read_u32::<BigEndian>().unwrap();
    let block_size = block_size / (elem_size as u32);
    let blob = &input[12..];

    //if elements % 8 != 0 {
    //    return Err(format!("Size ({}) must be a multiple of 8 for efficient bitshuffle.",elements));
    //}
    let mut output = vec![0u8; elem_size * elements];
    //let out = lz4_decompress(blob, None).unwrap(); // Check if LZ4 encodinf is ok
    //Err("Not implemented".to_string())

    let ret  = unsafe {
        bshuf_decompress_lz4(
            blob.as_ptr(),
            output.as_mut_ptr(),
            elements as usize /elem_size,
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

pub fn decompress_bitshuffle_lz4(compressed_data: &[u8], element_size: usize) -> IOResult<Vec<u8>> {
    match bshuf_untrans_bit_elem(&compressed_data, element_size) {
        Ok(out) => {Ok(out)}
        Err(e) => { return Err(io::Error::new(io::ErrorKind::Other, e)); }
    }
    //let output = lz4_decompress(&buffer, None)?;
    //Ok(output)
}

pub fn decompress_lz4(compressed_data: &[u8]) -> IOResult<Vec<u8>> {
    let output = lz4_decompress(compressed_data, None)?;
    Ok(output)
}
