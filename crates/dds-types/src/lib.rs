//! # dds-types — Foundation Types for the DDS Stack
//!
//! This crate provides the shared primitive types used across all DDS crates:
//! GUIDs, `QoS` policies, time representations, return codes, instance handles,
//! status types, builtin topic data, locators, and vendor identifiers.
//!
//! All types follow the OMG DDS DCPS 1.4 and DDSI-RTPS 2.5 specifications.
//!
//! # Modules
//!
//! - [`guid`] — RTPS entity identification (`Prefix`, `EntityId`, GUID)
//! - [`qos`] — All 22 `QoS` policies as Rust structs/enums
//! - [`time`] — Duration and Timestamp with spec-defined constants
//! - [`return_code`] — `DdsResult<T>` and `DdsError` error types
//! - [`instance`] — Instance handles and key hashing
//! - [`status`] — Communication status types
//! - [`builtin_topics`] — Builtin topic data structures for discovery
//! - [`locator`] — Network locator (transport address)
//! - [`vendor`] — Vendor identification

#![forbid(unsafe_code)]
#![warn(
    rust_2018_idioms,
    nonstandard_style,
    future_incompatible
)]
#![allow(
    clippy::blanket_clippy_restriction_lints,
    reason = "restriction lints are enabled individually via workspace lint config"
)]
pub mod builtin_topics;
pub mod guid;
pub mod instance;
pub mod locator;
pub mod policy_id;
pub mod qos;
pub mod return_code;
pub mod status;
pub mod time;
pub mod vendor;
