//! End-to-end integration test exercising discovery, TypeLookup, QoS, data path,
//! instance lifecycle, fragmentation, monitor snapshot, and type compatibility.

use dds::cdr::{CdrDeserialize, CdrSerialize, CdrDeserializer, CdrSerializer, CdrResult};
use dds::core::{
    check_partition_compatibility, check_qos_compatibility, check_type_compatibility,
    DomainParticipantFactory, TypeSupport,
};
use dds::types::locator::Locator;
use dds::types::qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos, DurabilityKind, PublisherQos,
    ReliabilityKind, SubscriberQos, TopicQos, TypeConsistencyKind,
};
use dds::xtypes::{
    make_get_types_request, type_lookup_instance_name, ExtensibilityKind, Member, StructureType,
    TypeIdentifier, TypeInformation, TypeLookupCall, TypeLookupGetTypeDependenciesResult,
    TypeLookupGetTypesResult, TypeLookupReturn, TypeObject,
};
use std::any::Any;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use std::time::Duration;

const E2E_DOMAIN: u32 = 55;
const E2E_TOPIC: &str = "E2EFullStackTopic";
const E2E_TYPE: &str = "E2EMessage";
const E2E_PARTITION: &str = "e2e-lab";

#[derive(Debug, Clone, PartialEq, Eq)]
struct E2EMessage {
    id: u32,
    payload: String,
}

impl CdrSerialize for E2EMessage {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_u32(self.id);
        serializer.serialize_str(&self.payload);
        Ok(())
    }
}

impl CdrDeserialize for E2EMessage {
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        Ok(Self {
            id: deserializer.deserialize_u32()?,
            payload: deserializer.deserialize_str()?,
        })
    }
}

struct E2ETypeSupport;

impl TypeSupport for E2ETypeSupport {
    fn get_type_name(&self) -> &str {
        E2E_TYPE
    }

    fn serialize(&self, value: &dyn Any) -> dds::types::return_code::DdsResult<Vec<u8>> {
        let msg = value
            .downcast_ref::<E2EMessage>()
            .ok_or_else(|| dds::types::return_code::DdsError::BadParameter("cast failed".into()))?;
        Ok(
            dds::cdr::serialize_to_bytes(msg, dds::cdr::Endianness::LittleEndian)
                .map_err(|e| dds::types::return_code::DdsError::Error(e.to_string()))?
                .to_vec(),
        )
    }

    fn deserialize(&self, bytes: &[u8]) -> dds::types::return_code::DdsResult<Box<dyn Any>> {
        let msg: E2EMessage =
            dds::cdr::deserialize_from_slice(bytes, dds::cdr::Endianness::LittleEndian)
                .map_err(|e| dds::types::return_code::DdsError::Error(e.to_string()))?;
        Ok(Box::new(msg))
    }

    fn get_key_hash(
        &self,
        value: &dyn Any,
    ) -> dds::types::return_code::DdsResult<dds::types::instance::InstanceHandle> {
        let msg = value
            .downcast_ref::<E2EMessage>()
            .ok_or_else(|| dds::types::return_code::DdsError::BadParameter("cast failed".into()))?;
        Ok(dds::types::instance::InstanceHandle::from_key_bytes(
            &msg.id.to_le_bytes(),
        ))
    }
}

fn e2e_type_object() -> TypeObject {
    TypeObject::Complete(StructureType {
        name: E2E_TYPE.to_string(),
        extensibility: ExtensibilityKind::Appendable,
        members: vec![
            Member {
                name: "id".to_string(),
                type_id: TypeIdentifier::TkUint32,
                is_key: true,
                is_optional: false,
            },
            Member {
                name: "payload".to_string(),
                type_id: TypeIdentifier::TiString8Large { bound: 0 },
                is_key: false,
                is_optional: false,
            },
        ],
    })
}

fn localhost_locator(port: u32) -> Locator {
    Locator::udpv4(std::net::Ipv4Addr::LOCALHOST, port)
}

fn wait_until(timeout: Duration, mut pred: impl FnMut() -> bool) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if pred() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    false
}

struct DiscoveryWire {
    topic: String,
    type_name: String,
    partition: Vec<String>,
    type_info: Option<TypeInformation>,
    writer_qos: Option<DataWriterQos>,
    reader_qos: Option<DataReaderQos>,
}

fn inject_remote_endpoint(
    local: &dds::core::DomainParticipant,
    remote_prefix: dds::types::guid::GuidPrefix,
    remote_unicast: Locator,
    endpoint: dds::discovery::DiscoveredEndpoint,
) {
    let mut disc = local.discovery.lock().unwrap();
    disc.process_spdp_packet(dds::discovery::DiscoveredParticipant {
        guid_prefix: remote_prefix,
        unicast_locators: vec![remote_unicast],
        multicast_locators: vec![],
        lease_duration: dds::types::time::Duration::from_secs(120),
        last_contact: std::time::Instant::now(),
    });
    disc.process_sedp_endpoint(endpoint);
}

fn wire_bidirectional_discovery(
    pub_participant: &dds::core::DomainParticipant,
    sub_participant: &dds::core::DomainParticipant,
    sub_subscriber_port: u32,
    writer_guid: dds::types::guid::Guid,
    reader_guid: dds::types::guid::Guid,
    wire: &DiscoveryWire,
) {
    let pub_locator = localhost_locator(pub_participant.unicast_port());
    let sub_locator = localhost_locator(sub_subscriber_port);

    inject_remote_endpoint(
        sub_participant,
        pub_participant.guid_prefix(),
        pub_locator,
        dds::discovery::DiscoveredEndpoint {
            guid: writer_guid,
            topic_name: wire.topic.clone(),
            type_name: wire.type_name.clone(),
            qos_writer: wire.writer_qos.clone(),
            qos_reader: None,
            partition: wire.partition.clone(),
            type_info: wire.type_info.clone(),
        },
    );

    inject_remote_endpoint(
        pub_participant,
        sub_participant.guid_prefix(),
        sub_locator,
        dds::discovery::DiscoveredEndpoint {
            guid: reader_guid,
            topic_name: wire.topic.clone(),
            type_name: wire.type_name.clone(),
            qos_writer: None,
            qos_reader: wire.reader_qos.clone(),
            partition: wire.partition.clone(),
            type_info: wire.type_info.clone(),
        },
    );

    sub_participant.run_matchmaking();
    pub_participant.run_matchmaking();
}

#[test]
fn e2e_validate_implemented_features() {
    let type_obj = e2e_type_object();
    let nested_dep = TypeIdentifier::TiCompleteConstructed([0xAB; 14]);
    let type_obj_with_dep = TypeObject::Complete(StructureType {
        name: "E2EWithDep".to_string(),
        extensibility: ExtensibilityKind::Final,
        members: vec![Member {
            name: "nested".to_string(),
            type_id: nested_dep.clone(),
            is_key: false,
            is_optional: false,
        }],
    });
    let type_id = type_obj.get_identifier();
    let type_id_with_dep = type_obj_with_dep.get_identifier();
    let type_info = TypeInformation {
        type_name: E2E_TYPE.to_string(),
        type_id: type_id.clone(),
    };

    let ts = Arc::new(E2ETypeSupport);

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
    impl dds::core::Listener for CountingWriterListener {}
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
        dds::types::guid::Guid::new(
            sub.guid_prefix(),
            dds::types::guid::EntityId::BUILTIN_TYPE_LOOKUP_REQUEST_DATA_WRITER,
        ),
        dds::types::guid::SequenceNumber(42),
        type_lookup_instance_name(&sub.guid_prefix()),
        vec![type_id.clone()],
    );
    sub
        .send_type_lookup_request(&get_types_req, &localhost_locator(pub_participant.unicast_port()))
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
                writer_guid: dds::types::guid::Guid::new(
                    sub.guid_prefix(),
                    dds::types::guid::EntityId::BUILTIN_TYPE_LOOKUP_REQUEST_DATA_WRITER,
                ),
                sequence_number: dds::types::guid::SequenceNumber(43),
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
            &localhost_locator(pub_participant.unicast_port()),
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

    let sample = E2EMessage {
        id: 100,
        payload: "e2e-primary".to_string(),
    };
    let handle = writer.register_instance(&sample).unwrap();
    writer.write(&sample).unwrap();

    let mut received_primary = None;
    assert!(wait_until(Duration::from_secs(3), || {
        if let Ok(boxed) = reader.read_next() {
            if let Some(msg) = boxed.downcast_ref::<E2EMessage>() {
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
    );
    let (still_there, info) = reader.read().unwrap();
    assert!(info.valid_data);
    assert_eq!(
        still_there.downcast_ref::<E2EMessage>().unwrap().payload,
        "e2e-primary"
    );
    let (taken, _) = reader.take().unwrap();
    assert_eq!(
        taken.downcast_ref::<E2EMessage>().unwrap().payload,
        "e2e-primary"
    );
    assert!(reader.read().is_err());

    writer.dispose(handle).unwrap();
    assert!(writer.write(&sample).is_err());

    let large = E2EMessage {
        id: 101,
        payload: "Z".repeat(6000),
    };
    writer.register_instance(&large).unwrap();
    writer.write(&large).unwrap();

    let mut received_large = None;
    assert!(wait_until(Duration::from_secs(4), || {
        if let Ok(boxed) = reader.read_next() {
            if let Some(msg) = boxed.downcast_ref::<E2EMessage>() {
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
