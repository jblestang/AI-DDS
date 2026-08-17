//! Live Shapes Demo interoperability tests against CycloneDDS (optional).
//!
//! Build peer apps (includes `shapes_publisher` / `shapes_subscriber`):
//! ```bash
//! export CYCLONEDDS_PREFIX=/path/to/cyclonedds-install
//! interop/scripts/build-cyclonedds-apps.sh
//! cargo test -p dds --test interop_shapes_cyclonedds -- --ignored --nocapture --test-threads=1
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;
use support::interop_common;

use support::interop_common::InteropVendor;
use support::shapes_vendor::{
    all_shapes_topics_aidds_to_vendor, all_shapes_topics_vendor_to_aidds,
    vendor_publishes_shapes_aidds_receives,
};

#[test]
#[ignore = "requires CycloneDDS shapes binaries (see interop/README.md)"]
fn shapes_cyclonedds_publishes_square_aidds_receives() {
    vendor_publishes_shapes_aidds_receives(InteropVendor::CycloneDds, "Square");
}

#[test]
#[ignore = "requires CycloneDDS shapes binaries (see interop/README.md)"]
fn shapes_cyclonedds_all_topics_to_aidds() {
    all_shapes_topics_vendor_to_aidds(InteropVendor::CycloneDds);
}

#[test]
#[ignore = "requires CycloneDDS shapes binaries (see interop/README.md)"]
fn shapes_aidds_all_topics_to_cyclonedds() {
    all_shapes_topics_aidds_to_vendor(InteropVendor::CycloneDds);
}
