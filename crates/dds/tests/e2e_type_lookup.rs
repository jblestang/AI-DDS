//! E2E: TypeLookup service over RTPS wire (XTypes §7.6.3.3).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{
    create_participant_pair, localhost_locator, type_info_for, type_with_nested_dependency,
    wait_until, wire_message_type_object,
};
use dds::types::guid::EntityId;
use dds::types::qos::DomainParticipantQos;
use dds::xtypes::{
    make_get_types_request, type_lookup_instance_name, TypeIdentifier, TypeLookupCall,
    TypeLookupGetTypeDependenciesResult, TypeLookupGetTypesResult, TypeLookupReturn,
};
use std::time::Duration;

const DOMAIN: u32 = 57;

#[test]
fn e2e_type_lookup_get_types_over_wire() {
    let pair = create_participant_pair(DOMAIN);
    let type_obj = wire_message_type_object();
    let type_id = type_obj.get_identifier();

    {
        let mut disc = pair.pub_participant.discovery.lock().unwrap();
        disc.register_type_object(type_obj.clone());
    }

    let _sub_rx = pair.sub_participant.spawn_receiver_loop();
    let _pub_rx = pair.pub_participant.spawn_receiver_loop();
    std::thread::sleep(Duration::from_millis(50));

    let req = make_get_types_request(
        dds::types::guid::Guid::new(
            pair.sub_participant.guid_prefix(),
            EntityId::BUILTIN_TYPE_LOOKUP_REQUEST_DATA_WRITER,
        ),
        dds::types::guid::SequenceNumber(100),
        type_lookup_instance_name(&pair.sub_participant.guid_prefix()),
        vec![type_id.clone()],
    );

    pair.sub_participant
        .send_type_lookup_request(&req, &localhost_locator(pair.pub_participant.unicast_port()))
        .expect("send getTypes");

    assert!(wait_until(Duration::from_secs(3), || {
        pair.sub_participant
            .poll_type_lookup_replies()
            .into_iter()
            .any(|reply| match reply.return_data {
                TypeLookupReturn::GetTypes(TypeLookupGetTypesResult::Ok(ref out)) => out
                    .types
                    .iter()
                    .any(|p| p.type_object == type_obj),
                _ => false,
            })
    }));
}

#[test]
fn e2e_type_lookup_get_type_dependencies_over_wire() {
    let pair = create_participant_pair(DOMAIN + 1);
    let (type_obj, nested_dep) = type_with_nested_dependency();
    let type_id = type_obj.get_identifier();

    {
        let mut disc = pair.pub_participant.discovery.lock().unwrap();
        disc.register_type_object(type_obj.clone());
    }

    let _sub_rx = pair.sub_participant.spawn_receiver_loop();
    let _pub_rx = pair.pub_participant.spawn_receiver_loop();
    std::thread::sleep(Duration::from_millis(50));

    let req = dds::xtypes::TypeLookupRequest {
        header: dds::xtypes::RequestHeader {
            request_id: dds::xtypes::SampleIdentity {
                writer_guid: dds::types::guid::Guid::new(
                    pair.sub_participant.guid_prefix(),
                    EntityId::BUILTIN_TYPE_LOOKUP_REQUEST_DATA_WRITER,
                ),
                sequence_number: dds::types::guid::SequenceNumber(101),
            },
            instance_name: type_lookup_instance_name(&pair.sub_participant.guid_prefix()),
        },
        data: TypeLookupCall::GetTypeDependencies(
            dds::xtypes::TypeLookupGetTypeDependenciesIn {
                type_ids: vec![type_id],
                continuation_point: None,
            },
        ),
    };

    pair.sub_participant
        .send_type_lookup_request(&req, &localhost_locator(pair.pub_participant.unicast_port()))
        .expect("send getTypeDependencies");

    assert!(wait_until(Duration::from_secs(3), || {
        pair.sub_participant
            .poll_type_lookup_replies()
            .into_iter()
            .any(|reply| {
                matches!(
                    reply.return_data,
                    TypeLookupReturn::GetTypeDependencies(
                        TypeLookupGetTypeDependenciesResult::Ok(ref out)
                    ) if out.dependent_typeids.iter().any(|d| {
                        d.dependent_typeids.contains(&nested_dep)
                    })
                )
            })
    }));
}

#[test]
fn e2e_type_lookup_unknown_type_returns_empty_result() {
    let pair = create_participant_pair(DOMAIN + 2);
    let unknown = TypeIdentifier::TiCompleteConstructed([0xFF; 14]);

    let _sub_rx = pair.sub_participant.spawn_receiver_loop();
    let _pub_rx = pair.pub_participant.spawn_receiver_loop();
    std::thread::sleep(Duration::from_millis(50));

    let req = make_get_types_request(
        dds::types::guid::Guid::new(
            pair.sub_participant.guid_prefix(),
            EntityId::BUILTIN_TYPE_LOOKUP_REQUEST_DATA_WRITER,
        ),
        dds::types::guid::SequenceNumber(102),
        type_lookup_instance_name(&pair.sub_participant.guid_prefix()),
        vec![unknown],
    );

    pair.sub_participant
        .send_type_lookup_request(&req, &localhost_locator(pair.pub_participant.unicast_port()))
        .expect("send getTypes for unknown");

    assert!(wait_until(Duration::from_secs(3), || {
        pair.sub_participant
            .poll_type_lookup_replies()
            .into_iter()
            .any(|reply| {
                matches!(
                    reply.return_data,
                    TypeLookupReturn::GetTypes(TypeLookupGetTypesResult::Ok(ref out))
                        if out.types.is_empty()
                )
            })
    }));
}

#[test]
fn e2e_type_lookup_with_type_info_in_discovery() {
    let pair = create_participant_pair(DOMAIN + 3);
    let type_obj = wire_message_type_object();
    let info = type_info_for(&type_obj);

    {
        let mut disc = pair.pub_participant.discovery.lock().unwrap();
        disc.register_type_object(type_obj.clone());
    }

    // Verify type_info can be attached to discovery wire metadata
    assert_eq!(info.type_name, "WireMessage");
    assert_eq!(info.type_id, type_obj.get_identifier());

    let _sub_rx = pair.sub_participant.spawn_receiver_loop();
    let _pub_rx = pair.pub_participant.spawn_receiver_loop();

    let req = make_get_types_request(
        dds::types::guid::Guid::new(
            pair.sub_participant.guid_prefix(),
            EntityId::BUILTIN_TYPE_LOOKUP_REQUEST_DATA_WRITER,
        ),
        dds::types::guid::SequenceNumber(103),
        type_lookup_instance_name(&pair.sub_participant.guid_prefix()),
        vec![info.type_id.clone()],
    );

    pair.sub_participant
        .send_type_lookup_request(&req, &localhost_locator(pair.pub_participant.unicast_port()))
        .expect("send getTypes with type_info id");

    assert!(wait_until(Duration::from_secs(3), || {
        !pair.sub_participant.poll_type_lookup_replies().is_empty()
    }));
}
