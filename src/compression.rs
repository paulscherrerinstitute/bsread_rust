use std::io;

use lz4::block::decompress as lz4_decompress;


pub fn decompress_bitshuffle_lz4( compressed_data: &[u8], element_size: usize,  num_elements: usize,) -> io::Result<Vec<u8>> {
    let mut buffer = vec![0u8; element_size * num_elements];
    //#TODO: BITSHUFFLE
    //bitshuffle(&compressed_data, &mut buffer, element_size)?;
    let output = lz4_decompress(&buffer, None)?;
    Ok(output)
}

pub fn decompress_lz4( compressed_data: &[u8] ) -> io::Result<Vec<u8>> {
    let output = lz4_decompress(compressed_data, None)?;
    Ok(output)
}
