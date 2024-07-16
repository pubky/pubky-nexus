#!/bin/bash

# Path to the queries file
QUERIES_FILE="/neo4j-example/indexer-migration.cypher"

# Read queries from the file and execute each one
while IFS= read -r query
do
    if [[ ! -z "$query" ]]; then
        cypher-shell -u neo4j -p 12345678 "$query"
    fi
done < "$QUERIES_FILE"