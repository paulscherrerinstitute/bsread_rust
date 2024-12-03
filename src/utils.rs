
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



use std::collections::{HashMap, VecDeque};

pub struct LimitedHashMap<K, V> {
    map: HashMap<K, V>,
    order: VecDeque<K>,
    max_size: usize,
}

impl<K, V> LimitedHashMap<K, V>
where
    K: std::hash::Hash + Eq + Clone,
{
    pub fn new(max_size: usize) -> Self {
        Self {
            map: HashMap::with_capacity(max_size),
            order: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    pub fn void() -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            max_size: 1
        }
    }

    pub fn is_void(& self) -> bool {
        self.map.capacity()== 0 && self.max_size==1
    }

    /// Insert a key-value pair. Drops the oldest updated entry if size exceeds max_size.
    pub fn insert(&mut self, key: K, value: V) {
        // If the key already exists, remove it from the order tracking
        if self.map.contains_key(&key) {
            self.order.retain(|k| k != &key);
        }
        // Insert the key-value pair
        self.map.insert(key.clone(), value);
        self.order.push_back(key.clone());

        // If size exceeds max_size, remove the oldest entry
        if self.map.len() > self.max_size {
            if let Some(oldest_key) = self.order.pop_front() {
                self.map.remove(&oldest_key);
            }
        }
    }

    /// Get a reference to the value associated with a key
    pub fn get(&self, key: &K) -> Option<&V> {
        self.map.get(key)
    }

    /// Get the current size of the map
    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn remove(& mut self, key: &K) -> Option<V> {
        if self.map.contains_key(&key) {
            self.order.retain(|k| k != key);
        }
        self.map.remove(key)
    }
    pub fn keys(&self) -> Vec<K>{
        Vec::from(self.order.clone())
    }
    pub fn keys2(&self) -> Vec<K>{
        self.map.keys().cloned().collect()
    }
}

