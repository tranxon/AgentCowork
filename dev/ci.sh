#!/bin/bash
# CI script for Rollball.AI
# Usage: ./dev/ci.sh [check|clippy|test|all]

set -e

MODE=${1:-all}

echo "=== Rollball.AI CI ==="

run_check() {
    echo "Running cargo check..."
    cargo check --all
}

run_clippy() {
    echo "Running cargo clippy..."
    cargo clippy --all-targets -- -D warnings
}

run_test() {
    echo "Running cargo test..."
    cargo test --all
}

case "$MODE" in
    check)
        run_check
        ;;
    clippy)
        run_clippy
        ;;
    test)
        run_test
        ;;
    all)
        run_check
        run_clippy
        run_test
        ;;
    *)
        echo "Unknown mode: $MODE"
        echo "Usage: $0 [check|clippy|test|all]"
        exit 1
        ;;
esac

echo "=== CI completed successfully ==="
