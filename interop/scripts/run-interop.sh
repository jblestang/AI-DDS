#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

echo "==> Wire compliance tests (no external deps)"
cargo test -p dds --test interop_wire

if [[ -n "${AIDDS_INTEROP_BIN:-}" ]] && [[ -x "${AIDDS_INTEROP_BIN}/interop_publisher" ]]; then
  echo "==> Live CycloneDDS interop tests"
  cargo test -p dds --test interop_cyclonedds -- --ignored --nocapture
else
  echo "==> Skipping live interop (set AIDDS_INTEROP_BIN to enable)"
fi

echo "==> Done"
