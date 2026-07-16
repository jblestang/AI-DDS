#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BUILD_DIR="${AIDDS_INTEROP_BUILD_DIR:-$ROOT/target/interop-opensplice}"
OSPL_HOME="${OSPL_HOME:-}"

# Default `c++` is often Clang, which may not find libstdc++ without extra -L flags.
export CC="${CC:-gcc}"
export CXX="${CXX:-g++}"

if [[ -z "$OSPL_HOME" ]]; then
  echo "OSPL_HOME is not set. Install OpenSplice / Vortex OpenSplice first." >&2
  echo "Example:" >&2
  echo "  export OSPL_HOME=/opt/VortexOpenSplice/HDE/x86_64.linux" >&2
  echo "  source \"\$OSPL_HOME/release.com\"" >&2
  exit 1
fi

if [[ -f "$OSPL_HOME/release.com" ]]; then
  # shellcheck disable=SC1090
  source "$OSPL_HOME/release.com"
fi

cmake -S "$ROOT/interop/opensplice" -B "$BUILD_DIR" \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_C_COMPILER="$CC" \
  -DCMAKE_CXX_COMPILER="$CXX"
cmake --build "$BUILD_DIR" -j"$(nproc)"

echo "Built:"
echo "  $BUILD_DIR/interop_publisher"
echo "  $BUILD_DIR/interop_subscriber"
