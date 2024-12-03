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
}

