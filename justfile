import "./celestia-devtools.just"

set shell := ["bash", "-c"]
# On Windows just resolves recipe shebangs through the shell named here; without
# it just falls back to `cygpath`, which Git for Windows does not put on PATH.
set windows-shell := ["bash.exe", "-c"]
# `set lists` enables which() (used by the imported celestia-devtools.just);
# `set unstable` gates it.
set unstable
set lists

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
