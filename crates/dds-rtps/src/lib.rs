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
use dds_cdr::{CdrDeserialize, CdrSerialize};
use core::net::SocketAddr;
use std::sync::{Arc, Mutex};

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
// RTPS Protocol Constants
// ──────────────────────────────────────────────────────────────────────────────

/// Fixed length of the RTPS Header in bytes.
pub const RTPS_HEADER_SIZE: usize = 20;

/// Fixed length of a submessage header (Kind, Flags, Length) in bytes.
pub const SUBMESSAGE_HEADER_SIZE: usize = 4;

/// Fixed length of the Data submessage fields before inline QoS and payload.
pub const DATA_SUBMESSAGE_FIXED_SIZE: usize = 20;

/// Fixed length of the DataFrag submessage fields before inline QoS and payload.
pub const DATA_FRAG_SUBMESSAGE_FIXED_SIZE: usize = 32;

/// Fixed length of the Gap submessage base fields.
pub const GAP_SUBMESSAGE_FIXED_SIZE: usize = 16;

/// Fixed length of the AckNack submessage base fields.
pub const ACKNACK_SUBMESSAGE_FIXED_SIZE: usize = 24;

/// Fixed length of the InfoTs submessage timestamp field.
pub const INFOTS_TIMESTAMP_SIZE: usize = 8;

/// Submessage flag indicating little-endian byte order.
pub const FLAG_LITTLE_ENDIAN: u8 = 0x01;

/// Submessage flag indicating the presence of Inline QoS.
pub const FLAG_INLINE_QOS: u8 = 0x02;

/// Submessage flag indicating the presence of a serialized Data payload.
pub const FLAG_DATA_PAYLOAD: u8 = 0x04;

/// Submessage flag indicating the presence of a serialized Key payload.
pub const FLAG_KEY_PAYLOAD: u8 = 0x08;

/// Submessage flag for Final flag in AckNack/Heartbeat.
pub const FLAG_FINAL: u8 = 0x02;

/// Submessage flag for Liveliness flag in Heartbeat.
pub const FLAG_LIVELINESS: u8 = 0x04;

/// Submessage flag for Multicast flag in InfoReply.
pub const FLAG_MULTICAST: u8 = 0x02;

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
// InfoSrc Submessage (RTPS §8.3.7.8)
// ──────────────────────────────────────────────────────────────────────────────

/// `InfoSrc` provides source context for following submessages in the same packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfoSrc {
    pub protocol_version: (u8, u8),
    pub vendor_id: VendorId,
    pub guid_prefix: GuidPrefix,
}

// ──────────────────────────────────────────────────────────────────────────────
// InfoDst Submessage (RTPS §8.3.7.7)
// ──────────────────────────────────────────────────────────────────────────────

/// `InfoDst` provides destination context for following submessages in the same packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfoDst {
    pub guid_prefix: GuidPrefix,
}

// ──────────────────────────────────────────────────────────────────────────────
// InfoReply Submessage (RTPS §8.3.7.6)
// ──────────────────────────────────────────────────────────────────────────────

/// `InfoReply` provides reply context for following submessages in the same packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoReply {
    pub unicast_locator_list: Vec<Locator>,
    pub multicast_locator_list: Option<Vec<Locator>>,
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
    /// Optional inline QoS parameters.
    pub inline_qos: Option<dds_cdr::ParameterList>,
    /// Serialized payload data representation.
    pub serialized_payload: Bytes,
}

// ──────────────────────────────────────────────────────────────────────────────
// DataFrag Submessage (RTPS §8.3.7.3)
// ──────────────────────────────────────────────────────────────────────────────

/// `DataFrag` submessage sent by writers when fragmenting large samples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataFrag {
    /// Destination reader ID.
    pub reader_id: EntityId,
    /// Source writer ID.
    pub writer_id: EntityId,
    /// Sequence number of the cache change.
    pub writer_sn: SequenceNumber,
    /// 1-based index of the first fragment in this submessage.
    pub fragment_starting_num: u32,
    /// Number of fragments in this submessage.
    pub fragments_in_submessage: u16,
    /// Size of each fragment in bytes.
    pub fragment_size: u16,
    /// Total size of the unsplit serialized data in bytes.
    pub data_size: u32,
    /// Serialized payload fragment representation.
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
    DataFrag(DataFrag),
    Heartbeat(Heartbeat),
    AckNack(AckNack),
    Gap(Gap),
    InfoSrc(InfoSrc),
    InfoDst(InfoDst),
    InfoReply(InfoReply),
    /// Unrecognized or unimplemented submessages (ignored).
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
            // 0 means it extends to the end of the message (except for PAD/INFO submessages with 0 payload)
            if kind_byte == SubmessageKind::InfoTs as u8 || kind_byte == SubmessageKind::Pad as u8 {
                offset
            } else {
                buf.len()
            }
        } else {
            core::cmp::min(offset + octets_to_next, buf.len())
        };

        let sub_payload = &buf[offset..submessage_end];

        if let Some(kind) = SubmessageKind::from_u8(kind_byte) {
            match kind {
                SubmessageKind::InfoTs => {
                    let has_timestamp = (flags & FLAG_INLINE_QOS) == 0; // flag bit 1 unset => has timestamp
                    if has_timestamp && sub_payload.len() >= INFOTS_TIMESTAMP_SIZE {
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
                SubmessageKind::InfoSrc => {
                    if sub_payload.len() >= RTPS_HEADER_SIZE {
                        let protocol_version = (sub_payload[4], sub_payload[5]);
                        let vendor_id = VendorId::new([sub_payload[6], sub_payload[7]]);
                        let mut guid_prefix = [0u8; 12];
                        guid_prefix.copy_from_slice(&sub_payload[8..20]);
                        submessages.push(Submessage::InfoSrc(InfoSrc {
                            protocol_version,
                            vendor_id,
                            guid_prefix: GuidPrefix::new(guid_prefix),
                        }));
                    }
                }
                SubmessageKind::InfoDst => {
                    if sub_payload.len() >= 12 {
                        let mut guid_prefix = [0u8; 12];
                        guid_prefix.copy_from_slice(&sub_payload[0..12]);
                        submessages.push(Submessage::InfoDst(InfoDst {
                            guid_prefix: GuidPrefix::new(guid_prefix),
                        }));
                    }
                }
                SubmessageKind::InfoReply => {
                    let multicast_flag = (flags & FLAG_MULTICAST) != 0;
                    // For simplicity, we just stub this out as it's complex to parse locator lists without dds-rtps locator list parser
                    // A proper implementation would parse the locator lists.
                    submessages.push(Submessage::InfoReply(InfoReply {
                        unicast_locator_list: vec![],
                        multicast_locator_list: if multicast_flag { Some(vec![]) } else { None },
                    }));
                }
                SubmessageKind::Data => {
                    if sub_payload.len() >= DATA_SUBMESSAGE_FIXED_SIZE {
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

                        let has_inline_qos = (flags & FLAG_INLINE_QOS) != 0;
                        let mut payload_start = DATA_SUBMESSAGE_FIXED_SIZE;
                        let mut inline_qos = None;

                        if has_inline_qos && sub_payload.len() > DATA_SUBMESSAGE_FIXED_SIZE {
                            let endian = if little_endian {
                                dds_cdr::Endianness::LittleEndian
                            } else {
                                dds_cdr::Endianness::BigEndian
                            };
                            let mut deserializer = dds_cdr::CdrDeserializer::new(&sub_payload[20..], endian);
                            if let Ok(qos) = dds_cdr::ParameterList::deserialize(&mut deserializer) {
                                payload_start = 20 + deserializer.offset();
                                inline_qos = Some(qos);
                            }
                        }

                        let payload = if sub_payload.len() > payload_start {
                            Bytes::copy_from_slice(&sub_payload[payload_start..])
                        } else {
                            Bytes::new()
                        };

                        submessages.push(Submessage::Data(Data {
                            reader_id,
                            writer_id,
                            writer_sn,
                            inline_qos,
                            serialized_payload: payload,
                        }));
                    }
                }
                SubmessageKind::DataFrag => {
                    if sub_payload.len() >= 32 {
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

                        let fragment_starting_num = if little_endian {
                            LittleEndian::read_u32(&sub_payload[20..24])
                        } else {
                            BigEndian::read_u32(&sub_payload[20..24])
                        };

                        let fragments_in_submessage = if little_endian {
                            LittleEndian::read_u16(&sub_payload[24..26])
                        } else {
                            BigEndian::read_u16(&sub_payload[24..26])
                        };

                        let fragment_size = if little_endian {
                            LittleEndian::read_u16(&sub_payload[26..28])
                        } else {
                            BigEndian::read_u16(&sub_payload[26..28])
                        };

                        let data_size = if little_endian {
                            LittleEndian::read_u32(&sub_payload[28..32])
                        } else {
                            BigEndian::read_u32(&sub_payload[28..32])
                        };

                        let payload = if sub_payload.len() > 32 {
                            Bytes::copy_from_slice(&sub_payload[32..])
                        } else {
                            Bytes::new()
                        };

                        submessages.push(Submessage::DataFrag(DataFrag {
                            reader_id,
                            writer_id,
                            writer_sn,
                            fragment_starting_num,
                            fragments_in_submessage,
                            fragment_size,
                            data_size,
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

                        let mut set_offset = 8;
                        if let Ok(reader_sn_state) = deserialize_sequence_number_set(
                            &sub_payload,
                            &mut set_offset,
                            little_endian,
                        ) {
                            let count = if sub_payload.len() >= set_offset + 4 {
                                if little_endian {
                                    LittleEndian::read_i32(&sub_payload[set_offset..set_offset + 4])
                                } else {
                                    BigEndian::read_i32(&sub_payload[set_offset..set_offset + 4])
                                }
                            } else {
                                0
                            };

                            submessages.push(Submessage::AckNack(AckNack {
                                reader_id,
                                writer_id,
                                reader_sn_state,
                                count,
                            }));
                        }
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

                        let mut set_offset = 16;
                        let gap_list = deserialize_sequence_number_set(
                            &sub_payload,
                            &mut set_offset,
                            little_endian,
                        )
                        .unwrap_or_default();

                        submessages.push(Submessage::Gap(Gap {
                            reader_id,
                            writer_id,
                            gap_start,
                            gap_list,
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

                let mut qos_buf = BytesMut::new();
                let mut data_flags = flags;
                if let Some(ref qos) = data.inline_qos {
                    data_flags |= 0x02;
                    let cdr_endian = match endian {
                        Endianness::LittleEndian => dds_cdr::Endianness::LittleEndian,
                        Endianness::BigEndian => dds_cdr::Endianness::BigEndian,
                    };
                    let mut serializer = dds_cdr::CdrSerializer::new(cdr_endian);
                    let _ = qos.serialize(&mut serializer);
                    qos_buf.put_slice(&serializer.into_bytes());
                }
                if !data.serialized_payload.is_empty() {
                    data_flags |= 0x04;
                }

                buf.put_u8(data_flags);

                let payload_len = data.serialized_payload.len();
                let qos_len = qos_buf.len();
                let submessage_len = 20 + qos_len + payload_len; // 20 bytes data header + qos + payload

                if is_le {
                    buf.put_u16_le(submessage_len as u16);
                    buf.put_u16_le(0); // extraFlags
                    buf.put_u16_le(20); // octetsToSerializedPayload
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
                if qos_len > 0 {
                    buf.put_slice(&qos_buf);
                }
                buf.put_slice(&data.serialized_payload);
            }
            Submessage::DataFrag(df) => {
                buf.put_u8(SubmessageKind::DataFrag as u8);
                buf.put_u8(flags);

                let payload_len = df.serialized_payload.len();
                let submessage_len = 32 + payload_len;

                if is_le {
                    buf.put_u16_le(submessage_len as u16);
                    buf.put_u16_le(0); // extraFlags
                    buf.put_u16_le(32); // octetsToSerializedPayload
                    buf.put_slice(df.reader_id.as_bytes());
                    buf.put_slice(df.writer_id.as_bytes());
                    let (sn_high, sn_low) = df.writer_sn.to_high_low();
                    buf.put_i32_le(sn_high);
                    buf.put_u32_le(sn_low);
                    buf.put_u32_le(df.fragment_starting_num);
                    buf.put_u16_le(df.fragments_in_submessage);
                    buf.put_u16_le(df.fragment_size);
                    buf.put_u32_le(df.data_size);
                } else {
                    buf.put_u16(submessage_len as u16);
                    buf.put_u16(0);
                    buf.put_u16(32);
                    buf.put_slice(df.reader_id.as_bytes());
                    buf.put_slice(df.writer_id.as_bytes());
                    let (sn_high, sn_low) = df.writer_sn.to_high_low();
                    buf.put_i32(sn_high);
                    buf.put_u32(sn_low);
                    buf.put_u32(df.fragment_starting_num);
                    buf.put_u16(df.fragments_in_submessage);
                    buf.put_u16(df.fragment_size);
                    buf.put_u32(df.data_size);
                }
                buf.put_slice(&df.serialized_payload);
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

                let mut sns_buf = BytesMut::new();
                serialize_sequence_number_set(&mut sns_buf, &ack.reader_sn_state, endian);

                let submessage_len = 4 + 4 + sns_buf.len() + 4; // readerId + writerId + sn_state + count
                if is_le {
                    buf.put_u16_le(submessage_len as u16);
                    buf.put_slice(ack.reader_id.as_bytes());
                    buf.put_slice(ack.writer_id.as_bytes());
                    buf.put_slice(&sns_buf);
                    buf.put_i32_le(ack.count);
                } else {
                    buf.put_u16(submessage_len as u16);
                    buf.put_slice(ack.reader_id.as_bytes());
                    buf.put_slice(ack.writer_id.as_bytes());
                    buf.put_slice(&sns_buf);
                    buf.put_i32(ack.count);
                }
            }
            Submessage::Gap(gap) => {
                buf.put_u8(SubmessageKind::Gap as u8);
                buf.put_u8(flags);

                let mut sns_buf = BytesMut::new();
                serialize_sequence_number_set(&mut sns_buf, &gap.gap_list, endian);

                let submessage_len = 4 + 4 + 8 + sns_buf.len(); // readerId + writerId + gapStart + gapList
                let (gap_high, gap_low) = gap.gap_start.to_high_low();

                if is_le {
                    buf.put_u16_le(submessage_len as u16);
                    buf.put_slice(gap.reader_id.as_bytes());
                    buf.put_slice(gap.writer_id.as_bytes());
                    buf.put_i32_le(gap_high);
                    buf.put_u32_le(gap_low);
                    buf.put_slice(&sns_buf);
                } else {
                    buf.put_u16(submessage_len as u16);
                    buf.put_slice(gap.reader_id.as_bytes());
                    buf.put_slice(gap.writer_id.as_bytes());
                    buf.put_i32(gap_high);
                    buf.put_u32(gap_low);
                    buf.put_slice(&sns_buf);
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

// ──────────────────────────────────────────────────────────────────────────────
// RTPS State Machine Engine Runner (RTPS §8.4)
// ──────────────────────────────────────────────────────────────────────────────



/// A background state machine runner for executing RTPS writer & reader cycles.
///
/// Holds a reference to a `StatefulWriter` and a `UdpTransport`. Each tick of
/// the loop serializes unsent `CacheChange`s into `DATA` submessages and sends
/// them to all matched reader proxies, advancing `next_unsent_sn` as it goes.
pub struct RtpsEngine {
    writer: Arc<Mutex<StatefulWriter>>,
    transport: Arc<UdpTransport>,
    encrypt_fn: Option<Arc<dyn Fn(&[u8], GuidPrefix) -> Option<Vec<u8>> + Send + Sync>>,
}

impl RtpsEngine {
    #[must_use]
    pub fn new(
        writer: Arc<Mutex<StatefulWriter>>,
        transport: Arc<UdpTransport>,
        encrypt_fn: Option<Arc<dyn Fn(&[u8], GuidPrefix) -> Option<Vec<u8>> + Send + Sync>>,
    ) -> Self {
        Self {
            writer,
            transport,
            encrypt_fn,
        }
    }

    /// Spawn background thread to send unsent `CacheChange`s to all reader proxies.
    #[must_use]
    pub fn spawn_run_loop(&self, interval: core::time::Duration) -> std::thread::JoinHandle<()> {
        let writer_clone = self.writer.clone();
        let transport_clone = self.transport.clone();
        let encrypt_fn_clone = self.encrypt_fn.clone();
        std::thread::spawn(move || {
            loop {
                Self::tick(
                    &writer_clone,
                    &transport_clone,
                    &encrypt_fn_clone,
                );
                std::thread::sleep(interval);
            }
        })
    }

    /// Single dispatch tick: send all pending CacheChanges to matched readers.
    ///
    /// For each reader proxy: find CacheChanges with SN >= proxy.next_unsent_sn,
    /// build RTPS messages (Header + InfoTs + Data), send them, then advance SN.
    pub fn tick(
        writer: &Arc<Mutex<StatefulWriter>>,
        transport: &UdpTransport,
        encrypt_fn: &Option<Arc<dyn Fn(&[u8], GuidPrefix) -> Option<Vec<u8>> + Send + Sync>>,
    ) {
        let mut w = writer.lock().unwrap();
        let guid_prefix = w.guid.prefix;

        // Collect indices and unsent changes without borrowing proxies mutably yet
        let proxy_count = w.reader_proxies.len();
        let mut to_send: Vec<(usize, Vec<CacheChange>)> = Vec::new();
        for idx in 0..proxy_count {
            let next_sn = w.reader_proxies[idx].next_unsent_sn;
            let unsent: Vec<CacheChange> = w
                .writer_cache
                .get_changes()
                .iter()
                .filter(|c| c.sequence_number >= next_sn)
                .cloned()
                .collect();
            if !unsent.is_empty() {
                to_send.push((idx, unsent));
            }
        }

        for (idx, changes) in to_send {
            let reader_id = w.reader_proxies[idx].remote_reader_guid.entity_id;
            let writer_id = w.guid.entity_id;
            let locators: Vec<Locator> = w.reader_proxies[idx]
                .unicast_locator_list
                .iter()
                .chain(w.reader_proxies[idx].multicast_locator_list.iter())
                .cloned()
                .collect();

            let header = RtpsHeader::new(guid_prefix);
            let mut max_sn = w.reader_proxies[idx].next_unsent_sn;

            let remote_prefix = w.reader_proxies[idx].remote_reader_guid.prefix;

            let max_payload = 1000;
            for change in &changes {
                let mut raw_bytes = change.data_value.to_vec();

                // Call generic encryption closure if present
                if let Some(ref enc) = encrypt_fn {
                    if let Some(enc_bytes) = enc(&raw_bytes, remote_prefix) {
                        raw_bytes = enc_bytes;
                    }
                }

                let payload_bytes = bytes::Bytes::from(raw_bytes);

                if payload_bytes.len() > max_payload {
                    let total_size = payload_bytes.len();
                    let num_frags = (total_size + max_payload - 1) / max_payload;
                    for f in 0..num_frags {
                        let start = f * max_payload;
                        let end = (start + max_payload).min(total_size);
                        let chunk = payload_bytes.slice(start..end);

                        let df = DataFrag {
                            reader_id,
                            writer_id,
                            writer_sn: change.sequence_number,
                            fragment_starting_num: (f + 1) as u32,
                            fragments_in_submessage: 1,
                            fragment_size: max_payload as u16,
                            data_size: total_size as u32,
                            serialized_payload: chunk,
                        };

                        let subs = [
                            Submessage::InfoTs(InfoTs { timestamp: change.source_timestamp }),
                            Submessage::DataFrag(df),
                        ];
                        let msg = serialize_rtps_message(&header, &subs, Endianness::LittleEndian);
                        for locator in &locators {
                            let _ = transport.send(&msg, locator);
                        }
                    }
                } else {
                    let subs = [
                        Submessage::InfoTs(InfoTs { timestamp: change.source_timestamp }),
                        Submessage::Data(Data {
                            reader_id,
                            writer_id,
                            writer_sn: change.sequence_number,
                            inline_qos: None,
                            serialized_payload: payload_bytes,
                        }),
                    ];
                    let msg = serialize_rtps_message(&header, &subs, Endianness::LittleEndian);
                    for locator in &locators {
                        let _ = transport.send(&msg, locator);
                    }
                }
                if change.sequence_number >= max_sn {
                    max_sn = SequenceNumber(change.sequence_number.0 + 1);
                }
            }
            w.reader_proxies[idx].next_unsent_sn = max_sn;
        }

        // Send Heartbeat to all matched reader proxies to drive reliable transfer state
        let first_sn = w.writer_cache.get_seq_num_min().unwrap_or(SequenceNumber(1));
        let last_sn = w.writer_cache.get_seq_num_max().unwrap_or(SequenceNumber(0));
        w.heartbeat_count += 1;
        let count = w.heartbeat_count;

        for idx in 0..proxy_count {
            let reader_id = w.reader_proxies[idx].remote_reader_guid.entity_id;
            let writer_id = w.guid.entity_id;
            let locators: Vec<Locator> = w.reader_proxies[idx]
                .unicast_locator_list
                .iter()
                .chain(w.reader_proxies[idx].multicast_locator_list.iter())
                .cloned()
                .collect();

            let hb = Heartbeat {
                reader_id,
                writer_id,
                first_sn,
                last_sn,
                count,
            };

            let subs = [Submessage::Heartbeat(hb)];
            let header = RtpsHeader::new(guid_prefix);
            let msg = serialize_rtps_message(&header, &subs, Endianness::LittleEndian);
            for locator in &locators {
                let _ = transport.send(&msg, locator);
            }
        }
    }
}

/// Container that stores the history of `CacheChanges` for a Reader or Writer.
///
/// Reference: RTPS §8.2.2.2
#[derive(Debug)]
pub struct HistoryCache {
    changes: Vec<CacheChange>,
    history_kind: dds_types::qos::HistoryKind,
    history_depth: i32,
    max_samples_per_instance: i32,
}

impl HistoryCache {
    #[must_use]
    pub const fn new(
        history_kind: dds_types::qos::HistoryKind,
        history_depth: i32,
        max_samples_per_instance: i32,
    ) -> Self {
        Self {
            changes: Vec::new(),
            history_kind,
            history_depth,
            max_samples_per_instance,
        }
    }

    pub fn add_change(&mut self, change: CacheChange) {
        // Enforce max_samples_per_instance
        if self.max_samples_per_instance != dds_types::qos::LENGTH_UNLIMITED
            && self.changes.len() >= self.max_samples_per_instance as usize
        {
            self.changes.remove(0);
        }

        // Keep changes ordered by sequence number
        if let Err(idx) = self
            .changes
            .binary_search_by_key(&change.sequence_number, |c| c.sequence_number)
        {
            self.changes.insert(idx, change);
        }

        // Enforce KeepLast
        if matches!(self.history_kind, dds_types::qos::HistoryKind::KeepLast) {
            while self.changes.len() > self.history_depth as usize {
                self.changes.remove(0);
            }
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
    pub heartbeat_count: i32,
}

impl StatefulWriter {
    #[must_use]
    pub const fn new(
        guid: Guid,
        history_kind: dds_types::qos::HistoryKind,
        history_depth: i32,
        max_samples_per_instance: i32,
    ) -> Self {
        Self {
            guid,
            reader_proxies: Vec::new(),
            writer_cache: HistoryCache::new(history_kind, history_depth, max_samples_per_instance),
            last_change_sequence_number: SequenceNumber(0),
            heartbeat_count: 0,
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
    pub fn new(
        guid: Guid,
        history_kind: dds_types::qos::HistoryKind,
        history_depth: i32,
        max_samples_per_instance: i32,
    ) -> Self {
        Self {
            guid,
            writer_cache: HistoryCache::new(history_kind, history_depth, max_samples_per_instance),
            reader_locators: Vec::new(),
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

/// Serialize a SequenceNumberSet to the byte buffer.
pub fn serialize_sequence_number_set(
    buf: &mut BytesMut,
    sns: &[SequenceNumber],
    endian: Endianness,
) {
    let is_le = endian == Endianness::LittleEndian;
    let base_sn = sns.first().copied().unwrap_or(SequenceNumber(1));
    let (base_high, base_low) = base_sn.to_high_low();

    // Serialize base
    if is_le {
        buf.put_i32_le(base_high);
        buf.put_u32_le(base_low);
    } else {
        buf.put_i32(base_high);
        buf.put_u32(base_low);
    }

    if sns.is_empty() {
        if is_le {
            buf.put_u32_le(0); // numBits = 0
        } else {
            buf.put_u32(0);
        }
        return;
    }

    let max_sn = sns.iter().max().copied().unwrap_or(base_sn);
    let num_bits = ((max_sn.0 - base_sn.0 + 1).max(0).min(256)) as u32;

    if is_le {
        buf.put_u32_le(num_bits);
    } else {
        buf.put_u32(num_bits);
    }

    let num_longs = ((num_bits + 31) / 32) as usize;
    let mut bitmap = vec![0_u32; num_longs];

    for sn in sns {
        let diff = sn.0 - base_sn.0;
        if diff >= 0 && diff < i64::from(num_bits) {
            let word_idx = (diff / 32) as usize;
            let bit_idx = 31 - (diff % 32) as u32;
            bitmap[word_idx] |= 1 << bit_idx;
        }
    }

    for word in bitmap {
        if is_le {
            buf.put_u32_le(word);
        } else {
            buf.put_u32(word);
        }
    }
}

/// Deserialize a SequenceNumberSet from a payload slice starting at offset.
/// Returns the list of sequence numbers.
pub fn deserialize_sequence_number_set(
    payload: &[u8],
    offset: &mut usize,
    little_endian: bool,
) -> RtpsResult<Vec<SequenceNumber>> {
    if *offset + 12 > payload.len() {
        return Err(RtpsError::InvalidMessage("SequenceNumberSet too short".into()));
    }

    let base_high = if little_endian {
        LittleEndian::read_i32(&payload[*offset..*offset + 4])
    } else {
        BigEndian::read_i32(&payload[*offset..*offset + 4])
    };
    let base_low = if little_endian {
        LittleEndian::read_u32(&payload[*offset + 4..*offset + 8])
    } else {
        BigEndian::read_u32(&payload[*offset + 4..*offset + 8])
    };
    let base_sn = SequenceNumber::from_high_low(base_high, base_low);

    let num_bits = if little_endian {
        LittleEndian::read_u32(&payload[*offset + 8..*offset + 12])
    } else {
        BigEndian::read_u32(&payload[*offset + 8..*offset + 12])
    };

    *offset += 12;

    let num_longs = ((num_bits + 31) / 32) as usize;
    if *offset + 4 * num_longs > payload.len() {
        return Err(RtpsError::InvalidMessage("SequenceNumberSet bitmap truncated".into()));
    }

    let mut bitmap = vec![0_u32; num_longs];
    for word in &mut bitmap {
        *word = if little_endian {
            LittleEndian::read_u32(&payload[*offset..*offset + 4])
        } else {
            BigEndian::read_u32(&payload[*offset..*offset + 4])
        };
        *offset += 4;
    }

    let mut sns = Vec::new();
    for i in 0..num_bits {
        let word_idx = (i / 32) as usize;
        let bit_idx = 31 - (i % 32);
        if (bitmap[word_idx] & (1 << bit_idx)) != 0 {
            sns.push(SequenceNumber(base_sn.0 + i64::from(i)));
        }
    }

    // Always ensure the base is returned if the set is empty but we want to specify it
    if sns.is_empty() && num_bits == 0 {
        // Just return empty list
    } else if sns.is_empty() {
        // Fallback: return at least base if it was explicitly marked
        sns.push(base_sn);
    }

    Ok(sns)
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
            inline_qos: None,
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
        let mut cache = HistoryCache::new(
            dds_types::qos::HistoryKind::KeepLast,
            10,
            dds_types::qos::LENGTH_UNLIMITED,
        );
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

        // Test with more complex bitmap sequence
        let ack_complex = AckNack {
            reader_id: EntityId::new([1, 0, 0, 4]),
            writer_id: EntityId::new([2, 0, 0, 3]),
            reader_sn_state: vec![SequenceNumber(10), SequenceNumber(12)],
            count: 7,
        };
        let subs_complex = vec![Submessage::AckNack(ack_complex.clone())];
        let msg_complex = serialize_rtps_message(&header, &subs_complex, Endianness::LittleEndian);
        let (_, parsed_subs_complex) = parse_rtps_message(&msg_complex).unwrap();
        assert_eq!(parsed_subs_complex.len(), 1);
        if let Submessage::AckNack(parsed_ack) = &parsed_subs_complex[0] {
            assert_eq!(parsed_ack.reader_sn_state, vec![SequenceNumber(10), SequenceNumber(12)]);
            assert_eq!(parsed_ack.count, 7);
        } else {
            panic!("Expected complex AckNack");
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
            gap_list: vec![SequenceNumber(22), SequenceNumber(25)],
        };

        let subs = vec![Submessage::Gap(gap.clone())];
        let msg = serialize_rtps_message(&header, &subs, Endianness::LittleEndian);
        let (_, parsed_subs) = parse_rtps_message(&msg).unwrap();

        assert_eq!(parsed_subs.len(), 1);
        if let Submessage::Gap(parsed_gap) = &parsed_subs[0] {
            assert_eq!(parsed_gap.reader_id, gap.reader_id);
            assert_eq!(parsed_gap.writer_id, gap.writer_id);
            assert_eq!(parsed_gap.gap_start, gap.gap_start);
            assert_eq!(parsed_gap.gap_list, gap.gap_list);
        } else {
            panic!("Expected Gap submessage");
        }
    }

    #[test]
    fn test_datafrag_submessage_serialization() {
        let prefix = GuidPrefix::new([0x01; 12]);
        let header = RtpsHeader::new(prefix);
        let df = DataFrag {
            reader_id: EntityId::new([1, 0, 0, 4]),
            writer_id: EntityId::new([2, 0, 0, 3]),
            writer_sn: SequenceNumber(100),
            fragment_starting_num: 5,
            fragments_in_submessage: 3,
            fragment_size: 1000,
            data_size: 5000,
            serialized_payload: Bytes::copy_from_slice(&[1, 2, 3, 4, 5]),
        };

        let subs = vec![Submessage::DataFrag(df.clone())];
        let msg = serialize_rtps_message(&header, &subs, Endianness::LittleEndian);
        let (_, parsed_subs) = parse_rtps_message(&msg).unwrap();

        assert_eq!(parsed_subs.len(), 1);
        if let Submessage::DataFrag(parsed_df) = &parsed_subs[0] {
            assert_eq!(parsed_df.reader_id, df.reader_id);
            assert_eq!(parsed_df.writer_id, df.writer_id);
            assert_eq!(parsed_df.writer_sn, df.writer_sn);
            assert_eq!(parsed_df.fragment_starting_num, df.fragment_starting_num);
            assert_eq!(parsed_df.fragments_in_submessage, df.fragments_in_submessage);
            assert_eq!(parsed_df.fragment_size, df.fragment_size);
            assert_eq!(parsed_df.data_size, df.data_size);
            assert_eq!(parsed_df.serialized_payload, df.serialized_payload);
        } else {
            panic!("Expected DataFrag submessage");
        }
    }

    #[test]
    fn test_data_with_inline_qos_serialization() {
        let prefix = GuidPrefix::new([0x01; 12]);
        let header = RtpsHeader::new(prefix);

        // Add parameter list for inline QoS
        let mut parameter_list = dds_cdr::ParameterList::new();
        // Use standard or customized PIDs. Let's use PID_KEY_HASH with some 16 bytes value.
        parameter_list.add(dds_cdr::ParameterId::PID_KEY_HASH, vec![7; 16]);

        let data = Data {
            reader_id: EntityId::new([1, 0, 0, 4]),
            writer_id: EntityId::new([2, 0, 0, 3]),
            writer_sn: SequenceNumber(42),
            inline_qos: Some(parameter_list),
            serialized_payload: Bytes::copy_from_slice(&[99, 100, 101]),
        };

        let subs = vec![Submessage::Data(data.clone())];
        let msg = serialize_rtps_message(&header, &subs, Endianness::LittleEndian);
        let (_, parsed_subs) = parse_rtps_message(&msg).unwrap();

        assert_eq!(parsed_subs.len(), 1);
        if let Submessage::Data(parsed_data) = &parsed_subs[0] {
            assert_eq!(parsed_data.reader_id, data.reader_id);
            assert_eq!(parsed_data.writer_id, data.writer_id);
            assert_eq!(parsed_data.writer_sn, data.writer_sn);
            assert!(parsed_data.inline_qos.is_some());
            let parsed_qos = parsed_data.inline_qos.as_ref().unwrap();
            let key_hash = parsed_qos.parameters.iter()
                .find(|p| p.parameter_id == dds_cdr::ParameterId::PID_KEY_HASH)
                .map(|p| &p.value);
            assert_eq!(key_hash, Some(&vec![7; 16]));
            assert_eq!(parsed_data.serialized_payload, data.serialized_payload);
        } else {
            panic!("Expected Data submessage with inline QoS");
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
        let mut writer = StatefulWriter::new(
            writer_guid,
            dds_types::qos::HistoryKind::KeepLast,
            10,
            dds_types::qos::LENGTH_UNLIMITED,
        );
        writer.last_change_sequence_number = SequenceNumber(5);
        for i in 1..=5 {
            writer.writer_cache.add_change(CacheChange {
                kind: ChangeKind::Alive,
                writer_guid,
                instance_handle: dds_types::instance::InstanceHandle::NIL,
                sequence_number: SequenceNumber(i),
                data_value: Bytes::copy_from_slice(b"sample"),
                source_timestamp: None,
            });
        }

        let proxy = ReaderProxy {
            remote_reader_guid: Guid::new(GuidPrefix::new([2; 12]), EntityId::new([0, 0, 2, 7])),
            unicast_locator_list: vec![],
            multicast_locator_list: vec![],
            next_unsent_sn: SequenceNumber(1),
        };
        writer.matched_reader_add(proxy);

        let shared_writer = Arc::new(Mutex::new(writer));
        let transport = Arc::new(UdpTransport::bind(0).unwrap());
        let engine = RtpsEngine::new(shared_writer.clone(), transport, None);
        let _handle = engine.spawn_run_loop(std::time::Duration::from_millis(10));

        // Sleep a short duration to let thread execute state transitions
        std::thread::sleep(std::time::Duration::from_millis(50));

        let w = shared_writer.lock().unwrap();
        assert!(w.reader_proxies[0].next_unsent_sn.0 > 1);
    }
}
