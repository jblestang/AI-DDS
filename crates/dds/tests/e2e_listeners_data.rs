//! E2E: DataReaderListener::on_data_available over the wire.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{
    create_participant_pair, default_wire, plain_type_support, spawn_receivers,
    wire_bidirectional_discovery, PlainMessage,
};
use dds::core::{DataReaderListener, Listener};
use dds::types::qos::{
    DataReaderQos, DataWriterQos, PublisherQos, ReliabilityKind, SubscriberQos, TopicQos,
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

const DOMAIN: u32 = 74;
const TOPIC: &str = "E2EDataAvailTopic";
const TYPE: &str = "PlainMessage";

struct DataAvailListener {
    count: Arc<AtomicU32>,
}

impl Listener for DataAvailListener {}
impl DataReaderListener for DataAvailListener {
    fn on_data_available(&self, _reader: &dds::core::DataReader) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn e2e_on_data_available_fires_over_wire() {
    let pair = create_participant_pair(DOMAIN);
    let ts = plain_type_support();

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

    let count = Arc::new(AtomicU32::new(0));
    reader.set_listener(Some(Arc::new(DataAvailListener {
        count: count.clone(),
    })));

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

    let mut wire = default_wire(TOPIC, TYPE);
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

    writer
        .write(&PlainMessage {
            content: "listener-wire".to_string(),
        })
        .unwrap();

    assert!(common::wait_until(Duration::from_secs(3), || {
        count.load(Ordering::SeqCst) >= 1
    }));
    assert!(
        count.load(Ordering::SeqCst) >= 1,
        "on_data_available should fire when sample arrives over wire"
    );
}
