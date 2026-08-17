#!/usr/bin/env bash
# Build Fast-CDR, Fast-DDS, and Fast-DDS-Gen into /tmp/fastdds-install
set -euo pipefail

PREFIX="${FASTDDS_PREFIX:-/tmp/fastdds-install}"
export CC="${CC:-gcc}"
export CXX="${CXX:-g++}"

# Fast DDS 2.14 installs CMake package "fastrtps" under share/ (not lib/cmake/fastdds).
FASTRTPS_CMAKE="$PREFIX/share/fastrtps/cmake/fastrtps-config.cmake"

ensure_build_deps() {
  if command -v apt-get >/dev/null 2>&1; then
    local missing=()
    for pkg in libasio-dev libtinyxml2-dev libfoonathan-memory-dev libssl-dev libboost-dev; do
      if ! dpkg-query -W -f='${Status}' "$pkg" 2>/dev/null | grep -q "install ok installed"; then
        missing+=("$pkg")
      fi
    done
    if ((${#missing[@]} > 0)); then
      echo "Installing Fast DDS build dependencies: ${missing[*]}"
      sudo apt-get update -qq
      sudo apt-get install -y -qq "${missing[@]}"
    fi
  fi
}

clone_if_missing() {
  local dir="$1" url="$2" branch="$3"
  if [[ ! -d "$dir/.git" ]]; then
    git clone --depth 1 --branch "$branch" "$url" "$dir"
  fi
}

ensure_build_deps

clone_if_missing /tmp/fastcdr https://github.com/eProsima/Fast-CDR.git v2.2.2
clone_if_missing /tmp/fastdds https://github.com/eProsima/Fast-DDS.git v2.14.4
clone_if_missing /tmp/fastdds-gen https://github.com/eProsima/Fast-DDS-Gen.git v4.0.1

if [[ ! -f "$PREFIX/lib/cmake/fastcdr/fastcdr-config.cmake" ]]; then
  cmake -S /tmp/fastcdr -B /tmp/fastcdr/build \
    -DCMAKE_INSTALL_PREFIX="$PREFIX" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_C_COMPILER="$CC" \
    -DCMAKE_CXX_COMPILER="$CXX"
  cmake --build /tmp/fastcdr/build -j"$(nproc)"
  cmake --install /tmp/fastcdr/build
fi

if [[ ! -f "$FASTRTPS_CMAKE" ]]; then
  cmake -S /tmp/fastdds -B /tmp/fastdds/build \
    -DCMAKE_INSTALL_PREFIX="$PREFIX" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_C_COMPILER="$CC" \
    -DCMAKE_CXX_COMPILER="$CXX"
  cmake --build /tmp/fastdds/build -j"$(nproc)"
  cmake --install /tmp/fastdds/build
fi

if [[ ! -f /tmp/fastdds-gen2/share/fastddsgen/java/fastddsgen.jar ]]; then
  clone_if_missing /tmp/fastdds-gen2 https://github.com/eProsima/Fast-DDS-Gen.git v3.3.1
  if [[ -f /tmp/fastdds-gen2/gradlew ]]; then
    (cd /tmp/fastdds-gen2 && JAVA_HOME="${JAVA_HOME:-/usr/lib/jvm/java-17-openjdk-amd64}" ./gradlew assemble)
  fi
fi

echo "Fast DDS installed to $PREFIX"
echo "fastddsgen: /tmp/fastdds-gen2/scripts/fastddsgen (v3.3.1 for Fast DDS 2.14)"
