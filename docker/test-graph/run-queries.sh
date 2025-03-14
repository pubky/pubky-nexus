#!/bin/bash

# Path to the queries file
MAIN_GRAPH="/test-graph/skunk.cypher"
# To achieve deterministic tests, each domain will have its own testing graph
TAGS_TEST_FILE="/test-graph/mocks/tags.cypher"
HOT_TAGS_TEST_FILE="/test-graph/mocks/hot-tags.cypher"
POSTS_TEST_FILE="/test-graph/mocks/posts.cypher"
FILES_TEST_FILE="/test-graph/mocks/files.cypher"
MUTES_TEST_FILE="/test-graph/mocks/mutes.cypher"

echo "Starting Cypher query execution..."

# Check if the queries file exists
if [[ ! -f "$MAIN_GRAPH" ]]; then
    echo "Error: Queries file not found."
    exit 1
fi

# Execute queries and time the execution
time cypher-shell -u neo4j -p 12345678 -f "$MAIN_GRAPH"
echo "Importing TAGs test graph..."
time cypher-shell -u neo4j -p 12345678 -f "$TAGS_TEST_FILE"
echo "Importing HOT TAGs test graph..."
time cypher-shell -u neo4j -p 12345678 -f "$HOT_TAGS_TEST_FILE"
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
