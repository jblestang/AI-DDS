#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BUILD_DIR="${AIDDS_INTEROP_BUILD_DIR:-$ROOT/target/interop-fastdds}"
FASTDDS_PREFIX="${FASTDDS_PREFIX:-${FASTDDS_ROOT:-}}"

# Default `c++` is often Clang, which may not find libstdc++ without extra -L flags.
export CC="${CC:-gcc}"
export CXX="${CXX:-g++}"

if [[ -z "$FASTDDS_PREFIX" ]]; then
  echo "FASTDDS_PREFIX is not set. Build and install Fast DDS first." >&2
  echo "Example:" >&2
  echo "  git clone --depth 1 --branch v3.1.1 https://github.com/eProsima/Fast-DDS.git /tmp/fastdds" >&2
  echo "  git clone --depth 1 --branch v4.0.0 https://github.com/eProsima/Fast-DDS-Gen.git /tmp/fastdds-gen" >&2
  echo "  # build Fast-CDR, Fast-DDS, install to /tmp/fastdds-install" >&2
  echo "  export FASTDDS_PREFIX=/tmp/fastdds-install" >&2
  echo "  export FASTDDSGEN=/tmp/fastdds-gen/scripts/fastddsgen" >&2
  exit 1
fi

CMAKE_ARGS=(
  -S "$ROOT/interop/fastdds"
  -B "$BUILD_DIR"
  -DCMAKE_PREFIX_PATH="$FASTDDS_PREFIX"
  -DCMAKE_BUILD_TYPE=Release
  -DCMAKE_C_COMPILER="$CC"
  -DCMAKE_CXX_COMPILER="$CXX"
)

if [[ -n "${FASTDDSGEN:-}" ]]; then
  export FASTDDSGEN
fi

cmake "${CMAKE_ARGS[@]}"
cmake --build "$BUILD_DIR" -j"$(nproc)"

echo "Built:"
echo "  $BUILD_DIR/interop_publisher"
echo "  $BUILD_DIR/interop_subscriber"
echo "  $BUILD_DIR/shapes_publisher"
echo "  $BUILD_DIR/shapes_subscriber"
