#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BUILD_DIR="${AIDDS_INTEROP_BUILD_DIR:-$ROOT/target/interop-opendds}"
OPENDDS_PREFIX="${OPENDDS_PREFIX:-${OPENDDS_ROOT:-}}"

export CC="${CC:-gcc}"
export CXX="${CXX:-g++}"

if [[ -z "$OPENDDS_PREFIX" ]]; then
  echo "OPENDDS_PREFIX is not set. Build and install OpenDDS first." >&2
  echo "Example:" >&2
  echo "  interop/scripts/build-opendds.sh" >&2
  echo "  export OPENDDS_PREFIX=/tmp/opendds-install" >&2
  exit 1
fi

if [[ ! -f "$OPENDDS_PREFIX/share/cmake/OpenDDS/OpenDDSConfig.cmake" ]]; then
  echo "OpenDDS CMake package not found under $OPENDDS_PREFIX" >&2
  echo "Run interop/scripts/build-opendds.sh first." >&2
  exit 1
fi

OPENDDS_SRC="${OPENDDS_SRC:-/tmp/opendds}"
ACE_BUILD="${OPENDDS_ACE_BUILD:-$OPENDDS_SRC/build/ace_tao}"
TAO_BUILD="${OPENDDS_TAO_BUILD:-$OPENDDS_SRC/build/ace_tao/TAO}"

# OpenDDS install omits ACE/TAO static libs; point CMake at the build tree.
if [[ ! -f "$ACE_BUILD/ace/libACE.a" && ! -L "$ACE_BUILD/lib/libACE.a" ]]; then
  echo "ACE build tree not found at $ACE_BUILD" >&2
  echo "Run interop/scripts/build-opendds.sh first (sets OPENDDS_SRC=$OPENDDS_SRC)." >&2
  exit 1
fi

CMAKE_ARGS=(
  -S "$ROOT/interop/opendds"
  -B "$BUILD_DIR"
  -DCMAKE_PREFIX_PATH="$OPENDDS_PREFIX"
  -DOPENDDS_BUILD_DIR="$OPENDDS_SRC/build"
  -DACE_ROOT="$ACE_BUILD"
  -DACE_LIB_DIR="$ACE_BUILD/lib"
  -DACE_BIN_DIR="$ACE_BUILD/bin"
  -DACE_INCLUDE_DIRS="$ACE_BUILD"
  -DTAO_ROOT="$TAO_BUILD"
  -DTAO_LIB_DIR="$ACE_BUILD/lib"
  -DTAO_BIN_DIR="$ACE_BUILD/bin"
  -DTAO_INCLUDE_DIR="$TAO_BUILD"
  -DTAO_INCLUDE_DIRS="$TAO_BUILD;$TAO_BUILD/orbsvcs"
  -DCMAKE_BUILD_TYPE=Release
  -DCMAKE_CXX_STANDARD=14
  -DCMAKE_C_COMPILER="$CC"
  -DCMAKE_CXX_COMPILER="$CXX"
)

cmake "${CMAKE_ARGS[@]}"

cmake --build "$BUILD_DIR" -j"$(nproc)"

echo "Built:"
echo "  $BUILD_DIR/interop_publisher"
echo "  $BUILD_DIR/interop_subscriber"
