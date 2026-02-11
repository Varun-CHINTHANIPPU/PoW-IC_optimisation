use candid::{CandidType, Deserialize};
use ic_cdk::{update, query};
use ic_cdk::api::time;
use ic_cdk::api::management_canister::main::{
    canister_status, CanisterIdRecord, CanisterStatusResponse,
};
use candid::Principal;

use std::cell::RefCell;

// ------------------------------------------------------------
// Configuration
// ------------------------------------------------------------

const DEFAULT_LOW_WATERMARK: u128 = 2_000_000_000_000; // 2T cycles
const DEFAULT_CRITICAL_WATERMARK: u128 = 500_000_000_000; // 0.5T

// ------------------------------------------------------------
// Public state
// ------------------------------------------------------------

#[derive(Clone, CandidType, Deserialize)]
pub struct WatchedCanister {
    pub canister: Principal,
    pub low_watermark: u128,
    pub critical_watermark: u128,
}

#[derive(Clone, CandidType, Deserialize)]
pub struct CanisterHealth {
    pub canister: Principal,
    pub cycles: u128,
    pub low_watermark: u128,
    pub critical_watermark: u128,
    pub is_low: bool,
    pub is_critical: bool,
    pub last_checked: u64,
}

#[derive(Clone, CandidType, Deserialize)]
pub struct RefuelerState {
    pub running: bool,
    pub watched: Vec<WatchedCanister>,
    pub last_report: Vec<CanisterHealth>,
    pub last_tick: u64,
}

thread_local! {
    static STATE: RefCell<RefuelerState> = RefCell::new(
        RefuelerState {
            running: false,
            watched: Vec::new(),
                                                        last_report: Vec::new(),
                                                        last_tick: 0,
        }
    );
}

// ------------------------------------------------------------
// Control API
// ------------------------------------------------------------

#[update]
pub fn start_refueler() {
    STATE.with(|s| {
        s.borrow_mut().running = true;
    });
}

#[update]
pub fn stop_refueler() {
    STATE.with(|s| {
        s.borrow_mut().running = false;
    });
}

#[update]
pub fn watch_canister(
    canister: Principal,
    low_watermark: Option<u128>,
    critical_watermark: Option<u128>,
) {
    let low = low_watermark.unwrap_or(DEFAULT_LOW_WATERMARK);
    let critical = critical_watermark.unwrap_or(DEFAULT_CRITICAL_WATERMARK);

    STATE.with(|s| {
        let mut st = s.borrow_mut();

        if st.watched.iter().any(|w| w.canister == canister) {
            return;
        }

        st.watched.push(WatchedCanister {
            canister,
            low_watermark: low,
            critical_watermark: critical,
        });
    });
}

#[update]
pub fn unwatch_canister(canister: Principal) {
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        st.watched.retain(|w| w.canister != canister);
    });
}

// ------------------------------------------------------------
// Read-only API
// ------------------------------------------------------------

#[query]
pub fn get_refueler_state() -> RefuelerState {
    STATE.with(|s| s.borrow().clone())
}

#[query]
pub fn last_report() -> Vec<CanisterHealth> {
    STATE.with(|s| s.borrow().last_report.clone())
}

// ------------------------------------------------------------
// Heartbeat
// ------------------------------------------------------------

#[ic_cdk::heartbeat]
fn heartbeat() {
    let should_run = STATE.with(|s| s.borrow().running);

    if !should_run {
        return;
    }

    ic_cdk::spawn(async {
        run_once().await;
    });
}

// ------------------------------------------------------------
// Core logic
// ------------------------------------------------------------

async fn run_once() {
    let watched = STATE.with(|s| s.borrow().watched.clone());

    if watched.is_empty() {
        return;
    }

    let mut report = Vec::new();

    for entry in watched.iter() {
        let rec = CanisterIdRecord {
            canister_id: entry.canister,
        };

        let status: Result<(CanisterStatusResponse,), _> =
        canister_status(rec).await;

        match status {
            Ok((st,)) => {
                let cycles = st.cycles;

                let is_critical = cycles < entry.critical_watermark;
                let is_low = cycles < entry.low_watermark;

                if is_critical {
                    ic_cdk::println!(
                        "[REFUELER] CRITICAL cycles for {} : {}",
                        entry.canister,
                        cycles
                    );
                } else if is_low {
                    ic_cdk::println!(
                        "[REFUELER] LOW cycles for {} : {}",
                        entry.canister,
                        cycles
                    );
                }

                report.push(CanisterHealth {
                    canister: entry.canister,
                    cycles: cycles.0.clone().try_into().unwrap_or(0u128),
                    low_watermark: entry.low_watermark,
                    critical_watermark: entry.critical_watermark,
                    is_low,
                    is_critical,
                    last_checked: time(),
                });
            }

            Err(e) => {
                ic_cdk::println!(
                    "[REFUELER] failed to query status for {} : {:?}",
                    entry.canister,
                    e
                );
            }
        }
    }

    STATE.with(|s| {
        let mut st = s.borrow_mut();
        st.last_report = report;
        st.last_tick = time();
    });
}
