# just: https://github.com/casey/just
# Install on macOS: brew install just

set dotenv-load := false
set shell := ["bash", "-cu"]

# Run the same checks you'd typically run in CI.
default: ci

# Ensure the repo toolchain + components are installed.
setup:
  . "$HOME/.cargo/env" && rustup toolchain install stable
  . "$HOME/.cargo/env" && rustup component add rustfmt clippy

fmt:
  . "$HOME/.cargo/env" && cargo fmt --all

fmt-check:
  . "$HOME/.cargo/env" && cargo fmt --all -- --check

clippy:
  . "$HOME/.cargo/env" && cargo clippy --workspace --all-targets -- -D warnings

test:
  . "$HOME/.cargo/env" && cargo test --workspace

ci: fmt-check clippy test

build:
  . "$HOME/.cargo/env" && cargo build --workspace

build-release:
  . "$HOME/.cargo/env" && cargo build --workspace --release

run:
  . "$HOME/.cargo/env" && cargo run -p squawk-app

run-release:
  . "$HOME/.cargo/env" && cargo run -p squawk-app --release
