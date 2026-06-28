//! # dds-xtypes — DDS `XTypes` 1.3 Type System
//!
//! Implements the Extensible and Dynamic Topic Types specification:
//! `TypeObject`, `TypeIdentifier`, type compatibility rules, and extensibility.
//!
//! Reference: `XTypes` §7

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
    reason = "DDS Extensible Types implementation requires standard library conversions, standard returns, and type system representations."
)]

use dds_cdr::{CdrDeserialize, CdrDeserializer, CdrResult, CdrSerialize, CdrSerializer};
use sha2::{Digest as _, Sha256};
use std::collections::HashMap;

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

/// OMG `XTypes` §7.3.1 `TypeIdentifier` representation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeIdentifier {
    Primitive(String),
    Hash([u8; 32]),
}

impl CdrSerialize for TypeIdentifier {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        match self {
            Self::Primitive(s) => {
                serializer.serialize_u8(0);
                serializer.serialize_str(s);
            }
            Self::Hash(h) => {
                serializer.serialize_u8(1);
                for &b in h {
                    serializer.serialize_u8(b);
                }
            }
        }
        Ok(())
    }
}

impl CdrDeserialize for TypeIdentifier {
    fn deserialize(deserializer: &mut CdrDeserializer) -> CdrResult<Self> {
        match deserializer.deserialize_u8()? {
            0 => Ok(Self::Primitive(deserializer.deserialize_str()?)),
            1 => {
                let mut h = [0u8; 32];
                for item in &mut h {
                    *item = deserializer.deserialize_u8()?;
                }
                Ok(Self::Hash(h))
            }
            v => Err(dds_cdr::CdrError::InvalidHeader(format!(
                "invalid TypeIdentifier kind: {v}"
            ))),
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
    fn deserialize(deserializer: &mut CdrDeserializer) -> CdrResult<Self> {
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
    fn deserialize(deserializer: &mut CdrDeserializer) -> CdrResult<Self> {
        let name = deserializer.deserialize_str()?;
        let extensibility = ExtensibilityKind::deserialize(deserializer)?;
        let len = deserializer.deserialize_u32()?;
        let mut members = Vec::with_capacity(len as usize);
        for _ in 0..len {
            members.push(Member::deserialize(deserializer)?);
        }
        Ok(Self {
            name,
            extensibility,
            members,
        })
    }
}

/// OMG `XTypes` §7.3.2 `TypeObject` definition
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeObject {
    Primitive(String),
    Structure(StructureType),
}

impl CdrSerialize for TypeObject {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        match self {
            Self::Primitive(s) => {
                serializer.serialize_u8(0);
                serializer.serialize_str(s);
            }
            Self::Structure(s) => {
                serializer.serialize_u8(1);
                s.serialize(serializer)?;
            }
        }
        Ok(())
    }
}

impl CdrDeserialize for TypeObject {
    fn deserialize(deserializer: &mut CdrDeserializer) -> CdrResult<Self> {
        match deserializer.deserialize_u8()? {
            0 => Ok(Self::Primitive(deserializer.deserialize_str()?)),
            1 => Ok(Self::Structure(StructureType::deserialize(deserializer)?)),
            v => Err(dds_cdr::CdrError::InvalidHeader(format!(
                "invalid TypeObject kind: {v}"
            ))),
        }
    }
}

impl TypeObject {
    /// Compute the `TypeIdentifier` for this `TypeObject` using SHA-256.
    #[must_use]
    pub fn get_identifier(&self) -> TypeIdentifier {
        match self {
            Self::Primitive(s) => TypeIdentifier::Primitive(s.clone()),
            Self::Structure(_) => {
                let bytes =
                    dds_cdr::serialize_to_bytes(self, dds_cdr::Endianness::LittleEndian).unwrap();
                let mut hasher = Sha256::new();
                hasher.update(&bytes);
                let hash_result: [u8; 32] = hasher.finalize().into();
                TypeIdentifier::Hash(hash_result)
            }
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
    fn deserialize(deserializer: &mut CdrDeserializer) -> CdrResult<Self> {
        Ok(Self {
            type_name: deserializer.deserialize_str()?,
            type_id: TypeIdentifier::deserialize(deserializer)?,
        })
    }
}

/// OMG `XTypes` §7.2.4 compatibility check: check if receiver can assign a payload from sender
#[must_use]
pub fn is_assignable_from(receiver: &TypeObject, sender: &TypeObject) -> bool {
    if receiver == sender {
        return true;
    }

    match (receiver, sender) {
        (TypeObject::Primitive(r_prim), TypeObject::Primitive(s_prim)) => r_prim == s_prim,
        (TypeObject::Structure(r_struct), TypeObject::Structure(s_struct)) => {
            // Offered (sender) extensibility must be <= requested (receiver) extensibility
            if (s_struct.extensibility as u8) > (r_struct.extensibility as u8) {
                return false;
            }

            match r_struct.extensibility {
                ExtensibilityKind::Final => {
                    // Must be structurally identical (same members in same order)
                    if r_struct.members.len() != s_struct.members.len() {
                        return false;
                    }
                    for (r_m, s_m) in r_struct.members.iter().zip(s_struct.members.iter()) {
                        if r_m.name != s_m.name || r_m.type_id != s_m.type_id {
                            return false;
                        }
                    }
                    true
                }
                ExtensibilityKind::Appendable => {
                    // Sender must contain all receiver members at the start in same order.
                    if s_struct.members.len() < r_struct.members.len() {
                        return false;
                    }
                    for (r_m, s_m) in r_struct.members.iter().zip(s_struct.members.iter()) {
                        if r_m.name != s_m.name || r_m.type_id != s_m.type_id {
                            return false;
                        }
                    }
                    true
                }
                ExtensibilityKind::Mutable => {
                    // Matched by name.
                    let sender_map: HashMap<&str, &Member> = s_struct
                        .members
                        .iter()
                        .map(|m| (m.name.as_str(), m))
                        .collect();

                    for r_m in &r_struct.members {
                        if let Some(s_m) = sender_map.get(r_m.name.as_str()) {
                            if r_m.type_id != s_m.type_id {
                                return false;
                            }
                        } else {
                            // Missing in sender is only allowed if it is optional & not a key in receiver
                            if r_m.is_key || !r_m.is_optional {
                                return false;
                            }
                        }
                    }
                    true
                }
            }
        }
        _ => false,
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_identifier_and_object_serialization() {
        let prim = TypeObject::Primitive("long".to_string());
        let prim_id = prim.get_identifier();
        assert_eq!(prim_id, TypeIdentifier::Primitive("long".to_string()));

        let s = TypeObject::Structure(StructureType {
            name: "Position".to_string(),
            extensibility: ExtensibilityKind::Appendable,
            members: vec![
                Member {
                    name: "x".to_string(),
                    type_id: TypeIdentifier::Primitive("long".to_string()),
                    is_key: true,
                    is_optional: false,
                },
                Member {
                    name: "y".to_string(),
                    type_id: TypeIdentifier::Primitive("long".to_string()),
                    is_key: false,
                    is_optional: true,
                },
            ],
        });

        let s_id = s.get_identifier();
        if let TypeIdentifier::Hash(_) = s_id {
            // Hashing succeeded
        } else {
            panic!("Expected hashed identifier");
        }
    }

    #[test]
    fn test_is_assignable_from_rules() {
        let member_x = Member {
            name: "x".to_string(),
            type_id: TypeIdentifier::Primitive("long".to_string()),
            is_key: true,
            is_optional: false,
        };
        let member_y = Member {
            name: "y".to_string(),
            type_id: TypeIdentifier::Primitive("long".to_string()),
            is_key: false,
            is_optional: false,
        };

        // Final Structs
        let r_final = TypeObject::Structure(StructureType {
            name: "Point".to_string(),
            extensibility: ExtensibilityKind::Final,
            members: vec![member_x.clone(), member_y.clone()],
        });

        let s_final_identical = r_final.clone();
        assert!(is_assignable_from(&r_final, &s_final_identical));

        let s_final_extra = TypeObject::Structure(StructureType {
            name: "Point".to_string(),
            extensibility: ExtensibilityKind::Final,
            members: vec![
                member_x.clone(),
                member_y.clone(),
                Member {
                    name: "z".to_string(),
                    type_id: TypeIdentifier::Primitive("long".to_string()),
                    is_key: false,
                    is_optional: false,
                },
            ],
        });
        assert!(!is_assignable_from(&r_final, &s_final_extra));

        // Appendable Structs
        let r_appendable = TypeObject::Structure(StructureType {
            name: "Point".to_string(),
            extensibility: ExtensibilityKind::Appendable,
            members: vec![member_x.clone(), member_y.clone()],
        });
        let s_appendable_extra = TypeObject::Structure(StructureType {
            name: "Point".to_string(),
            extensibility: ExtensibilityKind::Appendable,
            members: vec![
                member_x.clone(),
                member_y.clone(),
                Member {
                    name: "z".to_string(),
                    type_id: TypeIdentifier::Primitive("long".to_string()),
                    is_key: false,
                    is_optional: false,
                },
            ],
        });
        assert!(is_assignable_from(&r_appendable, &s_appendable_extra));

        // Mutable Structs
        let r_mutable = TypeObject::Structure(StructureType {
            name: "Point".to_string(),
            extensibility: ExtensibilityKind::Mutable,
            members: vec![
                member_x.clone(),
                Member {
                    name: "z".to_string(),
                    type_id: TypeIdentifier::Primitive("long".to_string()),
                    is_key: false,
                    is_optional: true, // optional member missing in sender is ok
                },
            ],
        });
        let s_mutable = TypeObject::Structure(StructureType {
            name: "Point".to_string(),
            extensibility: ExtensibilityKind::Mutable,
            members: vec![member_x.clone()],
        });
        assert!(is_assignable_from(&r_mutable, &s_mutable));
    }
}
