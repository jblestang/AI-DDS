//! Live interoperability tests against CycloneDDS (optional; requires built binaries).
//!
//! Build CycloneDDS peer apps:
//! ```bash
//! export CYCLONEDDS_PREFIX=/path/to/cyclonedds-install
//! interop/scripts/build-cyclonedds-apps.sh
//! cargo test -p dds --test interop_cyclonedds -- --ignored --nocapture --test-threads=1
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;
use support::interop_common;

use support::interop_vendor::{
    aidds_publishes_vendor_receives, bidirectional_discovery_matchmaking,
    vendor_publishes_aidds_receives,
};
use support::interop_common::InteropVendor;

#[test]
#[ignore = "requires CycloneDDS interop binaries (see interop/README.md)"]
fn interop_cyclonedds_publishes_aidds_receives() {
    vendor_publishes_aidds_receives(InteropVendor::CycloneDds);
}

#[test]
#[ignore = "requires CycloneDDS interop binaries (see interop/README.md)"]
fn interop_aidds_publishes_cyclonedds_receives() {
    aidds_publishes_vendor_receives(InteropVendor::CycloneDds);
}

#[test]
#[ignore = "requires CycloneDDS interop binaries (see interop/README.md)"]
fn interop_bidirectional_discovery_matchmaking() {
    bidirectional_discovery_matchmaking(InteropVendor::CycloneDds);
}
