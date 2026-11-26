#!/usr/bin/env bash
#
# Equivalence harness: prove a query rewrite returns identical results.
#
# A fix is only trustworthy if the new Cypher produces the SAME results as the
# old one on the same graph. Workflow around a fix to `<label>`:
#
#   ./seed.sh large
#   ./equiv.sh capture <label> /tmp/old.cypher     # BEFORE editing get.rs
#   # ...edit the query in get.rs...
#   ./equiv.sh capture <label> /tmp/new.cypher     # AFTER (re-dumps the real fn)
#   ./equiv.sh diff /tmp/old.cypher /tmp/new.cypher
#
# `capture` pulls the query's populated Cypher straight from the audit dumper
# (the real `nexus_common` fn), strips the PROFILE prefix, and writes a runnable
# query. `diff` runs both against the currently-seeded graph and compares the
# RETURN output (order-insensitive), so it works for aggregated and list queries.
set -euo pipefail
cd "$(dirname "$0")/../.."
PASS="${NEO4J_PASSWORD:-12345678}"

run() { docker exec -i neo4j cypher-shell -u neo4j -p "$PASS" --format plain < "$1"; }

case "${1:-}" in
  capture)
    label="${2:?usage: equiv.sh capture <label> <outfile>}"; out="${3:?missing outfile}"
    cargo build -q -p nexus-examples --bin graph-audit
    # Extract the labeled block, drop the markers and the leading PROFILE so the
    # query runs normally and returns rows.
    ./target/debug/graph-audit \
      | awk -v L="$label" '$0 ~ "@@(QUERY|WRITE) [0-9]+ "L"$"{f=1;next} /@@END/{f=0} f' \
      | sed '1{/^PROFILE$/d}' > "$out"
    [[ -s "$out" ]] || { echo "no query labeled '$label' found in dumper" >&2; exit 1; }
    echo "captured '$label' -> $out ($(wc -l <"$out") lines)"
    ;;
  diff)
    a="${2:?usage: equiv.sh diff <a.cypher> <b.cypher>}"; b="${3:?missing b}"
    # Sort so row order doesn't cause false diffs; identical multiset = equivalent.
    if diff <(run "$a" | sort) <(run "$b" | sort); then
      echo "IDENTICAL"
    else
      echo "DIFFERS (left=old, right=new above)"; exit 1
    fi
    ;;
  *)
    echo "usage: equiv.sh capture <label> <outfile> | diff <a.cypher> <b.cypher>" >&2
    exit 1 ;;
esac
