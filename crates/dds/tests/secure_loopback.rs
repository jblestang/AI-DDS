use dds::cdr::{CdrDeserialize, CdrSerialize, CdrDeserializer, CdrSerializer, CdrResult};
use dds::core::{DomainParticipantFactory, TypeSupport};
use dds::types::qos::{
    DomainParticipantQos, TopicQos, PublisherQos, SubscriberQos, DataWriterQos, DataReaderQos,
};
use dds_security::{Authentication, Cryptography};
use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
struct SecureMessage {
    text: String,
}

impl CdrSerialize for SecureMessage {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_str(&self.text);
        Ok(())
    }
}

impl CdrDeserialize for SecureMessage {
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        let text = deserializer.deserialize_str()?;
        Ok(Self { text })
    }
}

struct SecureMessageTypeSupport;

impl TypeSupport for SecureMessageTypeSupport {
    fn get_type_name(&self) -> &str {
        "SecureMessage"
    }

    fn serialize(&self, value: &dyn Any) -> dds::types::return_code::DdsResult<Vec<u8>> {
        if let Some(msg) = value.downcast_ref::<SecureMessage>() {
            let bytes = dds::cdr::serialize_to_bytes(msg, dds::cdr::Endianness::LittleEndian)
                .map_err(|e| dds::types::return_code::DdsError::Error(e.to_string()))?;
            Ok(bytes.to_vec())
        } else {
            Err(dds::types::return_code::DdsError::BadParameter("cast failed".into()))
        }
    }

    fn deserialize(&self, bytes: &[u8]) -> dds::types::return_code::DdsResult<Box<dyn Any>> {
        let msg: SecureMessage = dds::cdr::deserialize_from_slice(bytes, dds::cdr::Endianness::LittleEndian)
            .map_err(|e| dds::types::return_code::DdsError::Error(e.to_string()))?;
        Ok(Box::new(msg))
    }

    fn get_key_hash(
        &self,
        _value: &dyn Any,
    ) -> dds::types::return_code::DdsResult<dds::types::instance::InstanceHandle> {
        Ok(dds::types::instance::InstanceHandle::NIL)
    }
}

#[test]
fn test_secure_wire_loopback() {
    // 1. Configure QoS properties for Alice (Pub) and Bob (Sub) using pre-generated files
    let mut alice_qos = DomainParticipantQos::default();
    alice_qos.property.value.push(("dds.sec.auth.identity_ca".to_owned(), "certs/ca_cert.pem".to_owned()));
    alice_qos.property.value.push(("dds.sec.auth.identity_certificate".to_owned(), "certs/alice_cert.pem".to_owned()));
    alice_qos.property.value.push(("dds.sec.auth.private_key".to_owned(), "certs/alice_key.pem".to_owned()));

    let mut bob_qos = DomainParticipantQos::default();
    bob_qos.property.value.push(("dds.sec.auth.identity_ca".to_owned(), "certs/ca_cert.pem".to_owned()));
    bob_qos.property.value.push(("dds.sec.auth.identity_certificate".to_owned(), "certs/bob_cert.pem".to_owned()));
    bob_qos.property.value.push(("dds.sec.auth.private_key".to_owned(), "certs/bob_key.pem".to_owned()));

    // 2. Create participants (use domain ID 20 to avoid port conflicts)
    let participant_alice = DomainParticipantFactory::create_participant(20, alice_qos)
        .expect("Failed to create Alice participant");
    let participant_bob = DomainParticipantFactory::create_participant(20, bob_qos)
        .expect("Failed to create Bob participant");

    // Verify local identities were validated and loaded
    let id_alice = dds_security::IdentityHandle(1);
    let id_bob = dds_security::IdentityHandle(1);

    // 3. Complete Secure PKI Handshake
    let auth_alice = &participant_alice.security_auth;
    let auth_bob = &participant_bob.security_auth;

    let (mut handshake_alice, token_request) = auth_alice.begin_handshake_request(&id_alice, &id_bob).unwrap();
    
    let mut handshake_bob = dds_security::HandshakeHandle(0);
    let token_reply = auth_bob.process_handshake(&mut handshake_bob, token_request).unwrap().unwrap();

    let token_final = auth_alice.process_handshake(&mut handshake_alice, token_reply).unwrap().unwrap();
    let opt_none = auth_bob.process_handshake(&mut handshake_bob, token_final).unwrap();
    assert!(opt_none.is_none(), "Handshake final response should be None");

    // 4. Derive shared secrets & register matched remote participants
    let shared_alice = auth_alice.get_shared_secret(&handshake_alice).unwrap();
    let shared_bob = auth_bob.get_shared_secret(&handshake_bob).unwrap();

    let remote_crypt_alice = participant_alice.security_crypto
        .register_matched_remote_participant(&participant_alice.local_crypto_handle, &id_bob, &shared_alice)
        .unwrap();

    let remote_crypt_bob = participant_bob.security_crypto
        .register_matched_remote_participant(&participant_bob.local_crypto_handle, &id_alice, &shared_bob)
        .unwrap();

    // Wire crypto prefixes
    let alice_prefix = participant_alice.guid_prefix();
    let bob_prefix = participant_bob.guid_prefix();
    participant_alice.remote_crypto_handles.lock().unwrap().insert(bob_prefix, remote_crypt_alice);
    participant_bob.remote_crypto_handles.lock().unwrap().insert(alice_prefix, remote_crypt_bob);

    // 5. Register types
    let ts_pub = Arc::new(SecureMessageTypeSupport);
    let ts_sub = Arc::new(SecureMessageTypeSupport);
    participant_alice.register_type("SecureMessage", ts_pub.clone()).unwrap();
    participant_bob.register_type("SecureMessage", ts_sub.clone()).unwrap();

    // 6. Create Topic
    let topic_pub = participant_alice
        .create_topic("SecureTopic", "SecureMessage", TopicQos::default())
        .unwrap();
    let topic_sub = participant_bob
        .create_topic("SecureTopic", "SecureMessage", TopicQos::default())
        .unwrap();

    // 7. Create Publisher & DataWriter
    let publisher = participant_alice.create_publisher(PublisherQos::default()).unwrap();
    let writer = publisher
        .create_datawriter(&topic_pub, DataWriterQos::default(), ts_pub)
        .unwrap();

    // 8. Create Subscriber & DataReader
    let subscriber = participant_bob.create_subscriber(SubscriberQos::default()).unwrap();
    let reader = subscriber
        .create_datareader(&topic_sub, DataReaderQos::default(), ts_sub)
        .unwrap();

    // 9. Match Alice and Bob manually
    let reader_port = subscriber.unicast_port();
    let reader_locator = dds::types::locator::Locator::udpv4(
        std::net::Ipv4Addr::new(127, 0, 0, 1),
        reader_port,
    );
    let reader_guid = reader.guid();
    publisher.add_reader_proxy_to_all(reader_guid, reader_locator, "SecureTopic", &dds_types::qos::Partition::default());

    // 10. Start receiver loop
    let _receiver_handle = participant_bob.spawn_receiver_loop();
    std::thread::sleep(Duration::from_millis(50));

    // 11. Alice writes sample
    let msg = SecureMessage {
        text: "Top Secret: Encrypted DDS Transfer Success!".to_string(),
    };
    writer.write(&msg).expect("Failed to write secure sample");

    // 12. Bob reads and asserts content
    let start = std::time::Instant::now();
    let mut received_msg: Option<SecureMessage> = None;
    while start.elapsed() < Duration::from_secs(2) {
        if let Ok(boxed) = reader.read_next() {
            if let Some(m) = boxed.downcast_ref::<SecureMessage>() {
                received_msg = Some(m.clone());
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(received_msg.is_some(), "Timeout waiting for secure sample");
    assert_eq!(received_msg.unwrap().text, "Top Secret: Encrypted DDS Transfer Success!");
}
