// validator/src/lib.rs - Complete PoW validation
use candid::{CandidType, Deserialize, Principal};
use ic_cdk::{caller, query, update};
use sha2::{Digest, Sha256};

// ------------------------------------------------------------
// Types
// ------------------------------------------------------------

#[derive(Clone, CandidType, Deserialize)]
pub struct Block {
    pub height: u64,
    pub prev_hash: String,
    pub block_data: String,
    pub nonce: u64,
    pub difficulty: u32,
    pub hash: String,
    pub timestamp: u64,
    pub miner: Option<Principal>,
}

#[derive(Clone, CandidType, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub reason: Option<String>,
}

// ------------------------------------------------------------
// Hash verification
// ------------------------------------------------------------

fn hash_block(block_data: &str, nonce: u64) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(block_data.as_bytes());
    hasher.update(nonce.to_le_bytes());
    hasher.finalize().into()
}

fn hash_to_hex(bytes: &[u8; 32]) -> String {
    hex::encode(bytes)
}

fn meets_difficulty(hash: &[u8; 32], difficulty: u32) -> bool {
    let mut remaining = difficulty;

    for b in hash.iter() {
        if remaining == 0 {
            return true;
        }

        let z = b.leading_zeros();

        if z >= remaining {
            return true;
        }

        if z < 8 {
            return false;
        }

        remaining -= 8;
    }

    remaining == 0
}

// ------------------------------------------------------------
// Validation functions
// ------------------------------------------------------------

#[query]
pub fn verify_pow(block_data: String, nonce: u64, difficulty: u32) -> ValidationResult {
    let hash = hash_block(&block_data, nonce);

    if meets_difficulty(&hash, difficulty) {
        ValidationResult {
            valid: true,
            reason: None,
        }
    } else {
        ValidationResult {
            valid: false,
            reason: Some(format!(
                "Hash does not meet difficulty {}. Hash: {}",
                difficulty,
                hash_to_hex(&hash)
            )),
        }
    }
}

#[query]
pub fn verify_block(block: Block) -> ValidationResult {
    // Verify PoW
    let computed_hash = hash_block(&block.block_data, block.nonce);
    let computed_hash_hex = hash_to_hex(&computed_hash);

    // Check hash matches
    if computed_hash_hex != block.hash {
        return ValidationResult {
            valid: false,
            reason: Some(format!(
                "Hash mismatch. Expected: {}, Computed: {}",
                block.hash, computed_hash_hex
            )),
        };
    }

    // Check difficulty
    if !meets_difficulty(&computed_hash, block.difficulty) {
        return ValidationResult {
            valid: false,
            reason: Some(format!(
                "Hash does not meet difficulty requirement {}",
                block.difficulty
            )),
        };
    }

    // Check timestamp is reasonable (within 1 hour of now)
    let now = ic_cdk::api::time();
    let one_hour_ns = 3_600_000_000_000u64;

    if block.timestamp > now + one_hour_ns {
        return ValidationResult {
            valid: false,
            reason: Some("Block timestamp is in the future".to_string()),
        };
    }

    // All checks passed
    ValidationResult {
        valid: true,
        reason: None,
    }
}

#[query]
pub fn verify_chain_segment(blocks: Vec<Block>) -> ValidationResult {
    if blocks.is_empty() {
        return ValidationResult {
            valid: false,
            reason: Some("Empty chain segment".to_string()),
        };
    }

    // Verify each block individually
    for block in &blocks {
        let result = verify_block(block.clone());
        if !result.valid {
            return result;
        }
    }

    // Verify chain linkage
    for i in 1..blocks.len() {
        if blocks[i].prev_hash != blocks[i - 1].hash {
            return ValidationResult {
                valid: false,
                reason: Some(format!(
                    "Chain break at height {}: prev_hash doesn't match",
                    blocks[i].height
                )),
            };
        }

        if blocks[i].height != blocks[i - 1].height + 1 {
            return ValidationResult {
                valid: false,
                reason: Some(format!(
                    "Height mismatch at position {}: expected {}, got {}",
                    i,
                    blocks[i - 1].height + 1,
                    blocks[i].height
                )),
            };
        }
    }

    ValidationResult {
        valid: true,
        reason: None,
    }
}

// ------------------------------------------------------------
// Difficulty calculation helpers
// ------------------------------------------------------------

#[query]
pub fn calculate_difficulty_adjustment(
    current_difficulty: u32,
    target_block_time_seconds: u64,
    actual_block_times_seconds: Vec<u64>,
) -> u32 {
    if actual_block_times_seconds.is_empty() {
        return current_difficulty;
    }

    // Average actual block time
    let sum: u64 = actual_block_times_seconds.iter().sum();
    let avg_time = sum / actual_block_times_seconds.len() as u64;

    // Adjust difficulty
    // If blocks too fast → increase difficulty
    // If blocks too slow → decrease difficulty

    const MAX_ADJUSTMENT: u32 = 2; // Limit adjustment per period

    if avg_time < target_block_time_seconds / 2 {
        // Much too fast - increase difficulty
        current_difficulty.saturating_add(MAX_ADJUSTMENT)
    } else if avg_time < target_block_time_seconds {
        // Slightly too fast - increase difficulty
        current_difficulty.saturating_add(1)
    } else if avg_time > target_block_time_seconds * 2 {
        // Much too slow - decrease difficulty
        current_difficulty.saturating_sub(MAX_ADJUSTMENT).max(1)
    } else if avg_time > target_block_time_seconds {
        // Slightly too slow - decrease difficulty
        current_difficulty.saturating_sub(1).max(1)
    } else {
        // Just right
        current_difficulty
    }
}

// ------------------------------------------------------------
// Batch validation (for efficiency)
// ------------------------------------------------------------

#[derive(CandidType, Deserialize)]
pub struct BatchValidationResult {
    pub total: usize,
    pub valid: usize,
    pub invalid: usize,
    pub invalid_indices: Vec<usize>,
}

#[query]
pub fn batch_verify_pow(
    blocks: Vec<(String, u64, u32)>, // (block_data, nonce, difficulty)
) -> BatchValidationResult {
    let total = blocks.len();
    let mut valid = 0;
    let mut invalid = 0;
    let mut invalid_indices = Vec::new();

    for (i, (block_data, nonce, difficulty)) in blocks.iter().enumerate() {
        let result = verify_pow(block_data.clone(), *nonce, *difficulty);

        if result.valid {
            valid += 1;
        } else {
            invalid += 1;
            invalid_indices.push(i);
        }
    }

    BatchValidationResult {
        total,
        valid,
        invalid,
        invalid_indices,
    }
}

// ------------------------------------------------------------
// Utility functions
// ------------------------------------------------------------

#[query]
pub fn compute_hash(block_data: String, nonce: u64) -> String {
    let hash = hash_block(&block_data, nonce);
    hash_to_hex(&hash)
}

#[query]
pub fn check_difficulty_level(hash_hex: String, difficulty: u32) -> bool {
    if let Ok(bytes) = hex::decode(&hash_hex) {
        if bytes.len() == 32 {
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&bytes);
            return meets_difficulty(&hash, difficulty);
        }
    }
    false
}
