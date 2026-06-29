//! # dds-cdr — CDR / XCDR / XCDR2 Serialization
//!
//! Implements the Common Data Representation (CDR) and its extended variants
//! (XCDR1, XCDR2) for serializing DDS data types to/from wire format.
//!
//! Reference: RTPS §10, `XTypes` §7.4.3

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
#![allow(elided_lifetimes_in_paths)]
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
    reason = "CDR Serializer implementation requires standard library conversions, standard returns, and binary layout manipulation."
)]

use byteorder::{BigEndian, ByteOrder as _, LittleEndian};
use bytes::{BufMut as _, Bytes, BytesMut};

/// Representation of CDR serialization/deserialization errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CdrError {
    /// Unexpected end of input buffer.
    #[error("unexpected end of input buffer: expected {expected} bytes, but only {found} remain")]
    RemainingBytesMismatch { expected: usize, found: usize },

    /// Serialization/deserialization encountered invalid UTF-8 string encoding.
    #[error("invalid string encoding: {0}")]
    InvalidString(String),

    /// Deserialization encountered an invalid enum value.
    #[error("invalid enum value: {0}")]
    InvalidEnumValue(i32),

    /// An error occurred during type encapsulation header parsing.
    #[error("invalid encapsulation header: {0}")]
    InvalidHeader(String),

    /// Parameter List format error.
    #[error("parameter list error: {0}")]
    ParameterListError(String),

    /// Sequence/Array length mismatch or out of bounds.
    #[error("length mismatch: {0}")]
    LengthMismatch(String),
}

/// Helper Result type for the CDR crate.
pub type CdrResult<T> = Result<T, CdrError>;

/// Endianness format to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endianness {
    /// Big Endian byte ordering.
    BigEndian,
    /// Little Endian byte ordering.
    LittleEndian,
}

/// The serialization representation format (encapsulation scheme).
///
/// Reference: RTPS §10.2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncapsulationKind {
    /// Plain CDR Big Endian.
    CdrBe = 0x0000,
    /// Plain CDR Little Endian.
    CdrLe = 0x0001,
    /// Parameter List CDR Big Endian.
    PlCdrBe = 0x0002,
    /// Parameter List CDR Little Endian.
    PlCdrLe = 0x0003,
    /// Extended CDR2 Big Endian.
    DxtCdr2Be = 0x0010,
    /// Extended CDR2 Little Endian.
    DxtCdr2Le = 0x0011,
    /// Extended Parameter List CDR2 Big Endian.
    DxtPlCdr2Be = 0x0012,
    /// Extended Parameter List CDR2 Little Endian.
    DxtPlCdr2Le = 0x0013,
}

impl EncapsulationKind {
    /// Get the endianness associated with this encapsulation kind.
    #[must_use]
    pub const fn endianness(&self) -> Endianness {
        match self {
            Self::CdrBe | Self::PlCdrBe | Self::DxtCdr2Be | Self::DxtPlCdr2Be => {
                Endianness::BigEndian
            }
            Self::CdrLe | Self::PlCdrLe | Self::DxtCdr2Le | Self::DxtPlCdr2Le => {
                Endianness::LittleEndian
            }
        }
    }

    /// Check if this encapsulation uses Parameter List representation.
    #[must_use]
    pub const fn is_parameter_list(&self) -> bool {
        match self {
            Self::PlCdrBe | Self::PlCdrLe | Self::DxtPlCdr2Be | Self::DxtPlCdr2Le => true,
            _ => false,
        }
    }
}

/// Standard trait for serializing types into CDR wire representation.
pub trait CdrSerialize {
    /// Serialize this value into the given serializer.
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()>;
}

/// Standard trait for deserializing types from CDR wire representation.
pub trait CdrDeserialize: Sized {
    /// Deserialize an instance from the given deserializer.
    fn deserialize<'a>(deserializer: &mut CdrDeserializer<'a>) -> CdrResult<Self>;
}


// ──────────────────────────────────────────────────────────────────────────────
// CdrSerializer
// ──────────────────────────────────────────────────────────────────────────────

/// Serializer for encoding data structures into CDR binary format.
pub struct CdrSerializer {
    buf: BytesMut,
    endianness: Endianness,
}

impl CdrSerializer {
    /// Create a new serializer with the specified endianness.
    #[must_use]
    pub fn new(endianness: Endianness) -> Self {
        Self {
            buf: BytesMut::new(),
            endianness,
        }
    }

    /// Get a reference to the underlying serialized bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Consume the serializer and return the serialized bytes.
    #[must_use]
    pub fn into_bytes(self) -> Bytes {
        self.buf.freeze()
    }

    /// Write padding bytes to satisfy the alignment requirements.
    ///
    /// The CDR protocol requires primitive types to be aligned to their size.
    pub fn align(&mut self, alignment: usize) {
        let offset = self.buf.len();
        let rem = offset % alignment;
        if rem != 0 {
            let padding = alignment - rem;
            self.buf.put_bytes(0, padding);
        }
    }

    /// Retrieve the current buffer position (offset).
    #[must_use]
    pub fn position(&self) -> usize {
        self.buf.len()
    }

    /// Overwrite a u32 at a specific offset. Used for XCDR2 DHEADER (delimiter header).
    pub fn write_u32_at(&mut self, offset: usize, val: u32) {
        let slice = &mut self.buf[offset..offset + 4];
        match self.endianness {
            Endianness::BigEndian => BigEndian::write_u32(slice, val),
            Endianness::LittleEndian => LittleEndian::write_u32(slice, val),
        }
    }

    /// Serialize a single octet (u8/i8).
    pub fn serialize_u8(&mut self, val: u8) {
        self.buf.put_u8(val);
    }

    /// Serialize a signed octet.
    pub fn serialize_i8(&mut self, val: i8) {
        self.buf.put_i8(val);
    }

    /// Serialize a bool.
    pub fn serialize_bool(&mut self, val: bool) {
        self.serialize_u8(u8::from(val));
    }

    /// Serialize a 16-bit unsigned integer.
    pub fn serialize_u16(&mut self, val: u16) {
        self.align(2);
        match self.endianness {
            Endianness::BigEndian => self.buf.put_u16(val),
            Endianness::LittleEndian => self.buf.put_u16_le(val),
        }
    }

    /// Serialize a 16-bit signed integer.
    pub fn serialize_i16(&mut self, val: i16) {
        self.align(2);
        match self.endianness {
            Endianness::BigEndian => self.buf.put_i16(val),
            Endianness::LittleEndian => self.buf.put_i16_le(val),
        }
    }

    /// Serialize a 32-bit unsigned integer.
    pub fn serialize_u32(&mut self, val: u32) {
        self.align(4);
        match self.endianness {
            Endianness::BigEndian => self.buf.put_u32(val),
            Endianness::LittleEndian => self.buf.put_u32_le(val),
        }
    }

    /// Serialize a 32-bit signed integer.
    pub fn serialize_i32(&mut self, val: i32) {
        self.align(4);
        match self.endianness {
            Endianness::BigEndian => self.buf.put_i32(val),
            Endianness::LittleEndian => self.buf.put_i32_le(val),
        }
    }

    /// Serialize a 64-bit unsigned integer.
    pub fn serialize_u64(&mut self, val: u64) {
        self.align(8);
        match self.endianness {
            Endianness::BigEndian => self.buf.put_u64(val),
            Endianness::LittleEndian => self.buf.put_u64_le(val),
        }
    }

    /// Serialize a 64-bit signed integer.
    pub fn serialize_i64(&mut self, val: i64) {
        self.align(8);
        match self.endianness {
            Endianness::BigEndian => self.buf.put_i64(val),
            Endianness::LittleEndian => self.buf.put_i64_le(val),
        }
    }

    /// Serialize a 32-bit float.
    pub fn serialize_f32(&mut self, val: f32) {
        self.align(4);
        match self.endianness {
            Endianness::BigEndian => self.buf.put_f32(val),
            Endianness::LittleEndian => self.buf.put_f32_le(val),
        }
    }

    /// Serialize a 64-bit float.
    pub fn serialize_f64(&mut self, val: f64) {
        self.align(8);
        match self.endianness {
            Endianness::BigEndian => self.buf.put_f64(val),
            Endianness::LittleEndian => self.buf.put_f64_le(val),
        }
    }

    /// Serialize a string. CDR string format: 32-bit length (including NULL terminator) followed by bytes + NULL.
    pub fn serialize_str(&mut self, val: &str) {
        let len = val.len() as u32 + 1; // +1 for null terminator
        self.serialize_u32(len);
        self.buf.put_slice(val.as_bytes());
        self.buf.put_u8(0); // NULL terminator
    }

    /// XCDR2: Write a Delimiting Header (DHEADER) with an initial zero value. Returns the offset to patch later.
    pub fn write_dheader_placeholder(&mut self) -> usize {
        self.align(4);
        let offset = self.position();
        self.serialize_u32(0); // Initial length value (patched later)
        offset
    }

    /// XCDR2: Patch a previously written DHEADER with the actual length.
    pub fn patch_dheader(&mut self, offset: usize) {
        let length = (self.position() - offset - 4) as u32;
        self.write_u32_at(offset, length);
    }

    /// XCDR2: Write an Extended Member Header (EMHEADER).
    /// Format: [1 bit (MustUnderstand) | 1 bit (Reserved) | 14 bits (Length) | 16 bits (MemberId)]
    /// Or a larger version if length > 7. We'll use the short version for simplicity (assuming len < 65536).
    pub fn serialize_emheader(&mut self, member_id: u32, length: u32) {
        self.align(4);
        // Short EMHEADER:
        // [ 0 | 0 | Length (14 bits) | MemberId (16 bits) ]
        // We'll write it as a u32
        let header = ((length & 0x3FFF) << 16) | (member_id & 0xFFFF);
        self.serialize_u32(header);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// CdrDeserializer
// ──────────────────────────────────────────────────────────────────────────────

/// Deserializer for parsing CDR binary data.
pub struct CdrDeserializer<'a> {
    buf: &'a [u8],
    offset: usize,
    endianness: Endianness,
}

impl<'a> CdrDeserializer<'a> {
    /// Create a new deserializer over a slice of bytes.
    #[must_use]
    pub const fn new(buf: &'a [u8], endianness: Endianness) -> Self {
        Self {
            buf,
            offset: 0,
            endianness,
        }
    }

    /// Get the current offset in the buffer.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Get the remaining unread bytes in the buffer.
    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.buf.len() - self.offset
    }

    /// Align the read offset to the specified boundary.
    pub const fn align(&mut self, alignment: usize) -> CdrResult<()> {
        let rem = self.offset % alignment;
        if rem != 0 {
            let padding = alignment - rem;
            if self.offset + padding > self.buf.len() {
                return Err(CdrError::RemainingBytesMismatch {
                    expected: padding,
                    found: self.remaining(),
                });
            }
            self.offset += padding;
        }
        Ok(())
    }

    /// Read a single raw byte.
    pub fn deserialize_u8(&mut self) -> CdrResult<u8> {
        if self.offset + 1 > self.buf.len() {
            return Err(CdrError::RemainingBytesMismatch {
                expected: 1,
                found: self.remaining(),
            });
        }
        let val = self.buf[self.offset];
        self.offset += 1;
        Ok(val)
    }

    /// Read a signed byte.
    pub fn deserialize_i8(&mut self) -> CdrResult<i8> {
        self.deserialize_u8().map(|v| v as i8)
    }

    /// Read a bool.
    pub fn deserialize_bool(&mut self) -> CdrResult<bool> {
        self.deserialize_u8().map(|v| v != 0)
    }

    /// Read a 16-bit unsigned integer.
    pub fn deserialize_u16(&mut self) -> CdrResult<u16> {
        self.align(2)?;
        if self.offset + 2 > self.buf.len() {
            return Err(CdrError::RemainingBytesMismatch {
                expected: 2,
                found: self.remaining(),
            });
        }
        let slice = &self.buf[self.offset..self.offset + 2];
        let val = match self.endianness {
            Endianness::BigEndian => BigEndian::read_u16(slice),
            Endianness::LittleEndian => LittleEndian::read_u16(slice),
        };
        self.offset += 2;
        Ok(val)
    }

    /// Read a 16-bit signed integer.
    pub fn deserialize_i16(&mut self) -> CdrResult<i16> {
        self.align(2)?;
        if self.offset + 2 > self.buf.len() {
            return Err(CdrError::RemainingBytesMismatch {
                expected: 2,
                found: self.remaining(),
            });
        }
        let slice = &self.buf[self.offset..self.offset + 2];
        let val = match self.endianness {
            Endianness::BigEndian => BigEndian::read_i16(slice),
            Endianness::LittleEndian => LittleEndian::read_i16(slice),
        };
        self.offset += 2;
        Ok(val)
    }

    /// Read a 32-bit unsigned integer.
    pub fn deserialize_u32(&mut self) -> CdrResult<u32> {
        self.align(4)?;
        if self.offset + 4 > self.buf.len() {
            return Err(CdrError::RemainingBytesMismatch {
                expected: 4,
                found: self.remaining(),
            });
        }
        let slice = &self.buf[self.offset..self.offset + 4];
        let val = match self.endianness {
            Endianness::BigEndian => BigEndian::read_u32(slice),
            Endianness::LittleEndian => LittleEndian::read_u32(slice),
        };
        self.offset += 4;
        Ok(val)
    }

    /// Read a 32-bit signed integer.
    pub fn deserialize_i32(&mut self) -> CdrResult<i32> {
        self.align(4)?;
        if self.offset + 4 > self.buf.len() {
            return Err(CdrError::RemainingBytesMismatch {
                expected: 4,
                found: self.remaining(),
            });
        }
        let slice = &self.buf[self.offset..self.offset + 4];
        let val = match self.endianness {
            Endianness::BigEndian => BigEndian::read_i32(slice),
            Endianness::LittleEndian => LittleEndian::read_i32(slice),
        };
        self.offset += 4;
        Ok(val)
    }

    /// Read a 64-bit unsigned integer.
    pub fn deserialize_u64(&mut self) -> CdrResult<u64> {
        self.align(8)?;
        if self.offset + 8 > self.buf.len() {
            return Err(CdrError::RemainingBytesMismatch {
                expected: 8,
                found: self.remaining(),
            });
        }
        let slice = &self.buf[self.offset..self.offset + 8];
        let val = match self.endianness {
            Endianness::BigEndian => BigEndian::read_u64(slice),
            Endianness::LittleEndian => LittleEndian::read_u64(slice),
        };
        self.offset += 8;
        Ok(val)
    }

    /// Read a 64-bit signed integer.
    pub fn deserialize_i64(&mut self) -> CdrResult<i64> {
        self.align(8)?;
        if self.offset + 8 > self.buf.len() {
            return Err(CdrError::RemainingBytesMismatch {
                expected: 8,
                found: self.remaining(),
            });
        }
        let slice = &self.buf[self.offset..self.offset + 8];
        let val = match self.endianness {
            Endianness::BigEndian => BigEndian::read_i64(slice),
            Endianness::LittleEndian => LittleEndian::read_i64(slice),
        };
        self.offset += 8;
        Ok(val)
    }

    /// Read a f32.
    pub fn deserialize_f32(&mut self) -> CdrResult<f32> {
        self.align(4)?;
        if self.offset + 4 > self.buf.len() {
            return Err(CdrError::RemainingBytesMismatch {
                expected: 4,
                found: self.remaining(),
            });
        }
        let slice = &self.buf[self.offset..self.offset + 4];
        let val = match self.endianness {
            Endianness::BigEndian => BigEndian::read_f32(slice),
            Endianness::LittleEndian => LittleEndian::read_f32(slice),
        };
        self.offset += 4;
        Ok(val)
    }

    /// Read a f64.
    pub fn deserialize_f64(&mut self) -> CdrResult<f64> {
        self.align(8)?;
        if self.offset + 8 > self.buf.len() {
            return Err(CdrError::RemainingBytesMismatch {
                expected: 8,
                found: self.remaining(),
            });
        }
        let slice = &self.buf[self.offset..self.offset + 8];
        let val = match self.endianness {
            Endianness::BigEndian => BigEndian::read_f64(slice),
            Endianness::LittleEndian => LittleEndian::read_f64(slice),
        };
        self.offset += 8;
        Ok(val)
    }

    /// Read a string.
    pub fn deserialize_str(&mut self) -> CdrResult<String> {
        let len = self.deserialize_u32()? as usize;
        if len == 0 {
            return Err(CdrError::InvalidString("Null string has length 0".into()));
        }
        if self.offset + len > self.buf.len() {
            return Err(CdrError::RemainingBytesMismatch {
                expected: len,
                found: self.remaining(),
            });
        }
        let str_bytes = &self.buf[self.offset..self.offset + len - 1]; // omit null terminator
        let s = String::from_utf8(str_bytes.to_vec())
            .map_err(|e| CdrError::InvalidString(e.to_string()))?;
        self.offset += len;
        Ok(s)
    }

    /// Read raw slice from the buffer.
    pub fn read_slice(&mut self, len: usize) -> CdrResult<&'a [u8]> {
        if self.offset + len > self.buf.len() {
            return Err(CdrError::RemainingBytesMismatch {
                expected: len,
                found: self.remaining(),
            });
        }
        let slice = &self.buf[self.offset..self.offset + len];
        self.offset += len;
        Ok(slice)
    }

    /// Skip a specified number of bytes.
    pub const fn skip(&mut self, len: usize) -> CdrResult<()> {
        if self.offset + len > self.buf.len() {
            return Err(CdrError::RemainingBytesMismatch {
                expected: len,
                found: self.remaining(),
            });
        }
        self.offset += len;
        Ok(())
    }

    /// XCDR2: Read a Delimiting Header (DHEADER)
    pub fn deserialize_dheader(&mut self) -> CdrResult<u32> {
        self.deserialize_u32()
    }

    /// XCDR2: Read an Extended Member Header (EMHEADER)
    /// Returns (member_id, length)
    pub fn deserialize_emheader(&mut self) -> CdrResult<(u32, u32)> {
        let header = self.deserialize_u32()?;
        let length = (header >> 16) & 0x3FFF;
        let member_id = header & 0xFFFF;
        Ok((member_id, length))
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Encapsulation Header parsing/writing
// ──────────────────────────────────────────────────────────────────────────────

/// Represents type encapsulation header (typically prepended to data payloads).
///
/// Reference: RTPS §10.2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncapsulationHeader {
    /// The encoding format identifier.
    pub kind: EncapsulationKind,
    /// Serialization options (typically zero, unless representing options flags).
    pub options: [u8; 2],
}

impl EncapsulationHeader {
    /// Create a standard plain CDR encapsulation header.
    #[must_use]
    pub const fn new(kind: EncapsulationKind) -> Self {
        Self {
            kind,
            options: [0, 0],
        }
    }

    /// Write this encapsulation header to the serializer.
    pub fn serialize(&self, serializer: &mut CdrSerializer) {
        let val = self.kind as u16;
        // The encapsulation kind is always serialized in the endianness that matches the header itself,
        // or as specified by the standard. But we align it to 2.
        serializer.serialize_u16(val);
        serializer.serialize_u8(self.options[0]);
        serializer.serialize_u8(self.options[1]);
    }

    /// Read encapsulation header from raw byte stream.
    pub fn deserialize(deserializer: &mut CdrDeserializer) -> CdrResult<Self> {
        let kind_val = deserializer.deserialize_u16()?;
        let kind = match kind_val {
            0x0000 => EncapsulationKind::CdrBe,
            0x0001 => EncapsulationKind::CdrLe,
            0x0002 => EncapsulationKind::PlCdrBe,
            0x0003 => EncapsulationKind::PlCdrLe,
            0x0010 => EncapsulationKind::DxtCdr2Be,
            0x0011 => EncapsulationKind::DxtCdr2Le,
            0x0012 => EncapsulationKind::DxtPlCdr2Be,
            0x0013 => EncapsulationKind::DxtPlCdr2Le,
            other => {
                return Err(CdrError::InvalidHeader(format!(
                    "unsupported kind: {other:#06x}"
                )))
            }
        };
        let o0 = deserializer.deserialize_u8()?;
        let o1 = deserializer.deserialize_u8()?;
        Ok(Self {
            kind,
            options: [o0, o1],
        })
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Parameter List representation (PL-CDR, RTPS §9.6.3)
// ──────────────────────────────────────────────────────────────────────────────

/// A 16-bit identifier for a parameter in a `ParameterList`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParameterId(pub u16);

impl ParameterId {
    pub const PID_SENTINEL: Self = Self(0x0001);
    pub const PID_PAD: Self = Self(0x0000);
    pub const PID_KEY_HASH: Self = Self(0x0070);
    pub const PID_STATUS_INFO: Self = Self(0x0071);
}

/// A parameter is a (key, value) pair representing a field in a `ParameterList`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub parameter_id: ParameterId,
    pub value: Vec<u8>,
}

/// `ParameterList` is a sequence of Parameter elements ending with `PID_SENTINEL`.
///
/// Reference: RTPS §9.6.3
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParameterList {
    pub parameters: Vec<Parameter>,
}

impl ParameterList {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            parameters: Vec::new(),
        }
    }

    pub fn add(&mut self, parameter_id: ParameterId, value: Vec<u8>) {
        self.parameters.push(Parameter {
            parameter_id,
            value,
        });
    }
}

impl CdrSerialize for ParameterList {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        for param in &self.parameters {
            // Write PID
            serializer.serialize_u16(param.parameter_id.0);

            // Align length field to 2
            let length = param.value.len();
            // Pad length to a multiple of 4 as per RTPS specification §9.6.3
            // Since we prepend the unpadded u32 length prefix, the payload length is length + 4, aligned to 4.
            let total_len = length + 4;
            let padded_total = (total_len + 3) & !3;
            serializer.serialize_u16(padded_total as u16);

            // We write the actual unpadded length inside the first 2 bytes or similar? No, the standard states:
            // "The value is aligned to 4 bytes and padded. The length field matches the padded length."
            // However, some implementations use the payload format or custom padding fields.
            // Under RTPS 9.6.3, we serialize the exact unpadded length or padded length?
            // "length of the parameter value (i.e. not including the parameterId and length fields themselves)
            // aligned to 4 bytes."
            // To retrieve the original unpadded size for string or byte sequences, it is either self-describing,
            // or we must preserve the exact value length. Let's record the exact unpadded length by prepending
            // a custom payload length if necessary, or let's assume the parameters in our stack keep the padded size
            // but we decode only up to the padded size. Wait! If the value is a string, its internal length field
            // dictates the end. If it is arbitrary bytes, we should serialize the unpadded size in the Parameter representation,
            // or the parameter ID mapping defines the length.
            // Let's store the unpadded length in the first 2 bytes of the payload or serialize it as a parameter header.
            // Actually, the length field in RTPS parameter header MUST be the padded length.
            // If the parameter payload is a primitive (e.g. u32), length is 4.
            // If the parameter is a string, the string itself has a u32 length field inside the payload which determines the unpadded size.
            // For custom byte arrays, they are self-describing or we must strip the trailing zeros.
            // Let's serialize the actual length in the first 2 bytes of the payload for arbitrary raw byte parameters,
            // OR let's serialize the unpadded length as part of the parameter value length and pad it on the wire without changing the header length?
            // No, the header length must match the wire payload length (padded).
            // Let's store the actual unpadded length (u32) inside the payload first, followed by the bytes, to preserve exact bounds.
            serializer.serialize_u32(length as u32);
            serializer.buf.put_slice(&param.value);

            // Write padding bytes to match padded_len + 4 (for the u32 length prefix)
            let total_len = length + 4;
            let padded_total = (total_len + 3) & !3;
            if padded_total > total_len {
                serializer.buf.put_bytes(0, padded_total - total_len);
            }
        }

        // Write sentinel PID_SENTINEL (0x0001) with length 0
        serializer.serialize_u16(ParameterId::PID_SENTINEL.0);
        serializer.serialize_u16(0);
        Ok(())
    }
}

impl CdrDeserialize for ParameterList {
    fn deserialize(deserializer: &mut CdrDeserializer) -> CdrResult<Self> {
        let mut plist = Self::new();
        loop {
            let pid = deserializer.deserialize_u16()?;
            let length = deserializer.deserialize_u16()? as usize;

            if pid == ParameterId::PID_SENTINEL.0 {
                break;
            }

            if pid == ParameterId::PID_PAD.0 {
                deserializer.skip(length)?;
                continue;
            }

            // Read unpadded length
            let unpadded_len = deserializer.deserialize_u32()? as usize;
            let val = deserializer.read_slice(unpadded_len)?;

            // Skip the rest of the padded block
            let total_read = unpadded_len + 4;
            if length > total_read {
                deserializer.skip(length - total_read)?;
            }

            plist.add(ParameterId(pid), val.to_vec());
        }
        Ok(plist)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Serialization Helper Functions
// ──────────────────────────────────────────────────────────────────────────────

/// Convenience function to serialize any `CdrSerialize` value to a Bytes buffer.
pub fn serialize_to_bytes<T: CdrSerialize>(value: &T, endian: Endianness) -> CdrResult<Bytes> {
    let mut serializer = CdrSerializer::new(endian);
    value.serialize(&mut serializer)?;
    Ok(serializer.into_bytes())
}

/// Convenience function to deserialize any `CdrDeserialize` value from a slice.
pub fn deserialize_from_slice<T: CdrDeserialize>(slice: &[u8], endian: Endianness) -> CdrResult<T> {
    let mut deserializer = CdrDeserializer::new(slice, endian);
    T::deserialize(&mut deserializer)
}

// ──────────────────────────────────────────────────────────────────────────────
// Default implementations for primitives
// ──────────────────────────────────────────────────────────────────────────────

macro_rules! impl_cdr_primitive {
    ($type:ty, $ser:ident, $deser:ident) => {
        impl CdrSerialize for $type {
            fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
                serializer.$ser(*self);
                Ok(())
            }
        }

        impl CdrDeserialize for $type {
            fn deserialize(deserializer: &mut CdrDeserializer) -> CdrResult<Self> {
                deserializer.$deser()
            }
        }
    };
}

impl_cdr_primitive!(u8, serialize_u8, deserialize_u8);
impl_cdr_primitive!(i8, serialize_i8, deserialize_i8);
impl_cdr_primitive!(u16, serialize_u16, deserialize_u16);
impl_cdr_primitive!(i16, serialize_i16, deserialize_i16);
impl_cdr_primitive!(u32, serialize_u32, deserialize_u32);
impl_cdr_primitive!(i32, serialize_i32, deserialize_i32);
impl_cdr_primitive!(u64, serialize_u64, deserialize_u64);
impl_cdr_primitive!(i64, serialize_i64, deserialize_i64);
impl_cdr_primitive!(f32, serialize_f32, deserialize_f32);
impl_cdr_primitive!(f64, serialize_f64, deserialize_f64);
impl_cdr_primitive!(bool, serialize_bool, deserialize_bool);

impl CdrSerialize for String {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_str(self);
        Ok(())
    }
}

impl CdrDeserialize for String {
    fn deserialize(deserializer: &mut CdrDeserializer) -> CdrResult<Self> {
        deserializer.deserialize_str()
    }
}

// Vector serialization implementation: length (u32) followed by elements
impl<T: CdrSerialize> CdrSerialize for Vec<T> {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_u32(self.len() as u32);
        for item in self {
            item.serialize(serializer)?;
        }
        Ok(())
    }
}

impl<T: CdrDeserialize> CdrDeserialize for Vec<T> {
    fn deserialize(deserializer: &mut CdrDeserializer) -> CdrResult<Self> {
        let len = deserializer.deserialize_u32()? as usize;
        let mut vec = Self::with_capacity(len);
        for _ in 0..len {
            vec.push(T::deserialize(deserializer)?);
        }
        Ok(vec)
    }
}

// Option serialization: 1-byte boolean flag (present = true, absent = false) followed by the value
impl<T: CdrSerialize> CdrSerialize for Option<T> {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        match self {
            Some(val) => {
                serializer.serialize_bool(true);
                val.serialize(serializer)?;
            }
            None => {
                serializer.serialize_bool(false);
            }
        }
        Ok(())
    }
}

impl<T: CdrDeserialize> CdrDeserialize for Option<T> {
    fn deserialize(deserializer: &mut CdrDeserializer) -> CdrResult<Self> {
        let has_value = deserializer.deserialize_bool()?;
        if has_value {
            Ok(Some(T::deserialize(deserializer)?))
        } else {
            Ok(None)
        }
    }
}

// Array serialization/deserialization for common sizes
impl<T: CdrSerialize, const N: usize> CdrSerialize for [T; N] {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        for item in self {
            item.serialize(serializer)?;
        }
        Ok(())
    }
}

impl<T: CdrDeserialize, const N: usize> CdrDeserialize for [T; N] {
    fn deserialize(deserializer: &mut CdrDeserializer) -> CdrResult<Self> {
        // Safe initialization since we build the array element by element
        let mut list = Vec::with_capacity(N);
        for _ in 0..N {
            list.push(T::deserialize(deserializer)?);
        }
        let array: [T; N] = list
            .try_into()
            .map_err(|_| CdrError::LengthMismatch("Array size conversion failed".into()))?;
        Ok(array)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Manual implementations for dds-types builtin types
// ──────────────────────────────────────────────────────────────────────────────

impl CdrSerialize for dds_types::guid::GuidPrefix {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        self.0.serialize(serializer)
    }
}

impl CdrDeserialize for dds_types::guid::GuidPrefix {
    fn deserialize(deserializer: &mut CdrDeserializer) -> CdrResult<Self> {
        let bytes: [u8; 12] = CdrDeserialize::deserialize(deserializer)?;
        Ok(Self(bytes))
    }
}

impl CdrSerialize for dds_types::guid::EntityId {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        self.0.serialize(serializer)
    }
}

impl CdrDeserialize for dds_types::guid::EntityId {
    fn deserialize(deserializer: &mut CdrDeserializer) -> CdrResult<Self> {
        let bytes: [u8; 4] = CdrDeserialize::deserialize(deserializer)?;
        Ok(Self(bytes))
    }
}

impl CdrSerialize for dds_types::guid::Guid {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        self.prefix.serialize(serializer)?;
        self.entity_id.serialize(serializer)
    }
}

impl CdrDeserialize for dds_types::guid::Guid {
    fn deserialize(deserializer: &mut CdrDeserializer) -> CdrResult<Self> {
        let prefix = dds_types::guid::GuidPrefix::deserialize(deserializer)?;
        let entity_id = dds_types::guid::EntityId::deserialize(deserializer)?;
        Ok(Self { prefix, entity_id })
    }
}

impl CdrSerialize for dds_types::locator::Locator {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        // LocatorKind is i32, port is u32, address is [u8; 16]
        let kind_val = self.kind as i32;
        serializer.serialize_i32(kind_val);
        serializer.serialize_u32(self.port);
        self.address.serialize(serializer)
    }
}

impl CdrDeserialize for dds_types::locator::Locator {
    fn deserialize(deserializer: &mut CdrDeserializer) -> CdrResult<Self> {
        let kind_val = deserializer.deserialize_i32()?;
        let kind = dds_types::locator::LocatorKind::from_i32(kind_val);
        let port = deserializer.deserialize_u32()?;
        let address: [u8; 16] = CdrDeserialize::deserialize(deserializer)?;
        Ok(Self {
            kind,
            port,
            address,
        })
    }
}

impl CdrSerialize for dds_types::time::Duration {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_i32(self.seconds);
        serializer.serialize_u32(self.nanoseconds);
        Ok(())
    }
}

impl CdrDeserialize for dds_types::time::Duration {
    fn deserialize(deserializer: &mut CdrDeserializer) -> CdrResult<Self> {
        let seconds = deserializer.deserialize_i32()?;
        let nanoseconds = deserializer.deserialize_u32()?;
        Ok(Self {
            seconds,
            nanoseconds,
        })
    }
}

impl CdrSerialize for dds_types::time::Timestamp {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_u32(self.seconds);
        serializer.serialize_u32(self.nanoseconds);
        Ok(())
    }
}

impl CdrDeserialize for dds_types::time::Timestamp {
    fn deserialize(deserializer: &mut CdrDeserializer) -> CdrResult<Self> {
        let seconds = deserializer.deserialize_u32()?;
        let nanoseconds = deserializer.deserialize_u32()?;
        Ok(Self {
            seconds,
            nanoseconds,
        })
    }
}

impl CdrSerialize for dds_types::instance::InstanceHandle {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        self.0.serialize(serializer)
    }
}

impl CdrDeserialize for dds_types::instance::InstanceHandle {
    fn deserialize(deserializer: &mut CdrDeserializer) -> CdrResult<Self> {
        let bytes: [u8; 16] = CdrDeserialize::deserialize(deserializer)?;
        Ok(Self(bytes))
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_roundtrip() {
        let val_u32 = 42u32;
        let serialized = serialize_to_bytes(&val_u32, Endianness::LittleEndian).unwrap();
        let deserialized: u32 =
            deserialize_from_slice(&serialized, Endianness::LittleEndian).unwrap();
        assert_eq!(deserialized, val_u32);

        let val_f64 = 123.456f64;
        let serialized = serialize_to_bytes(&val_f64, Endianness::BigEndian).unwrap();
        let deserialized: f64 = deserialize_from_slice(&serialized, Endianness::BigEndian).unwrap();
        assert_eq!(deserialized, val_f64);
    }

    #[test]
    fn test_string_roundtrip() {
        let text = "Hello DDS World!".to_string();
        let serialized = serialize_to_bytes(&text, Endianness::LittleEndian).unwrap();
        let deserialized: String =
            deserialize_from_slice(&serialized, Endianness::LittleEndian).unwrap();
        assert_eq!(deserialized, text);
    }

    #[test]
    fn test_vec_roundtrip() {
        let values = vec![1i32, 2, 3, 4, 5];
        let serialized = serialize_to_bytes(&values, Endianness::LittleEndian).unwrap();
        let deserialized: Vec<i32> =
            deserialize_from_slice(&serialized, Endianness::LittleEndian).unwrap();
        assert_eq!(deserialized, values);
    }

    #[test]
    fn test_option_roundtrip() {
        let opt_some = Some(100u16);
        let serialized = serialize_to_bytes(&opt_some, Endianness::BigEndian).unwrap();
        let deserialized: Option<u16> =
            deserialize_from_slice(&serialized, Endianness::BigEndian).unwrap();
        assert_eq!(deserialized, opt_some);

        let opt_none: Option<u16> = None;
        let serialized = serialize_to_bytes(&opt_none, Endianness::BigEndian).unwrap();
        let deserialized: Option<u16> =
            deserialize_from_slice(&serialized, Endianness::BigEndian).unwrap();
        assert_eq!(deserialized, opt_none);
    }

    #[test]
    fn test_array_roundtrip() {
        let arr = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let serialized = serialize_to_bytes(&arr, Endianness::LittleEndian).unwrap();
        let deserialized: [u8; 10] =
            deserialize_from_slice(&serialized, Endianness::LittleEndian).unwrap();
        assert_eq!(deserialized, arr);
    }

    #[test]
    fn test_encapsulation_header() {
        let header = EncapsulationHeader::new(EncapsulationKind::CdrLe);
        let mut serializer = CdrSerializer::new(Endianness::LittleEndian);
        header.serialize(&mut serializer);

        let mut deserializer = CdrDeserializer::new(serializer.bytes(), Endianness::LittleEndian);
        let parsed = EncapsulationHeader::deserialize(&mut deserializer).unwrap();
        assert_eq!(parsed, header);
    }

    #[test]
    fn test_alignment() {
        let mut serializer = CdrSerializer::new(Endianness::LittleEndian);
        serializer.serialize_u8(1); // offset 1
        serializer.serialize_u32(100); // should align to 4, adding 3 bytes of padding
        assert_eq!(serializer.bytes().len(), 8); // 1 + 3 (padding) + 4 (u32)
    }

    #[test]
    fn test_parameter_list_roundtrip() {
        let mut plist = ParameterList::new();
        plist.add(ParameterId(10), vec![1, 2, 3]); // Length 3 + 4 prefix = 7, padded to 8
        plist.add(ParameterId(20), vec![4, 5]); // Length 2 + 4 prefix = 6, padded to 8

        let serialized = serialize_to_bytes(&plist, Endianness::LittleEndian).unwrap();

        // Expected layout:
        // Parameter 1: PID (2B) + Length (2B) = 4B. Value: Prefix (4B) + 3B payload + 1B pad = 8B. Total 12B.
        // Parameter 2: PID (2B) + Length (2B) = 4B. Value: Prefix (4B) + 2B payload + 2B pad = 8B. Total 12B.
        // Sentinel: PID_SENTINEL (2B) + 0 (2B) = 4B.
        // Total = 12 + 12 + 4 = 28 bytes
        assert_eq!(serialized.len(), 28);

        let deserialized: ParameterList =
            deserialize_from_slice(&serialized, Endianness::LittleEndian).unwrap();
        assert_eq!(deserialized.parameters.len(), 2);
        assert_eq!(deserialized.parameters[0].parameter_id, ParameterId(10));
        assert_eq!(deserialized.parameters[0].value, vec![1, 2, 3]);
        assert_eq!(deserialized.parameters[1].parameter_id, ParameterId(20));
        assert_eq!(deserialized.parameters[1].value, vec![4, 5]);
    }

    #[test]
    fn test_builtin_types_roundtrip() {
        let prefix = dds_types::guid::GuidPrefix::new([5; 12]);
        let ent = dds_types::guid::EntityId::new([0x00, 0x01, 0x00, 0xc2]);
        let guid = dds_types::guid::Guid::new(prefix, ent);
        let loc = dds_types::locator::Locator::udpv4(std::net::Ipv4Addr::new(127, 0, 0, 1), 7400);
        let dur = dds_types::time::Duration::from_secs(42);
        let ts = dds_types::time::Timestamp::new(100, 500_000_000);
        let handle = dds_types::instance::InstanceHandle::new([0xAA; 16]);

        // 1. GuidPrefix
        let ser = serialize_to_bytes(&prefix, Endianness::LittleEndian).unwrap();
        assert_eq!(
            deserialize_from_slice::<dds_types::guid::GuidPrefix>(&ser, Endianness::LittleEndian)
                .unwrap(),
            prefix
        );

        // 2. EntityId
        let ser = serialize_to_bytes(&ent, Endianness::LittleEndian).unwrap();
        assert_eq!(
            deserialize_from_slice::<dds_types::guid::EntityId>(&ser, Endianness::LittleEndian)
                .unwrap(),
            ent
        );

        // 3. Guid
        let ser = serialize_to_bytes(&guid, Endianness::LittleEndian).unwrap();
        assert_eq!(
            deserialize_from_slice::<dds_types::guid::Guid>(&ser, Endianness::LittleEndian)
                .unwrap(),
            guid
        );

        // 4. Locator
        let ser = serialize_to_bytes(&loc, Endianness::LittleEndian).unwrap();
        assert_eq!(
            deserialize_from_slice::<dds_types::locator::Locator>(&ser, Endianness::LittleEndian)
                .unwrap(),
            loc
        );

        // 5. Duration
        let ser = serialize_to_bytes(&dur, Endianness::LittleEndian).unwrap();
        assert_eq!(
            deserialize_from_slice::<dds_types::time::Duration>(&ser, Endianness::LittleEndian)
                .unwrap(),
            dur
        );

        // 6. Timestamp
        let ser = serialize_to_bytes(&ts, Endianness::LittleEndian).unwrap();
        assert_eq!(
            deserialize_from_slice::<dds_types::time::Timestamp>(&ser, Endianness::LittleEndian)
                .unwrap(),
            ts
        );

        // 7. InstanceHandle
        let ser = serialize_to_bytes(&handle, Endianness::LittleEndian).unwrap();
        assert_eq!(
            deserialize_from_slice::<dds_types::instance::InstanceHandle>(
                &ser,
                Endianness::LittleEndian
            )
            .unwrap(),
            handle
        );
    }
}
