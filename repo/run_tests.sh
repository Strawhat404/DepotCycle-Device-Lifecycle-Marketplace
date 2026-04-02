#!/usr/bin/env bash
set -euo pipefail

# Copy .env.example to .env if .env doesn't exist
if [ ! -f .env ]; then
  cp .env.example .env
fi

if command -v cargo >/dev/null 2>&1; then
  cargo test -p backend
  cargo test -p unit_tests
  cargo test -p API_tests
else
  if ! command -v docker >/dev/null 2>&1; then
    echo "Neither cargo nor docker is available to run the test suite." >&2
    exit 1
  fi

  docker run --rm \
    -v "$(pwd):/work" \
    -w /work \
    rust:1.77 \
    bash -c 'source $HOME/.cargo/env && cargo test -p backend && cargo test -p unit_tests && cargo test -p API_tests'
fi
