//! # Instance — Instance handles and key hashing for DDS
//!
//! An `Handle` uniquely identifies a data instance within a Topic.
//! It is derived from the key fields of the data type via MD5 or SHA-256
//! hashing, depending on the key size.
//!
//! Reference: DCPS §2.2.1 — `InstanceHandle_t`.

use sha2::{Digest as _, Sha256};
use core::fmt;

/// A 16-byte handle uniquely identifying a data instance within a topic.
///
/// For keyed topics, this is computed by hashing the serialized key fields.
/// For keyless topics, a single "nil" handle is used.
///
/// Reference: DCPS §2.2.1, `XTypes` §7.6.6.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub struct Handle(pub [u8; 16]);

impl Handle {
    /// The nil handle — used for keyless topics or as a sentinel.
    pub const NIL: Self = Self([0; 16]);

    /// Returns the raw byte representation.
    #[must_use]
    #[inline]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        return &self.0
    }

    /// Compute an instance handle from serialized key bytes.
    ///
    /// If the key is ≤ 16 bytes, it is used directly (zero-padded).
    /// If the key is > 16 bytes, SHA-256 is used and truncated to 16 bytes.
    ///
    /// This follows the `XTypes` specification for key hash computation.
    #[must_use]
    #[inline]
    pub fn from_key_bytes(key_bytes: &[u8]) -> Self {
        if key_bytes.len() <= 16 {
            let mut handle = [u8::default(); 16];
            for (dest, &src) in handle.iter_mut().zip(key_bytes.iter()) {
                *dest = src;
            }
            return Self(handle);
        }
        let hash = Sha256::digest(key_bytes);
        let mut handle = [u8::default(); 16];
        for (dest, &src) in handle.iter_mut().zip(hash.iter().take(16)) {
            *dest = src;
        }
        return Self(handle);
    }
    /// Check whether this is the nil/sentinel handle.
    #[must_use]
    #[inline]
    pub fn is_nil(&self) -> bool {
        return self.0 == [0; 16]
    }

    /// Create an instance handle from raw bytes.
    #[must_use]
    #[inline]
    pub const fn new(bytes: [u8; 16]) -> Self {
        return Self(bytes)
    }

}

impl fmt::Debug for Handle {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match write!(f, "Handle(") {
            Ok(value) => value,
            Err(error) => return Err(error),
        }
        for (index, byte) in self.0.iter().enumerate() {
            if index > 0 && index.rem_euclid(4) == 0 {
                match write!(f, ":") {
                    Ok(value) => value,
                    Err(error) => return Err(error),
                }
            }
            match write!(f, "{byte:02x}") {
                Ok(value) => value,
                Err(error) => return Err(error),
            }
        }
        return write!(f, ")");
    }
}

impl fmt::Display for Handle {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_nil() {
            return write!(f, "NIL");
        }
        for byte in &self.0 {
            match write!(f, "{byte:02x}") {
                Ok(value) => value,
                Err(error) => return Err(error),
            }
        }
        return Ok(());
    }
}

impl Default for Handle {
    #[inline]
    fn default() -> Self {
        return Self::NIL
    }
}

pub type InstanceHandle = Handle;

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_handle_nil() {
        let handle = Handle::NIL;
        assert!(handle.is_nil());
        assert_eq!(handle.0, [0; 16]);
    }

    #[test]
    fn instance_handle_default_is_nil() {
        assert!(Handle::default().is_nil());
    }

    #[test]
    fn instance_handle_from_short_key() {
        // Key ≤ 16 bytes: used directly, zero-padded
        let key = [1u8, 2, 3, 4];
        let handle = Handle::from_key_bytes(&key);
        assert!(!handle.is_nil());
        assert_eq!(&handle.0[..4], &[1, 2, 3, 4]);
        assert_eq!(&handle.0[4..], &[0; 12]);
    }

    #[test]
    fn instance_handle_from_exact_16_byte_key() {
        let key = [0xAA; 16];
        let handle = Handle::from_key_bytes(&key);
        assert_eq!(handle.0, [0xAA; 16]);
    }

    #[test]
    fn instance_handle_from_long_key_uses_sha256() {
        // Key > 16 bytes: hashed with SHA-256, truncated to 16 bytes
        let key = [0xFF; 32];
        let handle = Handle::from_key_bytes(&key);

        // Verify it's deterministic
        let handle2 = Handle::from_key_bytes(&key);
        assert_eq!(handle, handle2);

        // Verify it's not just the first 16 bytes of the key
        assert_ne!(handle.0, [0xFF; 16]);
    }

    #[test]
    fn instance_handle_different_keys_produce_different_handles() {
        let h1 = Handle::from_key_bytes(&[1; 32]);
        let h2 = Handle::from_key_bytes(&[2; 32]);
        assert_ne!(h1, h2);
    }

    #[test]
    fn instance_handle_empty_key() {
        let handle = Handle::from_key_bytes(&[]);
        // Empty key produces a nil handle (all zeros)
        assert!(handle.is_nil());
    }

    #[test]
    fn instance_handle_debug_format() {
        let handle = Handle::new([0x01; 16]);
        let debug = format!("{handle:?}");
        assert!(debug.contains("Handle("));
        assert!(debug.contains("01010101"));
    }

    #[test]
    fn instance_handle_display_nil() {
        assert_eq!(format!("{}", Handle::NIL), "NIL");
    }

    #[test]
    fn instance_handle_display_non_nil() {
        let handle = Handle::new([0xAB; 16]);
        let display = format!("{handle}");
        assert!(!display.contains("NIL"));
        assert!(display.contains("ab"));
    }

    #[test]
    fn instance_handle_ordering() {
        let a = Handle::new([0; 16]);
        let b = Handle::new([1; 16]);
        assert!(a < b);
    }
}
