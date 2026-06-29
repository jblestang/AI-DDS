//! # GUID — Globally Unique Identifiers for RTPS Entities
//!
//! Every RTPS entity (participant, writer, reader) is identified by a GUID
//! composed of a 12-byte `GuidPrefix` (unique per participant) and a 4-byte
//! `EntityId` (unique within the participant).
//!
//! Reference: RTPS §8.2.4 — GUID, GuidPrefix, EntityId

use std::fmt;

// ──────────────────────────────────────────────────────────────────────────────
// GuidPrefix — 12 bytes identifying a DomainParticipant (RTPS §8.2.4.1)
// ──────────────────────────────────────────────────────────────────────────────

/// A 12-byte prefix that uniquely identifies a `DomainParticipant` in the
/// RTPS protocol. Combined with an `EntityId` it forms a full `GUID`.
///
/// The prefix is typically derived from a combination of host IP, process ID,
/// and a random component to ensure global uniqueness.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GuidPrefix(pub [u8; 12]);

impl GuidPrefix {
    /// The unknown/unset prefix — all zeros. Used as a sentinel value.
    pub const UNKNOWN: Self = Self([0; 12]);

    /// Create a new `GuidPrefix` from raw bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 12]) -> Self {
        Self(bytes)
    }

    /// Returns the raw byte representation.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 12] {
        &self.0
    }

    /// Check whether this is the unknown/sentinel prefix.
    #[must_use]
    pub const fn is_unknown(&self) -> bool {
        // Compare each byte since const fn can't use slice comparisons
        let b = &self.0;
        b[0] == 0
            && b[1] == 0
            && b[2] == 0
            && b[3] == 0
            && b[4] == 0
            && b[5] == 0
            && b[6] == 0
            && b[7] == 0
            && b[8] == 0
            && b[9] == 0
            && b[10] == 0
            && b[11] == 0
    }
}

impl Default for GuidPrefix {
    fn default() -> Self {
        Self::UNKNOWN
    }
}

impl fmt::Debug for GuidPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GuidPrefix(")?;
        for (i, byte) in self.0.iter().enumerate() {
            if i > 0 && i % 4 == 0 {
                write!(f, ":")?;
            }
            write!(f, "{byte:02x}")?;
        }
        write!(f, ")")
    }
}

impl fmt::Display for GuidPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, byte) in self.0.iter().enumerate() {
            if i > 0 && i % 4 == 0 {
                write!(f, ":")?;
            }
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// EntityId — 4 bytes identifying an entity within a participant (RTPS §8.2.4.2)
// ──────────────────────────────────────────────────────────────────────────────

/// The "kind" byte of an EntityId, identifying the entity's role.
/// The kind occupies the last byte of the 4-byte EntityId.
///
/// Reference: RTPS §8.2.4.2, Table 8.13
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EntityKind {
    /// Unknown or user-defined entity with unknown kind.
    Unknown = 0x00,
    /// User-defined DataWriter (no key).
    WriterNoKey = 0x03,
    /// User-defined DataWriter (with key).
    WriterWithKey = 0x02,
    /// User-defined DataReader (no key).
    ReaderNoKey = 0x04,
    /// User-defined DataReader (with key).
    ReaderWithKey = 0x07,
    /// Builtin participant entity.
    BuiltinParticipant = 0xc1,
    /// Builtin DataWriter (with key).
    BuiltinWriterWithKey = 0xc2,
    /// Builtin DataWriter (no key).
    BuiltinWriterNoKey = 0xc3,
    /// Builtin DataReader (no key).
    BuiltinReaderNoKey = 0xc4,
    /// Builtin DataReader (with key).
    BuiltinReaderWithKey = 0xc7,
}

impl EntityKind {
    /// Parse an entity kind from a raw byte. Returns `Unknown` for
    /// unrecognized values — this is forward-compatible per the spec.
    #[must_use]
    pub const fn from_byte(byte: u8) -> Self {
        match byte {
            0x00 => Self::Unknown,
            0x03 => Self::WriterNoKey,
            0x02 => Self::WriterWithKey,
            0x04 => Self::ReaderNoKey,
            0x07 => Self::ReaderWithKey,
            0xc1 => Self::BuiltinParticipant,
            0xc2 => Self::BuiltinWriterWithKey,
            0xc3 => Self::BuiltinWriterNoKey,
            0xc4 => Self::BuiltinReaderNoKey,
            0xc7 => Self::BuiltinReaderWithKey,
            _ => Self::Unknown,
        }
    }
}

/// A 4-byte entity identifier within a participant. The first 3 bytes are the
/// entity key, and the last byte is the `EntityKind`.
///
/// Well-known EntityIds are defined by the RTPS spec for builtin endpoints
/// (SPDP writers/readers, SEDP writers/readers, etc.).
///
/// Reference: RTPS §8.2.4.2
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityId(pub [u8; 4]);

impl EntityId {
    /// The unknown entity — all zeros. Used as a sentinel.
    pub const UNKNOWN: Self = Self([0x00, 0x00, 0x00, 0x00]);

    /// The participant itself (not a writer or reader).
    pub const PARTICIPANT: Self = Self([0x00, 0x00, 0x01, 0xc1]);

    // ── Builtin SPDP endpoints ──

    /// SPDP builtin participant writer.
    pub const SPDP_BUILTIN_PARTICIPANT_WRITER: Self = Self([0x00, 0x01, 0x00, 0xc2]);

    /// SPDP builtin participant reader.
    pub const SPDP_BUILTIN_PARTICIPANT_READER: Self = Self([0x00, 0x01, 0x00, 0xc7]);

    // ── Builtin SEDP endpoints ──

    /// SEDP builtin publications (DataWriter announcements) writer.
    pub const SEDP_BUILTIN_PUBLICATIONS_WRITER: Self = Self([0x00, 0x00, 0x03, 0xc2]);

    /// SEDP builtin publications reader.
    pub const SEDP_BUILTIN_PUBLICATIONS_READER: Self = Self([0x00, 0x00, 0x03, 0xc7]);

    /// SEDP builtin subscriptions (DataReader announcements) writer.
    pub const SEDP_BUILTIN_SUBSCRIPTIONS_WRITER: Self = Self([0x00, 0x00, 0x04, 0xc2]);

    /// SEDP builtin subscriptions reader.
    pub const SEDP_BUILTIN_SUBSCRIPTIONS_READER: Self = Self([0x00, 0x00, 0x04, 0xc7]);

    // ── Builtin participant message endpoints (liveliness) ──

    /// Participant message writer (used for liveliness).
    pub const PARTICIPANT_MESSAGE_WRITER: Self = Self([0x00, 0x02, 0x00, 0xc2]);

    /// Participant message reader (used for liveliness).
    pub const PARTICIPANT_MESSAGE_READER: Self = Self([0x00, 0x02, 0x00, 0xc7]);

    // ── Builtin TypeLookup endpoints (XTypes 1.3) ──

    /// TypeLookup request writer.
    pub const BUILTIN_TYPE_LOOKUP_REQUEST_DATA_WRITER: Self = Self([0x00, 0x03, 0x00, 0xc3]);

    /// TypeLookup request reader.
    pub const BUILTIN_TYPE_LOOKUP_REQUEST_DATA_READER: Self = Self([0x00, 0x03, 0x00, 0xc4]);

    /// TypeLookup reply writer.
    pub const BUILTIN_TYPE_LOOKUP_REPLY_DATA_WRITER: Self = Self([0x00, 0x03, 0x01, 0xc3]);

    /// TypeLookup reply reader.
    pub const BUILTIN_TYPE_LOOKUP_REPLY_DATA_READER: Self = Self([0x00, 0x03, 0x01, 0xc4]);

    /// Create a new `EntityId` from raw bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 4]) -> Self {
        Self(bytes)
    }

    /// Returns the raw byte representation.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 4] {
        &self.0
    }

    /// Extract the 3-byte entity key (first 3 bytes).
    #[must_use]
    pub const fn entity_key(&self) -> [u8; 3] {
        [self.0[0], self.0[1], self.0[2]]
    }

    /// Extract the entity kind (last byte).
    #[must_use]
    pub const fn kind(&self) -> EntityKind {
        EntityKind::from_byte(self.0[3])
    }
}

impl fmt::Debug for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EntityId({:02x}{:02x}{:02x}:{:02x})",
            self.0[0], self.0[1], self.0[2], self.0[3]
        )
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02x}{:02x}{:02x}:{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3]
        )
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// GUID — Full globally unique identifier (RTPS §8.2.4)
// ──────────────────────────────────────────────────────────────────────────────

/// A 16-byte Globally Unique Identifier for an RTPS entity.
///
/// Composed of a `GuidPrefix` (12 bytes, unique per participant) and an
/// `EntityId` (4 bytes, unique within the participant).
///
/// Reference: RTPS §8.2.4
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Guid {
    /// The 12-byte prefix identifying the owning participant.
    pub prefix: GuidPrefix,
    /// The 4-byte entity identifier within the participant.
    pub entity_id: EntityId,
}

impl Guid {
    /// The unknown GUID — sentinel value with all zeros.
    pub const UNKNOWN: Self = Self {
        prefix: GuidPrefix::UNKNOWN,
        entity_id: EntityId::UNKNOWN,
    };

    /// Construct a GUID from its components.
    #[must_use]
    pub const fn new(prefix: GuidPrefix, entity_id: EntityId) -> Self {
        Self { prefix, entity_id }
    }

    /// Serialize to a 16-byte array (prefix ++ entity_id).
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[..12].copy_from_slice(&self.prefix.0);
        buf[12..16].copy_from_slice(&self.entity_id.0);
        buf
    }

    /// Deserialize from a 16-byte array.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        let mut prefix = [0u8; 12];
        let mut entity_id = [0u8; 4];
        prefix.copy_from_slice(&bytes[..12]);
        entity_id.copy_from_slice(&bytes[12..16]);
        Self {
            prefix: GuidPrefix(prefix),
            entity_id: EntityId(entity_id),
        }
    }
}

impl fmt::Debug for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Guid({:?}|{:?})", self.prefix, self.entity_id)
    }
}

impl fmt::Display for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}|{}", self.prefix, self.entity_id)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Sequence Number — 64-bit sequence counter (RTPS §8.2.4.4)
// ──────────────────────────────────────────────────────────────────────────────

/// A 64-bit sequence number used to order cache changes within a writer.
///
/// The RTPS spec represents this as a pair of (high: i32, low: u32),
/// but we use a single i64 internally for simplicity. Conversion methods
/// are provided for wire format compatibility.
///
/// Reference: RTPS §8.2.4.4
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SequenceNumber(pub i64);

impl SequenceNumber {
    /// The unknown sequence number — sentinel value.
    pub const UNKNOWN: Self = Self(0);

    /// The first valid sequence number. Writers start at 1.
    pub const MIN: Self = Self(1);

    /// Create from the RTPS wire representation (high, low).
    #[must_use]
    pub const fn from_high_low(high: i32, low: u32) -> Self {
        Self(((high as i64) << 32) | (low as i64))
    }

    /// Convert to the RTPS wire representation (high, low).
    #[must_use]
    pub const fn to_high_low(self) -> (i32, u32) {
        let high = (self.0 >> 32) as i32;
        let low = self.0 as u32;
        (high, low)
    }

    /// Increment the sequence number by one.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

impl fmt::Display for SequenceNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SeqNum({})", self.0)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── GuidPrefix tests ──

    #[test]
    fn guid_prefix_unknown_is_all_zeros() {
        assert!(GuidPrefix::UNKNOWN.is_unknown());
        assert_eq!(GuidPrefix::UNKNOWN.as_bytes(), &[0u8; 12]);
    }

    #[test]
    fn guid_prefix_non_zero_is_not_unknown() {
        let prefix = GuidPrefix::new([1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert!(!prefix.is_unknown());
    }

    #[test]
    fn guid_prefix_debug_format() {
        let prefix = GuidPrefix::new([
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c,
        ]);
        let debug = format!("{prefix:?}");
        assert_eq!(debug, "GuidPrefix(01020304:05060708:090a0b0c)");
    }

    #[test]
    fn guid_prefix_display_format() {
        let prefix = GuidPrefix::new([0xaa; 12]);
        let display = format!("{prefix}");
        assert_eq!(display, "aaaaaaaa:aaaaaaaa:aaaaaaaa");
    }

    #[test]
    fn guid_prefix_equality() {
        let a = GuidPrefix::new([1; 12]);
        let b = GuidPrefix::new([1; 12]);
        let c = GuidPrefix::new([2; 12]);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn guid_prefix_ordering() {
        let a = GuidPrefix::new([0; 12]);
        let b = GuidPrefix::new([1; 12]);
        assert!(a < b);
    }

    // ── EntityId tests ──

    #[test]
    fn entity_id_unknown_is_all_zeros() {
        assert_eq!(EntityId::UNKNOWN.as_bytes(), &[0u8; 4]);
    }

    #[test]
    fn entity_id_key_extraction() {
        let eid = EntityId::new([0x01, 0x02, 0x03, 0xc2]);
        assert_eq!(eid.entity_key(), [0x01, 0x02, 0x03]);
    }

    #[test]
    fn entity_id_kind_extraction() {
        assert_eq!(EntityId::PARTICIPANT.kind(), EntityKind::BuiltinParticipant);
        assert_eq!(
            EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER.kind(),
            EntityKind::BuiltinWriterWithKey
        );
        assert_eq!(
            EntityId::SPDP_BUILTIN_PARTICIPANT_READER.kind(),
            EntityKind::BuiltinReaderWithKey
        );
    }

    #[test]
    fn entity_id_sedp_kinds() {
        assert_eq!(
            EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER.kind(),
            EntityKind::BuiltinWriterWithKey
        );
        assert_eq!(
            EntityId::SEDP_BUILTIN_PUBLICATIONS_READER.kind(),
            EntityKind::BuiltinReaderWithKey
        );
        assert_eq!(
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER.kind(),
            EntityKind::BuiltinWriterWithKey
        );
        assert_eq!(
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER.kind(),
            EntityKind::BuiltinReaderWithKey
        );
    }

    #[test]
    fn entity_id_debug_format() {
        let eid = EntityId::new([0x00, 0x01, 0x00, 0xc2]);
        assert_eq!(format!("{eid:?}"), "EntityId(000100:c2)");
    }

    // ── EntityKind tests ──

    #[test]
    fn entity_kind_from_known_bytes() {
        assert_eq!(EntityKind::from_byte(0x00), EntityKind::Unknown);
        assert_eq!(EntityKind::from_byte(0x02), EntityKind::WriterWithKey);
        assert_eq!(EntityKind::from_byte(0x03), EntityKind::WriterNoKey);
        assert_eq!(EntityKind::from_byte(0x04), EntityKind::ReaderNoKey);
        assert_eq!(EntityKind::from_byte(0x07), EntityKind::ReaderWithKey);
        assert_eq!(EntityKind::from_byte(0xc1), EntityKind::BuiltinParticipant);
        assert_eq!(
            EntityKind::from_byte(0xc2),
            EntityKind::BuiltinWriterWithKey
        );
        assert_eq!(EntityKind::from_byte(0xc3), EntityKind::BuiltinWriterNoKey);
        assert_eq!(EntityKind::from_byte(0xc4), EntityKind::BuiltinReaderNoKey);
        assert_eq!(
            EntityKind::from_byte(0xc7),
            EntityKind::BuiltinReaderWithKey
        );
    }

    #[test]
    fn entity_kind_unknown_for_unrecognized() {
        // Forward-compatibility: unknown byte values map to Unknown
        assert_eq!(EntityKind::from_byte(0xFF), EntityKind::Unknown);
        assert_eq!(EntityKind::from_byte(0x01), EntityKind::Unknown);
        assert_eq!(EntityKind::from_byte(0x99), EntityKind::Unknown);
    }

    // ── GUID tests ──

    #[test]
    fn guid_unknown_is_all_zeros() {
        let guid = Guid::UNKNOWN;
        assert_eq!(guid.to_bytes(), [0u8; 16]);
    }

    #[test]
    fn guid_round_trip_bytes() {
        let original = Guid::new(
            GuidPrefix::new([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]),
            EntityId::new([0x00, 0x01, 0x00, 0xc2]),
        );
        let bytes = original.to_bytes();
        let reconstructed = Guid::from_bytes(bytes);
        assert_eq!(original, reconstructed);
    }

    #[test]
    fn guid_bytes_layout() {
        // Verify that to_bytes produces prefix ++ entity_id
        let guid = Guid::new(GuidPrefix::new([0xAA; 12]), EntityId::new([0xBB; 4]));
        let bytes = guid.to_bytes();
        assert!(bytes[..12].iter().all(|&b| b == 0xAA));
        assert!(bytes[12..16].iter().all(|&b| b == 0xBB));
    }

    #[test]
    fn guid_display_format() {
        let guid = Guid::new(GuidPrefix::new([0x01; 12]), EntityId::new([0x02; 4]));
        let display = format!("{guid}");
        assert!(display.contains('|'));
    }

    // ── SequenceNumber tests ──

    #[test]
    fn sequence_number_min_starts_at_one() {
        assert_eq!(SequenceNumber::MIN.0, 1);
    }

    #[test]
    fn sequence_number_unknown_is_zero() {
        assert_eq!(SequenceNumber::UNKNOWN.0, 0);
    }

    #[test]
    fn sequence_number_next_increments() {
        let sn = SequenceNumber(42);
        assert_eq!(sn.next(), SequenceNumber(43));
    }

    #[test]
    fn sequence_number_high_low_round_trip() {
        // Test a value that uses both high and low parts
        let sn = SequenceNumber::from_high_low(1, 100);
        let (high, low) = sn.to_high_low();
        assert_eq!(high, 1);
        assert_eq!(low, 100);
        assert_eq!(sn.0, (1i64 << 32) | 100);
    }

    #[test]
    fn sequence_number_ordering() {
        let a = SequenceNumber(1);
        let b = SequenceNumber(2);
        assert!(a < b);
    }

    #[test]
    fn sequence_number_high_low_zero() {
        let sn = SequenceNumber::from_high_low(0, 0);
        assert_eq!(sn, SequenceNumber(0));
        let (h, l) = sn.to_high_low();
        assert_eq!(h, 0);
        assert_eq!(l, 0);
    }
}
