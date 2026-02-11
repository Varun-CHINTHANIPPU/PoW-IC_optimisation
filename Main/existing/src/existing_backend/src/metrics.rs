// metrics.rs - Comprehensive performance metrics
use std::cell::RefCell;
use candid::Principal;

use candid::{CandidType, Deserialize};
use ic_cdk::{query, update};

#[derive(Clone, CandidType, Deserialize, Default)]
pub struct MiningMetrics {
    // Mining performance
    pub total_chunks_mined: u64,
    pub total_hashes_computed: u64,
    pub successful_chunks: u64,
    pub failed_chunks: u64,

    // Timing
    pub total_mining_time_ns: u64,
    pub fastest_chunk_ns: u64,
    pub slowest_chunk_ns: u64,

    // Instructions
    pub total_instructions: u64,
    pub min_instructions_per_hash: u64,
    pub max_instructions_per_hash: u64,

    // Cache performance
    pub cache_hits: u64,
    pub cache_misses: u64,

    // Early termination
    pub early_terminations: u64,
    pub chunks_abandoned: u64,

    // Adaptive chunking
    pub adaptive_chunk_changes: u64,
    pub avg_chunk_size: u64,

    // Solutions found
    pub solutions_found: u64,
    pub last_solution_time: u64,
}

impl MiningMetrics {
    pub fn record_chunk(
        &mut self,
        hashes: u64,
        time_ns: u64,
        instructions: u64,
        found_solution: bool,
        early_terminated: bool,
    ) {
        self.total_chunks_mined += 1;
        self.total_hashes_computed += hashes;
        self.total_mining_time_ns += time_ns;
        self.total_instructions += instructions;

        if found_solution {
            self.successful_chunks += 1;
            self.solutions_found += 1;
            self.last_solution_time = ic_cdk::api::time();
        } else if early_terminated {
            self.chunks_abandoned += 1;
            self.early_terminations += 1;
        } else {
            self.failed_chunks += 1;
        }

        // Update timing stats
        if self.fastest_chunk_ns == 0 || time_ns < self.fastest_chunk_ns {
            self.fastest_chunk_ns = time_ns;
        }
        if time_ns > self.slowest_chunk_ns {
            self.slowest_chunk_ns = time_ns;
        }

        // Update instruction stats
        if hashes > 0 {
            let instr_per_hash = instructions / hashes;

            if self.min_instructions_per_hash == 0
                || instr_per_hash < self.min_instructions_per_hash
                {
                    self.min_instructions_per_hash = instr_per_hash;
                }

                if instr_per_hash > self.max_instructions_per_hash {
                    self.max_instructions_per_hash = instr_per_hash;
                }
        }
    }

    pub fn record_cache_hit(&mut self) {
        self.cache_hits += 1;
    }

    pub fn record_cache_miss(&mut self) {
        self.cache_misses += 1;
    }

    pub fn record_adaptive_change(&mut self, new_chunk_size: u64) {
        self.adaptive_chunk_changes += 1;
        // Running average
        if self.avg_chunk_size == 0 {
            self.avg_chunk_size = new_chunk_size;
        } else {
            self.avg_chunk_size =
            (self.avg_chunk_size + new_chunk_size) / 2;
        }
    }

    pub fn summary(&self) -> MetricsSummary {
        let cache_total = self.cache_hits + self.cache_misses;
        let cache_hit_rate = if cache_total > 0 {
            (self.cache_hits as f64 / cache_total as f64) * 100.0
        } else {
            0.0
        };

        let avg_time_per_chunk = if self.total_chunks_mined > 0 {
            self.total_mining_time_ns / self.total_chunks_mined
        } else {
            0
        };

        let avg_hashes_per_chunk = if self.total_chunks_mined > 0 {
            self.total_hashes_computed / self.total_chunks_mined
        } else {
            0
        };

        let avg_instructions_per_hash = if self.total_hashes_computed > 0 {
            self.total_instructions / self.total_hashes_computed
        } else {
            0
        };

        let hashes_per_second = if self.total_mining_time_ns > 0 {
            (self.total_hashes_computed as f64 / (self.total_mining_time_ns as f64 / 1_000_000_000.0))
            as u64
        } else {
            0
        };

        let early_termination_rate = if self.total_chunks_mined > 0 {
            (self.early_terminations as f64 / self.total_chunks_mined as f64) * 100.0
        } else {
            0.0
        };

        MetricsSummary {
            total_chunks: self.total_chunks_mined,
            total_hashes: self.total_hashes_computed,
            solutions_found: self.solutions_found,
            cache_hit_rate,
            early_termination_rate,
            avg_time_per_chunk_ms: avg_time_per_chunk / 1_000_000,
            avg_hashes_per_chunk,
            avg_instructions_per_hash,
            hashes_per_second,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, CandidType, Deserialize)]
pub struct MetricsSummary {
    pub total_chunks: u64,
    pub total_hashes: u64,
    pub solutions_found: u64,
    pub cache_hit_rate: f64,
    pub early_termination_rate: f64,
    pub avg_time_per_chunk_ms: u64,
    pub avg_hashes_per_chunk: u64,
    pub avg_instructions_per_hash: u64,
    pub hashes_per_second: u64,
}

// Global metrics instance
thread_local! {
    static METRICS: RefCell<MiningMetrics> = RefCell::new(MiningMetrics::default());
}

// ------------------------------------------------------------
// Public API
// ------------------------------------------------------------

pub fn record_chunk_result(
    hashes: u64,
    time_ns: u64,
    instructions: u64,
    found_solution: bool,
    early_terminated: bool,
) {
    METRICS.with(|m| {
        m.borrow_mut().record_chunk(
            hashes,
            time_ns,
            instructions,
            found_solution,
            early_terminated,
        )
    });
}

pub fn record_cache_hit() {
    METRICS.with(|m| m.borrow_mut().record_cache_hit());
}

pub fn record_cache_miss() {
    METRICS.with(|m| m.borrow_mut().record_cache_miss());
}

pub fn record_adaptive_change(new_chunk_size: u64) {
    METRICS.with(|m| m.borrow_mut().record_adaptive_change(new_chunk_size));
}

#[query]
pub fn get_metrics() -> MiningMetrics {
    METRICS.with(|m| m.borrow().clone())
}

#[query]
pub fn get_metrics_summary() -> MetricsSummary {
    METRICS.with(|m| m.borrow().summary())
}

#[update]
pub fn reset_metrics() {
    METRICS.with(|m| m.borrow_mut().reset());
}

/// Export metrics as CSV string for analysis
#[query]
pub fn export_metrics_csv() -> String {
    METRICS.with(|m| {
        let metrics = m.borrow();
        let summary = metrics.summary();

        format!(
            "metric,value\n\
total_chunks,{}\n\
total_hashes,{}\n\
solutions_found,{}\n\
cache_hits,{}\n\
cache_misses,{}\n\
cache_hit_rate_percent,{:.2}\n\
early_terminations,{}\n\
early_termination_rate_percent,{:.2}\n\
avg_time_per_chunk_ms,{}\n\
avg_hashes_per_chunk,{}\n\
avg_instructions_per_hash,{}\n\
hashes_per_second,{}\n\
min_instructions_per_hash,{}\n\
max_instructions_per_hash,{}\n",
metrics.total_chunks_mined,
metrics.total_hashes_computed,
metrics.solutions_found,
metrics.cache_hits,
metrics.cache_misses,
summary.cache_hit_rate,
metrics.early_terminations,
summary.early_termination_rate,
summary.avg_time_per_chunk_ms,
summary.avg_hashes_per_chunk,
summary.avg_instructions_per_hash,
summary.hashes_per_second,
metrics.min_instructions_per_hash,
metrics.max_instructions_per_hash,
        )
    })
}
