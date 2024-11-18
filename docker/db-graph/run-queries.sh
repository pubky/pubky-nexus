#!/bin/bash

# Path to the queries file
QUERIES_FILE="/db-graph/skunk.cypher"
# To achieve deterministic tests, each domain will have its own testing graph
TAGS_TEST_FILE="/db-graph/mocks/tags.cypher"
POSTS_TEST_FILE="/db-graph/mocks/posts.cypher"
FILES_TEST_FILE="/db-graph/mocks/files.cypher"
MUTES_TEST_FILE="/db-graph/mocks/mutes.cypher"

echo "Starting Cypher query execution..."

# Check if the queries file exists
if [[ ! -f "$QUERIES_FILE" ]]; then
    echo "Error: Queries file not found."
    exit 1
fi

# Execute queries and time the execution
time cypher-shell -u neo4j -p 12345678 -f "$QUERIES_FILE"
echo "Importing TAGs test graph..."
time cypher-shell -u neo4j -p 12345678 -f "$TAGS_TEST_FILE"
echo "Importing POSTs test graph..."
time cypher-shell -u neo4j -p 12345678 -f "$POSTS_TEST_FILE"
echo "Importing FILEs test graph..."
time cypher-shell -u neo4j -p 12345678 -f "$FILES_TEST_FILE"
echo "Importing MUTEs test graph..."
time cypher-shell -u neo4j -p 12345678 -f "$MUTES_TEST_FILE"

if [[ $? -eq 0 ]]; then
    echo "Queries executed successfully."
else
    echo "Error: Query execution failed."
    exit 1
fi

echo "Cypher query execution completed."
