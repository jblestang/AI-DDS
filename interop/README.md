# Interoperability Harness

This directory contains tools for testing **AI-DDS** against external DDS implementations for wire-format compliance and live interoperability.

All vendors share `interop/idl/InteropMessage.idl` and the same machine-parseable stdout protocol (`INTEROP_PUBLISH` / `INTEROP_RECEIVE`).

## Vendors

| Vendor | Peer apps | Default domain base | Build script |
|--------|-----------|---------------------|--------------|
| [CycloneDDS](https://github.com/eclipse-cyclonedds/cyclonedds) | C (`interop/cyclonedds/`) | 70 | `build-cyclonedds-apps.sh` |
| [Fast DDS](https://github.com/eProsima/Fast-DDS) | C++ (`interop/fastdds/`) | 80 | `build-fastdds-apps.sh` |
| [OpenSplice](https://www.zettascale.tech/opensplice) / Vortex | C++ isocpp2 (`interop/opensplice/`) | 90 | `build-opensplice-apps.sh` |

Each vendor runs the same three live tests (domains `base`, `base+1`, `base+2`):

1. Vendor publishes → AI-DDS receives
2. AI-DDS publishes → vendor receives
3. Bidirectional discovery + monitor snapshot

## Quick start (CycloneDDS)

```bash
# 1. Build and install CycloneDDS
git clone --depth 1 --branch 0.10.5 https://github.com/eclipse-cyclonedds/cyclonedds.git /tmp/cyclonedds
cmake -S /tmp/cyclonedds -B /tmp/cyclonedds/build -DCMAKE_BUILD_TYPE=Release
cmake --build /tmp/cyclonedds/build -j
cmake --install /tmp/cyclonedds/build --prefix /tmp/cyclonedds-install

# 2. Build interop peer applications
export CYCLONEDDS_PREFIX=/tmp/cyclonedds-install
./interop/scripts/build-cyclonedds-apps.sh

# 3. Run compliance + live interop tests
./interop/scripts/run-interop.sh
```

## Fast DDS setup

```bash
# Build Fast-CDR + Fast-DDS + Fast-DDS-Gen, install to a prefix, then:
export FASTDDS_PREFIX=/tmp/fastdds-install
export FASTDDSGEN=/path/to/fastddsgen   # optional if on PATH
# Uses g++ by default (see build-fastdds-apps.sh)
./interop/scripts/build-fastdds-apps.sh
cargo test -p dds --test interop_fastdds -- --ignored --nocapture --test-threads=1
```

## OpenSplice setup

```bash
export OSPL_HOME=/opt/VortexOpenSplice/HDE/x86_64.linux
source "$OSPL_HOME/release.com"
./interop/scripts/build-opensplice-apps.sh
cargo test -p dds --test interop_opensplice -- --ignored --nocapture --test-threads=1
```

## Test tiers

| Tier | Command | External deps |
|------|---------|---------------|
| Wire fixtures | `cargo test -p dds --test interop_wire` | No |
| CDR encapsulation | (included in `interop_wire`) | No |
| Live CycloneDDS | `cargo test -p dds --test interop_cyclonedds -- --ignored --test-threads=1` | Cyclone build |
| Live Fast DDS | `cargo test -p dds --test interop_fastdds -- --ignored --test-threads=1` | Fast DDS build |
| Live OpenSplice | `cargo test -p dds --test interop_opensplice -- --ignored --test-threads=1` | OpenSplice install |

Run all available vendors:

```bash
./interop/scripts/run-interop.sh
```

### Wire fixtures (`interop/wire/`)

Captured RTPS packets from CycloneDDS 0.10.5:

- `cyclonedds_spdp.bin` — SPDP participant announcement (PL-CDR)
- `cyclonedds_data_cdr_le.bin` — metatraffic/user DATA sample
- `cyclonedds_sedp.bin` — additional discovery capture

Regenerate: `./interop/scripts/capture-fixtures.sh`

## Shared IDL

```idl
module AiDdsInterop {
  struct Message {
    unsigned long id;
    string payload;
  };
};
```

Topic: `AiDdsInteropMessage`  
Type: `AiDdsInterop::Message`

Rust tests use `InteropTypeSupport` with **CdrLe encapsulation** for outbound user data and accept **CdrLe/CdrBe** on deserialize.

## Environment variables

| Variable | Description |
|----------|-------------|
| `CYCLONEDDS_PREFIX` | CycloneDDS install prefix |
| `FASTDDS_PREFIX` | Fast DDS install prefix (`CMAKE_PREFIX_PATH`) |
| `FASTDDSGEN` | Path to `fastddsgen` (optional if on `PATH`) |
| `OSPL_HOME` | OpenSplice / Vortex install root |
| `AIDDS_INTEROP_BIN_CYCLONEDDS` | Built Cyclone peer app directory |
| `AIDDS_INTEROP_BIN_FASTDDS` | Built Fast DDS peer app directory |
| `AIDDS_INTEROP_BIN_OPENSPLICE` | Built OpenSplice peer app directory |
| `AIDDS_INTEROP_BIN` | Legacy alias for Cyclone peer app directory |
| `AIDDS_INTEROP_DOMAIN` | DDS domain id for peer apps (per-run) |
| `AIDDS_INTEROP_WAIT_MATCH` | Publisher waits for reader (`1` default) |
| `AIDDS_INTEROP_TIMEOUT_MS` | Subscriber timeout |

Default build output directories:

- `target/interop-cyclonedds/`
- `target/interop-fastdds/`
- `target/interop-opensplice/`

## Compliance status

| Area | Status |
|------|--------|
| Parse CycloneDDS SPDP (PL-CDR) | Pass |
| Parse CycloneDDS RTPS framing | Pass |
| Live CycloneDDS pub/sub | Pass (`interop_cyclonedds`, `#[ignore]`) |
| Live Fast DDS pub/sub | Harness ready (`interop_fastdds`, `#[ignore]`) |
| Live OpenSplice pub/sub | Harness ready (`interop_opensplice`, `#[ignore]`) |
| CdrLe/CdrBe user-data encapsulation | Pass |

## Peer applications

Each vendor directory contains `interop_publisher` and `interop_subscriber` printing:

- `INTEROP_PUBLISH id=<n> payload=<s> domain=<d>`
- `INTEROP_RECEIVE id=<n> payload=<s> domain=<d>`

These lines are parsed by the Rust live interop tests in `crates/dds/tests/interop_*.rs`.
