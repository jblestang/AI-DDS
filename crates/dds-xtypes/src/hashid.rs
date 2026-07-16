//! XTypes member and operation ID hashing (§7.2.2.4.4.4.5).

/// Compute the XTypes `@hashid` member ID for a string (member or operation name).
///
/// Algorithm: MD5 UTF-8 bytes (no NUL), take first 4 bytes as little-endian `u32`,
/// then mask with `0x0FFF_FFFF`.
#[must_use]
pub fn hashid(name: &str) -> u32 {
    let digest = md5::compute(name.as_bytes());
    let bytes: [u8; 4] = digest.0[0..4].try_into().expect("md5 digest length");
    u32::from_le_bytes(bytes) & 0x0FFF_FFFF
}

/// Operation discriminator for `TypeLookup_Call` / `TypeLookup_Return` (`@hashid("getTypes")`).
pub const TYPE_LOOKUP_GET_TYPES_HASH: i32 = 0x0182_52D3;

/// Operation discriminator for dependency lookup (`@hashid("getDependencies")`).
pub const TYPE_LOOKUP_GET_DEPENDENCIES_HASH: i32 = 0x05AA_FB31;

/// Member hash for `type_ids` fields in TypeLookup `_In` structs.
pub const MEMBER_TYPE_IDS: u32 = 0x0C5_36065;

/// Member hash for `types` in `TypeLookup_getTypes_Out`.
pub const MEMBER_TYPES: u32 = 0x028_04AD1;

/// Member hash for `complete_to_minimal` in `TypeLookup_getTypes_Out`.
pub const MEMBER_COMPLETE_TO_MINIMAL: u32 = 0x0B8_E6577;

/// Member hash for `continuation_point`.
pub const MEMBER_CONTINUATION_POINT: u32 = 0x050_8E3D2;

/// Member hash for `dependent_typeids`.
pub const MEMBER_DEPENDENT_TYPEIDS: u32 = 0x0BA_4DFC9;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_hash_ids_match_spec() {
        assert_eq!(hashid("getTypes") as i32, TYPE_LOOKUP_GET_TYPES_HASH);
        assert_eq!(hashid("getDependencies") as i32, TYPE_LOOKUP_GET_DEPENDENCIES_HASH);
    }

    #[test]
    fn member_hash_ids_match_algorithm() {
        assert_eq!(hashid("type_ids"), MEMBER_TYPE_IDS);
        assert_eq!(hashid("types"), MEMBER_TYPES);
    }
}
