//! # dds-types — Foundation Types for the DDS Stack
//!
//! This crate provides the shared primitive types used across all DDS crates:
//! GUIDs, QoS policies, time representations, return codes, instance handles,
//! status types, builtin topic data, locators, and vendor identifiers.
//!
//! All types follow the OMG DDS DCPS 1.4 and DDSI-RTPS 2.5 specifications.
//!
//! # Modules
//!
//! - [`guid`] — RTPS entity identification (GuidPrefix, EntityId, GUID)
//! - [`qos`] — All 22 QoS policies as Rust structs/enums
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
    future_incompatible,
    clippy::all,
    clippy::restriction,
    clippy::pedantic,
    clippy::nursery
)]
#![allow(
    clippy::blanket_clippy_restriction_lints,
    clippy::implicit_return,
    clippy::pub_use,
    clippy::indexing_slicing,
    clippy::string_slice,
    clippy::absolute_paths,
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_lossless,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::missing_inline_in_public_items,
    clippy::shadow_reuse,
    clippy::shadow_same,
    clippy::shadow_unrelated,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::wildcard_imports,
    clippy::integer_division,
    clippy::integer_division_remainder_used,
    clippy::single_call_fn,
    clippy::default_numeric_fallback,
    clippy::arithmetic_side_effects,
    clippy::std_instead_of_core,
    clippy::std_instead_of_alloc,
    clippy::alloc_instead_of_core,
    clippy::arbitrary_source_item_ordering,
    clippy::min_ident_chars,
    clippy::exhaustive_enums,
    clippy::exhaustive_structs,
    clippy::module_name_repetitions,
    clippy::question_mark_used,
    clippy::single_char_lifetime_names,
    clippy::panic_in_result_fn,
    clippy::unwrap_used,
    clippy::unwrap_in_result,
    clippy::cognitive_complexity,
    clippy::tests_outside_test_module,
    clippy::missing_docs_in_private_items,
    clippy::pattern_type_mismatch,
    clippy::redundant_pub_crate,
    clippy::similar_names,
    clippy::else_if_without_else,
    clippy::unseparated_literal_suffix,
    clippy::separated_literal_suffix,
    reason = "DDS Types implementation requires standard library conversions, standard returns, and spec-defined structures."
)]

pub mod builtin_topics;
pub mod guid;
pub mod instance;
pub mod locator;
pub mod qos;
pub mod return_code;
pub mod status;
pub mod time;
pub mod vendor;
