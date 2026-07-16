//! Live interoperability tests against OpenSplice (optional; requires built binaries).
//!
//! Build OpenSplice peer apps:
//! ```bash
//! export OSPL_HOME=/path/to/opensplice-install
//! source "$OSPL_HOME/release.com"
//! interop/scripts/build-opensplice-apps.sh
//! cargo test -p dds --test interop_opensplice -- --ignored --nocapture --test-threads=1
//! ```

mod interop_common;

use interop_common::interop_vendor::{
    aidds_publishes_vendor_receives, bidirectional_discovery_matchmaking,
    vendor_publishes_aidds_receives,
};
use interop_common::InteropVendor;

#[test]
#[ignore = "requires OpenSplice interop binaries (see interop/README.md)"]
fn interop_opensplice_publishes_aidds_receives() {
    vendor_publishes_aidds_receives(InteropVendor::OpenSplice);
}

#[test]
#[ignore = "requires OpenSplice interop binaries (see interop/README.md)"]
fn interop_aidds_publishes_opensplice_receives() {
    aidds_publishes_vendor_receives(InteropVendor::OpenSplice);
}

#[test]
#[ignore = "requires OpenSplice interop binaries (see interop/README.md)"]
fn interop_bidirectional_discovery_matchmaking_opensplice() {
    bidirectional_discovery_matchmaking(InteropVendor::OpenSplice);
}
