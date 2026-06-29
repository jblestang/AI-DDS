use std::collections::HashMap;

/// Represents a dynamically reflected type in the XTypes type system.
pub trait DynamicType: Send + Sync {
    /// Get the name of this type.
    fn name(&self) -> &str;
    
    /// Get the kind of this type (e.g., Struct, Union, Enum).
    fn kind(&self) -> crate::TypeIdentifier;
}

/// A type-erased container holding dynamic fields, allowing runtime reflection
/// without requiring statically compiled Rust types.
#[derive(Debug, Clone, PartialEq)]
pub enum DynamicData {
    Int32(i32),
    UInt32(u32),
    Int16(i16),
    UInt16(u16),
    Int64(i64),
    UInt64(u64),
    Float32(f32),
    Float64(f64),
    Boolean(bool),
    String(String),
    Struct(HashMap<String, DynamicData>),
    Sequence(Vec<DynamicData>),
    Array(Vec<DynamicData>),
}

impl DynamicData {
    /// Create a new empty struct DynamicData.
    pub fn new_struct() -> Self {
        DynamicData::Struct(HashMap::new())
    }

    /// Insert a field into a struct. Returns false if not a struct.
    pub fn set_field(&mut self, name: &str, value: DynamicData) -> bool {
        if let DynamicData::Struct(map) = self {
            map.insert(name.to_owned(), value);
            true
        } else {
            false
        }
    }

    /// Retrieve a field from a struct.
    pub fn get_field(&self, name: &str) -> Option<&DynamicData> {
        if let DynamicData::Struct(map) = self {
            map.get(name)
        } else {
            None
        }
    }
}
