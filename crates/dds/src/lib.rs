//! # dds — Full DDS Stack Façade
//!
//! Re-exports all DDS crates under a single umbrella for convenience.
//! Use feature flags to control which components are included.

#![forbid(unsafe_code)]
#![warn(
    rust_2018_idioms,
    nonstandard_style,
    future_incompatible,
    clippy::all,
    clippy::restriction,
    clippy::pedantic,
    clippy::nursery
)]
#![allow(
    clippy::pub_use,
    clippy::blanket_clippy_restriction_lints,
    clippy::missing_docs_in_private_items,
    clippy::implicit_return,
    clippy::doc_markdown,
    reason = "Facade crates require re-exports, implicit returns are standard Rust style, and restriction lints are overly strict for this facade."
)]

/// Foundation types: GUIDs, `QoS`, time, locators, status.
pub use dds_types as types;

/// CDR/XCDR serialization.
pub use dds_cdr as cdr;

/// RTPS wire protocol engine.
pub use dds_rtps as rtps;

/// SPDP/SEDP discovery protocols.
pub use dds_discovery as discovery;

/// DCPS API (`DomainParticipant`, Topic, Pub/Sub).
pub use dds_core as core;

/// `XTypes` type system.
pub use dds_xtypes as xtypes;

/// DDS Security (opt-in via `security` feature).
#[cfg(feature = "security")]
pub use dds_security as security;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_facade_reexports() {
        // Verify default structures can be instantiated via facade paths
        let _prefix = types::guid::GuidPrefix::UNKNOWN;
        let _endian = cdr::Endianness::LittleEndian;

        #[cfg(feature = "security")]
        {
            let _auth = security::BuiltinAuthentication::new();
        }
    }
}
