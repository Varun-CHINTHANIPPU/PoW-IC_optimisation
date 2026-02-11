use candid::{CandidType, Deserialize, Principal};
use ic_cdk::{query, update};
use ic_cdk::api::caller;
use std::cell::RefCell;

// ------------------------------------------------------------
// Public chain state
// ------------------------------------------------------------

#[derive(Clone, CandidType, Deserialize)]
pub struct ChainTip {
    pub height: u64,
    pub block_hash: String,
    pub difficulty: u32,
    pub last_update_ns: u64,
}

// ------------------------------------------------------------
// Internal state
// ------------------------------------------------------------

#[derive(Clone)]
struct State {
    tip: ChainTip,
    validator: Principal,
}

// ------------------------------------------------------------

thread_local! {
    static STATE: RefCell<Option<State>> = RefCell::new(None);
}

// ------------------------------------------------------------
// Init
// ------------------------------------------------------------

#[update]
pub fn init_chain(
    genesis_hash: String,
    initial_difficulty: u32,
    validator: Principal,
) {
    let now = ic_cdk::api::time();

    let tip = ChainTip {
        height: 0,
        block_hash: genesis_hash,
        difficulty: initial_difficulty,
        last_update_ns: now,
    };

    STATE.with(|s| {
        *s.borrow_mut() = Some(State {
            tip,
            validator,
        });
    });
}

// ------------------------------------------------------------
// Read API (used by coordinator / monitoring)
// ------------------------------------------------------------

#[query]
pub fn get_tip() -> ChainTip {
    STATE.with(|s| {
        s.borrow()
        .as_ref()
        .expect("chain not initialized")
        .tip
        .clone()
    })
}

#[query]
pub fn get_difficulty() -> u32 {
    STATE.with(|s| {
        s.borrow()
        .as_ref()
        .expect("chain not initialized")
        .tip
        .difficulty
    })
}

#[query]
pub fn get_height() -> u64 {
    STATE.with(|s| {
        s.borrow()
        .as_ref()
        .expect("chain not initialized")
        .tip
        .height
    })
}

// ------------------------------------------------------------
// Write API (validator only)
// ------------------------------------------------------------

#[update]
pub fn submit_valid_block(
    new_block_hash: String,
    new_difficulty: Option<u32>,
) {
    let caller = caller();

    STATE.with(|s| {
        let mut st = s.borrow_mut();
        let st = st.as_mut().expect("chain not initialized");

        if caller != st.validator {
            ic_cdk::trap("only validator can submit blocks");
        }

        st.tip.height += 1;
        st.tip.block_hash = new_block_hash;

        if let Some(d) = new_difficulty {
            st.tip.difficulty = d;
        }

        st.tip.last_update_ns = ic_cdk::api::time();
    });
}

// ------------------------------------------------------------
// Validator rotation (optional but real-world useful)
// ------------------------------------------------------------

#[update]
pub fn set_validator(new_validator: Principal) {
    let caller = caller();

    STATE.with(|s| {
        let mut st = s.borrow_mut();
        let st = st.as_mut().expect("chain not initialized");

        if caller != st.validator {
            ic_cdk::trap("only current validator can change validator");
        }

        st.validator = new_validator;
    });
}

// ------------------------------------------------------------
// Safety / admin helpers
// ------------------------------------------------------------

#[query]
pub fn get_validator() -> Principal {
    STATE.with(|s| {
        s.borrow()
        .as_ref()
        .expect("chain not initialized")
        .validator
    })
}
