//! E2E: listener callbacks fired via wire matchmaking.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{
    create_participant_pair, default_wire, plain_type_support, spawn_receivers,
    wire_bidirectional_discovery, wire_sub_from_pub_writer,
};
use dds::core::{DataReaderListener, DataWriterListener, Listener};
use dds::types::qos::{DataReaderQos, DataWriterQos, PublisherQos, SubscriberQos, TopicQos};
use dds::types::status::{PublicationMatchedStatus, SubscriptionMatchedStatus};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use std::time::Duration;

const DOMAIN: u32 = 59;
const TOPIC: &str = "E2EListenerTopic";
const TYPE: &str = "PlainMessage";

struct PubMatchListener {
    matched: Arc<AtomicI32>,
}

impl Listener for PubMatchListener {}
impl DataWriterListener for PubMatchListener {
    fn on_publication_matched(&self, _writer: &dds::core::DataWriter, _status: PublicationMatchedStatus) {
        self.matched.fetch_add(1, Ordering::SeqCst);
    }
}

struct SubMatchListener {
    matched: AtomicBool,
}

impl Listener for SubMatchListener {}
impl DataReaderListener for SubMatchListener {
    fn on_data_available(&self, _reader: &dds::core::DataReader) {}
    fn on_subscription_matched(
        &self,
        _reader: &dds::core::DataReader,
        status: SubscriptionMatchedStatus,
    ) {
        if status.current_count_change > 0 {
            self.matched.store(true, Ordering::SeqCst);
        }
    }
}

#[test]
fn e2e_publication_matched_listener_on_wire_match() {
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
    let reader = subscriber
        .create_datareader(&sub_topic, DataReaderQos::default(), ts.clone())
        .unwrap();

    spawn_receivers(&pair);
    std::thread::sleep(Duration::from_millis(40));

    let publisher = pair
        .pub_participant
        .create_publisher(PublisherQos::default())
        .unwrap();
    let writer = publisher
        .create_datawriter(&pub_topic, DataWriterQos::default(), ts)
        .unwrap();

    let matched_count = Arc::new(AtomicI32::new(0));
    writer.set_listener(Some(Arc::new(PubMatchListener {
        matched: matched_count.clone(),
    })));

    wire_bidirectional_discovery(
        &pair.pub_participant,
        &pair.sub_participant,
        writer.guid(),
        reader.guid(),
        &default_wire(TOPIC, TYPE),
    );

    common::wait_until(Duration::from_secs(2), || matched_count.load(Ordering::SeqCst) >= 1);
    assert!(
        matched_count.load(Ordering::SeqCst) >= 1,
        "on_publication_matched should fire after wire matchmaking"
    );
}

#[test]
fn e2e_subscription_matched_listener_on_reverse_matchmaking() {
    let pair = create_participant_pair(DOMAIN + 1);
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
    let reader = subscriber
        .create_datareader(&sub_topic, DataReaderQos::default(), ts.clone())
        .unwrap();

    let listener = Arc::new(SubMatchListener {
        matched: AtomicBool::new(false),
    });
    reader.set_listener(Some(listener.clone()));

    let publisher = pair
        .pub_participant
        .create_publisher(PublisherQos::default())
        .unwrap();
    let writer = publisher
        .create_datawriter(&pub_topic, DataWriterQos::default(), ts)
        .unwrap();

    wire_sub_from_pub_writer(
        &pair.sub_participant,
        pair.pub_participant.guid_prefix(),
        common::localhost_locator(pair.pub_participant.unicast_port()),
        writer.guid(),
        &default_wire(TOPIC, TYPE),
    );

    common::wait_until(Duration::from_secs(2), || listener.matched.load(Ordering::SeqCst));
    assert!(
        listener.matched.load(Ordering::SeqCst),
        "on_subscription_matched should fire on reverse matchmaking"
    );
}
