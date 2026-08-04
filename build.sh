#!/usr/bin/env bash
set -euo pipefail

project="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
target_dir="${PARITY_PLAYGROUND_RUST_TARGET:-/var/tmp/parity-playground-rust}"

mkdir -p "${project}/bin"
cobc -x -free -Wall -fstatic-call \
  -o "${project}/bin/legacy" \
  "${project}/legacy/service.cob" \
  -Q "-Wl,--no-as-needed" \
  -Q "-Wl,-rpath,/usr/local/lib" \
  -lpq

CARGO_TARGET_DIR="${target_dir}" \
  cargo build --manifest-path "${project}/target/Cargo.toml" --locked
cp "${target_dir}/debug/inventory-service" "${project}/bin/target"
