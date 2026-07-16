//! E2E: read/take semantics over the wire.

mod common;

use common::{
    create_participant_pair, spawn_receivers, wire_bidirectional_discovery, wire_type_support,
    WireMessage,
};
use dds::types::qos::{
    DataReaderQos, DataWriterQos, PublisherQos, ReliabilityKind, SubscriberQos, TopicQos,
};
use std::time::Duration;

const DOMAIN: u32 = 64;
const TOPIC: &str = "E2EReadTakeTopic";
const TYPE: &str = "WireMessage";

#[test]
fn e2e_read_leaves_sample_available_over_wire() {
    let pair = create_participant_pair(DOMAIN);
    let ts = wire_type_support();

    pair.sub_participant.register_type(TYPE, ts.clone()).unwrap();
    pair.pub_participant.register_type(TYPE, ts.clone()).unwrap();

    let sub_topic = pair
        .sub_participant
        .create_topic(TOPIC, TYPE, TopicQos::default())
        .unwrap();
    let pub_topic = pair
        .pub_participant
        .create_topic(TOPIC, TYPE, TopicQos::default())
        .unwrap();

    let subscriber = pair
        .sub_participant
        .create_subscriber(SubscriberQos::default())
        .unwrap();
    let mut reader_qos = DataReaderQos::default();
    reader_qos.reliability.kind = ReliabilityKind::Reliable;
    let reader = subscriber
        .create_datareader(&sub_topic, reader_qos.clone(), ts.clone())
        .unwrap();

    spawn_receivers(&pair);

    let publisher = pair
        .pub_participant
        .create_publisher(PublisherQos::default())
        .unwrap();
    let mut writer_qos = DataWriterQos::default();
    writer_qos.reliability.kind = ReliabilityKind::Reliable;
    let writer = publisher
        .create_datawriter(&pub_topic, writer_qos.clone(), ts)
        .unwrap();

    let mut wire = common::default_wire(TOPIC, TYPE);
    wire.writer_qos = Some(writer_qos);
    wire.reader_qos = Some(reader_qos);

    wire_bidirectional_discovery(
        &pair.pub_participant,
        &pair.sub_participant,
        subscriber.unicast_port(),
        writer.guid(),
        reader.guid(),
        &wire,
    );

    let sample = WireMessage {
        id: 7,
        payload: "read-take-wire".to_string(),
    };
    writer.register_instance(&sample).unwrap();
    writer.write(&sample).unwrap();

    common::wait_until(Duration::from_secs(3), || reader.read().is_ok());

    let (boxed, info) = reader.read().unwrap();
    assert!(info.valid_data);
    assert_eq!(
        boxed.downcast_ref::<WireMessage>().unwrap().payload,
        "read-take-wire"
    );

    // read() again should still succeed
    let (boxed2, _) = reader.read().unwrap();
    assert_eq!(
        boxed2.downcast_ref::<WireMessage>().unwrap().payload,
        "read-take-wire"
    );
}

#[test]
fn e2e_take_removes_sample_over_wire() {
    let pair = create_participant_pair(DOMAIN + 1);
    let ts = wire_type_support();

    pair.sub_participant.register_type(TYPE, ts.clone()).unwrap();
    pair.pub_participant.register_type(TYPE, ts.clone()).unwrap();

    let sub_topic = pair
        .sub_participant
        .create_topic(TOPIC, TYPE, TopicQos::default())
        .unwrap();
    let pub_topic = pair
        .pub_participant
        .create_topic(TOPIC, TYPE, TopicQos::default())
        .unwrap();

    let subscriber = pair
        .sub_participant
        .create_subscriber(SubscriberQos::default())
        .unwrap();
    let mut reader_qos = DataReaderQos::default();
    reader_qos.reliability.kind = ReliabilityKind::Reliable;
    let reader = subscriber
        .create_datareader(&sub_topic, reader_qos.clone(), ts.clone())
        .unwrap();

    spawn_receivers(&pair);

    let publisher = pair
        .pub_participant
        .create_publisher(PublisherQos::default())
        .unwrap();
    let mut writer_qos = DataWriterQos::default();
    writer_qos.reliability.kind = ReliabilityKind::Reliable;
    let writer = publisher
        .create_datawriter(&pub_topic, writer_qos.clone(), ts)
        .unwrap();

    let mut wire = common::default_wire(TOPIC, TYPE);
    wire.writer_qos = Some(writer_qos);
    wire.reader_qos = Some(reader_qos);

    wire_bidirectional_discovery(
        &pair.pub_participant,
        &pair.sub_participant,
        subscriber.unicast_port(),
        writer.guid(),
        reader.guid(),
        &wire,
    );

    let sample = WireMessage {
        id: 8,
        payload: "take-me".to_string(),
    };
    writer.register_instance(&sample).unwrap();
    writer.write(&sample).unwrap();

    common::wait_until(Duration::from_secs(3), || reader.read().is_ok());

    let (boxed, info) = reader.take().unwrap();
    assert!(info.valid_data);
    assert_eq!(
        boxed.downcast_ref::<WireMessage>().unwrap().payload,
        "take-me"
    );
    assert!(reader.read().is_err(), "take() should remove the sample");
}
