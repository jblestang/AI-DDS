# Interoperability Harness

This directory contains tools for testing **AI-DDS** against external DDS implementations for wire-format compliance and live interoperability.

## Reference implementation: CycloneDDS

[CycloneDDS](https://github.com/eclipse-cyclonedds/cyclonedds) is used as the reference stack (DDSI-RTPS compliant, widely deployed).

### Setup

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
export AIDDS_INTEROP_BIN=$PWD/target/interop-cyclonedds
./interop/scripts/run-interop.sh
```

## Test tiers

| Tier | Command | Requires CycloneDDS |
|------|---------|---------------------|
| Wire fixtures | `cargo test -p dds --test interop_wire` | No |
| CDR encapsulation | (included in `interop_wire`) | No |
| Live interop | `cargo test -p dds --test interop_cyclonedds -- --ignored` | Yes |

### Wire fixtures (`interop/wire/`)

Captured RTPS packets from CycloneDDS 0.10.5. Validated on every CI run:

- `cyclonedds_spdp.bin` — SPDP participant announcement (PL-CDR)
- `cyclonedds_data_cdr_le.bin` — metatraffic/user DATA sample
- `cyclonedds_sedp.bin` — additional discovery capture

Regenerate after CycloneDDS upgrades:

```bash
./interop/scripts/capture-fixtures.sh
```

## Shared IDL

`interop/idl/InteropMessage.idl` defines the cross-vendor test type:

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

Rust tests use `InteropTypeSupport` with **CdrLe encapsulation** for user data.

## Environment variables

| Variable | Description |
|----------|-------------|
| `CYCLONEDDS_PREFIX` | CycloneDDS install prefix (for building peer apps) |
| `AIDDS_INTEROP_BIN` | Directory containing `interop_publisher` / `interop_subscriber` |
| `AIDDS_INTEROP_DOMAIN` | DDS domain id (default `70`) |
| `AIDDS_INTEROP_WAIT_MATCH` | CycloneDDS publisher waits for reader (`1` default) |
| `AIDDS_INTEROP_TIMEOUT_MS` | CycloneDDS subscriber timeout |

## Compliance status

| Area | Status |
|------|--------|
| Parse CycloneDDS SPDP (PL-CDR) | Pass |
| Parse CycloneDDS RTPS framing | Pass |
| Emit standard PL-CDR SPDP/SEDP | Implemented |
| Multicast SEDP receive | Implemented |
| Live discovery + data with CycloneDDS | In progress (live tests `#[ignore]`) |
| CdrLe user-data encapsulation | Pass |

## Peer applications

- `interop/cyclonedds/publisher.c` — publishes one `InteropMessage`
- `interop/cyclonedds/subscriber.c` — receives and prints `INTEROP_RECEIVE`

Both print machine-parseable lines for test orchestration.
