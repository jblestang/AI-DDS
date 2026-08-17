//! E2E: XTypes type compatibility and assignability checks.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::wire_message_type_object;
use dds::core::check_type_compatibility;
use dds::types::qos::{TypeConsistencyEnforcement, TypeConsistencyKind};
use dds::xtypes::{ExtensibilityKind, Member, StructureType, TypeIdentifier, TypeObject};

#[test]
fn e2e_identical_types_are_compatible() {
    let offered = wire_message_type_object();
    let requested = wire_message_type_object();
    let policy = TypeConsistencyEnforcement::default();

    assert!(check_type_compatibility(&policy, &offered, &requested));
    assert_eq!(policy.kind, TypeConsistencyKind::AllowTypeCoercion);
}

#[test]
fn e2e_disallow_type_coercion_requires_exact_match() {
    let offered = wire_message_type_object();
    let mut requested = wire_message_type_object();
    if let TypeObject::Complete(ref mut s) = requested {
        s.extensibility = ExtensibilityKind::Final;
    }

    let policy = TypeConsistencyEnforcement {
        kind: TypeConsistencyKind::DisallowTypeCoercion,
        ..Default::default()
    };

    assert!(!check_type_compatibility(&policy, &offered, &requested));
    assert!(check_type_compatibility(&policy, &offered, &offered));
}

#[test]
fn e2e_appendable_supertype_assignable_to_subtype() {
    let base = TypeObject::Complete(StructureType {
        name: "Base".to_string(),
        extensibility: ExtensibilityKind::Appendable,
        members: vec![Member {
            name: "id".to_string(),
            type_id: TypeIdentifier::TkUint32,
            is_key: true,
            is_optional: false,
        }],
    });

    let extended = TypeObject::Complete(StructureType {
        name: "Extended".to_string(),
        extensibility: ExtensibilityKind::Appendable,
        members: vec![
            Member {
                name: "id".to_string(),
                type_id: TypeIdentifier::TkUint32,
                is_key: true,
                is_optional: false,
            },
            Member {
                name: "extra".to_string(),
                type_id: TypeIdentifier::TiString8Large { bound: 0 },
                is_key: false,
                is_optional: false,
            },
        ],
    });

    let policy = TypeConsistencyEnforcement::default();
    // Extended (offered) is assignable to Base (requested) under coercion
    assert!(check_type_compatibility(&policy, &extended, &base));
}

#[test]
fn e2e_incompatible_member_types_fail_assignability() {
    let offered = TypeObject::Complete(StructureType {
        name: "Offered".to_string(),
        extensibility: ExtensibilityKind::Appendable,
        members: vec![Member {
            name: "value".to_string(),
            type_id: TypeIdentifier::TkFloat32,
            is_key: false,
            is_optional: false,
        }],
    });

    let requested = TypeObject::Complete(StructureType {
        name: "Requested".to_string(),
        extensibility: ExtensibilityKind::Appendable,
        members: vec![Member {
            name: "value".to_string(),
            type_id: TypeIdentifier::TkInt32,
            is_key: false,
            is_optional: false,
        }],
    });

    let policy = TypeConsistencyEnforcement::default();
    assert!(!check_type_compatibility(&policy, &offered, &requested));
}
