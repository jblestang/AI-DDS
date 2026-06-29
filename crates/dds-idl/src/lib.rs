//! # dds-idl — IDL 4.2 Parser
//!
//! Parses OMG IDL 4.2 files into a typed AST for code generation.
//!
//! Reference: IDL §7

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
    reason = "IDL Parser implementation requires standard library conversions, standard returns, and parser parsing loops."
)]

use nom::{
    bytes::complete::{tag, take_while1},
    character::complete::{char as nom_char, multispace0, multispace1},
    multi::{many0, separated_list0},
    sequence::{delimited, preceded, tuple},
    IResult,
};

/// Represents primitive types supported in IDL 4.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrimitiveType {
    Int16,
    Int32,
    Int64,
    Uint16,
    Uint32,
    Uint64,
    Float,
    Double,
    Boolean,
    Char,
    String,
}

/// General type representation allowing collection types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdlType {
    Primitive(PrimitiveType),
    Sequence(Box<Self>),
    Array(Box<Self>, usize),
    Map(PrimitiveType, Box<Self>),
}

/// Represents an IDL Annotation (e.g. `@key`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    pub name: String,
}

/// A field member inside an IDL Struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructMember {
    pub name: String,
    pub field_type: IdlType,
    pub annotations: Vec<Annotation>,
}

/// Represents an IDL Struct definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructDef {
    pub name: String,
    pub members: Vec<StructMember>,
    pub annotations: Vec<Annotation>,
}

/// Represents an IDL Enum definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<String>,
}

/// A single choice choice inside an IDL Union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionCase {
    pub label: String, // e.g. "case 1", "default"
    pub member: StructMember,
}

/// Represents an IDL Union definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionDef {
    pub name: String,
    pub switch_type: PrimitiveType,
    pub cases: Vec<UnionCase>,
}

/// Represents a namespace Module definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleDef {
    pub name: String,
    pub nodes: Vec<AstNode>,
}

/// Represents an IDL Bitmask definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitmaskDef {
    pub name: String,
    pub bit_bound: usize,
    pub flags: Vec<String>,
}

/// Represents an IDL Const definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstDef {
    pub name: String,
    pub const_type: PrimitiveType,
    pub value: String,
}

/// Root AST node representing an IDL file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AstNode {
    Struct(StructDef),
    Enum(EnumDef),
    Union(UnionDef),
    Module(ModuleDef),
    Bitmask(BitmaskDef),
    Const(ConstDef),
}

/// Parser helpers
fn identifier(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| c.is_alphanumeric() || c == '_')(input)
}

fn parse_primitive_type(input: &str) -> IResult<&str, PrimitiveType> {
    let (input, t) = identifier(input)?;
    let prim = match t {
        "short" => PrimitiveType::Int16,
        "long" => PrimitiveType::Int32,
        "long_long" | "longlong" => PrimitiveType::Int64,
        "unsigned_short" | "ushort" => PrimitiveType::Uint16,
        "unsigned_long" | "ulong" => PrimitiveType::Uint32,
        "unsigned_long_long" | "ulonglong" => PrimitiveType::Uint64,
        "float" => PrimitiveType::Float,
        "double" => PrimitiveType::Double,
        "boolean" | "bool" => PrimitiveType::Boolean,
        "char" => PrimitiveType::Char,
        "string" => PrimitiveType::String,
        _ => {
            return Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Tag,
            )))
        }
    };
    Ok((input, prim))
}

fn parse_idl_type(input: &str) -> IResult<&str, IdlType> {
    let input = input.trim_start();

    if let Ok((rem, _)) = tag::<_, _, nom::error::Error<&str>>("sequence")(input) {
        let (rem, _) = multispace0(rem)?;
        let (rem, _) = tag("<")(rem)?;
        let (rem, nested) = parse_idl_type(rem)?;
        let (rem, _) = preceded(multispace0, tag(">"))(rem)?;
        return Ok((rem, IdlType::Sequence(Box::new(nested))));
    }

    if let Ok((rem, _)) = tag::<_, _, nom::error::Error<&str>>("map")(input) {
        let (rem, _) = multispace0(rem)?;
        let (rem, _) = tag("<")(rem)?;
        let (rem, key_type) = parse_primitive_type(rem.trim_start())?;
        let (rem, _) = preceded(multispace0, tag(","))(rem)?;
        let (rem, value_type) = parse_idl_type(rem)?;
        let (rem, _) = preceded(multispace0, tag(">"))(rem)?;
        return Ok((rem, IdlType::Map(key_type, Box::new(value_type))));
    }

    let (input, prim) = parse_primitive_type(input)?;

    // Check for optional array bounds suffix like "[10]"
    let trimmed = input.trim_start();
    if let Ok((rem, _)) = tag::<_, _, nom::error::Error<&str>>("[")(trimmed) {
        let (rem, digits) = take_while1(|c: char| c.is_ascii_digit())(rem)?;
        let (rem, _) = tag("]")(rem)?;
        let size = digits.parse::<usize>().unwrap_or(0);
        return Ok((
            rem,
            IdlType::Array(Box::new(IdlType::Primitive(prim)), size),
        ));
    }

    Ok((input, IdlType::Primitive(prim)))
}

/// Parses an annotation (e.g. `@key` or `@id(1)`).
fn parse_annotation(input: &str) -> IResult<&str, Annotation> {
    let (input, _) = nom_char('@')(input)?;
    let (input, name) = identifier(input)?;
    let (input, _) = many0(delimited(nom_char('('), identifier, nom_char(')')))(input)?;
    Ok((
        input,
        Annotation {
            name: name.to_owned(),
        },
    ))
}

/// Parses zero or more annotations.
fn parse_annotations(input: &str) -> IResult<&str, Vec<Annotation>> {
    return many0(preceded(multispace0, parse_annotation))(input)
}

fn parse_struct_member(input: &str) -> IResult<&str, StructMember> {
    let (input, annotations) = parse_annotations(input)?;
    let (input, (mut field_type, _, name, _)) = tuple((
        preceded(multispace0, parse_idl_type),
        multispace1,
        identifier,
        multispace0,
    ))(input)?;

    // Check for optional array bounds after member name like "long x[10];"
    let mut remaining = input;
    if let Ok((rem, _)) = tag::<_, _, nom::error::Error<&str>>("[")(remaining) {
        let (rem, digits) = take_while1(|c: char| c.is_ascii_digit())(rem)?;
        let (rem, _) = tag("]")(rem)?;
        let size = digits.parse::<usize>().unwrap_or(0);
        field_type = IdlType::Array(Box::new(field_type), size);
        remaining = rem;
    }

    let (remaining, _) = preceded(multispace0, tag(";"))(remaining)?;

    Ok((
        remaining,
        StructMember {
            name: name.to_owned(),
            field_type,
            annotations,
        },
    ))
}

/// Parses a single IDL struct block.
pub fn parse_struct(input: &str) -> IResult<&str, StructDef> {
    let (input, annotations) = parse_annotations(input)?;
    let (input, (_, _, name, _, _, members, _, _, _)) = tuple((
        preceded(multispace0, tag("struct")),
        multispace1,
        identifier,
        multispace0,
        tag("{"),
        delimited(
            multispace0,
            many0(preceded(multispace0, parse_struct_member)),
            multispace0,
        ),
        tag("}"),
        multispace0,
        tag(";"),
    ))(input)?;

    Ok((
        input,
        StructDef {
            name: name.to_owned(),
            members,
            annotations,
        },
    ))
}

/// Parses a single IDL enum block.
pub fn parse_enum(input: &str) -> IResult<&str, EnumDef> {
    let (input, (_, _, name, _, _, variants, _, _, _)) = tuple((
        tag("enum"),
        multispace1,
        identifier,
        multispace0,
        tag("{"),
        delimited(
            multispace0,
            separated_list0(
                preceded(multispace0, tag(",")),
                preceded(multispace0, identifier),
            ),
            multispace0,
        ),
        tag("}"),
        multispace0,
        tag(";"),
    ))(input)?;

    let variants = variants.into_iter().map(|s| s.to_owned()).collect();

    Ok((
        input,
        EnumDef {
            name: name.to_owned(),
            variants,
        },
    ))
}

/// Parses a single IDL union case.
fn parse_union_case(input: &str) -> IResult<&str, UnionCase> {
    let (input, label) = preceded(
        multispace0,
        nom::branch::alt((
            tag("default"),
            preceded(tag("case"), preceded(multispace1, identifier)),
        )),
    )(input)?;

    let (input, _) = preceded(multispace0, tag(":"))(input)?;
    let (input, member) = parse_struct_member(input)?;

    Ok((
        input,
        UnionCase {
            label: label.to_owned(),
            member,
        },
    ))
}

/// Parses a single IDL union block.
pub fn parse_union(input: &str) -> IResult<&str, UnionDef> {
    let (input, (_, _, name, _, _, _, _, switch_type, _, _, _, _, cases, _, _, _)) = tuple((
        tag("union"),
        multispace1,
        identifier,
        multispace1,
        tag("switch"),
        multispace0,
        tag("("),
        preceded(multispace0, parse_primitive_type),
        multispace0,
        tag(")"),
        multispace0,
        tag("{"),
        delimited(multispace0, many0(parse_union_case), multispace0),
        tag("}"),
        multispace0,
        tag(";"),
    ))(input)?;

    Ok((
        input,
        UnionDef {
            name: name.to_owned(),
            switch_type,
            cases,
        },
    ))
}

/// Parses a single IDL module block.
pub fn parse_module(input: &str) -> IResult<&str, ModuleDef> {
    let (input, (_, _, name, _, _, nodes, _, _, _)) = tuple((
        tag("module"),
        multispace1,
        identifier,
        multispace0,
        tag("{"),
        delimited(
            multispace0,
            many0(preceded(multispace0, parse_ast_node)),
            multispace0,
        ),
        tag("}"),
        multispace0,
        tag(";"),
    ))(input)?;

    Ok((
        input,
        ModuleDef {
            name: name.to_owned(),
            nodes,
        },
    ))
}

/// Parses a single IDL bitmask block.
pub fn parse_bitmask(input: &str) -> IResult<&str, BitmaskDef> {
    let (input, (_, _, name, _, _, flags, _, _, _)) = tuple((
        tag("bitmask"),
        multispace1,
        identifier,
        multispace0,
        tag("{"),
        delimited(
            multispace0,
            separated_list0(
                preceded(multispace0, tag(",")),
                preceded(multispace0, identifier),
            ),
            multispace0,
        ),
        tag("}"),
        multispace0,
        tag(";"),
    ))(input)?;

    let flags = flags.into_iter().map(|s| s.to_owned()).collect();

    Ok((
        input,
        BitmaskDef {
            name: name.to_owned(),
            bit_bound: 32, // Default bound
            flags,
        },
    ))
}

/// Parses a single IDL const definition.
pub fn parse_const(input: &str) -> IResult<&str, ConstDef> {
    let (input, (_, _, const_type, _, name, _, _, _, value, _, _)) = tuple((
        tag("const"),
        multispace1,
        parse_primitive_type,
        multispace1,
        identifier,
        multispace0,
        tag("="),
        multispace0,
        take_while1(|c: char| c.is_alphanumeric() || c == '.' || c == '-'),
        multispace0,
        tag(";"),
    ))(input)?;

    Ok((
        input,
        ConstDef {
            name: name.to_owned(),
            const_type,
            value: value.to_owned(),
        },
    ))
}

fn parse_ast_node(input: &str) -> IResult<&str, AstNode> {
    if let Ok((rem, m)) = parse_module(input) {
        Ok((rem, AstNode::Module(m)))
    } else if let Ok((rem, e)) = parse_enum(input) {
        Ok((rem, AstNode::Enum(e)))
    } else if let Ok((rem, u)) = parse_union(input) {
        Ok((rem, AstNode::Union(u)))
    } else if let Ok((rem, b)) = parse_bitmask(input) {
        Ok((rem, AstNode::Bitmask(b)))
    } else if let Ok((rem, c)) = parse_const(input) {
        Ok((rem, AstNode::Const(c)))
    } else {
        let (rem, s) = parse_struct(input)?;
        Ok((rem, AstNode::Struct(s)))
    }
}

/// Parse a full IDL content stream into AST nodes.
pub fn parse_idl(input: &str) -> Result<Vec<AstNode>, String> {
    let mut nodes = Vec::new();
    let mut remaining = input.trim();

    while !remaining.is_empty() {
        if let Ok((rem, node)) = parse_ast_node(remaining) {
            nodes.push(node);
            remaining = rem.trim();
        } else {
            return Err(format!("parsing failed near: {remaining}"));
        }
    }

    Ok(nodes)
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_struct_member_primitives() {
        let input = "long x;";
        let (_, member) = parse_struct_member(input).unwrap();
        assert_eq!(member.name, "x");
        assert_eq!(member.field_type, IdlType::Primitive(PrimitiveType::Int32));
        assert!(member.annotations.is_empty());
    }

    #[test]
    fn test_parse_struct_member_collections() {
        let seq_input = "sequence<long> x;";
        let (_, member) = parse_struct_member(seq_input).unwrap();
        assert_eq!(
            member.field_type,
            IdlType::Sequence(Box::new(IdlType::Primitive(PrimitiveType::Int32)))
        );

        let array_input = "long x[15];";
        let (_, member2) = parse_struct_member(array_input).unwrap();
        assert_eq!(
            member2.field_type,
            IdlType::Array(Box::new(IdlType::Primitive(PrimitiveType::Int32)), 15)
        );

        let map_input = "map<string, long> counts;";
        let (_, member3) = parse_struct_member(map_input).unwrap();
        assert_eq!(
            member3.field_type,
            IdlType::Map(
                PrimitiveType::String,
                Box::new(IdlType::Primitive(PrimitiveType::Int32))
            )
        );
    }

    #[test]
    fn test_parse_module() {
        let idl = "module Geometry {\n  struct Point {\n    long x;\n    long y;\n  };\n};";
        let nodes = parse_idl(idl).unwrap();
        assert_eq!(nodes.len(), 1);
        if let AstNode::Module(m) = &nodes[0] {
            assert_eq!(m.name, "Geometry");
            assert_eq!(m.nodes.len(), 1);
        } else {
            panic!("Expected Module node");
        }
    }
}
