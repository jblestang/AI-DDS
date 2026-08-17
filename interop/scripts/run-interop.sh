#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

# Vendor peer binaries need runtime libraries from optional SDK installs.
if [[ -d /tmp/fastdds-install/lib ]]; then
  export LD_LIBRARY_PATH="/tmp/fastdds-install/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
fi
if [[ -d /tmp/cyclonedds-install/lib ]]; then
  export LD_LIBRARY_PATH="/tmp/cyclonedds-install/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
fi
if [[ -d /tmp/opendds-install/lib ]]; then
  export LD_LIBRARY_PATH="/tmp/opendds-install/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
fi

echo "==> Wire compliance tests (no external deps)"
cargo test -p dds --test interop_wire
cargo test -p dds --test interop_shapes_wire

run_vendor_tests() {
  local name="$1"
  local test_crate="$2"
  local bin_dir="$3"

  if [[ -x "${bin_dir}/interop_publisher" ]]; then
    echo "==> Live ${name} interop tests"
    if ! cargo test -p dds --test "${test_crate}" -- --ignored --nocapture --test-threads=1; then
      echo "WARNING: ${name} live interop tests failed"
      return 1
    fi
  else
    echo "==> Skipping ${name} live interop (${bin_dir}/interop_publisher not found)"
  fi
}

run_shapes_tests() {
  local name="$1"
  local test_crate="$2"
  local bin_dir="$3"

  if [[ -x "${bin_dir}/shapes_publisher" ]]; then
    echo "==> Live ${name} Shapes Demo tests (Square/Circle/Triangle)"
    if ! cargo test -p dds --test "${test_crate}" -- --ignored --nocapture --test-threads=1; then
      echo "WARNING: ${name} Shapes Demo tests failed"
      return 1
    fi
  else
    echo "==> Skipping ${name} Shapes Demo (${bin_dir}/shapes_publisher not found; rebuild peer apps)"
  fi
}

# Legacy AIDDS_INTEROP_BIN still maps to CycloneDDS when set.
CYCLONE_BIN="${AIDDS_INTEROP_BIN_CYCLONEDDS:-${AIDDS_INTEROP_BIN:-$ROOT/target/interop-cyclonedds}}"
FASTDDS_BIN="${AIDDS_INTEROP_BIN_FASTDDS:-$ROOT/target/interop-fastdds}"
OPENDDS_BIN="${AIDDS_INTEROP_BIN_OPENDDS:-$ROOT/target/interop-opendds}"

run_vendor_tests "CycloneDDS" interop_cyclonedds "$CYCLONE_BIN" || true
run_vendor_tests "Fast DDS" interop_fastdds "$FASTDDS_BIN" || true
run_vendor_tests "OpenDDS" interop_opendds "$OPENDDS_BIN" || true

run_shapes_tests "CycloneDDS" interop_shapes_cyclonedds "$CYCLONE_BIN" || true
run_shapes_tests "Fast DDS" interop_shapes_fastdds "$FASTDDS_BIN" || true

echo "==> Done"
