//! Live Shapes Demo interoperability tests against Fast DDS (optional).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;
use support::interop_common;

use support::interop_common::InteropVendor;
use support::shapes_vendor::{
    all_shapes_topics_aidds_to_vendor, all_shapes_topics_vendor_to_aidds,
    vendor_publishes_shapes_aidds_receives,
};

#[test]
#[ignore = "requires Fast DDS shapes binaries (see interop/README.md)"]
fn shapes_fastdds_publishes_square_aidds_receives() {
    // Unique domain offset per test (RTPS multicast port = base + 250 * domain).
    vendor_publishes_shapes_aidds_receives(InteropVendor::FastDds, "Square", 0);
}

#[test]
#[ignore = "requires Fast DDS shapes binaries (see interop/README.md)"]
fn shapes_fastdds_all_topics_to_aidds() {
    // +10..+12 — avoid colliding with square (+0) and aidds→vendor (+20..+22).
    all_shapes_topics_vendor_to_aidds(InteropVendor::FastDds, 10);
}

#[test]
#[ignore = "requires Fast DDS shapes binaries (see interop/README.md)"]
fn shapes_aidds_all_topics_to_fastdds() {
    all_shapes_topics_aidds_to_vendor(InteropVendor::FastDds, 20);
}
