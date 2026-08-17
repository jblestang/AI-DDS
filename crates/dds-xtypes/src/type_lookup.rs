//! XTypes §7.6.3.3 built-in TypeLookup service (DDS-RPC basic mapping, XCDR2).
//!
//! Implements `TypeLookup_Request` / `TypeLookup_Reply` with `getTypes` and
//! `getTypeDependencies` operations.

use crate::hashid::{
    MEMBER_COMPLETE_TO_MINIMAL, MEMBER_CONTINUATION_POINT, MEMBER_DEPENDENT_TYPEIDS,
    MEMBER_TYPE_IDS, MEMBER_TYPES, TYPE_LOOKUP_GET_DEPENDENCIES_HASH,
    TYPE_LOOKUP_GET_TYPES_HASH,
};
use crate::{TypeIdentifier, TypeObject};
use dds_cdr::{
    CdrDeserialize, CdrDeserializer, CdrError, CdrResult, CdrSerialize, CdrSerializer,
    EncapsulationHeader, EncapsulationKind, Endianness,
};
use dds_types::guid::{EntityId, Guid, GuidPrefix, SequenceNumber};
use std::collections::{HashMap, HashSet};

/// DDS return code used as union discriminator in TypeLookup result types.
pub const DDS_RETCODE_OK: i32 = 0;

/// `dds::SampleIdentity` (DDS-RPC).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SampleIdentity {
    pub writer_guid: Guid,
    pub sequence_number: SequenceNumber,
}

impl SampleIdentity {
    fn serialize(&self, ser: &mut CdrSerializer) -> CdrResult<()> {
        ser.append_bytes(self.writer_guid.prefix.as_bytes());
        ser.append_bytes(self.writer_guid.entity_id.as_bytes());
        let (high, low) = self.sequence_number.to_high_low();
        ser.serialize_i32(high);
        ser.serialize_u32(low);
        Ok(())
    }

    fn deserialize(de: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        let mut prefix = [0u8; 12];
        for b in &mut prefix {
            *b = de.deserialize_u8()?;
        }
        let mut entity = [0u8; 4];
        for b in &mut entity {
            *b = de.deserialize_u8()?;
        }
        let high = de.deserialize_i32()?;
        let low = de.deserialize_u32()?;
        Ok(Self {
            writer_guid: Guid::new(GuidPrefix::new(prefix), EntityId::new(entity)),
            sequence_number: SequenceNumber::from_high_low(high, low),
        })
    }
}

/// `dds::rpc::RequestHeader` — used by both request and reply (XTypes §7.6.3.3.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestHeader {
    pub request_id: SampleIdentity,
    /// `dds.builtin.TOS.<hex-guid>` per §7.6.3.3.4.
    pub instance_name: String,
}

impl RequestHeader {
    fn serialize(&self, ser: &mut CdrSerializer) -> CdrResult<()> {
        self.request_id.serialize(ser)?;
        serialize_bounded_string(ser, &self.instance_name, 255)?;
        Ok(())
    }

    fn deserialize(de: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        Ok(Self {
            request_id: SampleIdentity::deserialize(de)?,
            instance_name: deserialize_bounded_string(de, 255)?,
        })
    }
}

/// Build the service `instanceName` for a participant GUID prefix (24 lowercase hex digits).
#[must_use]
pub fn type_lookup_instance_name(prefix: &GuidPrefix) -> String {
    let mut s = String::from("dds.builtin.TOS.");
    for b in prefix.as_bytes() {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// `@extensibility(FINAL)` pair used in `getTypes` replies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeIdentifierTypeObjectPair {
    pub type_identifier: TypeIdentifier,
    pub type_object: TypeObject,
}

impl TypeIdentifierTypeObjectPair {
    fn serialize(&self, ser: &mut CdrSerializer) -> CdrResult<()> {
        self.type_identifier.serialize(ser)?;
        self.type_object.serialize(ser)
    }

    fn deserialize(de: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        Ok(Self {
            type_identifier: TypeIdentifier::deserialize(de)?,
            type_object: TypeObject::deserialize(de)?,
        })
    }
}

/// `@extensibility(FINAL)` mapping from COMPLETE to MINIMAL identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeIdentifierPair {
    pub type_identifier1: TypeIdentifier,
    pub type_identifier2: TypeIdentifier,
}

impl TypeIdentifierPair {
    fn serialize(&self, ser: &mut CdrSerializer) -> CdrResult<()> {
        self.type_identifier1.serialize(ser)?;
        self.type_identifier2.serialize(ser)
    }

    fn deserialize(de: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        Ok(Self {
            type_identifier1: TypeIdentifier::deserialize(de)?,
            type_identifier2: TypeIdentifier::deserialize(de)?,
        })
    }
}

/// Annex B `TypeIdentfierWithSize` (spec spelling).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeIdentifierWithSize {
    pub type_id: TypeIdentifier,
    pub typeobject_serialized_size: u32,
}

impl TypeIdentifierWithSize {
    fn serialize(&self, ser: &mut CdrSerializer) -> CdrResult<()> {
        self.type_id.serialize(ser)?;
        ser.serialize_u32(self.typeobject_serialized_size);
        Ok(())
    }

    fn deserialize(de: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        Ok(Self {
            type_id: TypeIdentifier::deserialize(de)?,
            typeobject_serialized_size: de.deserialize_u32()?,
        })
    }
}

/// Annex B `TypeIdentifierWithDependencies`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeIdentifierWithDependencies {
    pub typeid_with_size: TypeIdentifierWithSize,
    pub dependent_typeid_count: i32,
    pub dependent_typeids: Vec<TypeIdentifier>,
}

impl TypeIdentifierWithDependencies {
    fn serialize(&self, ser: &mut CdrSerializer) -> CdrResult<()> {
        self.typeid_with_size.serialize(ser)?;
        ser.serialize_i32(self.dependent_typeid_count);
        serialize_type_identifier_seq(ser, &self.dependent_typeids)
    }

    fn deserialize(de: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        Ok(Self {
            typeid_with_size: TypeIdentifierWithSize::deserialize(de)?,
            dependent_typeid_count: de.deserialize_i32()?,
            dependent_typeids: deserialize_type_identifier_seq(de)?,
        })
    }
}

/// `TypeLookup_getTypes_In` (@extensibility MUTABLE).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLookupGetTypesIn {
    pub type_ids: Vec<TypeIdentifier>,
}

/// `TypeLookup_getTypes_Out` (@extensibility MUTABLE).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLookupGetTypesOut {
    pub types: Vec<TypeIdentifierTypeObjectPair>,
    pub complete_to_minimal: Vec<TypeIdentifierPair>,
}

/// `TypeLookup_getTypeDependencies_In` (@extensibility MUTABLE).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLookupGetTypeDependenciesIn {
    pub type_ids: Vec<TypeIdentifier>,
    pub continuation_point: Option<Vec<u8>>,
}

/// `TypeLookup_getTypeDependencies_Out` (@extensibility MUTABLE).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLookupGetTypeDependenciesOut {
    pub dependent_typeids: Vec<TypeIdentifierWithDependencies>,
    pub continuation_point: Option<Vec<u8>>,
}

/// `TypeLookup_getTypes_Result` union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeLookupGetTypesResult {
    Ok(TypeLookupGetTypesOut),
}

/// `TypeLookup_getTypeDependencies_Result` union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeLookupGetTypeDependenciesResult {
    Ok(TypeLookupGetTypeDependenciesOut),
}

/// `TypeLookup_Call` service request union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeLookupCall {
    GetTypes(TypeLookupGetTypesIn),
    GetTypeDependencies(TypeLookupGetTypeDependenciesIn),
}

/// `TypeLookup_Return` service reply union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeLookupReturn {
    GetTypes(TypeLookupGetTypesResult),
    GetTypeDependencies(TypeLookupGetTypeDependenciesResult),
}

/// `@RPCRequestType` `TypeLookup_Request`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLookupRequest {
    pub header: RequestHeader,
    pub data: TypeLookupCall,
}

/// `@RPCReplyType` `TypeLookup_Reply`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLookupReply {
    pub header: RequestHeader,
    pub return_data: TypeLookupReturn,
}

// ── XCDR2 wire codec ─────────────────────────────────────────────────────────

fn serialize_bounded_string(ser: &mut CdrSerializer, val: &str, max: usize) -> CdrResult<()> {
    if val.len() >= max {
        return Err(CdrError::InvalidHeader(format!(
            "bounded string exceeds {max} characters"
        )));
    }
    ser.serialize_str(val);
    Ok(())
}

fn deserialize_bounded_string(de: &mut CdrDeserializer<'_>, max: usize) -> CdrResult<String> {
    let s = de.deserialize_str()?;
    if s.len() >= max {
        return Err(CdrError::InvalidHeader(format!(
            "bounded string exceeds {max} characters"
        )));
    }
    Ok(s)
}

fn serialize_type_identifier_seq(ser: &mut CdrSerializer, ids: &[TypeIdentifier]) -> CdrResult<()> {
    ser.serialize_u32(ids.len() as u32);
    for id in ids {
        id.serialize(ser)?;
    }
    Ok(())
}

fn deserialize_type_identifier_seq(de: &mut CdrDeserializer<'_>) -> CdrResult<Vec<TypeIdentifier>> {
    let len = de.deserialize_u32()? as usize;
    let mut ids = Vec::with_capacity(len);
    for _ in 0..len {
        ids.push(TypeIdentifier::deserialize(de)?);
    }
    Ok(ids)
}

fn serialize_member_blob<F>(write_body: F) -> CdrResult<Vec<u8>>
where
    F: FnOnce(&mut CdrSerializer) -> CdrResult<()>,
{
    let mut inner = CdrSerializer::new(Endianness::LittleEndian);
    write_body(&mut inner)?;
    Ok(inner.into_bytes().to_vec())
}

fn write_mutable_struct<F>(ser: &mut CdrSerializer, write_members: F) -> CdrResult<()>
where
    F: FnOnce(&mut CdrSerializer) -> CdrResult<()>,
{
    let dheader_off = ser.write_dheader_placeholder();
    write_members(ser)?;
    ser.patch_dheader(dheader_off);
    Ok(())
}

fn write_mutable_member(
    ser: &mut CdrSerializer,
    member_id: u32,
    body: &[u8],
) -> CdrResult<()> {
    ser.serialize_emheader(member_id, body.len() as u32);
    ser.append_bytes(body);
    Ok(())
}

impl TypeLookupGetTypesIn {
    fn serialize_xcdr2(&self, ser: &mut CdrSerializer) -> CdrResult<()> {
        write_mutable_struct(ser, |ser| {
            let body = serialize_member_blob(|inner| serialize_type_identifier_seq(inner, &self.type_ids))?;
            write_mutable_member(ser, MEMBER_TYPE_IDS, &body)
        })
    }

    fn deserialize_xcdr2(de: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        let start = de.offset();
        let dlen = de.deserialize_dheader()? as usize;
        let end = start + 4 + dlen;
        let mut type_ids = Vec::new();
        while de.offset() < end {
            let (member_id, length) = de.deserialize_emheader()?;
            let member_start = de.offset();
            if member_id == MEMBER_TYPE_IDS {
                type_ids = deserialize_type_identifier_seq(de)?;
            } else {
                de.skip(length as usize)?;
            }
            let consumed = de.offset() - member_start;
            if consumed < length as usize {
                de.skip(length as usize - consumed)?;
            }
        }
        Ok(Self { type_ids })
    }
}

impl TypeLookupGetTypesOut {
    fn serialize_xcdr2(&self, ser: &mut CdrSerializer) -> CdrResult<()> {
        write_mutable_struct(ser, |ser| {
            let types_body = serialize_member_blob(|inner| {
                inner.serialize_u32(self.types.len() as u32);
                for pair in &self.types {
                    pair.serialize(inner)?;
                }
                Ok(())
            })?;
            write_mutable_member(ser, MEMBER_TYPES, &types_body)?;
            let c2m_body = serialize_member_blob(|inner| {
                inner.serialize_u32(self.complete_to_minimal.len() as u32);
                for pair in &self.complete_to_minimal {
                    pair.serialize(inner)?;
                }
                Ok(())
            })?;
            write_mutable_member(ser, MEMBER_COMPLETE_TO_MINIMAL, &c2m_body)
        })
    }

    fn deserialize_xcdr2(de: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        let start = de.offset();
        let dlen = de.deserialize_dheader()? as usize;
        let end = start + 4 + dlen;
        let mut types = Vec::new();
        let mut complete_to_minimal = Vec::new();
        while de.offset() < end {
            let (member_id, length) = de.deserialize_emheader()?;
            let member_start = de.offset();
            match member_id {
                MEMBER_TYPES => {
                    let len = de.deserialize_u32()? as usize;
                    for _ in 0..len {
                        types.push(TypeIdentifierTypeObjectPair::deserialize(de)?);
                    }
                }
                MEMBER_COMPLETE_TO_MINIMAL => {
                    let len = de.deserialize_u32()? as usize;
                    for _ in 0..len {
                        complete_to_minimal.push(TypeIdentifierPair::deserialize(de)?);
                    }
                }
                _ => de.skip(length as usize)?,
            }
            let consumed = de.offset() - member_start;
            if consumed < length as usize {
                de.skip(length as usize - consumed)?;
            }
        }
        Ok(Self {
            types,
            complete_to_minimal,
        })
    }
}

impl TypeLookupGetTypeDependenciesIn {
    fn serialize_xcdr2(&self, ser: &mut CdrSerializer) -> CdrResult<()> {
        write_mutable_struct(ser, |ser| {
            let ids_body =
                serialize_member_blob(|inner| serialize_type_identifier_seq(inner, &self.type_ids))?;
            write_mutable_member(ser, MEMBER_TYPE_IDS, &ids_body)?;
            if let Some(cp) = &self.continuation_point {
                let cp_body = serialize_member_blob(|inner| serialize_octet_seq32(inner, cp))?;
                write_mutable_member(ser, MEMBER_CONTINUATION_POINT, &cp_body)?;
            }
            Ok(())
        })
    }

    fn deserialize_xcdr2(de: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        let start = de.offset();
        let dlen = de.deserialize_dheader()? as usize;
        let end = start + 4 + dlen;
        let mut type_ids = Vec::new();
        let mut continuation_point = None;
        while de.offset() < end {
            let (member_id, length) = de.deserialize_emheader()?;
            let member_start = de.offset();
            match member_id {
                MEMBER_TYPE_IDS => type_ids = deserialize_type_identifier_seq(de)?,
                MEMBER_CONTINUATION_POINT => {
                    continuation_point = Some(deserialize_octet_seq32(de)?);
                }
                _ => de.skip(length as usize)?,
            }
            let consumed = de.offset() - member_start;
            if consumed < length as usize {
                de.skip(length as usize - consumed)?;
            }
        }
        Ok(Self {
            type_ids,
            continuation_point,
        })
    }
}

impl TypeLookupGetTypeDependenciesOut {
    fn serialize_xcdr2(&self, ser: &mut CdrSerializer) -> CdrResult<()> {
        write_mutable_struct(ser, |ser| {
            let dep_body = serialize_member_blob(|inner| {
                inner.serialize_u32(self.dependent_typeids.len() as u32);
                for dep in &self.dependent_typeids {
                    dep.serialize(inner)?;
                }
                Ok(())
            })?;
            write_mutable_member(ser, MEMBER_DEPENDENT_TYPEIDS, &dep_body)?;
            if let Some(cp) = &self.continuation_point {
                let cp_body = serialize_member_blob(|inner| serialize_octet_seq32(inner, cp))?;
                write_mutable_member(ser, MEMBER_CONTINUATION_POINT, &cp_body)?;
            }
            Ok(())
        })
    }

    fn deserialize_xcdr2(de: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        let start = de.offset();
        let dlen = de.deserialize_dheader()? as usize;
        let end = start + 4 + dlen;
        let mut dependent_typeids = Vec::new();
        let mut continuation_point = None;
        while de.offset() < end {
            let (member_id, length) = de.deserialize_emheader()?;
            let member_start = de.offset();
            match member_id {
                MEMBER_DEPENDENT_TYPEIDS => {
                    let len = de.deserialize_u32()? as usize;
                    for _ in 0..len {
                        dependent_typeids.push(TypeIdentifierWithDependencies::deserialize(de)?);
                    }
                }
                MEMBER_CONTINUATION_POINT => {
                    continuation_point = Some(deserialize_octet_seq32(de)?);
                }
                _ => de.skip(length as usize)?,
            }
            let consumed = de.offset() - member_start;
            if consumed < length as usize {
                de.skip(length as usize - consumed)?;
            }
        }
        Ok(Self {
            dependent_typeids,
            continuation_point,
        })
    }
}

fn serialize_octet_seq32(ser: &mut CdrSerializer, data: &[u8]) -> CdrResult<()> {
    if data.len() > 32 {
        return Err(CdrError::InvalidHeader(
            "continuation_point exceeds 32 bytes".into(),
        ));
    }
    ser.serialize_u32(data.len() as u32);
    for b in data {
        ser.serialize_u8(*b);
    }
    Ok(())
}

fn deserialize_octet_seq32(de: &mut CdrDeserializer<'_>) -> CdrResult<Vec<u8>> {
    let len = de.deserialize_u32()? as usize;
    if len > 32 {
        return Err(CdrError::InvalidHeader(
            "continuation_point exceeds 32 bytes".into(),
        ));
    }
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        out.push(de.deserialize_u8()?);
    }
    Ok(out)
}

impl TypeLookupCall {
    fn serialize_xcdr2(&self, ser: &mut CdrSerializer) -> CdrResult<()> {
        match self {
            Self::GetTypes(body) => {
                ser.serialize_i32(TYPE_LOOKUP_GET_TYPES_HASH);
                body.serialize_xcdr2(ser)
            }
            Self::GetTypeDependencies(body) => {
                ser.serialize_i32(TYPE_LOOKUP_GET_DEPENDENCIES_HASH);
                body.serialize_xcdr2(ser)
            }
        }
    }

    fn deserialize_xcdr2(de: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        match de.deserialize_i32()? {
            TYPE_LOOKUP_GET_TYPES_HASH => {
                Ok(Self::GetTypes(TypeLookupGetTypesIn::deserialize_xcdr2(de)?))
            }
            TYPE_LOOKUP_GET_DEPENDENCIES_HASH => Ok(Self::GetTypeDependencies(
                TypeLookupGetTypeDependenciesIn::deserialize_xcdr2(de)?,
            )),
            v => Err(CdrError::InvalidHeader(format!(
                "unknown TypeLookup_Call discriminator: {v}"
            ))),
        }
    }
}

impl TypeLookupGetTypesResult {
    fn serialize_xcdr2(&self, ser: &mut CdrSerializer) -> CdrResult<()> {
        match self {
            Self::Ok(out) => {
                ser.serialize_i32(DDS_RETCODE_OK);
                out.serialize_xcdr2(ser)
            }
        }
    }

    fn deserialize_xcdr2(de: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        match de.deserialize_i32()? {
            DDS_RETCODE_OK => Ok(Self::Ok(TypeLookupGetTypesOut::deserialize_xcdr2(de)?)),
            v => Err(CdrError::InvalidHeader(format!(
                "unknown TypeLookup_getTypes_Result: {v}"
            ))),
        }
    }
}

impl TypeLookupGetTypeDependenciesResult {
    fn serialize_xcdr2(&self, ser: &mut CdrSerializer) -> CdrResult<()> {
        match self {
            Self::Ok(out) => {
                ser.serialize_i32(DDS_RETCODE_OK);
                out.serialize_xcdr2(ser)
            }
        }
    }

    fn deserialize_xcdr2(de: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        match de.deserialize_i32()? {
            DDS_RETCODE_OK => Ok(Self::Ok(
                TypeLookupGetTypeDependenciesOut::deserialize_xcdr2(de)?,
            )),
            v => Err(CdrError::InvalidHeader(format!(
                "unknown TypeLookup_getTypeDependencies_Result: {v}"
            ))),
        }
    }
}

impl TypeLookupReturn {
    fn serialize_xcdr2(&self, ser: &mut CdrSerializer) -> CdrResult<()> {
        match self {
            Self::GetTypes(result) => {
                ser.serialize_i32(TYPE_LOOKUP_GET_TYPES_HASH);
                result.serialize_xcdr2(ser)
            }
            Self::GetTypeDependencies(result) => {
                ser.serialize_i32(TYPE_LOOKUP_GET_DEPENDENCIES_HASH);
                result.serialize_xcdr2(ser)
            }
        }
    }

    fn deserialize_xcdr2(de: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        match de.deserialize_i32()? {
            TYPE_LOOKUP_GET_TYPES_HASH => Ok(Self::GetTypes(TypeLookupGetTypesResult::deserialize_xcdr2(
                de,
            )?)),
            TYPE_LOOKUP_GET_DEPENDENCIES_HASH => Ok(Self::GetTypeDependencies(
                TypeLookupGetTypeDependenciesResult::deserialize_xcdr2(de)?,
            )),
            v => Err(CdrError::InvalidHeader(format!(
                "unknown TypeLookup_Return discriminator: {v}"
            ))),
        }
    }
}

impl TypeLookupRequest {
    /// Serialize with XCDR2 encapsulation header (`DxtCdr2Le`).
    pub fn to_wire_bytes(&self) -> CdrResult<Vec<u8>> {
        let mut ser = CdrSerializer::new(Endianness::LittleEndian);
        EncapsulationHeader::new(EncapsulationKind::DxtCdr2Le).serialize(&mut ser);
        self.serialize_xcdr2(&mut ser)?;
        Ok(ser.into_bytes().to_vec())
    }

    /// Deserialize from XCDR2 wire bytes (with or without encapsulation header).
    pub fn from_wire_bytes(bytes: &[u8]) -> CdrResult<Self> {
        let mut de = CdrDeserializer::new(bytes, Endianness::LittleEndian);
        if bytes.len() >= 4 {
            // Encapsulation identifier is big-endian on the wire (DDS CDR §10.2).
            let kind = u16::from_be_bytes([bytes[0], bytes[1]]);
            if matches!(kind, 0x0010 | 0x0011 | 0x0012 | 0x0013) {
                let _ = EncapsulationHeader::deserialize(&mut de)?;
            } else {
                de = CdrDeserializer::new(bytes, Endianness::LittleEndian);
            }
        }
        Self::deserialize_xcdr2(&mut de)
    }

    fn serialize_xcdr2(&self, ser: &mut CdrSerializer) -> CdrResult<()> {
        self.header.serialize(ser)?;
        self.data.serialize_xcdr2(ser)
    }

    fn deserialize_xcdr2(de: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        Ok(Self {
            header: RequestHeader::deserialize(de)?,
            data: TypeLookupCall::deserialize_xcdr2(de)?,
        })
    }
}

impl TypeLookupReply {
    /// Serialize with XCDR2 encapsulation header (`DxtCdr2Le`).
    pub fn to_wire_bytes(&self) -> CdrResult<Vec<u8>> {
        let mut ser = CdrSerializer::new(Endianness::LittleEndian);
        EncapsulationHeader::new(EncapsulationKind::DxtCdr2Le).serialize(&mut ser);
        self.serialize_xcdr2(&mut ser)?;
        Ok(ser.into_bytes().to_vec())
    }

    /// Deserialize from XCDR2 wire bytes (with or without encapsulation header).
    pub fn from_wire_bytes(bytes: &[u8]) -> CdrResult<Self> {
        let mut de = CdrDeserializer::new(bytes, Endianness::LittleEndian);
        if bytes.len() >= 4 {
            // Encapsulation identifier is big-endian on the wire (DDS CDR §10.2).
            let kind = u16::from_be_bytes([bytes[0], bytes[1]]);
            if matches!(kind, 0x0010 | 0x0011 | 0x0012 | 0x0013) {
                let _ = EncapsulationHeader::deserialize(&mut de)?;
            } else {
                de = CdrDeserializer::new(bytes, Endianness::LittleEndian);
            }
        }
        Self::deserialize_xcdr2(&mut de)
    }

    fn serialize_xcdr2(&self, ser: &mut CdrSerializer) -> CdrResult<()> {
        self.header.serialize(ser)?;
        self.return_data.serialize_xcdr2(ser)
    }

    fn deserialize_xcdr2(de: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        Ok(Self {
            header: RequestHeader::deserialize(de)?,
            return_data: TypeLookupReturn::deserialize_xcdr2(de)?,
        })
    }
}

// ── Service logic ────────────────────────────────────────────────────────────

fn is_hash_type_id(id: &TypeIdentifier) -> bool {
    matches!(
        id,
        TypeIdentifier::TiMinimalConstructed(_) | TypeIdentifier::TiCompleteConstructed(_)
    )
}

fn direct_dependencies(obj: &TypeObject) -> Vec<TypeIdentifier> {
    let members = match obj {
        TypeObject::Minimal(s) | TypeObject::Complete(s) => &s.members,
    };
    members
        .iter()
        .map(|m| m.type_id.clone())
        .filter(is_hash_type_id)
        .collect()
}

fn serialized_typeobject_size(obj: &TypeObject) -> u32 {
    dds_cdr::serialize_to_bytes(obj, Endianness::LittleEndian)
        .map(|b| b.len() as u32)
        .unwrap_or(0)
}

/// Handle a `getTypes` operation against the local type-object database.
#[must_use]
pub fn handle_get_types(
    request: &TypeLookupGetTypesIn,
    db: &HashMap<TypeIdentifier, TypeObject>,
) -> TypeLookupGetTypesResult {
    let mut types = Vec::new();
    let mut complete_to_minimal = Vec::new();

    for type_id in &request.type_ids {
        let Some(obj) = db.get(type_id) else {
            continue;
        };
        types.push(TypeIdentifierTypeObjectPair {
            type_identifier: type_id.clone(),
            type_object: obj.clone(),
        });

        if matches!(type_id, TypeIdentifier::TiMinimalConstructed(_))
            && matches!(obj, TypeObject::Complete(_))
        {
            let minimal_obj = TypeObject::Minimal(match obj {
                TypeObject::Complete(s) => s.clone(),
                TypeObject::Minimal(s) => s.clone(),
            });
            if let (Ok(type_identifier1), Ok(type_identifier2)) =
                (obj.get_identifier(), minimal_obj.get_identifier())
            {
                complete_to_minimal.push(TypeIdentifierPair {
                    type_identifier1,
                    type_identifier2,
                });
            }
        }
    }

    TypeLookupGetTypesResult::Ok(TypeLookupGetTypesOut {
        types,
        complete_to_minimal,
    })
}

/// Handle a `getTypeDependencies` operation.
#[must_use]
pub fn handle_get_type_dependencies(
    request: &TypeLookupGetTypeDependenciesIn,
    db: &HashMap<TypeIdentifier, TypeObject>,
) -> TypeLookupGetTypeDependenciesResult {
    let mut seen = HashSet::new();
    let mut dependent_typeids = Vec::new();

    for root_id in &request.type_ids {
        if !is_hash_type_id(root_id) {
            continue;
        }
        let Some(root_obj) = db.get(root_id) else {
            continue;
        };
        let deps: Vec<TypeIdentifier> = direct_dependencies(root_obj)
            .into_iter()
            .filter(|d| seen.insert(d.clone()))
            .collect();
        dependent_typeids.push(TypeIdentifierWithDependencies {
            typeid_with_size: TypeIdentifierWithSize {
                type_id: root_id.clone(),
                typeobject_serialized_size: serialized_typeobject_size(root_obj),
            },
            dependent_typeid_count: deps.len() as i32,
            dependent_typeids: deps,
        });
    }

    TypeLookupGetTypeDependenciesResult::Ok(TypeLookupGetTypeDependenciesOut {
        dependent_typeids,
        continuation_point: None,
    })
}

/// Build a compliant `TypeLookup_Reply` for an incoming request.
#[must_use]
pub fn serve_type_lookup_request(
    request: &TypeLookupRequest,
    db: &HashMap<TypeIdentifier, TypeObject>,
) -> TypeLookupReply {
    let return_data = match &request.data {
        TypeLookupCall::GetTypes(in_params) => TypeLookupReturn::GetTypes(handle_get_types(
            in_params,
            db,
        )),
        TypeLookupCall::GetTypeDependencies(in_params) => {
            TypeLookupReturn::GetTypeDependencies(handle_get_type_dependencies(in_params, db))
        }
    };
    TypeLookupReply {
        header: request.header.clone(),
        return_data,
    }
}

/// Construct a `getTypes` request for the given type identifiers.
#[must_use]
pub fn make_get_types_request(
    writer_guid: Guid,
    sequence_number: SequenceNumber,
    instance_name: String,
    type_ids: Vec<TypeIdentifier>,
) -> TypeLookupRequest {
    TypeLookupRequest {
        header: RequestHeader {
            request_id: SampleIdentity {
                writer_guid,
                sequence_number,
            },
            instance_name,
        },
        data: TypeLookupCall::GetTypes(TypeLookupGetTypesIn { type_ids }),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::{ExtensibilityKind, Member, StructureType};

    fn sample_type_object(name: &str) -> TypeObject {
        TypeObject::Complete(StructureType {
            name: name.to_string(),
            extensibility: ExtensibilityKind::Appendable,
            members: vec![Member {
                name: "x".to_string(),
                type_id: TypeIdentifier::TkInt32,
                is_key: false,
                is_optional: false,
            }],
        })
    }

    #[test]
    fn type_lookup_request_reply_xcdr2_roundtrip() {
        let obj = sample_type_object("Point");
        let type_id = obj.get_identifier();

        let request = make_get_types_request(
            Guid::new(GuidPrefix::new([9; 12]), EntityId::PARTICIPANT),
            SequenceNumber(7),
            type_lookup_instance_name(&GuidPrefix::new([9; 12])),
            vec![type_id.clone()],
        );

        let req_wire = request.to_wire_bytes().unwrap();
        let decoded_req = TypeLookupRequest::from_wire_bytes(&req_wire).unwrap();
        assert_eq!(decoded_req.header.request_id.sequence_number, SequenceNumber(7));
        assert!(matches!(
            decoded_req.data,
            TypeLookupCall::GetTypes(ref inner) if inner.type_ids == vec![type_id.clone()]
        ));

        let mut db = HashMap::new();
        db.insert(type_id, obj.clone());
        let reply = serve_type_lookup_request(&decoded_req, &db);
        let rep_wire = reply.to_wire_bytes().unwrap();
        let decoded_rep = TypeLookupReply::from_wire_bytes(&rep_wire).unwrap();

        match decoded_rep.return_data {
            TypeLookupReturn::GetTypes(TypeLookupGetTypesResult::Ok(out)) => {
                assert_eq!(out.types.len(), 1);
                assert_eq!(out.types[0].type_object, obj);
            }
            _ => panic!("expected getTypes reply"),
        }
    }

    #[test]
    fn get_type_dependencies_lists_hash_deps() {
        let nested_id = TypeIdentifier::TiCompleteConstructed([1; 14]);
        let root = TypeObject::Complete(StructureType {
            name: "Root".to_string(),
            extensibility: ExtensibilityKind::Final,
            members: vec![Member {
                name: "child".to_string(),
                type_id: nested_id.clone(),
                is_key: false,
                is_optional: false,
            }],
        });
        let root_id = root.get_identifier();
        let mut db = HashMap::new();
        db.insert(root_id.clone(), root);

        let request = TypeLookupRequest {
            header: RequestHeader {
                request_id: SampleIdentity {
                    writer_guid: Guid::new(GuidPrefix::new([1; 12]), EntityId::PARTICIPANT),
                    sequence_number: SequenceNumber(1),
                },
                instance_name: type_lookup_instance_name(&GuidPrefix::new([1; 12])),
            },
            data: TypeLookupCall::GetTypeDependencies(TypeLookupGetTypeDependenciesIn {
                type_ids: vec![root_id],
                continuation_point: None,
            }),
        };

        let reply = serve_type_lookup_request(&request, &db);
        match reply.return_data {
            TypeLookupReturn::GetTypeDependencies(TypeLookupGetTypeDependenciesResult::Ok(out)) => {
                assert_eq!(out.dependent_typeids.len(), 1);
                assert_eq!(out.dependent_typeids[0].dependent_typeid_count, 1);
                assert_eq!(out.dependent_typeids[0].dependent_typeids[0], nested_id);
            }
            _ => panic!("expected getTypeDependencies reply"),
        }
    }

    #[test]
    fn instance_name_format() {
        let prefix = GuidPrefix::new([0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0, 0, 0, 0]);
        assert_eq!(
            type_lookup_instance_name(&prefix),
            "dds.builtin.TOS.0123456789abcdef00000000"
        );
    }
}
