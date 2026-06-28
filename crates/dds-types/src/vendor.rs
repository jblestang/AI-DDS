//! # Vendor — DDS Vendor Identification
//!
//! Each RTPS implementation is identified by a 2-byte vendor ID. This is
//! sent in the RTPS message header for diagnostics and interoperability.
//!
//! Reference: RTPS §8.2.4.2 — VendorId

use std::fmt;

/// A 2-byte vendor identifier. The first byte is the major ID (assigned by
/// OMG), and the second is a minor/sub-ID managed by the vendor.
///
/// Reference: RTPS §8.2.4.2, Table 8.11
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct VendorId(pub [u8; 2]);

impl VendorId {
    /// Unknown vendor — the default/sentinel value.
    pub const UNKNOWN: Self = Self([0x00, 0x00]);

    /// Our vendor ID. Using a placeholder value in the "experimental" range.
    /// A real vendor ID should be registered with the OMG.
    // TODO: Register a proper vendor ID with OMG when ready.
    pub const THIS_IMPLEMENTATION: Self = Self([0x01, 0x42]);

    /// Create a `VendorId` from raw bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 2]) -> Self {
        Self(bytes)
    }

    /// Check whether this is the unknown/unset vendor.
    #[must_use]
    pub const fn is_unknown(&self) -> bool {
        self.0[0] == 0 && self.0[1] == 0
    }
}

impl fmt::Debug for VendorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VendorId({:02x}.{:02x})", self.0[0], self.0[1])
    }
}

impl fmt::Display for VendorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02x}.{:02x}", self.0[0], self.0[1])
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vendor_id_unknown() {
        assert!(VendorId::UNKNOWN.is_unknown());
        assert_eq!(VendorId::UNKNOWN.0, [0, 0]);
    }

    #[test]
    fn vendor_id_this_implementation_is_not_unknown() {
        assert!(!VendorId::THIS_IMPLEMENTATION.is_unknown());
    }

    #[test]
    fn vendor_id_debug_format() {
        let vid = VendorId::new([0x01, 0x0F]);
        assert_eq!(format!("{vid:?}"), "VendorId(01.0f)");
    }

    #[test]
    fn vendor_id_display_format() {
        let vid = VendorId::new([0xAB, 0xCD]);
        assert_eq!(format!("{vid}"), "ab.cd");
    }

    #[test]
    fn vendor_id_equality() {
        let a = VendorId::new([1, 2]);
        let b = VendorId::new([1, 2]);
        let c = VendorId::new([3, 4]);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
