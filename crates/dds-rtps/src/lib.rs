//! # dds-rtps — DDSI-RTPS 2.5 Wire Protocol Engine
//!
//! Implements the Real-Time Publish-Subscribe wire protocol for DDS
//! interoperability: message parsing, submessages, writer/reader state
//! machines, and UDP transport.
//!
//! Reference: DDSI-RTPS §8

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
    clippy::large_stack_arrays,
    reason = "RTPS Wire Protocol implementation requires standard library conversions, standard returns, and UDP socket structures."
)]

use byteorder::{BigEndian, ByteOrder as _, LittleEndian};
use bytes::{BufMut as _, Bytes, BytesMut};
use dds_types::guid::{EntityId, Guid, GuidPrefix, SequenceNumber};
use dds_types::locator::Locator;
use dds_types::time::Timestamp;
use dds_types::vendor::VendorId;
use core::net::SocketAddr;

/// Unified RTPS engine error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RtpsError {
    /// Serialization/deserialization failed.
    #[error("serialization/deserialization failed: {0}")]
    SerializationError(String),

    /// Invalid message structure or headers.
    #[error("invalid RTPS message: {0}")]
    InvalidMessage(String),

    /// IO or Network transport error.
    #[error("transport error: {0}")]
    TransportError(String),
}

/// Helper Result type.
pub type RtpsResult<T> = Result<T, RtpsError>;

// ──────────────────────────────────────────────────────────────────────────────
// SubmessageKind definitions (RTPS §8.3.7)
// ──────────────────────────────────────────────────────────────────────────────

/// Enumeration identifying each submessage type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SubmessageKind {
    Pad = 0x01,
    AckNack = 0x06,
    Heartbeat = 0x07,
    Gap = 0x08,
    InfoTs = 0x09,
    InfoSrc = 0x0c,
    InfoReply = 0x0f,
    InfoDst = 0x0e,
    Data = 0x15,
    DataFrag = 0x16,
    NackFrag = 0x12,
    HeartbeatFrag = 0x13,
}

impl SubmessageKind {
    /// Parse from raw byte.
    #[must_use]
    pub const fn from_u8(val: u8) -> Option<Self> {
        match val {
            0x01 => Some(Self::Pad),
            0x06 => Some(Self::AckNack),
            0x07 => Some(Self::Heartbeat),
            0x08 => Some(Self::Gap),
            0x09 => Some(Self::InfoTs),
            0x0c => Some(Self::InfoSrc),
            0x0f => Some(Self::InfoReply),
            0x0e => Some(Self::InfoDst),
            0x15 => Some(Self::Data),
            0x16 => Some(Self::DataFrag),
            0x12 => Some(Self::NackFrag),
            0x13 => Some(Self::HeartbeatFrag),
            _ => None,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// RTPS Header (RTPS §8.3.3)
// ──────────────────────────────────────────────────────────────────────────────

/// Fixed RTPS Message Header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtpsHeader {
    /// Protocol version. Standard is (2, 5) for RTPS 2.5.
    pub version: (u8, u8),
    /// Identifies vendor implementation.
    pub vendor_id: VendorId,
    /// Uniquely identifies the participant source of this message.
    pub guid_prefix: GuidPrefix,
}

impl RtpsHeader {
    /// Create a standard header for this implementation.
    #[must_use]
    pub const fn new(guid_prefix: GuidPrefix) -> Self {
        Self {
            version: (2, 5),
            vendor_id: VendorId::THIS_IMPLEMENTATION,
            guid_prefix,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Submessage Header (RTPS §8.3.5)
// ──────────────────────────────────────────────────────────────────────────────

/// Shared header present at the start of every RTPS submessage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubmessageHeader {
    /// Identifies type of submessage.
    pub kind: SubmessageKind,
    /// Submessage flags (endianness, content options).
    pub flags: u8,
    /// Length of the submessage payload following this header.
    pub octets_to_next_header: u16,
}

// ──────────────────────────────────────────────────────────────────────────────
// InfoTs Submessage (RTPS §8.3.7.9)
// ──────────────────────────────────────────────────────────────────────────────

/// `InfoTs` provides timestamp context for following submessages in the same packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfoTs {
    /// Absolute timestamp, or None if the timestamp is invalid/omitted.
    pub timestamp: Option<Timestamp>,
}

// ──────────────────────────────────────────────────────────────────────────────
// Data Submessage (RTPS §8.3.7.2)
// ──────────────────────────────────────────────────────────────────────────────

/// Data submessage used to publish topic sample modifications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Data {
    /// ID of the reader entity that should receive this data (or `EntityId::UNKNOWN`).
    pub reader_id: EntityId,
    /// ID of the writer entity that generated this data.
    pub writer_id: EntityId,
    /// Sequence number of the cache change.
    pub writer_sn: SequenceNumber,
    /// Serialized payload data representation.
    pub serialized_payload: Bytes,
}

// ──────────────────────────────────────────────────────────────────────────────
// Heartbeat Submessage (RTPS §8.3.7.5)
// ──────────────────────────────────────────────────────────────────────────────

/// Heartbeat submessage sent by writers to notify readers of available sequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Heartbeat {
    /// Destination reader ID.
    pub reader_id: EntityId,
    /// Source writer ID.
    pub writer_id: EntityId,
    /// First sequence number available in the writer cache.
    pub first_sn: SequenceNumber,
    /// Last sequence number available in the writer cache.
    pub last_sn: SequenceNumber,
    /// Identifies the state of the writer.
    pub count: i32,
}

// ──────────────────────────────────────────────────────────────────────────────
// AckNack Submessage (RTPS §8.3.7.1)
// ──────────────────────────────────────────────────────────────────────────────

/// `AckNack` submessage sent by readers to acknowledge received data or request missing ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckNack {
    /// Destination writer ID.
    pub writer_id: EntityId,
    /// Source reader ID.
    pub reader_id: EntityId,
    /// Set of sequence numbers the reader has not received yet.
    pub reader_sn_state: Vec<SequenceNumber>,
    /// Identifies state of the reader.
    pub count: i32,
}

// ──────────────────────────────────────────────────────────────────────────────
// Gap Submessage (RTPS §8.3.7.4)
// ──────────────────────────────────────────────────────────────────────────────

/// Gap submessage sent by writers to tell readers that specific sequences are no longer relevant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gap {
    /// Destination reader ID.
    pub reader_id: EntityId,
    /// Source writer ID.
    pub writer_id: EntityId,
    /// First sequence number in the gap.
    pub gap_start: SequenceNumber,
    /// List of sequence numbers in the gap.
    pub gap_list: Vec<SequenceNumber>,
}

// ──────────────────────────────────────────────────────────────────────────────
// Representation of a full RTPS Submessage
// ──────────────────────────────────────────────────────────────────────────────

/// Unified container for any parsed RTPS submessage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Submessage {
    InfoTs(InfoTs),
    Data(Data),
    Heartbeat(Heartbeat),
    AckNack(AckNack),
    Gap(Gap),
    /// Placeholder for unimplemented submessages.
    Unsupported(SubmessageKind),
}

// ──────────────────────────────────────────────────────────────────────────────
// RTPS Message parser and formatter
// ──────────────────────────────────────────────────────────────────────────────

/// Parses a byte buffer into an RTPS Header and a list of Submessages.
pub fn parse_rtps_message(buf: &[u8]) -> RtpsResult<(RtpsHeader, Vec<Submessage>)> {
    if buf.len() < 20 {
        return Err(RtpsError::InvalidMessage(
            "message too short to contain header".into(),
        ));
    }

    // Check protocol ID prefix "RTPS"
    if &buf[0..4] != b"RTPS" {
        return Err(RtpsError::InvalidMessage(
            "invalid protocol ID (expected 'RTPS')".into(),
        ));
    }

    let version_major = buf[4];
    let version_minor = buf[5];
    let vendor_id = VendorId::new([buf[6], buf[7]]);
    let mut prefix_bytes = [0_u8; 12];
    prefix_bytes.copy_from_slice(&buf[8..20]);
    let guid_prefix = GuidPrefix::new(prefix_bytes);

    let header = RtpsHeader {
        version: (version_major, version_minor),
        vendor_id,
        guid_prefix,
    };

    let mut submessages = Vec::new();
    let mut offset = 20;

    while offset < buf.len() {
        if offset + 4 > buf.len() {
            break; // Truncated submessage header, ignore or error
        }

        let kind_byte = buf[offset];
        let flags = buf[offset + 1];
        let little_endian = (flags & 0x01) != 0;

        let len_slice = &buf[offset + 2..offset + 4];
        let octets_to_next = if little_endian {
            LittleEndian::read_u16(len_slice)
        } else {
            BigEndian::read_u16(len_slice)
        } as usize;

        offset += 4; // move past header

        let submessage_end = if octets_to_next == 0 {
            // 0 means it extends to the end of the message (except for PAD/INFO)
            buf.len()
        } else {
            core::cmp::min(offset + octets_to_next, buf.len())
        };

        let sub_payload = &buf[offset..submessage_end];

        if let Some(kind) = SubmessageKind::from_u8(kind_byte) {
            match kind {
                SubmessageKind::InfoTs => {
                    let has_timestamp = (flags & 0x02) == 0; // flag bit 1 unset => has timestamp
                    if has_timestamp && sub_payload.len() >= 8 {
                        let sec = if little_endian {
                            LittleEndian::read_u32(&sub_payload[0..4])
                        } else {
                            BigEndian::read_u32(&sub_payload[0..4])
                        };
                        let fraction = if little_endian {
                            LittleEndian::read_u32(&sub_payload[4..8])
                        } else {
                            BigEndian::read_u32(&sub_payload[4..8])
                        };
                        // Convert fraction to nanoseconds
                        let nano = ((u64::from(fraction) * 1_000_000_000) >> 32) as u32;
                        submessages.push(Submessage::InfoTs(InfoTs {
                            timestamp: Some(Timestamp::new(sec, nano)),
                        }));
                    } else {
                        submessages.push(Submessage::InfoTs(InfoTs { timestamp: None }));
                    }
                }
                SubmessageKind::Data => {
                    if sub_payload.len() >= 20 {
                        // Data submessage headers are aligned/padded. Read readerId, writerId, writerSN
                        // In RTPS §8.3.7.2.2:
                        // - extraFlags: 2 octets
                        // - octetsToSerializedPayload: 2 octets
                        // - readerId: EntityId (4 octets) -> offset starts at 4
                        // - writerId: EntityId (4 octets) -> offset starts at 8
                        // - writerSN: SequenceNumber (8 octets) -> offset starts at 12
                        let mut reader_bytes = [0_u8; 4];
                        reader_bytes.copy_from_slice(&sub_payload[4..8]);
                        let reader_id = EntityId::new(reader_bytes);

                        let mut writer_bytes = [0_u8; 4];
                        writer_bytes.copy_from_slice(&sub_payload[8..12]);
                        let writer_id = EntityId::new(writer_bytes);

                        let sn_high = if little_endian {
                            LittleEndian::read_i32(&sub_payload[12..16])
                        } else {
                            BigEndian::read_i32(&sub_payload[12..16])
                        };
                        let sn_low = if little_endian {
                            LittleEndian::read_u32(&sub_payload[16..20])
                        } else {
                            BigEndian::read_u32(&sub_payload[16..20])
                        };
                        let writer_sn = SequenceNumber::from_high_low(sn_high, sn_low);

                        // Omit inline QoS for now. Actual payload is at the end.
                        let payload_start = 20; // simplified offset for base DATA submessage
                        let payload = if sub_payload.len() > payload_start {
                            Bytes::copy_from_slice(&sub_payload[payload_start..])
                        } else {
                            Bytes::new()
                        };

                        submessages.push(Submessage::Data(Data {
                            reader_id,
                            writer_id,
                            writer_sn,
                            serialized_payload: payload,
                        }));
                    }
                }
                SubmessageKind::Heartbeat => {
                    if sub_payload.len() >= 28 {
                        let mut reader_bytes = [0_u8; 4];
                        reader_bytes.copy_from_slice(&sub_payload[0..4]);
                        let reader_id = EntityId::new(reader_bytes);

                        let mut writer_bytes = [0_u8; 4];
                        writer_bytes.copy_from_slice(&sub_payload[4..8]);
                        let writer_id = EntityId::new(writer_bytes);

                        let fsn_high = if little_endian {
                            LittleEndian::read_i32(&sub_payload[8..12])
                        } else {
                            BigEndian::read_i32(&sub_payload[8..12])
                        };
                        let fsn_low = if little_endian {
                            LittleEndian::read_u32(&sub_payload[12..16])
                        } else {
                            BigEndian::read_u32(&sub_payload[12..16])
                        };
                        let first_sn = SequenceNumber::from_high_low(fsn_high, fsn_low);

                        let lsn_high = if little_endian {
                            LittleEndian::read_i32(&sub_payload[16..20])
                        } else {
                            BigEndian::read_i32(&sub_payload[16..20])
                        };
                        let lsn_low = if little_endian {
                            LittleEndian::read_u32(&sub_payload[20..24])
                        } else {
                            BigEndian::read_u32(&sub_payload[20..24])
                        };
                        let last_sn = SequenceNumber::from_high_low(lsn_high, lsn_low);

                        let count = if little_endian {
                            LittleEndian::read_i32(&sub_payload[24..28])
                        } else {
                            BigEndian::read_i32(&sub_payload[24..28])
                        };

                        submessages.push(Submessage::Heartbeat(Heartbeat {
                            reader_id,
                            writer_id,
                            first_sn,
                            last_sn,
                            count,
                        }));
                    }
                }
                SubmessageKind::AckNack => {
                    if sub_payload.len() >= 16 {
                        let mut reader_bytes = [0_u8; 4];
                        reader_bytes.copy_from_slice(&sub_payload[0..4]);
                        let reader_id = EntityId::new(reader_bytes);

                        let mut writer_bytes = [0_u8; 4];
                        writer_bytes.copy_from_slice(&sub_payload[4..8]);
                        let writer_id = EntityId::new(writer_bytes);

                        // Read bitmap base
                        let base_high = if little_endian {
                            LittleEndian::read_i32(&sub_payload[8..12])
                        } else {
                            BigEndian::read_i32(&sub_payload[8..12])
                        };
                        let base_low = if little_endian {
                            LittleEndian::read_u32(&sub_payload[12..16])
                        } else {
                            BigEndian::read_u32(&sub_payload[12..16])
                        };
                        let base_sn = SequenceNumber::from_high_low(base_high, base_low);

                        // Read count (simplifying: optional bitmap elements skipped for base payload parse)
                        let count = if sub_payload.len() >= 20 {
                            if little_endian {
                                LittleEndian::read_i32(&sub_payload[16..20])
                            } else {
                                BigEndian::read_i32(&sub_payload[16..20])
                            }
                        } else {
                            0
                        };

                        submessages.push(Submessage::AckNack(AckNack {
                            reader_id,
                            writer_id,
                            reader_sn_state: vec![base_sn],
                            count,
                        }));
                    }
                }
                SubmessageKind::Gap => {
                    if sub_payload.len() >= 16 {
                        let mut reader_bytes = [0_u8; 4];
                        reader_bytes.copy_from_slice(&sub_payload[0..4]);
                        let reader_id = EntityId::new(reader_bytes);

                        let mut writer_bytes = [0_u8; 4];
                        writer_bytes.copy_from_slice(&sub_payload[4..8]);
                        let writer_id = EntityId::new(writer_bytes);

                        let gap_high = if little_endian {
                            LittleEndian::read_i32(&sub_payload[8..12])
                        } else {
                            BigEndian::read_i32(&sub_payload[8..12])
                        };
                        let gap_low = if little_endian {
                            LittleEndian::read_u32(&sub_payload[12..16])
                        } else {
                            BigEndian::read_u32(&sub_payload[12..16])
                        };
                        let gap_start = SequenceNumber::from_high_low(gap_high, gap_low);

                        submessages.push(Submessage::Gap(Gap {
                            reader_id,
                            writer_id,
                            gap_start,
                            gap_list: Vec::new(),
                        }));
                    }
                }
                _ => {
                    submessages.push(Submessage::Unsupported(kind));
                }
            }
        }

        offset = submessage_end;
    }

    Ok((header, submessages))
}

/// Serializes an RTPS message (Header + Submessages) to a byte vector.
pub fn serialize_rtps_message(
    header: &RtpsHeader,
    submessages: &[Submessage],
    endian: Endianness,
) -> Bytes {
    let mut buf = BytesMut::new();

    // 1. Write Header
    buf.put_slice(b"RTPS");
    buf.put_u8(header.version.0);
    buf.put_u8(header.version.1);
    buf.put_slice(&header.vendor_id.0);
    buf.put_slice(header.guid_prefix.as_bytes());

    // 2. Write Submessages
    for sub in submessages {
        let is_le = endian == Endianness::LittleEndian;
        let flags = u8::from(is_le);

        match sub {
            Submessage::InfoTs(info) => {
                buf.put_u8(SubmessageKind::InfoTs as u8);
                if let Some(ts) = info.timestamp {
                    buf.put_u8(flags); // bit 1 is 0 => has timestamp
                    if is_le {
                        buf.put_u16_le(8); // payload length
                        buf.put_u32_le(ts.seconds);
                        let fraction = (u64::from(ts.nanoseconds) << 32) / 1_000_000_000;
                        buf.put_u32_le(fraction as u32);
                    } else {
                        buf.put_u16(8);
                        buf.put_u32(ts.seconds);
                        let fraction = (u64::from(ts.nanoseconds) << 32) / 1_000_000_000;
                        buf.put_u32(fraction as u32);
                    }
                } else {
                    buf.put_u8(flags | 0x02); // bit 1 is 1 => no timestamp
                    if is_le {
                        buf.put_u16_le(0);
                    } else {
                        buf.put_u16(0);
                    }
                }
            }
            Submessage::Data(data) => {
                buf.put_u8(SubmessageKind::Data as u8);
                buf.put_u8(flags);

                let payload_len = data.serialized_payload.len();
                let submessage_len = 20 + payload_len; // 20 bytes data header + payload

                if is_le {
                    buf.put_u16_le(submessage_len as u16);
                    buf.put_u16_le(0); // extraFlags
                    buf.put_u16_le(20); // octetsToSerializedPayload (offset to payload from after header flags)
                    buf.put_slice(data.reader_id.as_bytes());
                    buf.put_slice(data.writer_id.as_bytes());
                    let (high, low) = data.writer_sn.to_high_low();
                    buf.put_i32_le(high);
                    buf.put_u32_le(low);
                } else {
                    buf.put_u16(submessage_len as u16);
                    buf.put_u16(0);
                    buf.put_u16(20);
                    buf.put_slice(data.reader_id.as_bytes());
                    buf.put_slice(data.writer_id.as_bytes());
                    let (high, low) = data.writer_sn.to_high_low();
                    buf.put_i32(high);
                    buf.put_u32(low);
                }
                buf.put_slice(&data.serialized_payload);
            }
            Submessage::Heartbeat(hb) => {
                buf.put_u8(SubmessageKind::Heartbeat as u8);
                buf.put_u8(flags);

                if is_le {
                    buf.put_u16_le(28); // submessage size
                    buf.put_slice(hb.reader_id.as_bytes());
                    buf.put_slice(hb.writer_id.as_bytes());
                    let (f_high, f_low) = hb.first_sn.to_high_low();
                    buf.put_i32_le(f_high);
                    buf.put_u32_le(f_low);
                    let (l_high, l_low) = hb.last_sn.to_high_low();
                    buf.put_i32_le(l_high);
                    buf.put_u32_le(l_low);
                    buf.put_i32_le(hb.count);
                } else {
                    buf.put_u16(28);
                    buf.put_slice(hb.reader_id.as_bytes());
                    buf.put_slice(hb.writer_id.as_bytes());
                    let (f_high, f_low) = hb.first_sn.to_high_low();
                    buf.put_i32(f_high);
                    buf.put_u32(f_low);
                    let (l_high, l_low) = hb.last_sn.to_high_low();
                    buf.put_i32(l_high);
                    buf.put_u32(l_low);
                    buf.put_i32(hb.count);
                }
            }
            Submessage::AckNack(ack) => {
                buf.put_u8(SubmessageKind::AckNack as u8);
                buf.put_u8(flags);

                // For base serialization, assume reader_sn_state has at least 1 element (the base sequence number)
                let base_sn = ack
                    .reader_sn_state
                    .first()
                    .copied()
                    .unwrap_or(SequenceNumber(1));
                let (base_high, base_low) = base_sn.to_high_low();

                if is_le {
                    buf.put_u16_le(20); // 4 + 4 + 8 + 4 = 20 B
                    buf.put_slice(ack.reader_id.as_bytes());
                    buf.put_slice(ack.writer_id.as_bytes());
                    buf.put_i32_le(base_high);
                    buf.put_u32_le(base_low);
                    buf.put_i32_le(ack.count);
                } else {
                    buf.put_u16(20);
                    buf.put_slice(ack.reader_id.as_bytes());
                    buf.put_slice(ack.writer_id.as_bytes());
                    buf.put_i32(base_high);
                    buf.put_u32(base_low);
                    buf.put_i32(ack.count);
                }
            }
            Submessage::Gap(gap) => {
                buf.put_u8(SubmessageKind::Gap as u8);
                buf.put_u8(flags);

                let (gap_high, gap_low) = gap.gap_start.to_high_low();

                if is_le {
                    buf.put_u16_le(16); // 4 + 4 + 8 = 16 B
                    buf.put_slice(gap.reader_id.as_bytes());
                    buf.put_slice(gap.writer_id.as_bytes());
                    buf.put_i32_le(gap_high);
                    buf.put_u32_le(gap_low);
                } else {
                    buf.put_u16(16);
                    buf.put_slice(gap.reader_id.as_bytes());
                    buf.put_slice(gap.writer_id.as_bytes());
                    buf.put_i32(gap_high);
                    buf.put_u32(gap_low);
                }
            }
            _ => {}
        }
    }

    buf.freeze()
}

// ──────────────────────────────────────────────────────────────────────────────
// History Cache and Cache Changes (RTPS §8.2.2)
// ──────────────────────────────────────────────────────────────────────────────

/// Identifies the category of a change made to an instance in the `HistoryCache`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Alive,
    NotAliveDisposed,
    NotAliveUnregistered,
}

/// Represents an individual change stored in a `HistoryCache`.
///
/// Reference: RTPS §8.2.2.1
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheChange {
    pub kind: ChangeKind,
    pub writer_guid: Guid,
    pub instance_handle: dds_types::instance::InstanceHandle,
    pub sequence_number: SequenceNumber,
    pub data_value: Bytes,
    pub source_timestamp: Option<Timestamp>,
}

/// Container that stores the history of `CacheChanges` for a Reader or Writer.
///
/// Reference: RTPS §8.2.2.2
#[derive(Debug, Default)]
pub struct HistoryCache {
    changes: Vec<CacheChange>,
}

impl HistoryCache {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            changes: Vec::new(),
        }
    }

    pub fn add_change(&mut self, change: CacheChange) {
        // Keep changes ordered by sequence number
        if let Err(idx) = self
            .changes
            .binary_search_by_key(&change.sequence_number, |c| c.sequence_number)
        {
            self.changes.insert(idx, change);
        }
    }

    pub fn remove_change(&mut self, sequence_number: SequenceNumber) {
        self.changes
            .retain(|c| c.sequence_number != sequence_number);
    }

    #[must_use]
    pub fn get_changes(&self) -> &[CacheChange] {
        &self.changes
    }

    #[must_use]
    pub fn get_seq_num_min(&self) -> Option<SequenceNumber> {
        self.changes.first().map(|c| c.sequence_number)
    }

    #[must_use]
    pub fn get_seq_num_max(&self) -> Option<SequenceNumber> {
        self.changes.last().map(|c| c.sequence_number)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Stateless and Stateful RTPS Writers (RTPS §8.4.7 / §8.4.8)
// ──────────────────────────────────────────────────────────────────────────────

/// Representation of a remote Reader used by a `StatefulWriter`.
#[derive(Debug, Clone)]
pub struct ReaderProxy {
    pub remote_reader_guid: Guid,
    pub unicast_locator_list: Vec<Locator>,
    pub multicast_locator_list: Vec<Locator>,
    pub next_unsent_sn: SequenceNumber,
}

/// Stateful RTPS Writer that tracks the state of matched remote readers.
#[derive(Debug)]
pub struct StatefulWriter {
    pub guid: Guid,
    pub reader_proxies: Vec<ReaderProxy>,
    pub writer_cache: HistoryCache,
    pub last_change_sequence_number: SequenceNumber,
}

impl StatefulWriter {
    #[must_use]
    pub const fn new(guid: Guid) -> Self {
        Self {
            guid,
            reader_proxies: Vec::new(),
            writer_cache: HistoryCache::new(),
            last_change_sequence_number: SequenceNumber(0),
        }
    }

    pub fn matched_reader_add(&mut self, proxy: ReaderProxy) {
        self.reader_proxies.push(proxy);
    }

    pub fn matched_reader_remove(&mut self, reader_guid: &Guid) {
        self.reader_proxies
            .retain(|p| &p.remote_reader_guid != reader_guid);
    }
}

/// Stateless RTPS Writer that sends data to all matched readers without tracking state.
#[derive(Debug)]
pub struct StatelessWriter {
    pub guid: Guid,
    pub reader_locators: Vec<Locator>,
    pub writer_cache: HistoryCache,
    pub last_change_sequence_number: SequenceNumber,
}

impl StatelessWriter {
    #[must_use]
    pub const fn new(guid: Guid) -> Self {
        Self {
            guid,
            reader_locators: Vec::new(),
            writer_cache: HistoryCache::new(),
            last_change_sequence_number: SequenceNumber(0),
        }
    }

    pub fn reader_locator_add(&mut self, locator: Locator) {
        if !self.reader_locators.contains(&locator) {
            self.reader_locators.push(locator);
        }
    }

    pub fn reader_locator_remove(&mut self, locator: &Locator) {
        self.reader_locators.retain(|l| l != locator);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// RTPS State Machine Engine Runner (RTPS §8.4)
// ──────────────────────────────────────────────────────────────────────────────

use std::sync::{Arc, Mutex};

/// A background state machine runner for executing RTPS writer & reader cycles.
pub struct RtpsEngine {
    writer: Arc<Mutex<StatefulWriter>>,
}

impl RtpsEngine {
    #[must_use]
    pub const fn new(writer: Arc<Mutex<StatefulWriter>>) -> Self {
        Self { writer }
    }

    /// Spawn background thread to run state checks periodically.
    #[must_use] 
    pub fn spawn_run_loop(&self, interval: core::time::Duration) -> std::thread::JoinHandle<()> {
        let writer_clone = self.writer.clone();
        std::thread::spawn(move || {
            loop {
                {
                    let mut writer = writer_clone.lock().unwrap();
                    let last_sn = writer.last_change_sequence_number;
                    // State machine behavior: increment unsent sequence numbers towards max SN
                    for proxy in &mut writer.reader_proxies {
                        if proxy.next_unsent_sn <= last_sn {
                            proxy.next_unsent_sn = SequenceNumber(proxy.next_unsent_sn.0 + 1);
                        }
                    }
                }
                std::thread::sleep(interval);
            }
        })
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// UDP Network Transport Driver (RTPS §8.2.6)
// ──────────────────────────────────────────────────────────────────────────────

use std::net::UdpSocket;

/// Driver for sending and receiving RTPS messages over UDP (unicast & multicast).
pub struct UdpTransport {
    socket: UdpSocket,
}

impl UdpTransport {
    /// Bind to a local port for unicast and multicast reception.
    pub fn bind(port: u16) -> RtpsResult<Self> {
        let addr = format!("0.0.0.0:{port}");
        let socket = UdpSocket::bind(&addr)
            .map_err(|e| RtpsError::TransportError(format!("bind failed: {e}")))?;

        // Configure socket to allow multicast loopback and non-blocking reads
        socket.set_multicast_loop_v4(true).map_err(|e| {
            RtpsError::TransportError(format!("set multicast loopback failed: {e}"))
        })?;
        socket
            .set_nonblocking(true)
            .map_err(|e| RtpsError::TransportError(format!("set nonblocking failed: {e}")))?;

        Ok(Self { socket })
    }

    /// Joint multicast group.
    pub fn join_multicast_group(&self, multicast_addr: core::net::Ipv4Addr) -> RtpsResult<()> {
        let any_interface = core::net::Ipv4Addr::UNSPECIFIED;
        self.socket
            .join_multicast_v4(&multicast_addr, &any_interface)
            .map_err(|e| RtpsError::TransportError(format!("join multicast failed: {e}")))?;
        Ok(())
    }

    /// Send serialized bytes to a remote Locator.
    pub fn send(&self, bytes: &[u8], destination: &Locator) -> RtpsResult<()> {
        if let Some(socket_addr) = destination.to_socket_addr() {
            self.socket
                .send_to(bytes, socket_addr)
                .map_err(|e| RtpsError::TransportError(format!("send_to failed: {e}")))?;
        }
        Ok(())
    }

    /// Try to receive a packet from the network.
    ///
    /// Returns Ok(None) if no data is currently available (non-blocking).
    pub fn recv(&self) -> RtpsResult<Option<(Vec<u8>, SocketAddr)>> {
        let mut buf = [0_u8; 65535];
        match self.socket.recv_from(&mut buf) {
            Ok((len, addr)) => {
                let bytes = buf[0..len].to_vec();
                Ok(Some((bytes, addr)))
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(RtpsError::TransportError(format!("recv_from failed: {e}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endianness {
    BigEndian,
    LittleEndian,
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rtps_header_serialization() {
        let prefix = GuidPrefix::new([0x01; 12]);
        let header = RtpsHeader::new(prefix);
        let msg = serialize_rtps_message(&header, &[], Endianness::LittleEndian);

        let (parsed_header, parsed_subs) = parse_rtps_message(&msg).unwrap();
        assert_eq!(parsed_header, header);
        assert!(parsed_subs.is_empty());
    }

    #[test]
    fn test_infots_serialization() {
        let prefix = GuidPrefix::new([0x01; 12]);
        let header = RtpsHeader::new(prefix);
        let ts = Timestamp::new(100, 500_000_000);
        let subs = vec![Submessage::InfoTs(InfoTs {
            timestamp: Some(ts),
        })];

        let msg = serialize_rtps_message(&header, &subs, Endianness::LittleEndian);
        let (_, parsed_subs) = parse_rtps_message(&msg).unwrap();

        assert_eq!(parsed_subs.len(), 1);
        if let Submessage::InfoTs(info) = &parsed_subs[0] {
            assert_eq!(info.timestamp, Some(ts));
        } else {
            panic!("Expected InfoTs submessage");
        }
    }

    #[test]
    fn test_data_submessage_serialization() {
        let prefix = GuidPrefix::new([0x01; 12]);
        let header = RtpsHeader::new(prefix);
        let payload = Bytes::copy_from_slice(&[10, 20, 30]);

        let data = Data {
            reader_id: EntityId::new([1, 0, 0, 4]),
            writer_id: EntityId::new([2, 0, 0, 3]),
            writer_sn: SequenceNumber(15),
            serialized_payload: payload.clone(),
        };

        let subs = vec![Submessage::Data(data.clone())];
        let msg = serialize_rtps_message(&header, &subs, Endianness::LittleEndian);
        let (_, parsed_subs) = parse_rtps_message(&msg).unwrap();

        assert_eq!(parsed_subs.len(), 1);
        if let Submessage::Data(parsed_data) = &parsed_subs[0] {
            assert_eq!(parsed_data.reader_id, data.reader_id);
            assert_eq!(parsed_data.writer_id, data.writer_id);
            assert_eq!(parsed_data.writer_sn, data.writer_sn);
            assert_eq!(parsed_data.serialized_payload, data.serialized_payload);
        } else {
            panic!("Expected Data submessage");
        }
    }

    #[test]
    fn test_history_cache() {
        let mut cache = HistoryCache::new();
        let writer_guid = Guid::new(GuidPrefix::new([1; 12]), EntityId::UNKNOWN);
        let instance = dds_types::instance::InstanceHandle::NIL;

        let change1 = CacheChange {
            kind: ChangeKind::Alive,
            writer_guid,
            instance_handle: instance,
            sequence_number: SequenceNumber(1),
            data_value: Bytes::copy_from_slice(b"one"),
            source_timestamp: None,
        };

        let change2 = CacheChange {
            kind: ChangeKind::Alive,
            writer_guid,
            instance_handle: instance,
            sequence_number: SequenceNumber(2),
            data_value: Bytes::copy_from_slice(b"two"),
            source_timestamp: None,
        };

        cache.add_change(change2.clone());
        cache.add_change(change1.clone()); // Test insertion ordering

        assert_eq!(cache.get_seq_num_min(), Some(SequenceNumber(1)));
        assert_eq!(cache.get_seq_num_max(), Some(SequenceNumber(2)));
        assert_eq!(cache.get_changes().len(), 2);

        cache.remove_change(SequenceNumber(1));
        assert_eq!(cache.get_seq_num_min(), Some(SequenceNumber(2)));
        assert_eq!(cache.get_changes().len(), 1);
    }

    #[test]
    fn test_acknack_submessage_serialization() {
        let prefix = GuidPrefix::new([0x01; 12]);
        let header = RtpsHeader::new(prefix);
        let ack = AckNack {
            reader_id: EntityId::new([1, 0, 0, 4]),
            writer_id: EntityId::new([2, 0, 0, 3]),
            reader_sn_state: vec![SequenceNumber(10)],
            count: 5,
        };

        let subs = vec![Submessage::AckNack(ack.clone())];
        let msg = serialize_rtps_message(&header, &subs, Endianness::LittleEndian);
        let (_, parsed_subs) = parse_rtps_message(&msg).unwrap();

        assert_eq!(parsed_subs.len(), 1);
        if let Submessage::AckNack(parsed_ack) = &parsed_subs[0] {
            assert_eq!(parsed_ack.reader_id, ack.reader_id);
            assert_eq!(parsed_ack.writer_id, ack.writer_id);
            assert_eq!(parsed_ack.reader_sn_state, ack.reader_sn_state);
            assert_eq!(parsed_ack.count, ack.count);
        } else {
            panic!("Expected AckNack submessage");
        }
    }

    #[test]
    fn test_gap_submessage_serialization() {
        let prefix = GuidPrefix::new([0x01; 12]);
        let header = RtpsHeader::new(prefix);
        let gap = Gap {
            reader_id: EntityId::new([1, 0, 0, 4]),
            writer_id: EntityId::new([2, 0, 0, 3]),
            gap_start: SequenceNumber(20),
            gap_list: Vec::new(),
        };

        let subs = vec![Submessage::Gap(gap.clone())];
        let msg = serialize_rtps_message(&header, &subs, Endianness::LittleEndian);
        let (_, parsed_subs) = parse_rtps_message(&msg).unwrap();

        assert_eq!(parsed_subs.len(), 1);
        if let Submessage::Gap(parsed_gap) = &parsed_subs[0] {
            assert_eq!(parsed_gap.reader_id, gap.reader_id);
            assert_eq!(parsed_gap.writer_id, gap.writer_id);
            assert_eq!(parsed_gap.gap_start, gap.gap_start);
        } else {
            panic!("Expected Gap submessage");
        }
    }

    #[test]
    fn test_udp_transport_loopback() {
        // Bind sender on an OS-assigned port, receiver on another OS-assigned port
        let receiver = UdpTransport::bind(0).unwrap();
        let receiver_port = receiver.socket.local_addr().unwrap().port();

        let sender = UdpTransport::bind(0).unwrap();
        let dest = Locator::udpv4(std::net::Ipv4Addr::new(127, 0, 0, 1), receiver_port as u32);

        let data_to_send = b"RTPS_TEST_PAYLOAD";
        sender.send(data_to_send, &dest).unwrap();

        // Allow OS to process socket buffer loopback
        std::thread::sleep(std::time::Duration::from_millis(50));

        let received = receiver.recv().unwrap();
        assert!(received.is_some());
        let (bytes, _) = received.unwrap();
        assert_eq!(bytes, data_to_send);
    }

    #[test]
    fn test_rtps_engine_loop() {
        let writer_guid = Guid::new(GuidPrefix::new([1; 12]), EntityId::new([0, 0, 1, 2]));
        let mut writer = StatefulWriter::new(writer_guid);
        writer.last_change_sequence_number = SequenceNumber(5);

        let proxy = ReaderProxy {
            remote_reader_guid: Guid::new(GuidPrefix::new([2; 12]), EntityId::new([0, 0, 2, 7])),
            unicast_locator_list: vec![],
            multicast_locator_list: vec![],
            next_unsent_sn: SequenceNumber(1),
        };
        writer.matched_reader_add(proxy);

        let shared_writer = Arc::new(Mutex::new(writer));
        let engine = RtpsEngine::new(shared_writer.clone());
        let _handle = engine.spawn_run_loop(std::time::Duration::from_millis(10));

        // Sleep a short duration to let thread execute state transitions
        std::thread::sleep(std::time::Duration::from_millis(50));

        let w = shared_writer.lock().unwrap();
        assert!(w.reader_proxies[0].next_unsent_sn.0 > 1);
    }
}
