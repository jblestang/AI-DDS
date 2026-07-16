//! Live interoperability tests against Fast DDS (optional; requires built binaries).
//!
//! Build Fast DDS peer apps:
//! ```bash
//! export FASTDDS_PREFIX=/path/to/fastdds-install
//! export FASTDDSGEN=/path/to/fastddsgen   # optional if on PATH
//! interop/scripts/build-fastdds-apps.sh
//! cargo test -p dds --test interop_fastdds -- --ignored --nocapture --test-threads=1
//! ```

mod interop_common;

use interop_common::interop_vendor::{
    aidds_publishes_vendor_receives, bidirectional_discovery_matchmaking,
    vendor_publishes_aidds_receives,
};
use interop_common::InteropVendor;

#[test]
#[ignore = "requires Fast DDS interop binaries (see interop/README.md)"]
fn interop_fastdds_publishes_aidds_receives() {
    vendor_publishes_aidds_receives(InteropVendor::FastDds);
}

#[test]
#[ignore = "requires Fast DDS interop binaries (see interop/README.md)"]
fn interop_aidds_publishes_fastdds_receives() {
    aidds_publishes_vendor_receives(InteropVendor::FastDds);
}

#[test]
#[ignore = "requires Fast DDS interop binaries (see interop/README.md)"]
fn interop_bidirectional_discovery_matchmaking_fastdds() {
    bidirectional_discovery_matchmaking(InteropVendor::FastDds);
}
