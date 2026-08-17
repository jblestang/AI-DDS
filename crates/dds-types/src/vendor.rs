//! # Vendor — DDS Vendor Identification
//!
//! Each RTPS implementation is identified by a 2-byte vendor ID. This is
//! sent in the RTPS message header for diagnostics and interoperability.
//!
//! Reference: RTPS §8.2.4.2 — `Id`.

use core::fmt;

/// A 2-byte vendor identifier. The first byte is the major ID (assigned by
/// OMG), and the second is a minor/sub-ID managed by the vendor.
///
/// Reference: RTPS §8.2.4.2, Table 8.11.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Id(pub [u8; 2]);

impl Id {
    /// AI-DDS Vendor ID.
    pub const THIS_IMPLEMENTATION: Self = Self([0x01, 0x1A]);

    /// Unknown vendor — the default/sentinel value.
    pub const UNKNOWN: Self = Self([0x00, 0x00]);

    /// Check whether this is the unknown/unset vendor.
    #[must_use]
    #[inline]
    pub const fn is_unknown(&self) -> bool {
        return self.0[0] == 0 && self.0[1] == 0
    }
    /// Create a `Id` from raw bytes.
    #[must_use]
    #[inline]
    pub const fn new(bytes: [u8; 2]) -> Self {
        return Self(bytes)
    }

}

impl fmt::Debug for Id {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        return write!(f, "Id({:02x}.{:02x})", self.0[0], self.0[1])
    }
}

impl fmt::Display for Id {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        return write!(f, "{:02x}.{:02x}", self.0[0], self.0[1])
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
        assert!(Id::UNKNOWN.is_unknown());
        assert_eq!(Id::UNKNOWN.0, [0, 0]);
    }

    #[test]
    fn vendor_id_this_implementation_is_not_unknown() {
        assert!(!Id::THIS_IMPLEMENTATION.is_unknown());
    }

    #[test]
    fn vendor_id_debug_format() {
        let vid = Id::new([0x01, 0x0F]);
        assert_eq!(format!("{vid:?}"), "Id(01.0f)");
    }

    #[test]
    fn vendor_id_display_format() {
        let vid = Id::new([0xAB, 0xCD]);
        assert_eq!(format!("{vid}"), "ab.cd");
    }

    #[test]
    fn vendor_id_equality() {
        let a = Id::new([1, 2]);
        let b = Id::new([1, 2]);
        let c = Id::new([3, 4]);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
