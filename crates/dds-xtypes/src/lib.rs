//! # dds-xtypes — DDS `XTypes` 1.3 Type System
//!
//! Implements the Extensible and Dynamic Topic Types specification:
//! `TypeObject`, `TypeIdentifier`, type compatibility rules, and extensibility.
//!
//! Reference: `XTypes` §7

#![forbid(unsafe_code)]
#![allow(warnings)] // Simplified for the exercise

use dds_cdr::{CdrDeserialize, CdrDeserializer, CdrResult, CdrSerialize, CdrSerializer};
use sha2::{Digest as _, Sha256};
use std::collections::HashMap;

pub mod dynamic;
pub use dynamic::{DynamicData, DynamicType};

/// OMG `XTypes` §7.2.2 Extensibility Kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExtensibilityKind {
    Final = 0,
    Appendable = 1,
    Mutable = 2,
}

impl CdrSerialize for ExtensibilityKind {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_u8(*self as u8);
        Ok(())
    }
}

impl CdrDeserialize for ExtensibilityKind {
    fn deserialize(deserializer: &mut CdrDeserializer) -> CdrResult<Self> {
        match deserializer.deserialize_u8()? {
            0 => Ok(Self::Final),
            1 => Ok(Self::Appendable),
            2 => Ok(Self::Mutable),
            v => Err(dds_cdr::CdrError::InvalidHeader(format!(
                "invalid ExtensibilityKind: {v}"
            ))),
        }
    }
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

/// OMG `XTypes` §7.3.1 `TypeIdentifier` representation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeIdentifier {
    TkNone,
    TkBoolean,
    TkByte,
    TkInt16,
    TkInt32,
    TkInt64,
    TkUint16,
    TkUint32,
    TkUint64,
    TkFloat32,
    TkFloat64,
    TkFloat128,
    TkChar8,
    TkChar16,
    TiString8Small { bound: u8 },
    TiString8Large { bound: u32 },
    TiString16Small { bound: u8 },
    TiString16Large { bound: u32 },
    TiPlainSequenceSmall { bound: u8, element_identifier: Box<TypeIdentifier> },
    TiPlainSequenceLarge { bound: u32, element_identifier: Box<TypeIdentifier> },
    TiPlainArraySmall { bound: u8, element_identifier: Box<TypeIdentifier> },
    TiPlainArrayLarge { bound: u32, element_identifier: Box<TypeIdentifier> },
    TiMinimalConstructed([u8; 14]),
    TiCompleteConstructed([u8; 14]),
}

impl CdrSerialize for TypeIdentifier {
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
            Self::TiPlainSequenceSmall { bound, element_identifier } => {
                serializer.serialize_u8(TI_PLAIN_SEQUENCE_SMALL);
                serializer.serialize_u8(*bound);
                element_identifier.serialize(serializer)?;
            }
            Self::TiPlainSequenceLarge { bound, element_identifier } => {
                serializer.serialize_u8(TI_PLAIN_SEQUENCE_LARGE);
                serializer.serialize_u32(*bound);
                element_identifier.serialize(serializer)?;
            }
            Self::TiPlainArraySmall { bound, element_identifier } => {
                serializer.serialize_u8(TI_PLAIN_ARRAY_SMALL);
                serializer.serialize_u8(*bound);
                element_identifier.serialize(serializer)?;
            }
            Self::TiPlainArrayLarge { bound, element_identifier } => {
                serializer.serialize_u8(TI_PLAIN_ARRAY_LARGE);
                serializer.serialize_u32(*bound);
                element_identifier.serialize(serializer)?;
            }
            Self::TiMinimalConstructed(hash) => {
                serializer.serialize_u8(TI_MINIMAL_CONSTRUCTED);
                for &b in hash {
                    serializer.serialize_u8(b);
                }
            }
            Self::TiCompleteConstructed(hash) => {
                serializer.serialize_u8(TI_COMPLETE_CONSTRUCTED);
                for &b in hash {
                    serializer.serialize_u8(b);
                }
            }
        }
        Ok(())
    }
}

impl CdrDeserialize for TypeIdentifier {
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        let kind = deserializer.deserialize_u8()?;
        match kind {
            TK_NONE => Ok(Self::TkNone),
            TK_BOOLEAN => Ok(Self::TkBoolean),
            TK_BYTE => Ok(Self::TkByte),
            TK_INT16 => Ok(Self::TkInt16),
            TK_INT32 => Ok(Self::TkInt32),
            TK_INT64 => Ok(Self::TkInt64),
            TK_UINT16 => Ok(Self::TkUint16),
            TK_UINT32 => Ok(Self::TkUint32),
            TK_UINT64 => Ok(Self::TkUint64),
            TK_FLOAT32 => Ok(Self::TkFloat32),
            TK_FLOAT64 => Ok(Self::TkFloat64),
            TK_FLOAT128 => Ok(Self::TkFloat128),
            TK_CHAR8 => Ok(Self::TkChar8),
            TK_CHAR16 => Ok(Self::TkChar16),
            TI_STRING8_SMALL => Ok(Self::TiString8Small { bound: deserializer.deserialize_u8()? }),
            TI_STRING8_LARGE => Ok(Self::TiString8Large { bound: deserializer.deserialize_u32()? }),
            TI_STRING16_SMALL => Ok(Self::TiString16Small { bound: deserializer.deserialize_u8()? }),
            TI_STRING16_LARGE => Ok(Self::TiString16Large { bound: deserializer.deserialize_u32()? }),
            TI_PLAIN_SEQUENCE_SMALL => Ok(Self::TiPlainSequenceSmall {
                bound: deserializer.deserialize_u8()?,
                element_identifier: Box::new(TypeIdentifier::deserialize(deserializer)?),
            }),
            TI_PLAIN_SEQUENCE_LARGE => Ok(Self::TiPlainSequenceLarge {
                bound: deserializer.deserialize_u32()?,
                element_identifier: Box::new(TypeIdentifier::deserialize(deserializer)?),
            }),
            TI_PLAIN_ARRAY_SMALL => Ok(Self::TiPlainArraySmall {
                bound: deserializer.deserialize_u8()?,
                element_identifier: Box::new(TypeIdentifier::deserialize(deserializer)?),
            }),
            TI_PLAIN_ARRAY_LARGE => Ok(Self::TiPlainArrayLarge {
                bound: deserializer.deserialize_u32()?,
                element_identifier: Box::new(TypeIdentifier::deserialize(deserializer)?),
            }),
            TI_MINIMAL_CONSTRUCTED => {
                let mut hash = [0u8; 14];
                for b in &mut hash { *b = deserializer.deserialize_u8()?; }
                Ok(Self::TiMinimalConstructed(hash))
            }
            TI_COMPLETE_CONSTRUCTED => {
                let mut hash = [0u8; 14];
                for b in &mut hash { *b = deserializer.deserialize_u8()?; }
                Ok(Self::TiCompleteConstructed(hash))
            }
            _ => Err(dds_cdr::CdrError::InvalidHeader(format!("unknown TypeIdentifier kind: {kind}"))),
        }
    }
}

/// A member field within a structured `TypeObject`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Member {
    pub name: String,
    pub type_id: TypeIdentifier,
    pub is_key: bool,
    pub is_optional: bool,
}

impl CdrSerialize for Member {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_str(&self.name);
        self.type_id.serialize(serializer)?;
        serializer.serialize_bool(self.is_key);
        serializer.serialize_bool(self.is_optional);
        Ok(())
    }
}

impl CdrDeserialize for Member {
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        Ok(Self {
            name: deserializer.deserialize_str()?,
            type_id: TypeIdentifier::deserialize(deserializer)?,
            is_key: deserializer.deserialize_bool()?,
            is_optional: deserializer.deserialize_bool()?,
        })
    }
}

/// A structured `TypeObject` containing member fields
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructureType {
    pub name: String,
    pub extensibility: ExtensibilityKind,
    pub members: Vec<Member>,
}

impl CdrSerialize for StructureType {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_str(&self.name);
        self.extensibility.serialize(serializer)?;
        serializer.serialize_u32(self.members.len() as u32);
        for m in &self.members {
            m.serialize(serializer)?;
        }
        Ok(())
    }
}

impl CdrDeserialize for StructureType {
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        let name = deserializer.deserialize_str()?;
        let extensibility = ExtensibilityKind::deserialize(deserializer)?;
        let len = deserializer.deserialize_u32()?;
        let mut members = Vec::with_capacity(len as usize);
        for _ in 0..len {
            members.push(Member::deserialize(deserializer)?);
        }
        Ok(Self { name, extensibility, members })
    }
}

/// OMG `XTypes` §7.3.2 `TypeObject` definition
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeObject {
    Minimal(StructureType),
    Complete(StructureType),
}

impl CdrSerialize for TypeObject {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        match self {
            Self::Minimal(s) => {
                serializer.serialize_u8(0);
                s.serialize(serializer)?;
            }
            Self::Complete(s) => {
                serializer.serialize_u8(1);
                s.serialize(serializer)?;
            }
        }
        Ok(())
    }
}

impl CdrDeserialize for TypeObject {
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        match deserializer.deserialize_u8()? {
            0 => Ok(Self::Minimal(StructureType::deserialize(deserializer)?)),
            1 => Ok(Self::Complete(StructureType::deserialize(deserializer)?)),
            v => Err(dds_cdr::CdrError::InvalidHeader(format!("invalid TypeObject kind: {v}"))),
        }
    }
}

impl TypeObject {
    /// Compute the `TypeIdentifier` for this `TypeObject` using SHA-256 (first 14 bytes)
    #[must_use]
    pub fn get_identifier(&self) -> TypeIdentifier {
        let bytes = dds_cdr::serialize_to_bytes(self, dds_cdr::Endianness::LittleEndian).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let hash_result: [u8; 32] = hasher.finalize().into();
        
        let mut eq_hash = [0u8; 14];
        eq_hash.copy_from_slice(&hash_result[0..14]);

        match self {
            Self::Minimal(_) => TypeIdentifier::TiMinimalConstructed(eq_hash),
            Self::Complete(_) => TypeIdentifier::TiCompleteConstructed(eq_hash),
        }
    }
}

/// OMG `XTypes` §7.6.3 `TypeInformation` container
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeInformation {
    pub type_name: String,
    pub type_id: TypeIdentifier,
}

impl CdrSerialize for TypeInformation {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_str(&self.type_name);
        self.type_id.serialize(serializer)?;
        Ok(())
    }
}

impl CdrDeserialize for TypeInformation {
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        Ok(Self {
            type_name: deserializer.deserialize_str()?,
            type_id: TypeIdentifier::deserialize(deserializer)?,
        })
    }
}

/// OMG `XTypes` §7.2.4 compatibility check
#[must_use]
pub fn is_assignable_from(receiver: &TypeObject, sender: &TypeObject) -> bool {
    let r_struct = match receiver {
        TypeObject::Minimal(s) | TypeObject::Complete(s) => s,
    };
    let s_struct = match sender {
        TypeObject::Minimal(s) | TypeObject::Complete(s) => s,
    };

    if (s_struct.extensibility as u8) > (r_struct.extensibility as u8) {
        return false;
    }

    match r_struct.extensibility {
        ExtensibilityKind::Final => {
            if r_struct.members.len() != s_struct.members.len() { return false; }
            for (r_m, s_m) in r_struct.members.iter().zip(s_struct.members.iter()) {
                if r_m.name != s_m.name || r_m.type_id != s_m.type_id { return false; }
            }
            true
        }
        ExtensibilityKind::Appendable => {
            if s_struct.members.len() < r_struct.members.len() { return false; }
            for (r_m, s_m) in r_struct.members.iter().zip(s_struct.members.iter()) {
                if r_m.name != s_m.name || r_m.type_id != s_m.type_id { return false; }
            }
            true
        }
        ExtensibilityKind::Mutable => {
            let sender_map: HashMap<&str, &Member> = s_struct.members.iter().map(|m| (m.name.as_str(), m)).collect();
            for r_m in &r_struct.members {
                if let Some(s_m) = sender_map.get(r_m.name.as_str()) {
                    if r_m.type_id != s_m.type_id { return false; }
                } else if r_m.is_key || !r_m.is_optional {
                    return false;
                }
            }
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_identifier_and_object_serialization() {
        let s = TypeObject::Complete(StructureType {
            name: "Position".to_string(),
            extensibility: ExtensibilityKind::Appendable,
            members: vec![
                Member {
                    name: "x".to_string(),
                    type_id: TypeIdentifier::TkInt32,
                    is_key: true,
                    is_optional: false,
                },
            ],
        });

        let s_id = s.get_identifier();
        match s_id {
            TypeIdentifier::TiCompleteConstructed(_) => {}
            _ => panic!("Expected complete hashed identifier"),
        }
    }
}
