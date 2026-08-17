//! E2E: DDS Security encrypted payload delivery over RTPS wire.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{default_wire, plain_type_support, read_next_plain, wire_bidirectional_discovery, PlainMessage};
use dds::core::DomainParticipantFactory;
use dds_security::{Authentication, Cryptography};
use dds::types::qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos, PublisherQos, ReliabilityKind,
    SubscriberQos,     TopicQos,
};
use std::time::Duration;

const DOMAIN: u32 = 91;
const TOPIC: &str = "E2ESecureWireTopic";
const TYPE: &str = "PlainMessage";

fn secure_participant_qos(
    identity_cert: &str,
    private_key: &str,
) -> DomainParticipantQos {
    let mut qos = DomainParticipantQos::default();
    qos.property.value.push((
        "dds.sec.auth.identity_ca".to_owned(),
        "certs/ca_cert.pem".to_owned(),
    ));
    qos.property.value.push((
        "dds.sec.auth.identity_certificate".to_owned(),
        identity_cert.to_owned(),
    ));
    qos.property
        .value
        .push(("dds.sec.auth.private_key".to_owned(), private_key.to_owned()));
    qos
}

fn complete_secure_handshake(
    alice: &dds::core::DomainParticipant,
    bob: &dds::core::DomainParticipant,
) {
    let id_alice = dds_security::IdentityHandle(1);
    let id_bob = dds_security::IdentityHandle(1);

    let auth_alice = &alice.security_auth;
    let auth_bob = &bob.security_auth;

    let (mut handshake_alice, token_request) = auth_alice
        .begin_handshake_request(&id_alice, &id_bob)
        .expect("alice handshake request");
    let mut handshake_bob = dds_security::HandshakeHandle(0);
    let token_reply = auth_bob
        .process_handshake(&mut handshake_bob, token_request)
        .expect("bob handshake reply")
        .expect("bob handshake reply token");
    let token_final = auth_alice
        .process_handshake(&mut handshake_alice, token_reply)
        .expect("alice handshake final")
        .expect("alice handshake final token");
    assert!(
        auth_bob
            .process_handshake(&mut handshake_bob, token_final)
            .expect("bob handshake complete")
            .is_none()
    );

    let shared_alice = auth_alice.get_shared_secret(&handshake_alice).unwrap();
    let shared_bob = auth_bob.get_shared_secret(&handshake_bob).unwrap();

    let remote_crypt_alice = alice
        .security_crypto
        .register_matched_remote_participant(&alice.local_crypto_handle, &id_bob, &shared_alice)
        .unwrap();
    let remote_crypt_bob = bob
        .security_crypto
        .register_matched_remote_participant(&bob.local_crypto_handle, &id_alice, &shared_bob)
        .unwrap();

    alice
        .remote_crypto_handles
        .lock()
        .unwrap()
        .insert(bob.guid_prefix(), remote_crypt_bob);
    bob.remote_crypto_handles
        .lock()
        .unwrap()
        .insert(alice.guid_prefix(), remote_crypt_alice);
}

#[test]
fn e2e_secure_encrypted_delivery_over_wire() {
    let alice_qos = secure_participant_qos("certs/alice_cert.pem", "certs/alice_key.pem");
    let bob_qos = secure_participant_qos("certs/bob_cert.pem", "certs/bob_key.pem");

    let alice = DomainParticipantFactory::create_participant(DOMAIN, alice_qos)
        .expect("alice participant");
    let bob = DomainParticipantFactory::create_participant(DOMAIN, bob_qos).expect("bob participant");

    complete_secure_handshake(&alice, &bob);

    let ts = plain_type_support();
    alice.register_type(TYPE, ts.clone()).unwrap();
    bob.register_type(TYPE, ts.clone()).unwrap();

    let pub_topic = alice
        .create_topic(TOPIC, TYPE, TopicQos::default())
        .unwrap();
    let sub_topic = bob.create_topic(TOPIC, TYPE, TopicQos::default()).unwrap();

    let publisher = alice.create_publisher(PublisherQos::default()).unwrap();
    let mut writer_qos = DataWriterQos::default();
    writer_qos.reliability.kind = ReliabilityKind::Reliable;
    let writer = publisher
        .create_datawriter(&pub_topic, writer_qos.clone(), ts.clone())
        .unwrap();

    let subscriber = bob.create_subscriber(SubscriberQos::default()).unwrap();
    let mut reader_qos = DataReaderQos::default();
    reader_qos.reliability.kind = ReliabilityKind::Reliable;
    let reader = subscriber
        .create_datareader(&sub_topic, reader_qos.clone(), ts)
        .unwrap();

    let _ = alice.spawn_receiver_loop();
    let _ = bob.spawn_receiver_loop();

    let mut wire = default_wire(TOPIC, TYPE);
    wire.writer_qos = Some(writer_qos);
    wire.reader_qos = Some(reader_qos);

    wire_bidirectional_discovery(
        &alice,
        &bob,
        subscriber.unicast_port(),
        writer.guid(),
        reader.guid(),
        &wire,
    );

    let sample = PlainMessage {
        content: "encrypted-over-wire".to_string(),
    };
    writer.write(&sample).unwrap();

    assert_eq!(
        read_next_plain(&reader, Duration::from_secs(3)).as_ref(),
        Some(&sample)
    );
}
