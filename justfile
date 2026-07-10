import "./celestia-devtools.just"

set windows-shell := ["C:/Program Files/Git/bin/bash.exe", "-c"]
set shell := ["bash", "-c"]
# `set windows-shell` only governs linewise (non-shebang) recipes on Windows.
# Shebang recipes bypass it and force `just` to call `cygpath` to translate the
# interpreter path — which Git for Windows keeps off PATH, so they die with
# "could not find cygpath executable". To avoid that, every multi-line recipe
# below uses the `[script('bash')]` attribute instead of a `#!` shebang:
# `[script]` resolves the interpreter via PATH (PATHEXT-aware) and never calls
# cygpath. See casey/just#2828 and the just manual (Script Recipes).
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
