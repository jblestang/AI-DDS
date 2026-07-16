#!/usr/bin/env bash
# Build OpenDDS (ACE/TAO + DCPS) into /tmp/opendds-install
set -euo pipefail

PREFIX="${OPENDDS_PREFIX:-/tmp/opendds-install}"
export CC="${CC:-gcc}"
export CXX="${CXX:-g++}"
BRANCH="${OPENDDS_BRANCH:-DDS-3.30}"
SRC="${OPENDDS_SRC:-/tmp/opendds}"

clone_if_missing() {
  if [[ ! -d "$SRC/.git" ]]; then
    git clone --depth 1 --branch "$BRANCH" https://github.com/OpenDDS/OpenDDS.git "$SRC"
  fi
}

if [[ ! -f "$PREFIX/share/cmake/OpenDDS/OpenDDSConfig.cmake" ]]; then
  clone_if_missing
  cmake -S "$SRC" -B "$SRC/build" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="$PREFIX" \
    -DCMAKE_CXX_STANDARD=14 \
    -DCMAKE_C_COMPILER="$CC" \
    -DCMAKE_CXX_COMPILER="$CXX"
  cmake --build "$SRC/build" -j"$(nproc)"
  cmake --install "$SRC/build"
  mkdir -p "$PREFIX/share/ace" "$PREFIX/share/tao"
  cp "$SRC/build/ace_tao/VERSION.txt" "$PREFIX/share/ace/VERSION.txt"
  cp "$SRC/build/ace_tao/TAO/VERSION.txt" "$PREFIX/share/tao/VERSION.txt"
fi

echo "OpenDDS installed to $PREFIX"
echo "Set OPENDDS_PREFIX=$PREFIX and extend LD_LIBRARY_PATH with $PREFIX/lib"
