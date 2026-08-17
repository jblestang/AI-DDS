//! Integration tests for XTypes §7.6.3.3 TypeLookup wire service.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dds_xtypes::{
    make_get_types_request, serve_type_lookup_request, type_lookup_instance_name, ExtensibilityKind,
    Member, StructureType, TypeLookupCall, TypeLookupGetTypesResult, TypeLookupReturn, TypeObject,
};
use std::collections::HashMap;

#[test]
fn type_lookup_get_types_wire_serve() {
    let obj = TypeObject::Complete(StructureType {
        name: "SensorReading".to_string(),
        extensibility: ExtensibilityKind::Appendable,
        members: vec![Member {
            name: "value".to_string(),
            type_id: dds_xtypes::TypeIdentifier::TkFloat32,
            is_key: false,
            is_optional: false,
        }],
    });
    let type_id = obj.get_identifier();
    let mut db = HashMap::new();
    db.insert(type_id.clone(), obj.clone());

    let prefix = dds_types::guid::GuidPrefix::new([0xAB; 12]);
    let request = make_get_types_request(
        dds_types::guid::Guid::new(prefix, dds_types::guid::EntityId::PARTICIPANT),
        dds_types::guid::SequenceNumber(99),
        type_lookup_instance_name(&prefix),
        vec![type_id],
    );

    let wire = request.to_wire_bytes().expect("encode request");
    let decoded = dds_xtypes::TypeLookupRequest::from_wire_bytes(&wire).expect("decode request");
    let reply = serve_type_lookup_request(&decoded, &db);
    let rep_wire = reply.to_wire_bytes().expect("encode reply");
    let decoded_rep =
        dds_xtypes::TypeLookupReply::from_wire_bytes(&rep_wire).expect("decode reply");

    assert_eq!(decoded_rep.header.request_id.sequence_number.0, 99);
    match decoded_rep.return_data {
        TypeLookupReturn::GetTypes(TypeLookupGetTypesResult::Ok(out)) => {
            assert_eq!(out.types.len(), 1);
            assert_eq!(out.types[0].type_object, obj);
        }
        _ => panic!("expected getTypes result"),
    }
}

#[test]
fn type_lookup_call_discriminators_match_spec() {
    use dds_xtypes::hashid::{
        TYPE_LOOKUP_GET_DEPENDENCIES_HASH, TYPE_LOOKUP_GET_TYPES_HASH,
    };
    assert_eq!(TYPE_LOOKUP_GET_TYPES_HASH, 0x0182_52D3);
    assert_eq!(TYPE_LOOKUP_GET_DEPENDENCIES_HASH, 0x05AA_FB31);

    let req = make_get_types_request(
        dds_types::guid::Guid::new(
            dds_types::guid::GuidPrefix::new([1; 12]),
            dds_types::guid::EntityId::PARTICIPANT,
        ),
        dds_types::guid::SequenceNumber(1),
        "dds.builtin.TOS.test".to_string(),
        vec![],
    );
    assert!(matches!(req.data, TypeLookupCall::GetTypes(_)));
}
