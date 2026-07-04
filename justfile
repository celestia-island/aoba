import "./celestia-devtools.just"

set shell := ["bash", "-c"]

default:
    @just --list

fmt:
    just fmt-markdown .
    cargo fmt --all
    cargo clippy --all-targets --all-features -- -D warnings

fmt-check:
    just fmt-markdown . --check
    cargo fmt --all -- --check

check:
    cargo check --all-targets --all-features

test:
    cargo test --lib

build:
    just cache-guard
    cargo build

build-release:
    just cache-guard
    cargo build --release

clean:
    cargo clean

ci: fmt-check check test
