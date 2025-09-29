#!/bin/bash
# Simple CI environment validation script for Copilot testing
# This script validates that the basic CI environment is working correctly

set -e

echo "🚀 Testing Copilot CI Container Environment"
echo "=========================================="

# Basic environment checks
echo "✅ Checking basic shell environment..."
echo "Current user: $(whoami)"
echo "Current directory: $(pwd)"
echo "Available memory: $(free -h | head -2)"

# Rust toolchain check
echo "✅ Checking Rust toolchain..."
rustc --version
cargo --version

# Basic compilation check
echo "✅ Testing basic compilation..."
echo 'fn main() { println!("Hello from Copilot CI!"); }' > /tmp/test_ci.rs
rustc /tmp/test_ci.rs -o /tmp/test_ci
/tmp/test_ci
rm -f /tmp/test_ci.rs /tmp/test_ci

# System dependencies check
echo "✅ Checking system dependencies..."
pkg-config --version > /dev/null && echo "pkg-config: ✅"
socat -V > /dev/null 2>&1 && echo "socat: ✅" || echo "socat: ⚠️ (may be expected)"

echo "🎉 Copilot CI Container Environment Test Completed Successfully!"