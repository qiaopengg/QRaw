use std::collections::HashMap;
use std::sync::Mutex;
use std::time::SystemTime;

use super::types::FocusRegion;

struct CacheEntry {
    regions: Vec<FocusRegion>,
    modified: SystemTime,
}

pub(super) struct FocusCache {
    cache: Mutex<HashMap<String, CacheEntry>>,
    order: Mutex<std::collections::VecDeque<String>>,
    max_size: usize,
}

impl FocusCache {
    pub(super) fn new(max_size: usize) -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            order: Mutex::new(std::collections::VecDeque::new()),
            max_size,
        }
    }
    pub(super) fn get(&self, key: &str, modified: SystemTime) -> Option<Vec<FocusRegion>> {
        let mut cache = self.cache.lock().unwrap();
        match cache.get(key) {
            Some(entry) if entry.modified == modified => Some(entry.regions.clone()),
            Some(_) => {
                cache.remove(key);
                drop(cache);

                let mut order = self.order.lock().unwrap();
                if let Some(pos) = order.iter().position(|k| k == key) {
                    order.remove(pos);
                }
                None
            }
            None => None,
        }
    }
    pub(super) fn insert(&self, key: String, regions: Vec<FocusRegion>, modified: SystemTime) {
        let mut cache = self.cache.lock().unwrap();
        let mut order = self.order.lock().unwrap();
        if let Some(pos) = order.iter().position(|k| k == &key) {
            order.remove(pos);
            cache.remove(&key);
        }
        if cache.len() >= self.max_size {
            if let Some(oldest) = order.pop_front() {
                cache.remove(&oldest);
            }
        }
        order.push_back(key.clone());
        cache.insert(key, CacheEntry { regions, modified });
    }
    #[allow(dead_code)]
    pub(super) fn invalidate(&self, key: &str) {
        let mut cache = self.cache.lock().unwrap();
        let mut order = self.order.lock().unwrap();
        cache.remove(key);
        if let Some(pos) = order.iter().position(|k| k == key) {
            order.remove(pos);
        }
    }
}
