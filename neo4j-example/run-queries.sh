#!/bin/bash

# Path to the queries file
QUERIES_FILE="/neo4j-example/indexer-migration.cypher"

# Read queries from the file and execute each one
# time it
time cypher-shell -u neo4j -p 12345678 -f "$QUERIES_FILE"