//! IDL to Rust code generator.

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
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unnecessary_debug_formatting,
    clippy::use_debug,
    reason = "IDL compiler CLI tool requires IO operations, print debugging, standard returns, and CLI arguments."
)]

use dds_idl::{parse_idl, AstNode, EnumDef, IdlType, PrimitiveType, StructDef, UnionDef, BitmaskDef, ConstDef};
use std::env;
use std::fs;
use std::path::Path;

const fn primitive_to_rust(prim: &PrimitiveType) -> &str {
    match prim {
        PrimitiveType::Int16 => "i16",
        PrimitiveType::Int32 => "i32",
        PrimitiveType::Int64 => "i64",
        PrimitiveType::Uint16 => "u16",
        PrimitiveType::Uint32 => "u32",
        PrimitiveType::Uint64 => "u64",
        PrimitiveType::Float => "f32",
        PrimitiveType::Double => "f64",
        PrimitiveType::Boolean => "bool",
        PrimitiveType::Char => "char",
        PrimitiveType::String => "String",
    }
}

fn idl_type_to_rust(idl_type: &IdlType) -> String {
    match idl_type {
        IdlType::Primitive(prim) => primitive_to_rust(prim).to_owned(),
        IdlType::Sequence(nested) => format!("Vec<{}>", idl_type_to_rust(nested)),
        IdlType::Array(nested, size) => format!("[{}; {}]", idl_type_to_rust(nested), size),
        IdlType::Map(key, value) => format!(
            "std::collections::HashMap<{}, {}>",
            primitive_to_rust(key),
            idl_type_to_rust(value)
        ),
    }
}

fn generate_member_serialize(prefix: &str, name: &str, idl_type: &IdlType) -> String {
    match idl_type {
        IdlType::Primitive(prim) => match prim {
            PrimitiveType::Int16 => format!("        serializer.serialize_i16({prefix}{name});"),
            PrimitiveType::Int32 => format!("        serializer.serialize_i32({prefix}{name});"),
            PrimitiveType::Int64 => format!("        serializer.serialize_i64({prefix}{name});"),
            PrimitiveType::Uint16 => format!("        serializer.serialize_u16({prefix}{name});"),
            PrimitiveType::Uint32 => format!("        serializer.serialize_u32({prefix}{name});"),
            PrimitiveType::Uint64 => format!("        serializer.serialize_u64({prefix}{name});"),
            PrimitiveType::Float => format!("        serializer.serialize_f32({prefix}{name});"),
            PrimitiveType::Double => format!("        serializer.serialize_f64({prefix}{name});"),
            PrimitiveType::Boolean => format!("        serializer.serialize_bool({prefix}{name});"),
            PrimitiveType::Char => {
                format!("        serializer.serialize_u8({prefix}{name} as u8);")
            }
            PrimitiveType::String => format!("        serializer.serialize_str(&{prefix}{name});"),
        },
        IdlType::Sequence(nested) => {
            let nested_ser = generate_member_serialize("", "item", nested);
            format!(
                "        serializer.serialize_u32({prefix}{name}.len() as u32);
        for item in &{prefix}{name} {{
            {nested_ser}
        }}"
            )
        }
        IdlType::Array(nested, _) => {
            let nested_ser = generate_member_serialize("", "item", nested);
            format!(
                "        for item in &{prefix}{name} {{
            {nested_ser}
        }}"
            )
        }
        IdlType::Map(key_prim, value_type) => {
            let key_ser = generate_member_serialize("", "k", &IdlType::Primitive(key_prim.clone()));
            let val_ser = generate_member_serialize("", "v", value_type);
            format!(
                "        serializer.serialize_u32({prefix}{name}.len() as u32);
        for (k, v) in &{prefix}{name} {{
            {key_ser}
            {val_ser}
        }}"
            )
        }
    }
}

fn generate_member_deserialize_statement(name: &str, idl_type: &IdlType) -> String {
    match idl_type {
        IdlType::Primitive(prim) => match prim {
            PrimitiveType::Int16 => {
                format!("        let {name} = deserializer.deserialize_i16()?;")
            }
            PrimitiveType::Int32 => {
                format!("        let {name} = deserializer.deserialize_i32()?;")
            }
            PrimitiveType::Int64 => {
                format!("        let {name} = deserializer.deserialize_i64()?;")
            }
            PrimitiveType::Uint16 => {
                format!("        let {name} = deserializer.deserialize_u16()?;")
            }
            PrimitiveType::Uint32 => {
                format!("        let {name} = deserializer.deserialize_u32()?;")
            }
            PrimitiveType::Uint64 => {
                format!("        let {name} = deserializer.deserialize_u64()?;")
            }
            PrimitiveType::Float => {
                format!("        let {name} = deserializer.deserialize_f32()?;")
            }
            PrimitiveType::Double => {
                format!("        let {name} = deserializer.deserialize_f64()?;")
            }
            PrimitiveType::Boolean => {
                format!("        let {name} = deserializer.deserialize_bool()?;")
            }
            PrimitiveType::Char => {
                format!("        let {name} = deserializer.deserialize_u8().map(|v| v as char)?;")
            }
            PrimitiveType::String => {
                format!("        let {name} = deserializer.deserialize_str()?;")
            }
        },
        IdlType::Sequence(nested) => {
            let item_deser = generate_member_deserialize_statement("item", nested);
            format!(
                "        let {name}_len = deserializer.deserialize_u32()?;
        let mut {name} = Vec::with_capacity({name}_len as usize);
        for _ in 0..{name}_len {{
            {item_deser}
            {name}.push(item);
        }}"
            )
        }
        IdlType::Array(nested, size) => {
            let item_deser = generate_member_deserialize_statement("item", nested);
            format!(
                r#"        let mut {name}_vec = Vec::with_capacity({size});
        for _ in 0..{size} {{
            {item_deser}
            {name}_vec.push(item);
        }}
        let {name} = {name}_vec.try_into().map_err(|_| dds_cdr::CdrError::InvalidHeader("array size mismatch".into()))?;"#
            )
        }
        IdlType::Map(key_prim, value_type) => {
            let key_deser =
                generate_member_deserialize_statement("key", &IdlType::Primitive(key_prim.clone()));
            let val_deser = generate_member_deserialize_statement("val", value_type);
            format!(
                "        let {name}_len = deserializer.deserialize_u32()?;
        let mut {name} = std::collections::HashMap::with_capacity({name}_len as usize);
        for _ in 0..{name}_len {{
            {key_deser}
            {val_deser}
            {name}.insert(key, val);
        }}"
            )
        }
    }
}

/// Generates complete Rust source code representing the parsed IDL structs.
#[must_use] 
pub fn generate_rust_struct(struct_def: &StructDef) -> String {
    let struct_name = &struct_def.name;

    // Build struct fields
    let mut fields = String::new();
    for member in &struct_def.members {
        fields.push_str(&format!(
            "    pub {}: {},\n",
            member.name,
            idl_type_to_rust(&member.field_type)
        ));
    }

    // Build serialization steps
    let mut ser_body = String::new();
    for member in &struct_def.members {
        ser_body.push_str(&generate_member_serialize(
            "self.",
            &member.name,
            &member.field_type,
        ));
        ser_body.push('\n');
    }

    // Build deserialization steps
    let mut deser_statements = String::new();
    for member in &struct_def.members {
        deser_statements.push_str(&generate_member_deserialize_statement(
            &member.name,
            &member.field_type,
        ));
        deser_statements.push('\n');
    }

    let mut deser_fields = String::new();
    for member in &struct_def.members {
        deser_fields.push_str(&format!("            {},\n", member.name));
    }

    // Check annotations for @key attributes
    let has_key = struct_def
        .members
        .iter()
        .any(|m| m.annotations.iter().any(|a| a.name == "key"));
    let get_key_body = if has_key {
        let mut key_ser = String::new();
        for m in &struct_def.members {
            if m.annotations.iter().any(|a| a.name == "key") {
                key_ser.push_str(&format!("            // serialize key field {}\n", m.name));
                key_ser.push_str(&generate_member_serialize("val.", &m.name, &m.field_type));
                key_ser.push('\n');
            }
        }
        format!(
            r#"        if let Some(val) = value.downcast_ref::<{struct_name}>() {{
            let mut serializer = dds_cdr::CdrSerializer::new(dds_cdr::Endianness::LittleEndian);
{key_ser}
            Ok(dds_types::instance::InstanceHandle::new(&serializer.into_bytes()))
        }} else {{
            Err(dds_types::return_code::DdsError::BadParameter("cast to {struct_name} failed".into()))
        }}"#
        )
    } else {
        "        Ok(dds_types::instance::InstanceHandle::NIL)".to_owned()
    };

    format!(
        r#"// Generated automatically by dds-idlc. Do not edit manually.

#[derive(Debug, Clone, PartialEq)]
pub struct {struct_name} {{
{fields}}}

impl dds_cdr::CdrSerialize for {struct_name} {{
    fn serialize(&self, serializer: &mut dds_cdr::CdrSerializer) -> dds_cdr::CdrResult<()> {{
{ser_body}        Ok(())
    }}
}}

impl dds_cdr::CdrDeserialize for {struct_name} {{
    fn deserialize(deserializer: &mut dds_cdr::CdrDeserializer) -> dds_cdr::CdrResult<Self> {{
{deser_statements}        Ok(Self {{
{deser_fields}        }})
    }}
}}

pub struct {struct_name}TypeSupport;

impl dds_core::TypeSupport for {struct_name}TypeSupport {{
    fn get_type_name(&self) -> &str {{
        "{struct_name}"
    }}

    fn serialize(&self, value: &dyn std::any::Any) -> dds_types::return_code::DdsResult<Vec<u8>> {{
        if let Some(val) = value.downcast_ref::<{struct_name}>() {{
            let bytes = dds_cdr::serialize_to_bytes(val, dds_cdr::Endianness::LittleEndian)
                .map_err(|e| dds_types::return_code::DdsError::Error(e.to_string()))?;
            Ok(bytes.to_vec())
        }} else {{
            Err(dds_types::return_code::DdsError::BadParameter("cast to {struct_name} failed".into()))
        }}
    }}

    fn deserialize(&self, bytes: &[u8]) -> dds_types::return_code::DdsResult<Box<dyn std::any::Any>> {{
        let val: {struct_name} = dds_cdr::deserialize_from_slice(bytes, dds_cdr::Endianness::LittleEndian)
            .map_err(|e| dds_types::return_code::DdsError::Error(e.to_string()))?;
        Ok(Box::new(val))
    }}

    fn get_key_hash(&self, value: &dyn std::any::Any) -> dds_types::return_code::DdsResult<dds_types::instance::InstanceHandle> {{
{get_key_body}
    }}
}}
"#
    )
}

/// Generates Rust representation for an IDL Enum.
#[must_use] 
pub fn generate_rust_enum(enum_def: &EnumDef) -> String {
    let name = &enum_def.name;
    let mut variants = String::new();
    for v in &enum_def.variants {
        variants.push_str(&format!("    {v},\n"));
    }

    let mut deser_match = String::new();
    for (i, v) in enum_def.variants.iter().enumerate() {
        deser_match.push_str(&format!("            {i} => Ok(Self::{v}),\n"));
    }

    format!(
        r#"#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum {name} {{
{variants}}}

impl dds_cdr::CdrSerialize for {name} {{
    fn serialize(&self, serializer: &mut dds_cdr::CdrSerializer) -> dds_cdr::CdrResult<()> {{
        serializer.serialize_u32(*self as u32);
        Ok(())
    }}
}}

impl dds_cdr::CdrDeserialize for {name} {{
    fn deserialize(deserializer: &mut dds_cdr::CdrDeserializer) -> dds_cdr::CdrResult<Self> {{
        let val = deserializer.deserialize_u32()?;
        match val {{
{deser_match}            _ => Err(dds_cdr::CdrError::InvalidHeader(format!("invalid enum variant: {{val}}"))),
        }}
    }}
}}
"#
    )
}

/// Generates Rust representation for an IDL Union.
#[must_use] 
pub fn generate_rust_union(union_def: &UnionDef) -> String {
    let name = &union_def.name;

    let mut variants = String::new();
    for case in &union_def.cases {
        let rust_type = idl_type_to_rust(&case.member.field_type);
        let case_name = &case.member.name;
        // Capitalize variant name
        let mut chars = case_name.chars();
        let cap_name = match chars.next() {
            None => String::new(),
            Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        };
        variants.push_str(&format!("    {cap_name}({rust_type}),\n"));
    }

    format!(
        "#[derive(Debug, Clone, PartialEq)]
pub enum {name} {{
{variants}}}
"
    )
}

/// Generates Rust representation for an IDL Bitmask.
#[must_use]
pub fn generate_rust_bitmask(bitmask_def: &BitmaskDef) -> String {
    let name = &bitmask_def.name;
    let mut flags = String::new();
    for (i, flag) in bitmask_def.flags.iter().enumerate() {
        flags.push_str(&format!("        const {flag} = 1 << {i};\n"));
    }

    format!(
        r#"bitflags::bitflags! {{
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct {name}: u32 {{
{flags}    }}
}}

impl dds_cdr::CdrSerialize for {name} {{
    fn serialize(&self, serializer: &mut dds_cdr::CdrSerializer) -> dds_cdr::CdrResult<()> {{
        serializer.serialize_u32(self.bits());
        Ok(())
    }}
}}

impl dds_cdr::CdrDeserialize for {name} {{
    fn deserialize(deserializer: &mut dds_cdr::CdrDeserializer) -> dds_cdr::CdrResult<Self> {{
        let val = deserializer.deserialize_u32()?;
        Self::from_bits(val).ok_or_else(|| dds_cdr::CdrError::InvalidHeader(format!("invalid bitmask: {{val}}")))
    }}
}}
"#
    )
}

/// Generates Rust representation for an IDL Const.
#[must_use]
pub fn generate_rust_const(const_def: &ConstDef) -> String {
    let name = &const_def.name;
    let rust_type = primitive_to_rust(&const_def.const_type);
    let value = &const_def.value;
    format!("pub const {name}: {rust_type} = {value};\n")
}

/// Generates Rust content recursively from `AstNode` trees.
#[must_use] 
pub fn generate_rust_node(node: &AstNode) -> String {
    match node {
        AstNode::Struct(s) => generate_rust_struct(s),
        AstNode::Enum(e) => generate_rust_enum(e),
        AstNode::Union(u) => generate_rust_union(u),
        AstNode::Bitmask(b) => generate_rust_bitmask(b),
        AstNode::Const(c) => generate_rust_const(c),
        AstNode::Module(m) => {
            let mut inner = String::new();
            for child in &m.nodes {
                inner.push_str(&generate_rust_node(child));
                inner.push('\n');
            }
            format!(
                "pub mod {} {{
    use super::*;
{}
}}",
                m.name, inner
            )
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: dds-idlc <input.idl> <output.rs>");
        std::process::exit(1);
    }

    let input_path = Path::new(&args[1]);
    let output_path = Path::new(&args[2]);

    let content = match fs::read_to_string(input_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read input IDL file: {e}");
            std::process::exit(1);
        }
    };

    let nodes = match parse_idl(&content) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("Parse error: {e}");
            std::process::exit(1);
        }
    };

    let mut generated_code = String::new();
    for node in nodes {
        generated_code.push_str(&generate_rust_node(&node));
        generated_code.push('\n');
    }

    if let Err(e) = fs::write(output_path, generated_code) {
        eprintln!("Failed to write output Rust file: {e}");
        std::process::exit(1);
    }

    println!("Code generation complete: {output_path:?}");
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_rust_output_with_keys() {
        let key_anno = dds_idl::Annotation {
            name: "key".to_string(),
        };
        let s = StructDef {
            name: "Position".to_string(),
            members: vec![
                dds_idl::StructMember {
                    name: "id".to_string(),
                    field_type: IdlType::Primitive(PrimitiveType::Int32),
                    annotations: vec![key_anno],
                },
                dds_idl::StructMember {
                    name: "x".to_string(),
                    field_type: IdlType::Primitive(PrimitiveType::Int32),
                    annotations: vec![],
                },
            ],
            annotations: vec![],
        };

        let code = generate_rust_struct(&s);
        assert!(code.contains("pub struct Position"));
        assert!(code.contains("InstanceHandle::new"));
        assert!(code.contains("serialize_i32(val.id)"));
    }

    #[test]
    fn test_generate_rust_nested_module_and_sequence() {
        let node = AstNode::Module(dds_idl::ModuleDef {
            name: "Data".to_string(),
            nodes: vec![AstNode::Struct(StructDef {
                name: "Payload".to_string(),
                members: vec![dds_idl::StructMember {
                    name: "history".to_string(),
                    field_type: IdlType::Sequence(Box::new(IdlType::Primitive(
                        PrimitiveType::Int32,
                    ))),
                    annotations: vec![],
                }],
                annotations: vec![],
            })],
        });

        let code = generate_rust_node(&node);
        assert!(code.contains("pub mod Data"));
        assert!(code.contains("pub history: Vec<i32>,"));
        assert!(code.contains("let history_len = deserializer.deserialize_u32()?;"));
    }

    #[test]
    fn test_generate_rust_array_and_map() {
        let s = StructDef {
            name: "CollectionHolder".to_string(),
            members: vec![
                dds_idl::StructMember {
                    name: "arr".to_string(),
                    field_type: IdlType::Array(
                        Box::new(IdlType::Primitive(PrimitiveType::Int32)),
                        5,
                    ),
                    annotations: vec![],
                },
                dds_idl::StructMember {
                    name: "dict".to_string(),
                    field_type: IdlType::Map(
                        PrimitiveType::String,
                        Box::new(IdlType::Primitive(PrimitiveType::Int32)),
                    ),
                    annotations: vec![],
                },
            ],
            annotations: vec![],
        };

        let code = generate_rust_struct(&s);
        assert!(code.contains("pub arr: [i32; 5],"));
        assert!(code.contains("pub dict: std::collections::HashMap<String, i32>,"));
        assert!(code.contains("let mut arr_vec = Vec::with_capacity(5);"));
        assert!(code.contains("let dict_len = deserializer.deserialize_u32()?;"));
    }
}
