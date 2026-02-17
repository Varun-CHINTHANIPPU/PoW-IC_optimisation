use ic_cdk::{query, update};
use ic_cdk::api::time;
use sha2::{Sha256, Digest};
use sha2::digest::FixedOutput;

mod cache;
mod metrics;
mod advanced;

pub use advanced::{
    start_advanced_mining,
    stop_advanced_mining,
    get_advanced_status,
    get_cache_stats,
    clear_cache,
    is_cached,
    get_metrics,
    get_metrics_summary,
    reset_metrics,
    export_metrics_csv,
};

// ------------------------------------------------------------
// Internal helpers
// ------------------------------------------------------------

pub fn hash_to_hex(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

pub fn meets_difficulty(hash: &[u8; 32], difficulty: u32) -> bool {
    let mut remaining = difficulty;
    for b in hash.iter() {
        if remaining == 0 { return true; }
        let z = b.leading_zeros();
        if z >= remaining { return true; }
        if z < 8 { return false; }
        remaining -= 8;
    }
    remaining == 0
}

// ------------------------------------------------------------
// MiningStatus enum (kept for backward compatibility)
// ------------------------------------------------------------

#[derive(candid::CandidType, serde::Deserialize, Clone)]
pub enum MiningStatus {
    Found {
        hash: String,
        nonce: u64,
    },
    Continue {
        next_nonce: u64,
    },
}

// ------------------------------------------------------------
// SHA mid-state helper
// ------------------------------------------------------------

#[derive(Clone)]
pub struct HashMidState {
    hasher: Sha256,
}

impl HashMidState {
    pub fn new(block_data: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(block_data.as_bytes());
        Self { hasher }
    }

    pub fn finalize_with_nonce(&self, nonce: u64) -> [u8; 32] {
        let mut h = self.hasher.clone();
        h.update(nonce.to_le_bytes());
        h.finalize_fixed().into()
    }
}

// ------------------------------------------------------------
// Core mining functions (with MiningStatus enum)
// ------------------------------------------------------------

#[update]
pub fn mine_chunk_with_midstate(
    block_data: String,
    difficulty: u32,
    start_nonce: u64,
    chunk_size: u64,
) -> (MiningStatus, u64) {
    let mid = HashMidState::new(&block_data);
    let mut nonce = start_nonce;
    let end = start_nonce.saturating_add(chunk_size);
    let mut attempts = 0u64;

    while nonce < end {
        let h = mid.finalize_with_nonce(nonce);
        if meets_difficulty(&h, difficulty) {
            return (MiningStatus::Found { hash: hash_to_hex(&h), nonce }, attempts);
        }
        nonce += 1;
        attempts += 1;
    }
    (MiningStatus::Continue { next_nonce: end }, attempts)
}

#[update]
pub fn mine_chunk_naive(
    block_data: String,
    difficulty: u32,
    start_nonce: u64,
    chunk_size: u64,
) -> (MiningStatus, u64) {
    let mut nonce = start_nonce;
    let end = start_nonce.saturating_add(chunk_size);
    let mut attempts = 0u64;

    while nonce < end {
        let mut hasher = Sha256::new();
        hasher.update(block_data.as_bytes());
        hasher.update(nonce.to_le_bytes());
        let hash: [u8; 32] = hasher.finalize_fixed().into();

        if meets_difficulty(&hash, difficulty) {
            return (MiningStatus::Found { hash: hash_to_hex(&hash), nonce }, attempts);
        }
        nonce += 1;
        attempts += 1;
    }
    (MiningStatus::Continue { next_nonce: end }, attempts)
}

// ------------------------------------------------------------
// SIMPLE mining function - no enum, no field order issues
// Returns: (found, nonce, hash, attempts)
// Used by coordinator to avoid Candid variant decoding issues
// ------------------------------------------------------------

#[update]
pub fn mine_chunk_simple(
    block_data: String,
    difficulty: u32,
    start_nonce: u64,
    chunk_size: u64,
) -> (bool, u64, String, u64) {
    let mid = HashMidState::new(&block_data);
    let mut nonce = start_nonce;
    let end = start_nonce.saturating_add(chunk_size);
    let mut attempts = 0u64;

    while nonce < end {
        let h = mid.finalize_with_nonce(nonce);
        if meets_difficulty(&h, difficulty) {
            return (true, nonce, hash_to_hex(&h), attempts);
        }
        nonce += 1;
        attempts += 1;
    }
    (false, end, String::new(), attempts)
}

// ------------------------------------------------------------
// Benchmark functions
// ------------------------------------------------------------

#[update]
pub fn benchmark_naive_chunk(
    block_data: String,
    difficulty: u32,
    start_nonce: u64,
    chunk_size: u64,
) -> (MiningStatus, u64, u64) {
    let t0 = time();
    let (status, attempts) = mine_chunk_naive(block_data, difficulty, start_nonce, chunk_size);
    let t1 = time();
    (status, attempts, t1 - t0)
}

#[update]
pub fn benchmark_midstate_chunk(
    block_data: String,
    difficulty: u32,
    start_nonce: u64,
    chunk_size: u64,
) -> (MiningStatus, u64, u64) {
    let t0 = time();
    let (status, attempts) = mine_chunk_with_midstate(block_data, difficulty, start_nonce, chunk_size);
    let t1 = time();
    (status, attempts, t1 - t0)
}

#[update]
pub fn benchmark_one_chunk(
    block_data: String,
    difficulty: u32,
    start_nonce: u64,
    chunk_size: u64,
) -> (u64, u64) {
    let t0 = time();
    let (_status, attempts) = mine_chunk_with_midstate(block_data, difficulty, start_nonce, chunk_size);
    let t1 = time();
    (attempts, t1 - t0)
}

// ------------------------------------------------------------
// Hash test helpers
// ------------------------------------------------------------

#[query]
pub fn test_naive_hash(block_data: String, nonce: u64) -> String {
    let mut h = Sha256::new();
    h.update(block_data.as_bytes());
    h.update(nonce.to_le_bytes());
    let arr: [u8; 32] = h.finalize_fixed().into(); hash_to_hex(&arr)
}

#[query]
pub fn test_midstate_hash(block_data: String, nonce: u64) -> String {
    let mid = HashMidState::new(&block_data);
    hash_to_hex(&mid.finalize_with_nonce(nonce))
}

// --------------------------------------------------------
// INSTRUCTION-BASED BENCHMARKS (work on local replica!)
// --------------------------------------------------------

/// Naive mining - returns (attempts, instructions_used)
#[update]
pub fn bench_naive_instructions(
    block_data: String,
    difficulty: u32,
    start_nonce: u64,
    chunk_size: u64,
) -> (u64, u64) {
    let i0 = ic_cdk::api::instruction_counter();
    let (_status, attempts) = mine_chunk_naive(block_data, difficulty, start_nonce, chunk_size);
    let i1 = ic_cdk::api::instruction_counter();
    (attempts, i1 - i0)
}

/// Midstate mining - returns (attempts, instructions_used)
#[update]
pub fn bench_midstate_instructions(
    block_data: String,
    difficulty: u32,
    start_nonce: u64,
    chunk_size: u64,
) -> (u64, u64) {
    let i0 = ic_cdk::api::instruction_counter();
    let (_status, attempts) = mine_chunk_with_midstate(block_data, difficulty, start_nonce, chunk_size);
    let i1 = ic_cdk::api::instruction_counter();
    (attempts, i1 - i0)
}
