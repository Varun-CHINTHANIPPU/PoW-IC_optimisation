// cache.rs - LRU cache for mined blocks
use std::cell::RefCell;
use std::collections::HashMap;
use candid::Principal;

use candid::{CandidType, Deserialize};
use ic_cdk::{query, update};

const MAX_CACHE_SIZE: usize = 1000;

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct CacheEntry {
    pub nonce: u64,
    pub hash: String,
    pub difficulty: u32,
    pub hits: u64,
    pub created_at: u64,
    pub last_accessed: u64,
}

pub struct LRUCache {
    entries: HashMap<String, CacheEntry>,
    access_order: Vec<String>, // LRU tracking
}

impl LRUCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            access_order: Vec::new(),
        }
    }

    pub fn get(&mut self, block_data: &str, difficulty: u32) -> Option<CacheEntry> {
        let key = Self::make_key(block_data, difficulty);

        if let Some(entry) = self.entries.get_mut(&key) {
            // Update access stats
            entry.hits += 1;
            entry.last_accessed = ic_cdk::api::time();

            // Move to end (most recently used)
            if let Some(pos) = self.access_order.iter().position(|k| k == &key) {
                self.access_order.remove(pos);
            }
            self.access_order.push(key.clone());

            return Some(entry.clone());
        }

        None
    }

    pub fn insert(&mut self, block_data: String, difficulty: u32, nonce: u64, hash: String) {
        let key = Self::make_key(&block_data, difficulty);

        // Evict LRU if at capacity
        if self.entries.len() >= MAX_CACHE_SIZE && !self.entries.contains_key(&key) {
            if let Some(lru_key) = self.access_order.first().cloned() {
                self.entries.remove(&lru_key);
                self.access_order.remove(0);
            }
        }

        let now = ic_cdk::api::time();

        self.entries.insert(
            key.clone(),
                            CacheEntry {
                                nonce,
                                hash,
                                difficulty,
                                hits: 0,
                                created_at: now,
                                last_accessed: now,
                            },
        );

        self.access_order.push(key);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.access_order.clear();
    }

    fn make_key(block_data: &str, difficulty: u32) -> String {
        format!("{}:{}", block_data, difficulty)
    }

    pub fn stats(&self) -> CacheStats {
        let total_hits: u64 = self.entries.values().map(|e| e.hits).sum();

        CacheStats {
            size: self.entries.len(),
            capacity: MAX_CACHE_SIZE,
            total_hits,
            hit_rate: if self.entries.is_empty() {
                0.0
            } else {
                total_hits as f64 / self.entries.len() as f64
            },
        }
    }
}

#[derive(Clone, CandidType, Deserialize)]
pub struct CacheStats {
    pub size: usize,
    pub capacity: usize,
    pub total_hits: u64,
    pub hit_rate: f64,
}

// Global cache instance
thread_local! {
    static CACHE: RefCell<LRUCache> = RefCell::new(LRUCache::new());
}

// ------------------------------------------------------------
// Public API for miner canister
// ------------------------------------------------------------

/// Try to get cached solution for block
pub fn cache_lookup(block_data: &str, difficulty: u32) -> Option<(u64, String)> {
    CACHE.with(|c| {
        c.borrow_mut()
        .get(block_data, difficulty)
        .map(|entry| (entry.nonce, entry.hash))
    })
}

/// Store successful mining result in cache
pub fn cache_store(block_data: String, difficulty: u32, nonce: u64, hash: String) {
    CACHE.with(|c| {
        c.borrow_mut().insert(block_data, difficulty, nonce, hash);
    });
}

/// Get cache statistics
#[query]
pub fn get_cache_stats() -> CacheStats {
    CACHE.with(|c| c.borrow().stats())
}

/// Clear all cache entries
#[update]
pub fn clear_cache() {
    CACHE.with(|c| c.borrow_mut().clear());
}

/// Check if block is in cache (for testing)
#[query]
pub fn is_cached(block_data: String, difficulty: u32) -> bool {
    CACHE.with(|c| c.borrow_mut().get(&block_data, difficulty).is_some())
}
