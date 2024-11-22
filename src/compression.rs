use std::io;
use lz4::block::decompress as lz4_decompress;

fn bshuf_untrans_bit_elem(input: &[u8], output: &mut [u8], size: usize, elem_size: usize, ) -> Result<(), String> {
    if size % 8 != 0 {
        return Err(format!("Size ({}) must be a multiple of 8 for efficient bitshuffle.",size));
    }
    if output.len() < size * elem_size {
        return Err("Output buffer is smaller than required size.".to_string());
    }
    Err("Not implemented".to_string())
}

pub fn decompress_bitshuffle_lz4(compressed_data: &[u8], element_size: usize, num_elements: usize) -> io::Result<Vec<u8>> {
    let mut buffer = vec![0u8; element_size * num_elements];
    match bshuf_untrans_bit_elem(&compressed_data, buffer.as_mut_slice(), num_elements, element_size) {
        Ok(_) => {}
        Err(e) => { return Err(io::Error::new(io::ErrorKind::Other, e)); }
    }
    let output = lz4_decompress(&buffer, None)?;
    Ok(output)
}

pub fn decompress_lz4(compressed_data: &[u8]) -> io::Result<Vec<u8>> {
    let output = lz4_decompress(compressed_data, None)?;
    Ok(output)
}
