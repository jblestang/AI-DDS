use std::collections::HashMap;

/// Represents a dynamically reflected type in the `XTypes` type system.
pub trait Reflect: Send + Sync {
    /// Get the kind of this type (e.g., Struct, Union, Enum).
    fn kind(&self) -> crate::TypeIdentifier;

    /// Get the name of this type.
    fn name(&self) -> &str;
}

/// A type-erased container holding dynamic fields, allowing runtime reflection
/// without requiring statically compiled Rust types.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Data {
    Array(Vec<Self>),
    Boolean(bool),
    Float32(f32),
    Float64(f64),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    Sequence(Vec<Self>),
    String(String),
    Struct(HashMap<String, Self>),
    UInt16(u16),
    UInt32(u32),
    UInt64(u64),
}

impl Data {
    /// Retrieve a field from a struct.
    #[must_use]
    #[inline]
    pub fn get_field(&self, name: &str) -> Option<&Self> {
        match self {
            Self::Struct(map) => return map.get(name),
            Self::Array(_)
            | Self::Boolean(_)
            | Self::Float32(_)
            | Self::Float64(_)
            | Self::Int16(_)
            | Self::Int32(_)
            | Self::Int64(_)
            | Self::Sequence(_)
            | Self::String(_)
            | Self::UInt16(_)
            | Self::UInt32(_)
            | Self::UInt64(_) => return None,
        }
    }

    /// Create a new empty struct [`Data`].
    #[must_use]
    #[inline]
    pub fn new_struct() -> Self {
        return Self::Struct(HashMap::new());
    }

    /// Insert a field into a struct. Returns false if not a struct.
    #[inline]
    pub fn set_field(&mut self, name: &str, value: Self) -> bool {
        match self {
            Self::Struct(map) => {
                map.insert(name.to_owned(), value);
                return true;
            }
            Self::Array(_)
            | Self::Boolean(_)
            | Self::Float32(_)
            | Self::Float64(_)
            | Self::Int16(_)
            | Self::Int32(_)
            | Self::Int64(_)
            | Self::Sequence(_)
            | Self::String(_)
            | Self::UInt16(_)
            | Self::UInt32(_)
            | Self::UInt64(_) => return false,
        }
    }
}
