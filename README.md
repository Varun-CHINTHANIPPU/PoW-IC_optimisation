# PoW-IC_optimisation
## Overview

This project presents a fully implemented and benchmarked distributed Proof-of-Work (PoW) mining system on the Internet Computer (IC). The system evolves from a partially working single-miner prototype into a complete multi-canister architecture with dynamic scheduling, caching, metrics collection, and parallel execution.

The primary goal is to evaluate how PoW-style workloads behave under IC constraints and to identify practical optimization strategies.



## System Architecture

The system is composed of multiple cooperating canisters:

* Miner Canisters
  Execute SHA-256 hashing over assigned nonce ranges

* Coordinator Canister
  Dynamically distributes work, manages failures, and aggregates results

* Validator Canister
  Verifies correctness of discovered hashes and difficulty constraints

* Cache Layer
  Stores previously computed results to avoid recomputation

* Metrics Module
  Tracks instruction usage, hash rates, and execution statistics



## Design Goals

* Achieve parallel execution across independent canisters
* Minimize instruction cost per hash
* Ensure fault tolerance and zero coordination failure
* Provide reproducible benchmarking methodology
* Operate fully within IC execution constraints



## Core Components

### Dynamic Scheduler

The scheduler is responsible for distributing mining work across multiple miner canisters.

Key characteristics:

* Round-robin allocation of nonce ranges
* Timeout-based recovery (10 seconds)
* Retry mechanism with capped failures (max 3 per miner)
* Early termination broadcast when a valid solution is found
* Tracks total chunks assigned and miner status

This component enables near-linear scaling in parallel mining scenarios.



### LRU Cache

A custom LRU cache is implemented to reduce redundant hashing.

Details:

* Capacity: 1000 entries
* Backed by HashMap with access-order tracking
* Automatic eviction of least recently used entries
* Tracks hit/miss statistics

Impact:

* 75% cache hit rate in benchmark scenarios
* Eliminates recomputation for repeated inputs



### Metrics System

A dedicated metrics module tracks system performance at instruction level granularity.

Collected metrics:

* Total hashes computed
* Instructions per hash
* Minimum, maximum, and average instruction cost
* Cache hit rate
* Solution success rate

Supports CSV export for external analysis and reproducibility.



### Parallel Mining Engine

The mining engine integrates scheduler, cache, and metrics into a unified execution model.

Features:

* Adaptive chunk sizing (20K to 2M nonces)
* Heartbeat-driven autonomous execution
* Early termination after expected threshold (3× expected work)
* Multi-miner coordination with zero failure tolerance



## Critical Bug and Resolution

### Problem: Candid Type Mismatch

Parallel mining initially failed due to decoding errors in cross-canister calls.

Root cause:

* Field order mismatch in Candid enum serialization
* Coordinator expected: `{ hash, nonce }`
* Miner returned: `{ nonce, hash }`

Even after aligning definitions, failures persisted due to WASM caching behavior.



### Final Fix

Replaced enum-based responses with simple tuple structures:

```rust id="fixcode"
// OLD (unstable)
(MiningStatus, u64)

// NEW (stable)
(bool, u64, String, u64)
// (found, nonce, hash, attempts)
```

Outcome:

* Eliminated decoding ambiguity
* Removed dependency on Candid field ordering
* Achieved stable cross-canister communication



## Benchmark Methodology

A comprehensive benchmarking pipeline was implemented.

### Test Suite

* Total tests: 43
* Execution time: ~10 minutes
* Multiple difficulty levels evaluated (16, 18, 20)

### Benchmark Phases

1. Naive baseline mining
2. Mid-state optimization
3. Cache effectiveness
4. Parallel mining (2 miners)
5. Parallel mining (3 miners)
6. Aggregate metrics collection

### Tooling

* Shell scripts for automated execution
* Python-based analysis for accurate parsing
* Instruction-level measurement (not time-based)



## Results

### SHA Mid-State Optimization

* Instructions per hash:
  ~6056 → ~5840
* Improvement: ~3.6%
* Variance: ±0.01%

Observation:
The improvement is significantly lower than theoretical expectations (40–60%), indicating that IC's WASM runtime already performs substantial internal optimizations.



### Cache Effectiveness

* Total calls: 20
* Unique inputs: 5
* Cache hits: 15
* Hit rate: 75%

Impact:
Repeated computations are resolved instantly, providing effectively unbounded speedup for cached inputs.



### Parallel Mining Performance

* 2 miners: ~2× speedup
* 3 miners: ~3× speedup
* Failed miners: 0
* Reliability: 100% across all tests

Observation:
The system demonstrates near-linear scalability with zero coordination failure.



### System-Wide Metrics

* Average instructions per hash: ~5840
* Total hashes computed: 556,000+
* Solution success rate: 90%
* Early termination rate: 10%



## Key Insights

* Parallelism is the dominant source of performance improvement
* IC WASM runtime significantly reduces expected gains from micro-optimizations
* Instruction-based benchmarking is more reliable than timing on IC
* Candid serialization can introduce subtle but critical failures
* Cache effectiveness depends heavily on workload repetition


## Project Structure

```
src/
├── scheduler_enhanced.rs
├── cache.rs
├── metrics.rs
├── advanced_with_cache_metrics.rs
├── validator_complete.rs
├── lib_final.rs
├── scheduler_final.rs
```



## Getting Started

### Build

```
cargo build --target wasm32-unknown-unknown --release
```

### Deploy

```
dfx deploy
```

### Run Mining

```
dfx canister call coordinator start_dynamic_mining
```



## Benchmarking

### Run full benchmark suite

```
bash run_complete_benchmark.sh
```

### Analyze results

```
python3 analyze.py
```



## Contributions

* Complete multi-canister PoW system on Internet Computer
* Dynamic scheduler with fault tolerance and recovery
* Integrated caching and performance tracking
* Instruction-level benchmarking methodology
* Resolution of cross-canister Candid serialization issues
* Empirical evaluation of IC execution characteristics



## Final Results Summary

* Parallel speedup: 2–3×
* Cache hit rate: 75%
* Reliability: 100% (0 failures)
* Instructions per hash: ~5840



## Limitations

* VRF-based mining is implemented but not fully tested
* Benchmarks limited to controlled input sizes
* Results may vary on IC mainnet deployment



## Future Work

* Stabilize and benchmark VRF integration
* Deploy on IC mainnet for real-world latency measurements
* Explore deeper WASM-level optimizations
* Extend system to larger-scale distributed workloads



## Conclusion

This work demonstrates that a fully distributed, reliable, and optimized PoW mining system can be implemented on the Internet Computer.

While micro-optimizations provide limited gains, parallel execution yields significant and consistent improvements. The system achieves zero failures, reproducible results, and offers insights into IC’s execution model, making it suitable for academic and experimental use.
