#!/bin/bash

# Find all .rs files in the current directory and subdirectories
# Useful if you want to give full project context to your LLM

find . -path ./target -prune -o \( -name "*.rs" -o -name "*.toml" \) -print | while read -r file; do
  # Print the path to the file
  echo "$file"
  echo '```'
  # Print the content of the file
  cat "$file"
  echo '```'
done