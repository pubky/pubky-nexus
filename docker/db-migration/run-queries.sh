#!/bin/bash

# Path to the queries file
QUERIES_FILE="/db-migration/indexer.cypher"

echo "Starting Cypher query execution..."

# Check if the queries file exists
if [[ ! -f "$QUERIES_FILE" ]]; then
    echo "Error: Queries file not found."
    exit 1
fi

# Execute queries and time the execution
time cypher-shell -u neo4j -p 12345678 -f "$QUERIES_FILE"

if [[ $? -eq 0 ]]; then
    echo "Queries executed successfully."
else
    echo "Error: Query execution failed."
    exit 1
fi

echo "Cypher query execution completed."
