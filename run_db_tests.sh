#!/bin/bash
# Run SurrealDB integration tests with nextest

set -e  # Exit on error

echo "Running SurrealDB integration tests..."

# First build the project
cargo build --package cyrup-matrix

# Run the tests with nextest using the db profile
cargo nextest run --package cyrup-matrix --profile db "db_operations::"

echo "Tests completed successfully!"