//! End-to-end smoke test exercising discovery, TypeLookup, QoS, data path,
//! instance lifecycle, fragmentation, monitor snapshot, and type compatibility.

mod common;

use common::{
    type_info_for, type_with_nested_dependency, wait_until, wire_bidirectional_discovery,
    wire_message_type_object, wire_type_support, DiscoveryWire, WireMessage,
};
use dds::core::{
    check_partition_compatibility, check_qos_compatibility, check_type_compatibility,
    DomainParticipantFactory, Listener,
};
use dds::types::guid::{EntityId, Guid, SequenceNumber};
use dds::types::qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos, DurabilityKind, PublisherQos,
    ReliabilityKind, SubscriberQos, TopicQos, TypeConsistencyKind,
};
use dds::xtypes::{
    make_get_types_request, type_lookup_instance_name, TypeLookupCall,
    TypeLookupGetTypeDependenciesResult, TypeLookupGetTypesResult, TypeLookupReturn,
};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use std::time::Duration;

const E2E_DOMAIN: u32 = 55;
const E2E_TOPIC: &str = "E2EFullStackTopic";
const E2E_TYPE: &str = "WireMessage";
const E2E_PARTITION: &str = "e2e-lab";

#[test]
fn e2e_validate_implemented_features() {
    let type_obj = wire_message_type_object();
    let (type_obj_with_dep, nested_dep) = type_with_nested_dependency();
    let type_id = type_obj.get_identifier();
    let type_id_with_dep = type_obj_with_dep.get_identifier();
    let type_info = type_info_for(&type_obj);
    let ts = wire_type_support();

    let sub = DomainParticipantFactory::create_participant(E2E_DOMAIN, DomainParticipantQos::default())
        .expect("subscriber participant");
    sub.register_type(E2E_TYPE, ts.clone()).unwrap();
    let sub_topic = sub
        .create_topic(E2E_TOPIC, E2E_TYPE, TopicQos::default())
        .unwrap();

    let mut sub_reader_qos = DataReaderQos::default();
    sub_reader_qos.reliability.kind = ReliabilityKind::Reliable;

    let subscriber = sub.create_subscriber(SubscriberQos::default()).unwrap();
    let reader = subscriber
        .create_datareader(&sub_topic, sub_reader_qos.clone(), ts.clone())
        .unwrap();
    let _sub_rx = sub.spawn_receiver_loop();

    let pub_participant =
        DomainParticipantFactory::create_participant(E2E_DOMAIN, DomainParticipantQos::default())
            .expect("publisher participant");
    pub_participant.register_type(E2E_TYPE, ts.clone()).unwrap();
    {
        let mut disc = pub_participant.discovery.lock().unwrap();
        disc.register_type_object(type_obj.clone());
        disc.register_type_object(type_obj_with_dep.clone());
    }

    let pub_topic = pub_participant
        .create_topic(E2E_TOPIC, E2E_TYPE, TopicQos::default())
        .unwrap();

    let mut pub_pub_qos = PublisherQos::default();
    pub_pub_qos.partition.name = vec![E2E_PARTITION.to_string()];
    let mut writer_qos = DataWriterQos::default();
    writer_qos.durability.kind = DurabilityKind::TransientLocal;
    writer_qos.reliability.kind = ReliabilityKind::Reliable;

    let publisher = pub_participant.create_publisher(pub_pub_qos.clone()).unwrap();
    let writer = publisher
        .create_datawriter(&pub_topic, writer_qos.clone(), ts.clone())
        .unwrap();

    struct CountingWriterListener {
        matched: Arc<AtomicI32>,
    }
    impl Listener for CountingWriterListener {}
    impl dds::core::DataWriterListener for CountingWriterListener {
        fn on_publication_matched(
            &self,
            _writer: &dds::core::DataWriter,
            _status: dds::types::status::PublicationMatchedStatus,
        ) {
            self.matched.fetch_add(1, Ordering::SeqCst);
        }
    }
    let matched_count = Arc::new(AtomicI32::new(0));
    writer.set_listener(Some(Arc::new(CountingWriterListener {
        matched: matched_count.clone(),
    })));

    let _pub_rx = pub_participant.spawn_receiver_loop();
    std::thread::sleep(Duration::from_millis(60));

    assert!(check_partition_compatibility(
        &[E2E_PARTITION.to_string()],
        &pub_pub_qos.partition
    ));
    assert!(!check_partition_compatibility(
        &[String::from("other")],
        &pub_pub_qos.partition
    ));
    assert!(check_qos_compatibility(&writer_qos, &sub_reader_qos));

    let mut incompatible_writer = writer_qos.clone();
    incompatible_writer.reliability.kind = ReliabilityKind::BestEffort;
    let mut incompatible_reader = sub_reader_qos.clone();
    incompatible_reader.reliability.kind = ReliabilityKind::Reliable;
    assert!(!check_qos_compatibility(
        &incompatible_writer,
        &incompatible_reader
    ));

    let policy = dds::types::qos::TypeConsistencyEnforcement::default();
    assert!(check_type_compatibility(&policy, &type_obj, &type_obj));
    assert!(matches!(
        policy.kind,
        TypeConsistencyKind::AllowTypeCoercion
    ));

    let wire = DiscoveryWire {
        topic: E2E_TOPIC.to_string(),
        type_name: E2E_TYPE.to_string(),
        partition: vec![E2E_PARTITION.to_string()],
        type_info: Some(type_info.clone()),
        writer_qos: Some(writer_qos.clone()),
        reader_qos: Some(sub_reader_qos.clone()),
    };
    wire_bidirectional_discovery(
        &pub_participant,
        &sub,
        subscriber.unicast_port(),
        writer.guid(),
        reader.guid(),
        &wire,
    );

    let pub_snapshot = pub_participant.monitor_snapshot();
    let sub_snapshot = sub.monitor_snapshot();
    assert!(
        pub_snapshot
            .endpoints
            .iter()
            .any(|e| e.topic_name == E2E_TOPIC && e.is_writer)
    );
    assert!(
        sub_snapshot
            .endpoints
            .iter()
            .any(|e| e.topic_name == E2E_TOPIC && !e.is_writer)
    );
    assert!(
        pub_snapshot
            .endpoints
            .iter()
            .any(|e| e.guid == reader.guid())
    );

    let get_types_req = make_get_types_request(
        Guid::new(
            sub.guid_prefix(),
            EntityId::BUILTIN_TYPE_LOOKUP_REQUEST_DATA_WRITER,
        ),
        SequenceNumber(42),
        type_lookup_instance_name(&sub.guid_prefix()),
        vec![type_id.clone()],
    );
    sub
        .send_type_lookup_request(
            &get_types_req,
            &common::localhost_locator(pub_participant.unicast_port()),
        )
        .expect("send getTypes request");

    assert!(wait_until(Duration::from_secs(3), || {
        sub.poll_type_lookup_replies()
            .into_iter()
            .any(|reply| match reply.return_data {
                TypeLookupReturn::GetTypes(TypeLookupGetTypesResult::Ok(ref out)) => {
                    out.types.iter().any(|p| p.type_object == type_obj)
                }
                _ => false,
            })
    }));

    let dep_request = dds::xtypes::TypeLookupRequest {
        header: dds::xtypes::RequestHeader {
            request_id: dds::xtypes::SampleIdentity {
                writer_guid: Guid::new(
                    sub.guid_prefix(),
                    EntityId::BUILTIN_TYPE_LOOKUP_REQUEST_DATA_WRITER,
                ),
                sequence_number: SequenceNumber(43),
            },
            instance_name: type_lookup_instance_name(&sub.guid_prefix()),
        },
        data: TypeLookupCall::GetTypeDependencies(dds::xtypes::TypeLookupGetTypeDependenciesIn {
            type_ids: vec![type_id_with_dep.clone()],
            continuation_point: None,
        }),
    };
    sub
        .send_type_lookup_request(
            &dep_request,
            &common::localhost_locator(pub_participant.unicast_port()),
        )
        .expect("send getTypeDependencies request");

    assert!(wait_until(Duration::from_secs(3), || {
        sub.poll_type_lookup_replies().into_iter().any(|reply| {
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

    let sample = WireMessage {
        id: 100,
        payload: "e2e-primary".to_string(),
    };
    let handle = writer.register_instance(&sample).unwrap();
    writer.write(&sample).unwrap();

    let mut received_primary = None;
    assert!(wait_until(Duration::from_secs(3), || {
        if let Ok(boxed) = reader.read_next() {
            if let Some(msg) = boxed.downcast_ref::<WireMessage>() {
                received_primary = Some(msg.clone());
                return true;
            }
        }
        false
    }));
    assert_eq!(received_primary.as_ref(), Some(&sample));

    reader.push_sample(
        dds::types::instance::InstanceHandle::NIL,
        dds::cdr::serialize_to_bytes(&sample, dds::cdr::Endianness::LittleEndian)
            .unwrap()
            .to_vec(),
        None,
    );
    let (still_there, info) = reader.read().unwrap();
    assert!(info.valid_data);
    assert_eq!(
        still_there.downcast_ref::<WireMessage>().unwrap().payload,
        "e2e-primary"
    );
    let (taken, _) = reader.take().unwrap();
    assert_eq!(
        taken.downcast_ref::<WireMessage>().unwrap().payload,
        "e2e-primary"
    );
    assert!(reader.read().is_err());

    writer.dispose(handle).unwrap();
    assert!(writer.write(&sample).is_err());

    let large = WireMessage {
        id: 101,
        payload: "Z".repeat(6000),
    };
    writer.register_instance(&large).unwrap();
    writer.write(&large).unwrap();

    let mut received_large = None;
    assert!(wait_until(Duration::from_secs(4), || {
        if let Ok(boxed) = reader.read_next() {
            if let Some(msg) = boxed.downcast_ref::<WireMessage>() {
                received_large = Some(msg.clone());
                return true;
            }
        }
        false
    }));
    assert_eq!(received_large.as_ref(), Some(&large));

    pub_participant.enable_builtin_topics().unwrap();
    assert!(
        pub_participant
            .discovery
            .lock()
            .unwrap()
            .local_endpoints()
            .values()
            .any(|e| e.topic_name == dds::types::builtin_topics::PUBLICATION_TOPIC_NAME)
    );

    assert!(matched_count.load(Ordering::SeqCst) >= 1);
}
