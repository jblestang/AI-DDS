//! # Instance — Instance handles and key hashing for DDS
//!
//! An `InstanceHandle` uniquely identifies a data instance within a Topic.
//! It is derived from the key fields of the data type via MD5 or SHA-256
//! hashing, depending on the key size.
//!
//! Reference: DCPS §2.2.1 — InstanceHandle_t

use sha2::{Digest, Sha256};
use std::fmt;

/// A 16-byte handle uniquely identifying a data instance within a topic.
///
/// For keyed topics, this is computed by hashing the serialized key fields.
/// For keyless topics, a single "nil" handle is used.
///
/// Reference: DCPS §2.2.1, XTypes §7.6.6
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InstanceHandle(pub [u8; 16]);

impl InstanceHandle {
    /// The nil handle — used for keyless topics or as a sentinel.
    pub const NIL: Self = Self([0; 16]);

    /// Create an instance handle from raw bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Check whether this is the nil/sentinel handle.
    #[must_use]
    pub fn is_nil(&self) -> bool {
        self.0 == [0; 16]
    }

    /// Compute an instance handle from serialized key bytes.
    ///
    /// If the key is ≤ 16 bytes, it is used directly (zero-padded).
    /// If the key is > 16 bytes, SHA-256 is used and truncated to 16 bytes.
    ///
    /// This follows the XTypes specification for key hash computation.
    #[must_use]
    pub fn from_key_bytes(key_bytes: &[u8]) -> Self {
        if key_bytes.len() <= 16 {
            // Key fits directly — zero-pad to 16 bytes
            let mut handle = [0u8; 16];
            handle[..key_bytes.len()].copy_from_slice(key_bytes);
            Self(handle)
        } else {
            // Key too large — hash with SHA-256, take first 16 bytes
            let hash = Sha256::digest(key_bytes);
            let mut handle = [0u8; 16];
            handle.copy_from_slice(&hash[..16]);
            Self(handle)
        }
    }
}

impl fmt::Debug for InstanceHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "InstanceHandle(")?;
        for (i, byte) in self.0.iter().enumerate() {
            if i > 0 && i % 4 == 0 {
                write!(f, ":")?;
            }
            write!(f, "{byte:02x}")?;
        }
        write!(f, ")")
    }
}

impl fmt::Display for InstanceHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_nil() {
            write!(f, "NIL")
        } else {
            for byte in &self.0 {
                write!(f, "{byte:02x}")?;
            }
            Ok(())
        }
    }
}

impl Default for InstanceHandle {
    fn default() -> Self {
        Self::NIL
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_handle_nil() {
        let handle = InstanceHandle::NIL;
        assert!(handle.is_nil());
        assert_eq!(handle.0, [0; 16]);
    }

    #[test]
    fn instance_handle_default_is_nil() {
        assert!(InstanceHandle::default().is_nil());
    }

    #[test]
    fn instance_handle_from_short_key() {
        // Key ≤ 16 bytes: used directly, zero-padded
        let key = [1u8, 2, 3, 4];
        let handle = InstanceHandle::from_key_bytes(&key);
        assert!(!handle.is_nil());
        assert_eq!(&handle.0[..4], &[1, 2, 3, 4]);
        assert_eq!(&handle.0[4..], &[0; 12]);
    }

    #[test]
    fn instance_handle_from_exact_16_byte_key() {
        let key = [0xAA; 16];
        let handle = InstanceHandle::from_key_bytes(&key);
        assert_eq!(handle.0, [0xAA; 16]);
    }

    #[test]
    fn instance_handle_from_long_key_uses_sha256() {
        // Key > 16 bytes: hashed with SHA-256, truncated to 16 bytes
        let key = [0xFF; 32];
        let handle = InstanceHandle::from_key_bytes(&key);

        // Verify it's deterministic
        let handle2 = InstanceHandle::from_key_bytes(&key);
        assert_eq!(handle, handle2);

        // Verify it's not just the first 16 bytes of the key
        assert_ne!(handle.0, [0xFF; 16]);
    }

    #[test]
    fn instance_handle_different_keys_produce_different_handles() {
        let h1 = InstanceHandle::from_key_bytes(&[1; 32]);
        let h2 = InstanceHandle::from_key_bytes(&[2; 32]);
        assert_ne!(h1, h2);
    }

    #[test]
    fn instance_handle_empty_key() {
        let handle = InstanceHandle::from_key_bytes(&[]);
        // Empty key produces a nil handle (all zeros)
        assert!(handle.is_nil());
    }

    #[test]
    fn instance_handle_debug_format() {
        let handle = InstanceHandle::new([0x01; 16]);
        let debug = format!("{handle:?}");
        assert!(debug.contains("InstanceHandle("));
        assert!(debug.contains("01010101"));
    }

    #[test]
    fn instance_handle_display_nil() {
        assert_eq!(format!("{}", InstanceHandle::NIL), "NIL");
    }

    #[test]
    fn instance_handle_display_non_nil() {
        let handle = InstanceHandle::new([0xAB; 16]);
        let display = format!("{handle}");
        assert!(!display.contains("NIL"));
        assert!(display.contains("ab"));
    }

    #[test]
    fn instance_handle_ordering() {
        let a = InstanceHandle::new([0; 16]);
        let b = InstanceHandle::new([1; 16]);
        assert!(a < b);
    }
}
