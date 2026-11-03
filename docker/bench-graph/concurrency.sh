#!/usr/bin/env bash
#
# Concurrency / contention probe (read-only). Profiling measured queries in
# isolation; this asks: do the offenders parallelize or saturate the server, and
# does an offender burst degrade concurrent cheap queries (noisy neighbor)?
#
# NOTE: each cypher-shell is its own bolt connection straight to neo4j, so this
# exercises the SERVER's concurrency (CPU, page cache, locks), not the app's
# 16-connection pool. Gentle by default (C=4).
set -euo pipefail
cd "$(dirname "$0")/../.."
PASS="${NEO4J_PASSWORD:-12345678}"
M="${M:-3}"; C="${C:-4}"

cy() { docker exec -i neo4j cypher-shell -u neo4j -p "$PASS" --format verbose 2>&1; }
server_ms() { grep -oP 'results consumed after another \K[0-9]+' | head -1; }

bash docker/bench-graph/seed.sh "$M" >/dev/null
cargo build -q -p nexus-examples --bin graph-audit
DUMP=$(./target/debug/graph-audit)
extract() { awk -v L="$1" '$0 ~ "@@QUERY [0-9]+ "L"$"{f=1;next} /@@END/{f=0} f' <<<"$DUMP" | sed '1{/^PROFILE$/d}'; }
OFFENDER=$(extract get_global_influencers)
CHEAP=$(extract get_post_by_id)

echo "scale M=$M, concurrency C=$C"

# T_one: one offender, wall clock
t0=$(date +%s.%N); printf '%s\n' "$OFFENDER" | cy >/dev/null; t1=$(date +%s.%N)
T_ONE=$(awk -v a="$t0" -v b="$t1" 'BEGIN{printf "%.1f", b-a}')

# cheap query server-ms, idle
CHEAP_IDLE=$(printf 'PROFILE %s\n' "$CHEAP" | cy | server_ms)

# Fire C offenders concurrently; while they run, sample cheap server-ms; then wait.
pids=()
tc0=$(date +%s.%N)
for i in $(seq 1 "$C"); do ( printf '%s\n' "$OFFENDER" | cy >/dev/null ) & pids+=($!); done
sleep 0.5
CHEAP_LOAD=$(printf 'PROFILE %s\n' "$CHEAP" | cy | server_ms)
for p in "${pids[@]}"; do wait "$p"; done
tc1=$(date +%s.%N)
T_C=$(awk -v a="$tc0" -v b="$tc1" 'BEGIN{printf "%.1f", b-a}')

bash docker/bench-graph/seed.sh clean >/dev/null

echo
echo "1 offender wall-clock       : ${T_ONE}s"
echo "$C offenders concurrent      : ${T_C}s   (ideal-parallel ~${T_ONE}s, fully-serial ~$(awk -v a="$T_ONE" -v c="$C" 'BEGIN{printf "%.1f", a*c}')s)"
echo "  -> parallel efficiency     : $(awk -v one="$T_ONE" -v tc="$T_C" 'BEGIN{ if(tc>0) printf "%.0f%% (1/C=serial, 100%%=perfect)", 100*one/tc; else print "n/a"}')"
echo "cheap query server-ms idle  : ${CHEAP_IDLE:-NA} ms"
echo "cheap query server-ms @load : ${CHEAP_LOAD:-NA} ms   (degradation under $C-offender burst)"
