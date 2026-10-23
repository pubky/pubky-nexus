#!/usr/bin/env bash
# A private Redis, Neo4j and Postgres for the watcher test suites, in their own
# network namespace, so tests run on a fresh database without touching the
# shared ones on localhost. Tests and the mock loader hardcode localhost, so
# commands run inside that namespace, where the containers answer on the usual
# ports.
#
#   tests/migration/stack.sh up                  # start the three containers
#   tests/migration/stack.sh mock                # optional: load the mock data (~2 min)
#   tests/migration/stack.sh run cargo nextest run --offline -p nexus-watcher --test migration
#   tests/migration/stack.sh down                # remove the containers and their volumes
#
# Needs docker, sudo and nsenter. Pass --offline to cargo: the host's DNS
# resolver is not reachable from the namespace. Several stacks can coexist,
# named through NEXUS_STACK (default: nexus-stack).
set -euo pipefail

name="${NEXUS_STACK:-nexus-stack}"
script="$(readlink -f "${BASH_SOURCE[0]}")"
repo="$(cd "$(dirname "$script")/../../.." && pwd)"
neo4j_image="pubky-nexus/neo4j:5.26.27-gds2.13.10"
connection_string='postgres://test_user:test_pass@localhost:5432/postgres?pubky-test=true'

up() {
    if ! docker image inspect "$neo4j_image" >/dev/null 2>&1; then
        echo "building $neo4j_image"
        docker compose -f "$repo/docker/docker-compose.yml" build neo4j
    fi
    docker run -d --name "$name-redis" redis:8.0.6-alpine >/dev/null
    # The other two join the redis container's network namespace.
    docker run -d --name "$name-neo4j" --network "container:$name-redis" \
        -v "$repo/docker/test-graph:/test-graph:ro" \
        -e NEO4J_initial_dbms_default__database=neo4j \
        -e NEO4J_AUTH=neo4j/12345678 \
        -e NEO4J_server_memory_pagecache_size=512M \
        -e NEO4J_server_memory_heap_initial__size=1G \
        -e NEO4J_server_memory_heap_max__size=1G \
        -e NEO4J_db_transaction_timeout=15m \
        -e NEO4J_dbms_usage__report_enabled=false \
        -e NEO4J_client_allow__telemetry=false \
        -e 'NEO4J_PLUGINS=["graph-data-science"]' \
        -e 'NEO4J_dbms_security_procedures_unrestricted=gds.*' \
        -e NEO4J_dbms_security_procedures_allowlist=gds.graph.project,gds.graph.project.estimate,gds.graph.drop,gds.graph.list,gds.pageRank.mutate,gds.graph.nodeProperty.stream \
        "$neo4j_image" >/dev/null
    docker run -d --name "$name-postgres" --network "container:$name-redis" \
        -e POSTGRES_USER=test_user -e POSTGRES_PASSWORD=test_pass -e POSTGRES_DB=postgres \
        postgres:18-alpine >/dev/null
    echo -n "waiting for neo4j"
    for _ in $(seq 1 90); do
        if docker exec "$name-neo4j" cypher-shell -u neo4j -p 12345678 "RETURN 1" >/dev/null 2>&1; then
            echo " ready"
            return
        fi
        echo -n .
        sleep 2
    done
    echo " did not come up; see docker logs $name-neo4j" >&2
    exit 1
}

# Runs a command as the current user inside the stack's network namespace.
run() {
    local pid
    pid="$(docker inspect -f '{{.State.Pid}}' "$name-redis")"
    local env=(PATH="$PATH" HOME="$HOME" NEXUS_STACK="$name"
        CONTAINER_RUNTIME="$script"
        TEST_PUBKY_CONNECTION_STRING="$connection_string")
    [ -n "${CARGO_TARGET_DIR:-}" ] && env+=(CARGO_TARGET_DIR="$CARGO_TARGET_DIR")
    [ -n "${RUST_LOG:-}" ] && env+=(RUST_LOG="$RUST_LOG")
    exec sudo nsenter -t "$pid" -n -- sudo -u "$(id -un)" -H env "${env[@]}" "$@"
}

# Loads the mock data the main watcher suite expects. The migration suite does
# not need it.
mock() {
    run cargo run --offline -p nexusd -- db mock
}

down() {
    docker rm -f -v "$name-neo4j" "$name-postgres" "$name-redis" >/dev/null 2>&1 || true
    echo "removed $name"
}

# CONTAINER_RUNTIME for the mock loader, which runs `docker exec neo4j ...`:
# the container is ours, under another name.
docker_shim() {
    local args=()
    for a in "$@"; do
        [ "$a" = neo4j ] && a="$name-neo4j"
        args+=("$a")
    done
    exec docker "${args[@]}"
}

case "${1:-}" in
    up) up ;;
    run) shift; run "$@" ;;
    mock) mock ;;
    down) down ;;
    *) docker_shim "$@" ;;
esac
