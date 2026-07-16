//! E2E: endpoints on different topics must not exchange data.

mod common;

use common::{
    create_participant_pair, default_wire, plain_type_support, spawn_receivers,
    wire_pub_to_sub_reader, PlainMessage,
};
use dds::types::qos::{
    DataReaderQos, DataWriterQos, PublisherQos, ReliabilityKind, SubscriberQos, TopicQos,
};
use std::time::Duration;

const DOMAIN: u32 = 90;
const WRITER_TOPIC: &str = "E2EWriterTopic";
const READER_TOPIC: &str = "E2EReaderTopic";
const TYPE: &str = "PlainMessage";

#[test]
fn e2e_topic_name_mismatch_blocks_wire_delivery() {
    let pair = create_participant_pair(DOMAIN);
    let ts = plain_type_support();

    pair.sub_participant.register_type(TYPE, ts.clone()).unwrap();
    pair.pub_participant.register_type(TYPE, ts.clone()).unwrap();

    let sub_topic = pair
        .sub_participant
        .create_topic(READER_TOPIC, TYPE, TopicQos::default())
        .unwrap();
    let pub_topic = pair
        .pub_participant
        .create_topic(WRITER_TOPIC, TYPE, TopicQos::default())
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

    let publisher = pair.pub_participant.create_publisher(PublisherQos::default()).unwrap();
    let mut writer_qos = DataWriterQos::default();
    writer_qos.reliability.kind = ReliabilityKind::Reliable;
    let writer = publisher
        .create_datawriter(&pub_topic, writer_qos.clone(), ts)
        .unwrap();

    let mut wire = default_wire(READER_TOPIC, TYPE);
    wire.writer_qos = Some(writer_qos);
    wire.reader_qos = Some(reader_qos);

    wire_pub_to_sub_reader(
        &pair.pub_participant,
        pair.sub_participant.guid_prefix(),
        common::localhost_locator(subscriber.unicast_port()),
        reader.guid(),
        &wire,
    );

    writer
        .write(&PlainMessage {
            content: "wrong-topic".to_string(),
        })
        .unwrap();

    std::thread::sleep(Duration::from_millis(250));
    assert!(
        reader.read_next().is_err(),
        "Writer and reader on different topics must not match"
    );
}
