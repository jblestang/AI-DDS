use std::collections::HashMap;

/// Represents a dynamically reflected type in the `XTypes` type system.
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
    Struct(HashMap<String, Self>),
    Sequence(Vec<Self>),
    Array(Vec<Self>),
}

impl DynamicData {
    /// Create a new empty struct `DynamicData`.
    #[must_use]
    pub fn new_struct() -> Self {
        Self::Struct(HashMap::new())
    }

    /// Insert a field into a struct. Returns false if not a struct.
    pub fn set_field(&mut self, name: &str, value: Self) -> bool {
        if let Self::Struct(map) = self {
            map.insert(name.to_owned(), value);
            true
        } else {
            false
        }
    }

    /// Retrieve a field from a struct.
    #[must_use]
    pub fn get_field(&self, name: &str) -> Option<&Self> {
        if let Self::Struct(map) = self {
            map.get(name)
        } else {
            None
        }
    }
}
