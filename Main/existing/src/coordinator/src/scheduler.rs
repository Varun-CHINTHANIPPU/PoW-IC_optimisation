use std::cell::RefCell;

use candid::{CandidType, Deserialize, Principal};
use ic_cdk::api::{call::call, time};
use ic_cdk::spawn;

const ASSIGN_TIMEOUT_NS: u64 = 10_000_000_000; // 10s
const MAX_FAILURES: u32 = 3;

#[derive(Clone)]
pub struct MinerSlot {
    pub id: Principal,
    pub busy: bool,
    pub assigned_at: u64,
    pub failures: u32,
    pub total_chunks: u64,
    pub successful_chunks: u64,
}

pub struct CoordinatorState {
    pub miners: Vec<MinerSlot>,
    pub next_nonce: u64,
    pub chunk_size: u64,
    pub running: bool,
    pub rr_cursor: usize,
    pub solution_found: Option<(u64, String)>,
    pub total_chunks_assigned: u64,
    pub started_at: u64,
}

thread_local! {
    static STATE: RefCell<Option<CoordinatorState>> = RefCell::new(None);
}

// ------------------------------------------------------------
// Public API
// ------------------------------------------------------------

pub fn start_scheduler(miners: Vec<Principal>, start_nonce: u64, chunk_size: u64) {
    let slots = miners
    .into_iter()
    .map(|p| MinerSlot {
        id: p,
         busy: false,
         assigned_at: 0,
         failures: 0,
         total_chunks: 0,
         successful_chunks: 0,
    })
    .collect();

    STATE.with(|s| {
        *s.borrow_mut() = Some(CoordinatorState {
            miners: slots,
            next_nonce: start_nonce,
            chunk_size,
            running: true,
            rr_cursor: 0,
            solution_found: None,
            total_chunks_assigned: 0,
            started_at: time(),
        });
    });
}

pub fn stop_scheduler() {
    STATE.with(|s| {
        if let Some(st) = s.borrow_mut().as_mut() {
            st.running = false;
        }
    });
}

// ------------------------------------------------------------
// Heartbeat tick - called every heartbeat
// ------------------------------------------------------------

pub fn tick(block_data: String, difficulty: u32) {
    spawn(async move {
        schedule_once(block_data, difficulty).await;
    });
}

// ------------------------------------------------------------
// Core scheduling logic
// ------------------------------------------------------------

async fn schedule_once(block_data: String, difficulty: u32) {
    let now = time();

    // Stop if solution already found
    let already_solved = STATE.with(|cell| {
        cell.borrow()
        .as_ref()
        .and_then(|st| st.solution_found.as_ref())
        .is_some()
    });
    if already_solved {
        return;
    }

    // Pick next idle miner
    let picked = STATE.with(|cell| {
        let mut st = cell.borrow_mut();
        let st = st.as_mut()?;

        if !st.running || st.miners.is_empty() {
            return None;
        }

        // Reclaim timed-out miners
        for m in st.miners.iter_mut() {
            if m.busy && now.saturating_sub(m.assigned_at) > ASSIGN_TIMEOUT_NS {
                ic_cdk::println!(
                    "Miner {} timeout after {}s",
                    m.id,
                    (now - m.assigned_at) / 1_000_000_000
                );
                m.busy = false;
                m.assigned_at = 0;
                m.failures += 1;
            }
        }

        // Round-robin selection
        let n = st.miners.len();
        for _ in 0..n {
            let i = st.rr_cursor % n;
            st.rr_cursor = (st.rr_cursor + 1) % n;

            let slot = &mut st.miners[i];

            if slot.busy { continue; }

            if slot.failures >= MAX_FAILURES {
                ic_cdk::println!("Miner {} disabled (failures={})", slot.id, slot.failures);
                continue;
            }

            let start = st.next_nonce;
            st.next_nonce += st.chunk_size;
            st.total_chunks_assigned += 1;
            slot.busy = true;
            slot.assigned_at = now;
            slot.total_chunks += 1;

            return Some((i, slot.id, start, st.chunk_size));
        }
        None
    });

    let (slot_index, miner, start, size) = match picked {
        Some(v) => v,
        None => return,
    };

    // Call mine_chunk_simple - returns (found, nonce, hash, attempts)
    // Using primitive types avoids ALL Candid variant encoding issues
    let result = call::<(String, u32, u64, u64), (bool, u64, String, u64)>(
        miner,
        "mine_chunk_simple",
        (block_data.clone(), difficulty, start, size),
    )
    .await;

    match result {
        Ok((found, nonce, hash, _attempts)) => {
            if found {
                ic_cdk::println!(
                    "✅ SOLUTION FOUND by {} | nonce={} | hash={}",
                    miner, nonce, hash
                );

                STATE.with(|s| {
                    if let Some(st) = s.borrow_mut().as_mut() {
                        st.solution_found = Some((nonce, hash.clone()));
                        st.running = false;
                        if let Some(slot) = st.miners.get_mut(slot_index) {
                            slot.busy = false;
                            slot.successful_chunks += 1;
                        }
                    }
                });

                broadcast_stop().await;

            } else {
                // No solution found in this chunk - mark miner idle
                STATE.with(|s| {
                    if let Some(st) = s.borrow_mut().as_mut() {
                        if let Some(slot) = st.miners.get_mut(slot_index) {
                            slot.busy = false;
                            slot.assigned_at = 0;
                            slot.successful_chunks += 1;
                        }
                    }
                });
            }
        }

        Err(e) => {
            ic_cdk::println!("❌ Miner {} call failed: {:?}", miner, e);
            STATE.with(|s| {
                if let Some(st) = s.borrow_mut().as_mut() {
                    if let Some(slot) = st.miners.get_mut(slot_index) {
                        slot.busy = false;
                        slot.assigned_at = 0;
                        slot.failures += 1;
                    }
                }
            });
        }
    }
}

// ------------------------------------------------------------
// Broadcast stop to all miners
// ------------------------------------------------------------

async fn broadcast_stop() {
    let miners = STATE.with(|s| {
        s.borrow()
        .as_ref()
        .map(|st| st.miners.iter().map(|m| m.id).collect::<Vec<_>>())
        .unwrap_or_default()
    });

    ic_cdk::println!("📢 Broadcasting stop to {} miners", miners.len());

    for miner in miners {
        let _ = call::<(), ()>(miner, "stop_advanced_mining", ())
        .await
        .map_err(|e| {
            ic_cdk::println!("Failed to stop miner {}: {:?}", miner, e);
        });
    }
}

// ------------------------------------------------------------
// Stats
// ------------------------------------------------------------

#[derive(CandidType, Deserialize, Clone)]
pub struct SchedulerStats {
    pub running: bool,
    pub total_miners: u64,
    pub idle_miners: u64,
    pub busy_miners: u64,
    pub failed_miners: u64,
    pub total_chunks_assigned: u64,
    pub next_nonce: u64,
    pub solution: Option<(u64, String)>,
    pub uptime_seconds: u64,
}

pub fn get_scheduler_stats() -> Option<SchedulerStats> {
    STATE.with(|s| {
        let st = s.borrow();
        let st = st.as_ref()?;

        let now = time();
        let uptime = (now - st.started_at) / 1_000_000_000;

        Some(SchedulerStats {
            running: st.running,
            total_miners: st.miners.len() as u64,
             idle_miners: st.miners.iter().filter(|m| !m.busy).count() as u64,
             busy_miners: st.miners.iter().filter(|m| m.busy).count() as u64,
             failed_miners: st.miners.iter().filter(|m| m.failures >= MAX_FAILURES).count() as u64,
             total_chunks_assigned: st.total_chunks_assigned,
             next_nonce: st.next_nonce,
             solution: st.solution_found.clone(),
             uptime_seconds: uptime,
        })
    })
}

pub use get_scheduler_stats as stats;
