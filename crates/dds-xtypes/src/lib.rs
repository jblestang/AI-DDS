//! # dds-xtypes — DDS `XTypes` 1.3 Type System
//!
//! Implements the Extensible and Dynamic Topic Types specification:
//! `TypeObject`, `TypeIdentifier`, type compatibility rules, and extensibility.
//!
//! Reference: `XTypes` §7.

#![warn(
    rust_2018_idioms,
    nonstandard_style,
    future_incompatible
)]
#![allow(
    clippy::blanket_clippy_restriction_lints,
    reason = "restriction lints are enabled individually via workspace lint config"
)]

pub mod dynamic;
pub mod hashid;
pub mod type_lookup;

pub use dynamic::{Data, Reflect};
pub use hashid::hashid;
pub use type_lookup::{
    handle_get_type_dependencies, handle_get_types, make_get_types_request, serve_type_lookup_request,
    type_lookup_instance_name, RequestHeader, SampleIdentity, TypeIdentifierPair,
    TypeIdentifierTypeObjectPair, TypeIdentifierWithDependencies, TypeIdentifierWithSize,
    TypeLookupCall, TypeLookupGetTypeDependenciesIn, TypeLookupGetTypeDependenciesOut,
    TypeLookupGetTypeDependenciesResult, TypeLookupGetTypesIn, TypeLookupGetTypesOut,
    TypeLookupGetTypesResult, TypeLookupReply, TypeLookupRequest, TypeLookupReturn,
};

use dds_cdr::{CdrDeserialize, CdrDeserializer, CdrError, CdrResult, CdrSerialize, CdrSerializer};
use sha2::{Digest as _, Sha256};
use std::collections::HashMap;

/// Propagate a [`CdrResult`] value or return early on error.
macro_rules! cdr {
    ($expr:expr) => {
        match $expr {
            Ok(value) => value,
            Err(error) => return Err(error),
        }
    };
}

pub const TK_NONE: u8 = 0x00;
pub const TK_BOOLEAN: u8 = 0x01;
pub const TK_BYTE: u8 = 0x02;
pub const TK_INT16: u8 = 0x03;
pub const TK_INT32: u8 = 0x04;
pub const TK_INT64: u8 = 0x05;
pub const TK_UINT16: u8 = 0x06;
pub const TK_UINT32: u8 = 0x07;
pub const TK_UINT64: u8 = 0x08;
pub const TK_FLOAT32: u8 = 0x09;
pub const TK_FLOAT64: u8 = 0x0A;
pub const TK_FLOAT128: u8 = 0x0B;
pub const TK_CHAR8: u8 = 0x10;
pub const TK_CHAR16: u8 = 0x11;
pub const TI_STRING8_SMALL: u8 = 0x70;
pub const TI_STRING8_LARGE: u8 = 0x71;
pub const TI_STRING16_SMALL: u8 = 0x72;
pub const TI_STRING16_LARGE: u8 = 0x73;
pub const TI_PLAIN_SEQUENCE_SMALL: u8 = 0x81;
pub const TI_PLAIN_SEQUENCE_LARGE: u8 = 0x82;
pub const TI_PLAIN_ARRAY_SMALL: u8 = 0x91;
pub const TI_PLAIN_ARRAY_LARGE: u8 = 0x92;
pub const TI_MINIMAL_CONSTRUCTED: u8 = 0xF1;
pub const TI_COMPLETE_CONSTRUCTED: u8 = 0xF2;

/// OMG `XTypes` §7.2.2 Extensibility Kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[repr(u8)]
pub enum ExtensibilityKind {
    Appendable = 1,
    Final = 0,
    Mutable = 2,
}

impl ExtensibilityKind {
    /// Return the wire-format discriminant for this extensibility kind.
    #[inline]
    const fn wire_value(self) -> u8 {
        match self {
            Self::Appendable => return 1,
            Self::Final => return 0,
            Self::Mutable => return 2,
        }
    }
}

impl CdrSerialize for ExtensibilityKind {
    #[inline]
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_u8(self.wire_value());
        return Ok(());
    }
}

impl CdrDeserialize for ExtensibilityKind {
    #[inline]
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        let kind = cdr!(deserializer.deserialize_u8());
        match kind {
            0 => return Ok(Self::Final),
            1 => return Ok(Self::Appendable),
            2 => return Ok(Self::Mutable),
            kind_value => {
                return Err(CdrError::InvalidHeader(format!(
                    "invalid ExtensibilityKind: {kind_value}"
                )));
            }
        }
    }
}

/// OMG `XTypes` §7.3.1 `TypeIdentifier` representation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TypeIdentifier {
    TiCompleteConstructed([u8; 14]),
    TiMinimalConstructed([u8; 14]),
    TiPlainArrayLarge {
        bound: u32,
        element_identifier: Box<Self>,
    },
    TiPlainArraySmall {
        bound: u8,
        element_identifier: Box<Self>,
    },
    TiPlainSequenceLarge {
        bound: u32,
        element_identifier: Box<Self>,
    },
    TiPlainSequenceSmall {
        bound: u8,
        element_identifier: Box<Self>,
    },
    TiString16Large { bound: u32 },
    TiString16Small { bound: u8 },
    TiString8Large { bound: u32 },
    TiString8Small { bound: u8 },
    TkBoolean,
    TkByte,
    TkChar16,
    TkChar8,
    TkFloat128,
    TkFloat32,
    TkFloat64,
    TkInt16,
    TkInt32,
    TkInt64,
    TkNone,
    TkUint16,
    TkUint32,
    TkUint64,
}

impl CdrSerialize for TypeIdentifier {
    #[inline]
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        match self {
            Self::TkNone => serializer.serialize_u8(TK_NONE),
            Self::TkBoolean => serializer.serialize_u8(TK_BOOLEAN),
            Self::TkByte => serializer.serialize_u8(TK_BYTE),
            Self::TkInt16 => serializer.serialize_u8(TK_INT16),
            Self::TkInt32 => serializer.serialize_u8(TK_INT32),
            Self::TkInt64 => serializer.serialize_u8(TK_INT64),
            Self::TkUint16 => serializer.serialize_u8(TK_UINT16),
            Self::TkUint32 => serializer.serialize_u8(TK_UINT32),
            Self::TkUint64 => serializer.serialize_u8(TK_UINT64),
            Self::TkFloat32 => serializer.serialize_u8(TK_FLOAT32),
            Self::TkFloat64 => serializer.serialize_u8(TK_FLOAT64),
            Self::TkFloat128 => serializer.serialize_u8(TK_FLOAT128),
            Self::TkChar8 => serializer.serialize_u8(TK_CHAR8),
            Self::TkChar16 => serializer.serialize_u8(TK_CHAR16),
            Self::TiString8Small { bound } => {
                serializer.serialize_u8(TI_STRING8_SMALL);
                serializer.serialize_u8(*bound);
            }
            Self::TiString8Large { bound } => {
                serializer.serialize_u8(TI_STRING8_LARGE);
                serializer.serialize_u32(*bound);
            }
            Self::TiString16Small { bound } => {
                serializer.serialize_u8(TI_STRING16_SMALL);
                serializer.serialize_u8(*bound);
            }
            Self::TiString16Large { bound } => {
                serializer.serialize_u8(TI_STRING16_LARGE);
                serializer.serialize_u32(*bound);
            }
            Self::TiPlainSequenceSmall {
                bound,
                element_identifier,
            } => {
                serializer.serialize_u8(TI_PLAIN_SEQUENCE_SMALL);
                serializer.serialize_u8(*bound);
                cdr!(element_identifier.serialize(serializer));
            }
            Self::TiPlainSequenceLarge {
                bound,
                element_identifier,
            } => {
                serializer.serialize_u8(TI_PLAIN_SEQUENCE_LARGE);
                serializer.serialize_u32(*bound);
                cdr!(element_identifier.serialize(serializer));
            }
            Self::TiPlainArraySmall {
                bound,
                element_identifier,
            } => {
                serializer.serialize_u8(TI_PLAIN_ARRAY_SMALL);
                serializer.serialize_u8(*bound);
                cdr!(element_identifier.serialize(serializer));
            }
            Self::TiPlainArrayLarge {
                bound,
                element_identifier,
            } => {
                serializer.serialize_u8(TI_PLAIN_ARRAY_LARGE);
                serializer.serialize_u32(*bound);
                cdr!(element_identifier.serialize(serializer));
            }
            Self::TiMinimalConstructed(hash) => {
                serializer.serialize_u8(TI_MINIMAL_CONSTRUCTED);
                for byte in hash {
                    serializer.serialize_u8(*byte);
                }
            }
            Self::TiCompleteConstructed(hash) => {
                serializer.serialize_u8(TI_COMPLETE_CONSTRUCTED);
                for byte in hash {
                    serializer.serialize_u8(*byte);
                }
            }
        }
        return Ok(());
    }
}

impl CdrDeserialize for TypeIdentifier {
    #[inline]
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        let kind = cdr!(deserializer.deserialize_u8());
        match kind {
            TK_NONE => return Ok(Self::TkNone),
            TK_BOOLEAN => return Ok(Self::TkBoolean),
            TK_BYTE => return Ok(Self::TkByte),
            TK_INT16 => return Ok(Self::TkInt16),
            TK_INT32 => return Ok(Self::TkInt32),
            TK_INT64 => return Ok(Self::TkInt64),
            TK_UINT16 => return Ok(Self::TkUint16),
            TK_UINT32 => return Ok(Self::TkUint32),
            TK_UINT64 => return Ok(Self::TkUint64),
            TK_FLOAT32 => return Ok(Self::TkFloat32),
            TK_FLOAT64 => return Ok(Self::TkFloat64),
            TK_FLOAT128 => return Ok(Self::TkFloat128),
            TK_CHAR8 => return Ok(Self::TkChar8),
            TK_CHAR16 => return Ok(Self::TkChar16),
            TI_STRING8_SMALL => {
                return Ok(Self::TiString8Small {
                    bound: cdr!(deserializer.deserialize_u8()),
                });
            }
            TI_STRING8_LARGE => {
                return Ok(Self::TiString8Large {
                    bound: cdr!(deserializer.deserialize_u32()),
                });
            }
            TI_STRING16_SMALL => {
                return Ok(Self::TiString16Small {
                    bound: cdr!(deserializer.deserialize_u8()),
                });
            }
            TI_STRING16_LARGE => {
                return Ok(Self::TiString16Large {
                    bound: cdr!(deserializer.deserialize_u32()),
                });
            }
            TI_PLAIN_SEQUENCE_SMALL => {
                return Ok(Self::TiPlainSequenceSmall {
                    bound: cdr!(deserializer.deserialize_u8()),
                    element_identifier: Box::new(cdr!(Self::deserialize(deserializer))),
                });
            }
            TI_PLAIN_SEQUENCE_LARGE => {
                return Ok(Self::TiPlainSequenceLarge {
                    bound: cdr!(deserializer.deserialize_u32()),
                    element_identifier: Box::new(cdr!(Self::deserialize(deserializer))),
                });
            }
            TI_PLAIN_ARRAY_SMALL => {
                return Ok(Self::TiPlainArraySmall {
                    bound: cdr!(deserializer.deserialize_u8()),
                    element_identifier: Box::new(cdr!(Self::deserialize(deserializer))),
                });
            }
            TI_PLAIN_ARRAY_LARGE => {
                return Ok(Self::TiPlainArrayLarge {
                    bound: cdr!(deserializer.deserialize_u32()),
                    element_identifier: Box::new(cdr!(Self::deserialize(deserializer))),
                });
            }
            TI_MINIMAL_CONSTRUCTED => {
                let mut hash = [u8::default(); 14];
                for byte in &mut hash {
                    *byte = cdr!(deserializer.deserialize_u8());
                }
                return Ok(Self::TiMinimalConstructed(hash));
            }
            TI_COMPLETE_CONSTRUCTED => {
                let mut hash = [u8::default(); 14];
                for byte in &mut hash {
                    *byte = cdr!(deserializer.deserialize_u8());
                }
                return Ok(Self::TiCompleteConstructed(hash));
            }
            kind_value => {
                return Err(CdrError::InvalidHeader(format!(
                    "unknown TypeIdentifier kind: {kind_value}"
                )));
            }
        }
    }
}

/// A member field within a structured `TypeObject`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Member {
    pub is_key: bool,
    pub is_optional: bool,
    pub name: String,
    pub type_id: TypeIdentifier,
}

impl Member {
    /// Create a structure member with the given name, type, and key/optional flags.
    #[must_use]
    pub fn new(name: String, type_id: TypeIdentifier, is_key: bool, is_optional: bool) -> Self {
        Self {
            is_key,
            is_optional,
            name,
            type_id,
        }
    }
}

impl CdrSerialize for Member {
    #[inline]
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_str(&self.name);
        cdr!(self.type_id.serialize(serializer));
        serializer.serialize_bool(self.is_key);
        serializer.serialize_bool(self.is_optional);
        return Ok(());
    }
}

impl CdrDeserialize for Member {
    #[inline]
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        return Ok(Self {
            name: cdr!(deserializer.deserialize_str()),
            type_id: cdr!(TypeIdentifier::deserialize(deserializer)),
            is_key: cdr!(deserializer.deserialize_bool()),
            is_optional: cdr!(deserializer.deserialize_bool()),
        });
    }
}

/// A structured `TypeObject` containing member fields.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct StructureType {
    pub extensibility: ExtensibilityKind,
    pub members: Vec<Member>,
    pub name: String,
}

impl Default for StructureType {
    #[inline]
    fn default() -> Self {
        return Self {
            extensibility: ExtensibilityKind::Final,
            members: Vec::new(),
            name: String::new(),
        };
    }
}

impl StructureType {
    /// Create a structure type with the given name, extensibility, and members.
    #[must_use]
    #[inline]
    pub fn new(name: String, extensibility: ExtensibilityKind, members: Vec<Member>) -> Self {
        return Self {
            extensibility,
            members,
            name,
        };
    }
}

impl CdrSerialize for StructureType {
    #[inline]
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_str(&self.name);
        cdr!(self.extensibility.serialize(serializer));
        let member_count = match u32::try_from(self.members.len()) {
            Ok(count) => count,
            Err(_) => {
                return Err(CdrError::InvalidHeader(
                    "StructureType member count exceeds u32::MAX".to_owned(),
                ));
            }
        };
        serializer.serialize_u32(member_count);
        for member in &self.members {
            cdr!(member.serialize(serializer));
        }
        return Ok(());
    }
}

impl CdrDeserialize for StructureType {
    #[inline]
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        let name = cdr!(deserializer.deserialize_str());
        let extensibility = cdr!(ExtensibilityKind::deserialize(deserializer));
        let len = cdr!(deserializer.deserialize_u32());
        let capacity = match usize::try_from(len) {
            Ok(value) => value,
            Err(_) => {
                return Err(CdrError::InvalidHeader(
                    "StructureType member count exceeds platform usize".to_owned(),
                ));
            }
        };
        let mut members = Vec::with_capacity(capacity);
        for _index in 0..len {
            members.push(cdr!(Member::deserialize(deserializer)));
        }
        return Ok(Self {
            extensibility,
            members,
            name,
        });
    }
}

/// OMG `XTypes` §7.3.2 `TypeObject` definition.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TypeObject {
    Complete(StructureType),
    Minimal(StructureType),
}

impl CdrSerialize for TypeObject {
    #[inline]
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        match self {
            Self::Minimal(structure) => {
                serializer.serialize_u8(0);
                cdr!(structure.serialize(serializer));
            }
            Self::Complete(structure) => {
                serializer.serialize_u8(1);
                cdr!(structure.serialize(serializer));
            }
        }
        return Ok(());
    }
}

impl CdrDeserialize for TypeObject {
    #[inline]
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        let kind = cdr!(deserializer.deserialize_u8());
        match kind {
            0 => {
                return Ok(Self::Minimal(cdr!(StructureType::deserialize(
                    deserializer
                ))));
            }
            1 => {
                return Ok(Self::Complete(cdr!(StructureType::deserialize(
                    deserializer
                ))));
            }
            kind_value => {
                return Err(CdrError::InvalidHeader(format!(
                    "invalid TypeObject kind: {kind_value}"
                )));
            }
        }
    }
}

impl TypeObject {
    /// Compute the `TypeIdentifier` for this `TypeObject` using SHA-256 (first 14 bytes).
    ///
    /// # Errors
    ///
    /// Returns an error if CDR serialization of this type object fails.
    #[inline]
    pub fn get_identifier(&self) -> CdrResult<TypeIdentifier> {
        let bytes = cdr!(dds_cdr::serialize_to_bytes(
            self,
            dds_cdr::Endianness::LittleEndian
        ));
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let hash_result: [u8; 32] = hasher.finalize().into();

        let mut eq_hash = [u8::default(); 14];
        for (dest, &src) in eq_hash.iter_mut().zip(hash_result.iter().take(14)) {
            *dest = src;
        }

        match *self {
            Self::Minimal(_) => return Ok(TypeIdentifier::TiMinimalConstructed(eq_hash)),
            Self::Complete(_) => return Ok(TypeIdentifier::TiCompleteConstructed(eq_hash)),
        }
    }
}

/// OMG `XTypes` §7.6.3 `TypeInformation` container.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct TypeInformation {
    pub type_id: TypeIdentifier,
    pub type_name: String,
}

impl TypeInformation {
    #[must_use]
    #[inline]
    pub fn new(type_name: String, type_id: TypeIdentifier) -> Self {
        return Self { type_id, type_name };
    }
}

impl CdrSerialize for TypeInformation {
    #[inline]
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_str(&self.type_name);
        cdr!(self.type_id.serialize(serializer));
        return Ok(());
    }
}

impl CdrDeserialize for TypeInformation {
    #[inline]
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        return Ok(Self {
            type_name: cdr!(deserializer.deserialize_str()),
            type_id: cdr!(TypeIdentifier::deserialize(deserializer)),
        });
    }
}

/// OMG `XTypes` §7.2.4 compatibility check.
#[must_use]
#[inline]
pub fn is_assignable_from(receiver: &TypeObject, sender: &TypeObject) -> bool {
    let receiver_structure = match receiver {
        TypeObject::Minimal(structure) | TypeObject::Complete(structure) => structure,
    };
    let sender_structure = match sender {
        TypeObject::Minimal(structure) | TypeObject::Complete(structure) => structure,
    };

    if sender_structure.extensibility.wire_value()
        > receiver_structure.extensibility.wire_value()
    {
        return false;
    }

    match receiver_structure.extensibility {
        ExtensibilityKind::Appendable => {
            if sender_structure.members.len() < receiver_structure.members.len() {
                return false;
            }
            for (receiver_member, sender_member) in receiver_structure
                .members
                .iter()
                .zip(sender_structure.members.iter())
            {
                if receiver_member.name != sender_member.name
                    || receiver_member.type_id != sender_member.type_id
                {
                    return false;
                }
            }
            return true;
        }
        ExtensibilityKind::Final => {
            if receiver_structure.members.len() != sender_structure.members.len() {
                return false;
            }
            for (receiver_member, sender_member) in receiver_structure
                .members
                .iter()
                .zip(sender_structure.members.iter())
            {
                if receiver_member.name != sender_member.name
                    || receiver_member.type_id != sender_member.type_id
                {
                    return false;
                }
            }
            return true;
        }
        ExtensibilityKind::Mutable => {
            let sender_map: HashMap<&str, &Member> = sender_structure
                .members
                .iter()
                .map(|member| return (member.name.as_str(), member))
                .collect();
            for receiver_member in &receiver_structure.members {
                if let Some(sender_member) = sender_map.get(receiver_member.name.as_str()) {
                    if receiver_member.type_id != sender_member.type_id {
                        return false;
                    }
                } else if receiver_member.is_key || !receiver_member.is_optional {
                    return false;
                } else {
                    // Optional members may be absent in the sender type.
                }
            }
            return true;
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn test_type_identifier_and_object_serialization() {
        let type_object = TypeObject::Complete(StructureType {
            name: "Position".to_owned(),
            extensibility: ExtensibilityKind::Appendable,
            members: vec![Member {
                name: "x".to_owned(),
                type_id: TypeIdentifier::TkInt32,
                is_key: true,
                is_optional: false,
            }],
        });

        let type_id = match type_object.get_identifier() {
            Ok(identifier) => identifier,
            Err(error) => panic!("get_identifier failed: {error}"),
        };
        match type_id {
            TypeIdentifier::TiCompleteConstructed(_hash) => {}
            _ => panic!("Expected complete hashed identifier"),
        }
    }
}
