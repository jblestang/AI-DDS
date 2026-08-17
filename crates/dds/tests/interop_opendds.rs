//! Live interoperability tests against OpenDDS (optional; requires built binaries).
//!
//! Build OpenDDS peer apps:
//! ```bash
//! interop/scripts/build-opendds.sh
//! export OPENDDS_PREFIX=/tmp/opendds-install
//! interop/scripts/build-opendds-apps.sh
//! cargo test -p dds --test interop_opendds -- --ignored --nocapture --test-threads=1
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
#[ignore = "requires OpenDDS interop binaries (see interop/README.md)"]
fn interop_opendds_publishes_aidds_receives() {
    vendor_publishes_aidds_receives(InteropVendor::OpenDds);
}

#[test]
#[ignore = "requires OpenDDS interop binaries (see interop/README.md)"]
fn interop_aidds_publishes_opendds_receives() {
    aidds_publishes_vendor_receives(InteropVendor::OpenDds);
}

#[test]
#[ignore = "requires OpenDDS interop binaries (see interop/README.md)"]
fn interop_bidirectional_discovery_matchmaking_opendds() {
    bidirectional_discovery_matchmaking(InteropVendor::OpenDds);
}
