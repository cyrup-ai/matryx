# Root justfile for maxtryx project

# List all available commands
default:
    @just --list

# Run all tests with nextest
test:
    cargo nextest run

# Run all integration tests
test-integration:
    cargo nextest run --test "integration"

# Run only the database tests for matrix library
test-matrix-db:
    cd crates/cyrup-matrix && just test-db

# Run matrix tests with tracing enabled
test-matrix-trace:
    cd crates/cyrup-matrix && just test-db-trace

# Check formatting and linting
lint:
    cargo fmt -- --check
    cargo clippy --all-targets --all-features -- -D warnings

# Format code
fmt:
    cargo fmt

# Clean build artifacts
clean:
    cargo clean

# Build all packages
build:
    cargo build

# Build in release mode
release:
    cargo build --release

# Run migrations for all databases
migrate:
    cd crates/cyrup-matrix && just migrate

# Run a specific crate with arguments
run CRATE *ARGS:
    cargo run -p {{CRATE}} -- {{ARGS}}