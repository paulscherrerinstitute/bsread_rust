use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex};
use md5::{Md5, Digest};

/// Ordered HashMap with a maximum size
pub struct LimitedHashMap<K, V> {
    map: HashMap<K, V>,
    order: VecDeque<K>,
    max_size: usize,
}

impl<K, V> LimitedHashMap<K, V>
where
    K: std::hash::Hash + Eq + Clone,
{
    /// Creates a new LimitedHashMap given its maximum size
    pub fn new(max_size: usize) -> Self {
        Self {
            map: HashMap::with_capacity(max_size),
            order: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    /// Creates a void instance - to signalize a member that must be initialized
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

    /// Remove one element of the map
    pub fn remove(& mut self, key: &K) -> Option<V> {
        if self.map.contains_key(&key) {
            self.order.retain(|k| k != key);
        }
        self.map.remove(key)
    }

    /// Return a copy of the map keys
    pub fn keys(&self) -> Vec<K>{
        Vec::from(self.order.clone())
    }
}


/// Thread-safe FIFO queue with a maximum size
pub struct FifoQueue<K> {
    queue: Mutex<VecDeque<K>>,          // Thread-safe FIFO
    dropped_count: Mutex<u32>,        // Counter for dropped items
    max_size: usize,                  // Maximum size of the FIFO
}

impl<K> FifoQueue<K> {
    /// Creates a new FifoQueue given its maximum size
    pub  fn new(max_size: usize) -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            dropped_count: Mutex::new(0),
            max_size,
        }
    }

    /// Adds a message to the FIFO. Drops the oldest if the FIFO is full.
    pub  fn add(&self, message: K) {
        let mut queue = self.queue.lock().unwrap();
        let mut dropped_count = self.dropped_count.lock().unwrap();

        if queue.len() >= self.max_size {
            queue.pop_front(); // Drop the oldest element
            *dropped_count += 1; // Increment the dropped counter
        }
        queue.push_back(message);
    }

    /// Retrieves the next message from the FIFO, or `None` if empty.
    pub fn get(&self) -> Option<K> {
        let mut queue = self.queue.lock().unwrap();
        queue.pop_front()
    }

    /// Retrieves the total count of dropped messages.
    pub  fn get_dropped_count(&self) -> u32 {
        *self.dropped_count.lock().unwrap()
    }

    /// Retrieves the count of available messages.
    pub fn get_available_count(&self) -> usize {
        self.queue.lock().unwrap().len()
    }
}


pub fn get_hash(bytes: &[u8]) -> String{
    let mut hasher = Md5::new();
    hasher.update(bytes);
    let result = hasher.finalize();
    let hash_hex = format!("{:x}", result);
    hash_hex
}