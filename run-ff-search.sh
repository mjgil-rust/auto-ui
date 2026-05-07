#!/bin/bash
# Run ff-search UI in release mode

set -e

cd "$(dirname "$0")"

# Build release if needed (only ff-search, not the whole auto-ui workspace)
if [ ! -f ../ff-search/target/release/ff-search ]; then
    cargo build --release --manifest-path ../ff-search/Cargo.toml --features "egui,parallel"
fi

# Run with UI mode, defaulting to home directory
../ff-search/target/release/ff-search --ui "${1:-$HOME}"
