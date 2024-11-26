pub fn vec_to_hex_string(vec: &[u8]) -> String {
    vec.iter()
        .map(|byte| format!("0x{:02X}", byte)) // Format each byte as a two-digit hexadecimal
        .collect::<Vec<String>>()
        .join(", ") // Join all formatted strings with ", "
}


pub struct LimitedDebugArray<'a, T> {
    pub data: &'a [T],
    pub limit: usize,
}

impl<'a, T: std::fmt::Debug> std::fmt::Debug for LimitedDebugArray<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let len = self.data.len();
        let display_len = self.limit.min(len);
        let limited_data = &self.data[..display_len];
        write!(f, "{:?}", limited_data)?;
        if len > display_len {
            write!(f, " ... ({} more elements)", len - display_len)?;
        }
        Ok(())
    }
}

pub struct LimitedDebugVec<T> {
    pub data: Vec<T>,
    pub limit: usize,
}

impl<T: std::fmt::Debug> std::fmt::Debug for LimitedDebugVec<T>  {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let len = self.data.len();
        let display_len = self.limit.min(len);
        let limited_data = &self.data[..display_len];
        write!(f, "{:?}", limited_data)?; // Print the limited vector
        if len > display_len {
            write!(f, " ... ({} more elements)", len - display_len)?;
        }
        Ok(())
    }
}

