#!/bin/bash

# Complete Benchmark Suite for PoW Optimization on Internet Computer
# Measures: Naive baseline, SHA mid-state, cache, parallel mining (2 & 3 miners)
# Author: Generated for Master's thesis benchmarking
# Date: 2026-03-22

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Create results directory
RESULTS_DIR="benchmark_results"
mkdir -p "$RESULTS_DIR"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RESULT_FILE="$RESULTS_DIR/complete_results_${TIMESTAMP}.txt"

echo -e "${BLUE}======================================"
echo "PoW OPTIMIZATION BENCHMARK SUITE"
echo "======================================"
echo -e "Results will be saved to: ${RESULT_FILE}${NC}"
echo ""

# Redirect all output to both console and file
exec > >(tee -a "$RESULT_FILE") 2>&1

echo "Benchmark Start Time: $(date)"
echo "======================================"
echo ""

# ========================================
# PHASE 1: BASELINE - NAIVE MINING
# ========================================

echo -e "${YELLOW}======================================"
echo "PHASE 1: BASELINE - NAIVE MINING"
echo "======================================${NC}"
echo ""

# Reset metrics
echo "Resetting metrics and cache..."
dfx canister call existing_backend reset_metrics > /dev/null
dfx canister call existing_backend clear_cache > /dev/null

echo "Running naive baseline tests..."
echo ""

echo "Test 1.1: Naive mining (difficulty 16, 200K nonces)"
NAIVE_D16=$(dfx canister call existing_backend bench_naive_instructions \
  '("baseline_d16", 16 : nat32, 0 : nat64, 200000 : nat64)')
echo "$NAIVE_D16"

echo ""
echo "Test 1.2: Naive mining (difficulty 18, 200K nonces)"
NAIVE_D18=$(dfx canister call existing_backend bench_naive_instructions \
  '("baseline_d18", 18 : nat32, 0 : nat64, 200000 : nat64)')
echo "$NAIVE_D18"

echo ""
echo "Test 1.3: Naive mining (difficulty 20, 200K nonces)"
NAIVE_D20=$(dfx canister call existing_backend bench_naive_instructions \
  '("baseline_d20", 20 : nat32, 0 : nat64, 200000 : nat64)')
echo "$NAIVE_D20"

echo ""
echo -e "${GREEN}✓ PHASE 1 COMPLETE${NC}"
echo ""
sleep 2

# ========================================
# PHASE 2: SHA MID-STATE OPTIMIZATION
# ========================================

echo -e "${YELLOW}======================================"
echo "PHASE 2: SHA MID-STATE OPTIMIZATION"
echo "======================================${NC}"
echo ""

echo "Running mid-state optimized tests..."
echo ""

echo "Test 2.1: Midstate mining (difficulty 16, 200K nonces)"
MIDSTATE_D16=$(dfx canister call existing_backend bench_midstate_instructions \
  '("midstate_d16", 16 : nat32, 0 : nat64, 200000 : nat64)')
echo "$MIDSTATE_D16"

echo ""
echo "Test 2.2: Midstate mining (difficulty 18, 200K nonces)"
MIDSTATE_D18=$(dfx canister call existing_backend bench_midstate_instructions \
  '("midstate_d18", 18 : nat32, 0 : nat64, 200000 : nat64)')
echo "$MIDSTATE_D18"

echo ""
echo "Test 2.3: Midstate mining (difficulty 20, 200K nonces)"
MIDSTATE_D20=$(dfx canister call existing_backend bench_midstate_instructions \
  '("midstate_d20", 20 : nat32, 0 : nat64, 200000 : nat64)')
echo "$MIDSTATE_D20"

echo ""
echo -e "${GREEN}✓ PHASE 2 COMPLETE${NC}"
echo ""
sleep 2

# ========================================
# PHASE 3: CACHE EFFECTIVENESS
# ========================================

echo -e "${YELLOW}======================================"
echo "PHASE 3: LRU CACHE EFFECTIVENESS"
echo "======================================${NC}"
echo ""

dfx canister call existing_backend reset_metrics > /dev/null
dfx canister call existing_backend clear_cache > /dev/null

echo "Test 3.1: First mining (cache MISS expected)"
echo "Starting mining..."
dfx canister call existing_backend start_advanced_mining \
  '("cache_test_block", 16 : nat32, 0 : nat64, 100000 : nat64)' > /dev/null
sleep 3
echo "First call completed"

echo ""
echo "Test 3.2: Same block again (cache HIT expected)"
echo "Starting same block mining..."
START_TIME=$(date +%s%N)
dfx canister call existing_backend start_advanced_mining \
  '("cache_test_block", 16 : nat32, 0 : nat64, 100000 : nat64)' > /dev/null
END_TIME=$(date +%s%N)
CACHE_HIT_TIME=$(( (END_TIME - START_TIME) / 1000000 ))
echo "Second call completed in ${CACHE_HIT_TIME}ms (should be instant if cached)"

echo ""
echo "Cache statistics after 2 calls:"
dfx canister call existing_backend get_cache_stats

echo ""
echo "Test 3.3: Multiple blocks to measure realistic hit rate"
echo "Mining 20 blocks (5 unique blocks repeated)..."
for i in {1..20}; do
  block_num=$((i % 5))
  echo -n "."
  dfx canister call existing_backend start_advanced_mining \
    "(\"repeated_block_$block_num\", 16 : nat32, 0 : nat64, 50000 : nat64)" > /dev/null 2>&1
  sleep 1
done
echo ""

echo ""
echo "Final cache statistics:"
CACHE_STATS=$(dfx canister call existing_backend get_cache_stats)
echo "$CACHE_STATS"

echo ""
echo -e "${GREEN}✓ PHASE 3 COMPLETE${NC}"
echo ""
sleep 2

# ========================================
# PHASE 4: PARALLEL MINING (2 MINERS)
# ========================================

echo -e "${YELLOW}======================================"
echo "PHASE 4: PARALLEL MINING (2 MINERS)"
echo "======================================${NC}"
echo ""

MINER1=$(dfx canister id existing_backend)
MINER2=$(dfx canister id miner1)

echo "Miner 1: $MINER1"
echo "Miner 2: $MINER2"
echo ""

dfx canister call coordinator stop_dynamic_mining > /dev/null 2>&1 || true

for DIFF in 16 18 20; do
  echo ""
  echo "Test 4.$DIFF: Parallel mining - Difficulty $DIFF"
  echo "Starting 2-miner coordination..."
  
  dfx canister call coordinator start_dynamic_mining \
    "(vec {principal \"$MINER1\"; principal \"$MINER2\"}, \"parallel2_d${DIFF}\", $DIFF, 0, 300000)" > /dev/null
  
  echo "Waiting for solution..."
  sleep 12
  
  echo "Results:"
  PARALLEL2_STATS=$(dfx canister call coordinator get_scheduler_stats)
  echo "$PARALLEL2_STATS"
  
  dfx canister call coordinator stop_dynamic_mining > /dev/null
  sleep 2
done

echo ""
echo -e "${GREEN}✓ PHASE 4 COMPLETE${NC}"
echo ""
sleep 2

# ========================================
# PHASE 5: PARALLEL MINING (3 MINERS)
# ========================================

echo -e "${YELLOW}======================================"
echo "PHASE 5: PARALLEL MINING (3 MINERS)"
echo "======================================${NC}"
echo ""

MINER3=$(dfx canister id miner2)
echo "Miner 1: $MINER1"
echo "Miner 2: $MINER2"
echo "Miner 3: $MINER3"
echo ""

for DIFF in 16 18; do
  echo ""
  echo "Test 5.$DIFF: Parallel mining - Difficulty $DIFF (3 miners)"
  echo "Starting 3-miner coordination..."
  
  dfx canister call coordinator start_dynamic_mining \
    "(vec {principal \"$MINER1\"; principal \"$MINER2\"; principal \"$MINER3\"}, \"parallel3_d${DIFF}\", $DIFF, 0, 300000)" > /dev/null
  
  echo "Waiting for solution..."
  sleep 15
  
  echo "Results:"
  PARALLEL3_STATS=$(dfx canister call coordinator get_scheduler_stats)
  echo "$PARALLEL3_STATS"
  
  dfx canister call coordinator stop_dynamic_mining > /dev/null
  sleep 2
done

echo ""
echo -e "${GREEN}✓ PHASE 5 COMPLETE${NC}"
echo ""
sleep 2

# ========================================
# PHASE 6: COMPREHENSIVE METRICS
# ========================================

echo -e "${YELLOW}======================================"
echo "PHASE 6: COMPREHENSIVE METRICS"
echo "======================================${NC}"
echo ""

echo "Running extended mining session for metrics..."
dfx canister call existing_backend reset_metrics > /dev/null

for i in {1..10}; do
  echo "Mining session $i/10..."
  dfx canister call existing_backend start_advanced_mining \
    "(\"metrics_block_$i\", 16 : nat32, 0 : nat64, 100000 : nat64)" > /dev/null
  sleep 2
done

echo ""
echo "Metrics Summary:"
METRICS=$(dfx canister call existing_backend get_metrics_summary)
echo "$METRICS"

echo ""
echo "CSV Metrics Export:"
CSV=$(dfx canister call existing_backend export_metrics_csv)
echo "$CSV"

echo ""
echo "Final Cache Statistics:"
dfx canister call existing_backend get_cache_stats

echo ""
echo -e "${GREEN}✓ PHASE 6 COMPLETE${NC}"
echo ""

# ========================================
# SUMMARY AND ANALYSIS
# ========================================

echo -e "${BLUE}======================================"
echo "BENCHMARK SUMMARY"
echo "======================================${NC}"
echo ""
echo "Benchmark End Time: $(date)"
echo ""
echo "All results saved to: $RESULT_FILE"
echo ""
echo -e "${YELLOW}NEXT STEPS:${NC}"
echo ""
echo "Generate analysis report:"
echo "   ./ar.sh $RESULT_FILE"
echo ""
echo -e "${GREEN}✓✓✓ ALL BENCHMARKS COMPLETE ✓✓✓${NC}"
echo ""
