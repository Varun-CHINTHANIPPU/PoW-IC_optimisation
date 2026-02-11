mod scheduler;

use std::cell::RefCell;
use candid::{CandidType, Deserialize, Principal};
use ic_cdk::{update, heartbeat, query};  // Added query here
use ic_cdk::api::call::call;
use sha2::{Digest, Sha256};

use crate::scheduler::{start_scheduler, stop_scheduler, tick};
use crate::scheduler::{stats as scheduler_stats, SchedulerStats};

// ------------------------------------------------------------
// Target for heartbeat scheduler
// ------------------------------------------------------------

thread_local! {
    static TARGET: RefCell<Option<(String, u32)>> = RefCell::new(None);
}

// ------------------------------------------------------------
// Dynamic redistribution entrypoints
// ------------------------------------------------------------

#[update]
pub fn start_dynamic_mining(
    miners: Vec<Principal>,
    block_data: String,
    difficulty: u32,
    start_nonce: u64,
    chunk_size: u64,
) {
    TARGET.with(|t| {
        *t.borrow_mut() = Some((block_data.clone(), difficulty));
    });

    start_scheduler(miners, start_nonce, chunk_size);
}

#[update]
pub fn stop_dynamic_mining() {
    stop_scheduler();

    TARGET.with(|t| {
        *t.borrow_mut() = None;
    });
}

#[heartbeat]
fn coordinator_heartbeat() {
    TARGET.with(|t| {
        if let Some((ref block, diff)) = *t.borrow() {
            tick(block.clone(), diff);
        }
    });
}

// ------------------------------------------------------------
// Shared types (must match miner canister)
// ------------------------------------------------------------

#[derive(CandidType, Deserialize, Clone)]
pub enum MiningStatus {
    Found {
        hash: String,
        nonce: u64,
    },
    Continue {
        next_nonce: u64,
    },
}


#[derive(CandidType, Deserialize)]
pub struct MiningResult {
    pub found: bool,
    pub nonce: u64,
    pub hash: String,
}

// ------------------------------------------------------------
// Deterministic VRF-like helpers
// ------------------------------------------------------------

fn vrf_seed(prev_block_hash: &str, round: u64) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(prev_block_hash.as_bytes());
    h.update(round.to_le_bytes());
    h.finalize().into()
}

fn offset_for_miner(seed: &[u8; 32], miner_index: u64) -> u64 {
    let mut h = Sha256::new();
    h.update(seed);
    h.update(miner_index.to_le_bytes());
    let out = h.finalize();

    let mut buf = [0u8; 8];
    buf.copy_from_slice(&out[0..8]);
    u64::from_le_bytes(buf)
}

// ------------------------------------------------------------
// VRF based parallel coordinator (single round fan-out)
// ------------------------------------------------------------

#[update]
pub async fn start_vrf_parallel_mining(
    miner_canisters: Vec<Principal>,
    block_data: String,
    difficulty: u32,
    prev_block_hash: String,
    round: u64,
    base_start: u64,
    range_per_miner: u64,
) -> Option<MiningResult> {

    let seed = vrf_seed(&prev_block_hash, round);

    let mut calls = Vec::new();

    for (i, miner) in miner_canisters.iter().enumerate() {

        let offset = offset_for_miner(&seed, i as u64);

        let start =
        base_start
        .wrapping_add(offset)
        .wrapping_add((i as u64) * range_per_miner);

        let fut = call::<
        (String, u32, u64, u64),
        ((MiningStatus, u64),)
        >(
            *miner,
          "mine_chunk_with_midstate",
          (
              block_data.clone(),
           difficulty,
           start,
           range_per_miner,
          ),
        );

        calls.push(fut);
    }

    // First valid solution wins
    for fut in calls {
        if let Ok(((status, _attempts),)) = fut.await {
            if let MiningStatus::Found { nonce, hash } = status {
                return Some(MiningResult {
                    found: true,
                    nonce,
                    hash,
                });
            }
        }
    }

    None
}

// ------------------------------------------------------------
// Single miner helper (used by scheduler if needed later)
// ------------------------------------------------------------

#[update]
pub async fn assign_one_chunk(
    miner: Principal,
    block_data: String,
    difficulty: u32,
    start_nonce: u64,
    chunk_size: u64,
) -> Option<MiningResult> {

    let res = call::<
    (String, u32, u64, u64),
    ((MiningStatus, u64),)
    >(
        miner,
      "mine_chunk_with_midstate",
      (
          block_data,
       difficulty,
       start_nonce,
       chunk_size,
      ),
    )
    .await;

    if let Ok(((status, _attempts),)) = res {
        if let MiningStatus::Found { nonce, hash } = status {
            return Some(MiningResult {
                found: true,
                nonce,
                hash,
            });
        }
    }

    None
}

#[query]
pub fn get_scheduler_stats() -> Option<SchedulerStats> {
    scheduler_stats()
}

