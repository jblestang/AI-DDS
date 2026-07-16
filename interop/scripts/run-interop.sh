#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

echo "==> Wire compliance tests (no external deps)"
cargo test -p dds --test interop_wire

run_vendor_tests() {
  local name="$1"
  local test_crate="$2"
  local bin_dir="$3"

  if [[ -x "${bin_dir}/interop_publisher" ]]; then
    echo "==> Live ${name} interop tests"
    cargo test -p dds --test "${test_crate}" -- --ignored --nocapture --test-threads=1
  else
    echo "==> Skipping ${name} live interop (${bin_dir}/interop_publisher not found)"
  fi
}

# Legacy AIDDS_INTEROP_BIN still maps to CycloneDDS when set.
CYCLONE_BIN="${AIDDS_INTEROP_BIN_CYCLONEDDS:-${AIDDS_INTEROP_BIN:-$ROOT/target/interop-cyclonedds}}"
FASTDDS_BIN="${AIDDS_INTEROP_BIN_FASTDDS:-$ROOT/target/interop-fastdds}"
OPENSPLICE_BIN="${AIDDS_INTEROP_BIN_OPENSPLICE:-$ROOT/target/interop-opensplice}"

run_vendor_tests "CycloneDDS" interop_cyclonedds "$CYCLONE_BIN"
run_vendor_tests "Fast DDS" interop_fastdds "$FASTDDS_BIN"
run_vendor_tests "OpenSplice" interop_opensplice "$OPENSPLICE_BIN"

echo "==> Done"
