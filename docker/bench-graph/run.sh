#!/usr/bin/env bash
#
# Graph query performance audit: runner (v3).
#
# Rigor upgrades over a single small-vs-large ratio:
#  - Baseline subtraction: the dev graph has real (non-:Bench) data that the
#    production-label queries also scan. We profile once with :Bench cleaned (the
#    real-data-only cost B) and subtract it, so reported db hits are the work the
#    SYNTHETIC data actually causes. Subject-pointed queries have B~0; global
#    scanners have a real B that would otherwise inflate their floor.
#  - Multi-point scaling {1,3,10}: db-hit growth is fit to an exponent
#    (~1 linear, ~2 quadratic) instead of one ratio.
#  - Per-query plan dump (--format verbose) saved under plans/ for Phases 2-3.
#  - Per-query client timeout so a known-pathological query records TIMEOUT
#    instead of hanging the run.
#
# PROFILE db hits are ground truth; exec_ms is informational.
#
# Usage:  ./run.sh [output.md]        default: docker/bench-graph/report.md
#         SCALES="1 3 10 30" TIMEOUT=300 ./run.sh
set -euo pipefail
cd "$(dirname "$0")/../.."   # repo root

PASS="${NEO4J_PASSWORD:-12345678}"
OUT="${1:-docker/bench-graph/report.md}"
SCALES="${SCALES:-1 3 10}"           # multipliers; baseline (clean) is always added
PLAN_SCALE="${PLAN_SCALE:-3}"        # scale at which to save full PROFILE plans
TIMEOUT="${TIMEOUT:-150}"            # per-query seconds before recording TIMEOUT
COST_HITS=1000000
SLOW_MS=500
EXP_FLAG=1.3                          # exponent above this = super-linear
PLANDIR="docker/bench-graph/plans"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# Run a payload through cypher-shell with a wall-clock cap. Echoes output; exit
# 124 (from `timeout`) means the query was killed.
profile() { timeout "$TIMEOUT" docker exec -i neo4j cypher-shell -u neo4j -p "$PASS" --format verbose 2>&1; }

echo "building dumper..."
cargo build -q -p nexus-examples --bin graph-audit
DUMP="$TMP/queries.txt"
./target/debug/graph-audit > "$DUMP"
echo "audited queries: $(grep -cE '@@QUERY|@@WRITE' "$DUMP")"
mkdir -p "$PLANDIR"; rm -f "$PLANDIR"/*.txt

# Profile every block against the currently-seeded graph.
# $1 = column tag (e.g. B, m1, m10); $2 = 1 to also dump plans this pass.
profile_pass() {
  local tag="$1" dump_plans="${2:-0}"
  local res="$TMP/$tag.tsv" tier="" label="" buf="" is_write=0
  : > "$res"
  while IFS= read -r line; do
    case "$line" in
      @@QUERY*) local r="${line#@@QUERY }"; tier="${r%% *}"; label="${r#* }"; buf=""; is_write=0 ;;
      @@WRITE*) local r="${line#@@WRITE }"; tier="${r%% *}"; label="${r#* }"; buf=""; is_write=1 ;;
      @@END)
        local payload="$buf" out dbh ms rc
        [[ "$is_write" == "1" ]] && payload=":begin"$'\n'"$buf"$'\n'":rollback"
        set +e
        out="$(printf '%s\n' "$payload" | profile)"; rc=$?
        if [[ $rc -eq 124 ]] || grep -q 'TransactionTimedOut' <<<"$out"; then
          dbh="TIMEOUT"; ms="TIMEOUT"
        else
          dbh="$(grep -oP 'Total database accesses: \K[0-9]+' <<<"$out" | head -1)"
          ms="$(grep -oP 'results consumed after another \K[0-9]+' <<<"$out" | head -1)"
        fi
        set -e
        printf '%s\t%s\t%s\t%s\n' "$tier" "$label" "${dbh:-NA}" "${ms:-NA}" >> "$res"
        [[ "$dump_plans" == "1" ]] && printf '%s\n' "$out" > "$PLANDIR/${label}.txt"
        ;;
      *) buf+="$line"$'\n' ;;
    esac
  done < "$DUMP"
  return 0
}

echo "baseline pass (real data only)..."; bash docker/bench-graph/seed.sh clean >/dev/null; profile_pass B 0
for M in $SCALES; do
  echo "scale M=$M..."; bash docker/bench-graph/seed.sh "$M" >/dev/null
  dp=0; [[ "$M" == "$PLAN_SCALE" ]] && dp=1
  profile_pass "m$M" "$dp"
done
bash docker/bench-graph/seed.sh clean >/dev/null

lo="$(echo $SCALES | awk '{print $1}')"; hi="$(echo $SCALES | awk '{print $NF}')"

# Join everything by label; bench(M) = dbhits(M) - baseline; fit exponent lo..hi.
ROWS="$TMP/rows"; : > "$ROWS"
hdr="| tier | query | B | bench@$lo"; for M in $SCALES; do [[ "$M" == "$lo" ]] && continue; hdr+=" | bench@$M"; done
hdr+=" | exp | ms@$hi | signals |"
while IFS=$'\t' read -r tier label _ _; do
  bcol() { grep -P "^[0-9]+\t${1}\t" "$TMP/$2.tsv" | cut -f3 || true; }
  mcol() { grep -P "^[0-9]+\t${1}\t" "$TMP/$2.tsv" | cut -f4 || true; }
  local_B="$(bcol "$label" B)"; [[ "$local_B" =~ ^[0-9]+$ ]] || local_B=0
  # bench-attributable per scale
  declare -A bench
  for M in $SCALES; do
    raw="$(bcol "$label" "m$M")"
    if [[ "$raw" == "TIMEOUT" ]]; then bench[$M]="TIMEOUT"
    elif [[ "$raw" =~ ^[0-9]+$ ]]; then d=$((raw - local_B)); ((d<0)) && d=0; bench[$M]="$d"
    else bench[$M]="NA"; fi
  done
  blo="${bench[$lo]}"; bhi="${bench[$hi]}"; mshi="$(mcol "$label" "m$hi")"
  exp="NA"
  if [[ "$hi" != "$lo" && "$blo" =~ ^[0-9]+$ && "$bhi" =~ ^[0-9]+$ && "$blo" -gt 0 && "$bhi" -gt 0 ]]; then
    exp="$(awk -v a="$blo" -v b="$bhi" -v lo="$lo" -v hi="$hi" 'BEGIN{printf "%.2f", log(b/a)/log(hi/lo)}')"
  fi
  sig=""; sortkey=-1
  [[ "$exp" != "NA" ]] && awk -v e="$exp" -v t="$EXP_FLAG" 'BEGIN{exit !(e>t)}' && sig+="scaling "
  [[ "$bhi" =~ ^[0-9]+$ && "$bhi" -ge "$COST_HITS" ]] && sig+="**COST** "
  if [[ "$mshi" == "TIMEOUT" || "$bhi" == "TIMEOUT" ]]; then sig+="**TIMEOUT** "; sortkey=999999999
  elif [[ "$mshi" =~ ^[0-9]+$ && "$mshi" -ge "$SLOW_MS" ]]; then sig+="slow "; fi
  [[ "$bhi" =~ ^[0-9]+$ ]] && sortkey="$bhi"
  sig="${sig:-ok}"
  row="| $tier | $label | $local_B | $blo"
  for M in $SCALES; do [[ "$M" == "$lo" ]] && continue; row+=" | ${bench[$M]}"; done
  row+=" | $exp | $mshi | ${sig% } |"
  printf '%s\t%s\n' "$sortkey" "$row" >> "$ROWS"
  unset bench
done < "$TMP/m$lo.tsv"

{
  echo "# Graph query performance audit (v3: baseline-subtracted, multi-point)"
  echo
  echo "db hits shown are **bench-attributable** = raw db hits minus the real-data"
  echo "baseline B (profiled with :Bench cleaned). Scales are multipliers; bench@M is"
  echo "the synthetic work at multiplier M. **exp** = fitted growth exponent over"
  echo "$lo..$hi (~1 linear, ~2 quadratic). Sorted by bench@$hi (current cost)."
  echo
  echo "signals: scaling (exp>$EXP_FLAG), COST (>= $COST_HITS bench hits), slow (>= $SLOW_MS ms),"
  echo "TIMEOUT (exceeded ${TIMEOUT}s). Plans saved under plans/. tier: 0 hot, 1 write-path, 2 rare."
  echo
  echo "$hdr"
  echo "$hdr" | sed 's/[^|]/-/g'
  sort -t$'\t' -k1 -rn "$ROWS" | cut -f2-
} | tee "$OUT"
echo "report -> $OUT ; plans -> $PLANDIR/"
