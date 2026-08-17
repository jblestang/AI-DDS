#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BUILD_DIR="${AIDDS_INTEROP_BUILD_DIR:-$ROOT/target/interop-cyclonedds}"
CYCLONEDDS_PREFIX="${CYCLONEDDS_PREFIX:-${CYCLONEDDS_ROOT:-}}"

if [[ -z "$CYCLONEDDS_PREFIX" ]]; then
  echo "CYCLONEDDS_PREFIX is not set. Clone, build, and install CycloneDDS first." >&2
  echo "Example:" >&2
  echo "  git clone --depth 1 --branch 0.10.5 https://github.com/eclipse-cyclonedds/cyclonedds.git /tmp/cyclonedds" >&2
  echo "  cmake -S /tmp/cyclonedds -B /tmp/cyclonedds/build -DCMAKE_BUILD_TYPE=Release" >&2
  echo "  cmake --build /tmp/cyclonedds/build -j" >&2
  echo "  cmake --install /tmp/cyclonedds/build --prefix /tmp/cyclonedds-install" >&2
  echo "  export CYCLONEDDS_PREFIX=/tmp/cyclonedds-install" >&2
  exit 1
fi

cmake -S "$ROOT/interop/cyclonedds" -B "$BUILD_DIR" \
  -DCMAKE_PREFIX_PATH="$CYCLONEDDS_PREFIX" \
  -DCMAKE_BUILD_TYPE=Release
cmake --build "$BUILD_DIR" -j"$(nproc)"

echo "Built:"
echo "  $BUILD_DIR/interop_publisher"
echo "  $BUILD_DIR/interop_subscriber"
echo "  $BUILD_DIR/shapes_publisher"
echo "  $BUILD_DIR/shapes_subscriber"
