// advanced.rs - Enhanced with cache and metrics
use std::cell::RefCell;
use candid::Principal;

use candid::{CandidType, Deserialize};
use ic_cdk::{query, update};
use ic_cdk::api::time;
use ic_cdk::api::{canister_balance, instruction_counter};

use crate::{mine_chunk_with_midstate, MiningStatus};

use crate::cache;
use crate::metrics;

pub use cache::{get_cache_stats, clear_cache, is_cached};
pub use metrics::{get_metrics, get_metrics_summary, reset_metrics, export_metrics_csv};

#[derive(Clone, CandidType, Deserialize)]
pub struct AdvancedTask {
    pub running: bool,
    pub block_data: String,
    pub difficulty: u32,
    pub next_nonce: u64,
    pub chunk_size: u64,
    pub total_attempts: u64,
    pub started_at: u64,
}

thread_local! {
    static TASK: RefCell<Option<AdvancedTask>> = RefCell::new(None);
}

// ------------------------------------------------------------
// Public API
// ------------------------------------------------------------

#[update]
pub fn start_advanced_mining(
    block_data: String,
    difficulty: u32,
    start_nonce: u64,
    chunk_size: u64,
) {
    // Check cache first
    if let Some((cached_nonce, cached_hash)) = cache::cache_lookup(&block_data, difficulty) {
        ic_cdk::println!(
            "Cache hit! Block already mined: nonce={}, hash={}",
            cached_nonce,
            cached_hash
        );
        metrics::record_cache_hit();
        return;
    }

    metrics::record_cache_miss();

    let task = AdvancedTask {
        running: true,
        block_data,
        difficulty,
        next_nonce: start_nonce,
        chunk_size,
        total_attempts: 0,
        started_at: time(),
    };

    TASK.with(|t| *t.borrow_mut() = Some(task));
}

#[update]
pub fn stop_advanced_mining() {
    TASK.with(|t| {
        if let Some(mut task) = t.borrow().clone() {
            task.running = false;
            *t.borrow_mut() = Some(task);
        }
    });
}

#[query]
pub fn get_advanced_status() -> Option<AdvancedTask> {
    TASK.with(|t| t.borrow().clone())
}

// ------------------------------------------------------------
// Heartbeat mining with cache and metrics
// ------------------------------------------------------------

#[ic_cdk::heartbeat]
fn advanced_heartbeat() {
    TASK.with(|cell| {
        let mut opt = cell.borrow_mut();

        let mut task = match opt.take() {
            Some(t) => t,
              None => return,
        };

        if !task.running {
            *opt = Some(task);
            return;
        }

        // Adaptive chunk sizing
        let chunk = adaptive_chunk_size(task.difficulty);

        if chunk != task.chunk_size {
            metrics::record_adaptive_change(chunk);
            task.chunk_size = chunk;
        }

        // Track performance
        let t0 = time();
        let i0 = instruction_counter();

        let (status, attempts) = mine_chunk_with_midstate(
            task.block_data.clone(),
                                                          task.difficulty,
                                                          task.next_nonce,
                                                          chunk,
        );

        let t1 = time();
        let i1 = instruction_counter();

        task.total_attempts += attempts;

        // Statistical early termination
        let should_terminate = !should_continue_mining(task.total_attempts, task.difficulty);

        if should_terminate {
            ic_cdk::println!(
                "Early termination after {} attempts (expected ~{})",
                             task.total_attempts,
                             expected_attempts_for_difficulty(task.difficulty)
            );

            // Record metrics
            metrics::record_chunk_result(
                attempts,
                t1 - t0,
                i1 - i0,
                false,
                true, // early terminated
            );

            task.running = false;
            *opt = Some(task);
            return;
        }

        match status {
            MiningStatus::Found { nonce, hash } => {
                ic_cdk::println!(
                    "✅ Advanced miner found solution: nonce={} hash={}",
                    nonce,
                    hash
                );

                // Store in cache
                cache::cache_store(
                    task.block_data.clone(),
                                   task.difficulty,
                                   nonce,
                                   hash.clone(),
                );

                // Record metrics
                metrics::record_chunk_result(
                    attempts,
                    t1 - t0,
                    i1 - i0,
                    true, // found solution
                    false,
                );

                task.running = false;
                *opt = Some(task);
            }

            MiningStatus::Continue { next_nonce } => {
                // Record metrics
                metrics::record_chunk_result(
                    attempts,
                    t1 - t0,
                    i1 - i0,
                    false, // no solution
                    false,
                );

                task.next_nonce = next_nonce;
                *opt = Some(task);
            }
        }
    });
}

// ------------------------------------------------------------
// Adaptive chunk sizing
// ------------------------------------------------------------

fn adaptive_chunk_size(difficulty: u32) -> u64 {
    const BASE: u64 = 200_000;
    const MIN: u64 = 20_000;
    const MAX: u64 = 2_000_000;

    let cycles = canister_balance();

    // Easier difficulty → larger chunks
    let diff_factor = if difficulty < 24 {
        1u64 << (24 - difficulty)
    } else {
        1
    };

    // More cycles → larger chunks
    let cycle_factor: u64 = ((cycles / 100_000_000_000u64).clamp(1, 5)) as u64;

    let mut size = BASE.saturating_mul(diff_factor).saturating_mul(cycle_factor);

    if size < MIN {
        size = MIN;
    }
    if size > MAX {
        size = MAX;
    }

    size
}

// ------------------------------------------------------------
// Statistical early termination
// ------------------------------------------------------------

fn should_continue_mining(attempts_so_far: u64, difficulty: u32) -> bool {
    let expected = expected_attempts_for_difficulty(difficulty);
    attempts_so_far <= expected.saturating_mul(3)
}

fn expected_attempts_for_difficulty(difficulty: u32) -> u64 {
    if difficulty >= 64 {
        u64::MAX
    } else {
        1u64 << difficulty
    }
}
