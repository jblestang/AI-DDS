//! DDS Secure Publish/Subscribe loopback demonstration.
//!
//! Illustrates:
//! 1. DomainParticipant registration and X.509/identity validation.
//! 2. 3-step Authentication handshake (`Request -> Reply -> Final`) using ECDH.
//! 3. Access Control checks validating write and read permissions on topics.
//! 4. Cryptographic protection: encrypting payloads using AES-128-GCM, wrapping with
//!    `CryptoHeader` and `CryptoFooter` envelopes, and verifying decrypted payloads.
#![forbid(unsafe_code)]
#![warn(
    rust_2018_idioms,
    nonstandard_style,
    future_incompatible,
    clippy::all,
    clippy::restriction,
    clippy::pedantic,
    clippy::nursery
)]
#![allow(
    clippy::blanket_clippy_restriction_lints,
    clippy::implicit_return,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::string_slice,
    clippy::absolute_paths,
    clippy::missing_docs_in_private_items,
    clippy::std_instead_of_core,
    clippy::std_instead_of_alloc,
    clippy::alloc_instead_of_core,
    clippy::single_call_fn,
    clippy::default_numeric_fallback,
    clippy::shadow_reuse,
    clippy::question_mark_used,
    clippy::use_debug,
    clippy::unnecessary_debug_formatting,
    clippy::missing_assert_message,
    clippy::uninlined_format_args,
    clippy::integer_division,
    clippy::integer_division_remainder_used,
    clippy::cast_possible_truncation,
    clippy::as_conversions,
    clippy::too_many_lines,
    clippy::panic,
    clippy::field_reassign_with_default,
    clippy::clone_on_ref_ptr,
    clippy::arithmetic_side_effects,
    clippy::panic_in_result_fn,
    clippy::string_add,
    clippy::str_to_string,
    clippy::doc_markdown,
    clippy::arbitrary_source_item_ordering,
    clippy::unnecessary_literal_bound,
    clippy::option_if_let_else,
    clippy::min_ident_chars,
    clippy::little_endian_bytes,
    clippy::unused_trait_names,
    reason = "DDS examples require print logging, panic unwraps, and simplified structural configurations for demonstration purposes."
)]
#![allow(clippy::allow_attributes, reason = "Allow attributes needed for buffer literals")]
use dds::cdr::{CdrDeserialize, CdrSerialize, Endianness};
use dds::security::{
    AccessControl, Authentication, BuiltinAccessControl, BuiltinAuthentication,
    BuiltinCryptography, Cryptography, HandshakeHandle, HandshakeToken, IdentityHandle,
    PermissionsToken,
};
use dds::types::qos::DomainParticipantQos;
use std::net::UdpSocket;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
struct SecureMessage {
    id: u32,
    content: String,
}

impl CdrSerialize for SecureMessage {
    fn serialize(&self, serializer: &mut dds::cdr::CdrSerializer) -> dds::cdr::CdrResult<()> {
        serializer.serialize_u32(self.id);
        serializer.serialize_str(&self.content);
        Ok(())
    }
}

impl CdrDeserialize for SecureMessage {
    fn deserialize(deserializer: &mut dds::cdr::CdrDeserializer<'_>) -> dds::cdr::CdrResult<Self> {
        let id = deserializer.deserialize_u32()?;
        let content = deserializer.deserialize_str()?;
        Ok(Self { id, content })
    }
}

struct SecurePacket {
    header: dds::security::CryptoHeader,
    ciphertext: Vec<u8>,
    footer: dds::security::CryptoFooter,
}

impl CdrSerialize for SecurePacket {
    fn serialize(&self, serializer: &mut dds::cdr::CdrSerializer) -> dds::cdr::CdrResult<()> {
        self.header.serialize(serializer)?;
        serializer.serialize_u32(self.ciphertext.len() as u32);
        for b in &self.ciphertext {
            serializer.serialize_u8(*b);
        }
        self.footer.serialize(serializer)?;
        Ok(())
    }
}

impl CdrDeserialize for SecurePacket {
    fn deserialize(deserializer: &mut dds::cdr::CdrDeserializer<'_>) -> dds::cdr::CdrResult<Self> {
        let header = dds::security::CryptoHeader::deserialize(deserializer)?;
        let len = deserializer.deserialize_u32()? as usize;
        let mut ciphertext = Vec::with_capacity(len);
        for _ in 0..len {
            ciphertext.push(deserializer.deserialize_u8()?);
        }
        let footer = dds::security::CryptoFooter::deserialize(deserializer)?;
        Ok(Self { header, ciphertext, footer })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str);

    match mode {
        Some("pub") => {
            println!("==============================================================");
            println!("DDS SECURITY PUBLISHER PROCESS (INITIATOR)");
            println!("==============================================================");

            let socket = UdpSocket::bind("127.0.0.1:7928")?;
            socket.set_read_timeout(Some(Duration::from_secs(30)))?;
            println!("[Publisher] Ephemeral socket bound to 127.0.0.1:7928");

            let auth = BuiltinAuthentication::new();
            let crypto = BuiltinCryptography::new();

            let qos = DomainParticipantQos::default();
            let (alice_id, _) = auth.validate_local_identity(0, &qos)?;
            let bob_id = IdentityHandle(2);
            println!("[Publisher] Local identity validated successfully.");

            std::thread::sleep(Duration::from_millis(500));

            let (mut handshake_alice, token_req) = auth.begin_handshake_request(&alice_id, &bob_id)?;
            println!("[Publisher] Step 1: Handshake request token generated.");

            let req_bytes = dds::cdr::serialize_to_bytes(&token_req, Endianness::LittleEndian)?;
            socket.send_to(&req_bytes, "127.0.0.1:7929")?;
            println!("[Publisher] Step 1: Request token sent to subscriber.");

            #[allow(clippy::unseparated_literal_suffix, clippy::allow_attributes, reason = "Buffer literal format required")]
let mut buf = [0u8; 4096];
            let (len_reply, _) = socket.recv_from(&mut buf)?;
            let token_reply: HandshakeToken =
                dds::cdr::deserialize_from_slice(&buf[..len_reply], Endianness::LittleEndian)?;
            println!("[Publisher] Step 2: Handshake reply token received from subscriber.");

            let token_final = auth
                .process_handshake(&mut handshake_alice, token_reply)?
                .ok_or("Expected final handshake token")?;
            println!("[Publisher] Step 3: Handshake final token generated.");

            let final_bytes = dds::cdr::serialize_to_bytes(&token_final, Endianness::LittleEndian)?;
            socket.send_to(&final_bytes, "127.0.0.1:7929")?;
            println!("[Publisher] Step 3: Final token sent to subscriber.");

            let secret = auth.get_shared_secret(&handshake_alice)?;
            println!("[Publisher] Cryptographic handshake completed successfully.");

            let local_handle = crypto.register_local_participant(&alice_id)?;
            let remote_handle = crypto.register_matched_remote_participant(
                &local_handle,
                &bob_id,
                &secret,
            )?;

            let message = SecureMessage {
                id: 777,
                content: "Top Secret: Multi-process secure DDS payload".to_string(),
            };
            println!("\n[Publisher] Original Payload: {:?}", message);

            let payload = dds::cdr::serialize_to_bytes(&message, Endianness::LittleEndian)?;
            let (ciphertext, header, footer) =
                crypto.encrypt_payload(&payload, &local_handle, &remote_handle)?;
            println!("[Publisher] Payload encrypted via AES-128-GCM.");

            let packet = SecurePacket {
                header,
                ciphertext,
                footer,
            };
            let packet_bytes = dds::cdr::serialize_to_bytes(&packet, Endianness::LittleEndian)?;
            socket.send_to(&packet_bytes, "127.0.0.1:7929")?;
            println!("[Publisher] Encrypted SecurePacket sent to subscriber.");
            println!("==============================================================");
        }
        Some("sub") => {
            println!("==============================================================");
            println!("DDS SECURITY SUBSCRIBER PROCESS (RESPONDER)");
            println!("==============================================================");

            let socket = UdpSocket::bind("127.0.0.1:7929")?;
            socket.set_read_timeout(Some(Duration::from_secs(30)))?;
            println!("[Subscriber] Socket bound and listening on 127.0.0.1:7929");

            let auth = BuiltinAuthentication::new();
            let crypto = BuiltinCryptography::new();

            let qos = DomainParticipantQos::default();
            let (bob_id, _) = auth.validate_local_identity(0, &qos)?;
            let alice_id = IdentityHandle(1);
            println!("[Subscriber] Local identity validated successfully.");

            #[allow(clippy::separated_literal_suffix, clippy::allow_attributes, reason = "Buffer literal format required")]
let mut buf = [0_u8; 4096];
            let (len_req, _) = socket.recv_from(&mut buf)?;
            let token_req: HandshakeToken =
                dds::cdr::deserialize_from_slice(&buf[..len_req], Endianness::LittleEndian)?;
            println!("[Subscriber] Step 1: Handshake request token received from publisher.");

            let mut handshake_bob = HandshakeHandle(0);
            let token_reply = auth
                .process_handshake(&mut handshake_bob, token_req)?
                .ok_or("Expected handshake reply token")?;
            println!("[Subscriber] Step 2: Handshake reply token generated.");

            let reply_bytes = dds::cdr::serialize_to_bytes(&token_reply, Endianness::LittleEndian)?;
            socket.send_to(&reply_bytes, "127.0.0.1:7928")?;
            println!("[Subscriber] Step 2: Reply token sent to publisher.");

            let (len_final, _) = socket.recv_from(&mut buf)?;
            let token_final: HandshakeToken =
                dds::cdr::deserialize_from_slice(&buf[..len_final], Endianness::LittleEndian)?;
            println!("[Subscriber] Step 3: Handshake final token received.");

            let none_res = auth.process_handshake(&mut handshake_bob, token_final)?;
            assert!(none_res.is_none());
            println!("[Subscriber] Step 4: Handshake completed successfully.");

            let secret = auth.get_shared_secret(&handshake_bob)?;
            println!("[Subscriber] Shared session key derived successfully.");

            let local_handle = crypto.register_local_participant(&bob_id)?;
            let remote_handle = crypto.register_matched_remote_participant(
                &local_handle,
                &alice_id,
                &secret,
            )?;

            let (len_pkt, _) = socket.recv_from(&mut buf)?;
            let packet: SecurePacket =
                dds::cdr::deserialize_from_slice(&buf[..len_pkt], Endianness::LittleEndian)?;
            println!("\n[Subscriber] Encrypted SecurePacket received from publisher.");

            let decrypted_payload = crypto.decrypt_payload(
                &packet.ciphertext,
                &packet.header,
                &packet.footer,
                &local_handle,
                &remote_handle,
            )?;
            println!("[Subscriber] Ciphertext decrypted successfully using derived key.");

            let decoded: SecureMessage =
                dds::cdr::deserialize_from_slice(&decrypted_payload, Endianness::LittleEndian)?;
            println!("[Subscriber] Decoded Payload: {:?}", decoded);

            assert_eq!(decoded.id, 777);
            assert_eq!(decoded.content, "Top Secret: Multi-process secure DDS payload");
            println!("==============================================================");
        }
        None => {
            println!("==============================================================");
            println!("DDS SECURITY PLUGINS INTEGRATION DEMONSTRATION (Single-process mode)");
            println!("Tip: Run as two processes via:");
            println!("  cargo run --example secure_pubsub sub");
            println!("  cargo run --example secure_pubsub pub");
            println!("==============================================================");

            // 1. Initialize security plugins
            let auth = BuiltinAuthentication::new();
            let access = BuiltinAccessControl::new();
            let crypto = BuiltinCryptography::new();

            println!("[1/5] Validating Local Participant Identities...");
            let qos = DomainParticipantQos::default();
            let (alice_id, alice_token) = auth.validate_local_identity(0, &qos)?;
            let (bob_id, bob_token) = auth.validate_local_identity(0, &qos)?;
            println!(
                "  -> Participant Alice Identity Validated (subject: {:?})",
                alice_token.properties[0].1
            );
            println!(
                "  -> Participant Bob Identity Validated (subject: {:?})",
                bob_token.properties[0].1
            );

            println!("\n[2/5] Initiating Mutual 3-Step Handshake (ECDH key exchange)...");
            // Step 1: Initiator (Alice) starts handshake request
            let (mut handshake_alice, token_req) = auth.begin_handshake_request(&alice_id, &bob_id)?;
            println!(
                "  -> Step 1: Alice generates handshake Request. Local pub_key: {}",
                token_req.properties[1].1[..16].to_string() + "..."
            );

            // Step 2: Responder (Bob) processes Request and replies with Reply
            let mut handshake_bob = HandshakeHandle(0);
            let token_reply = auth
                .process_handshake(&mut handshake_bob, token_req)?
                .ok_or("Expected handshake reply")?;
            println!(
                "  -> Step 2: Bob processes Request and responds with Reply. Local pub_key: {}",
                token_reply.properties[1].1[..16].to_string() + "..."
            );

            // Step 3: Alice processes Reply and responds with Final
            let token_final = auth
                .process_handshake(&mut handshake_alice, token_reply)?
                .ok_or("Expected handshake final")?;
            println!("  -> Step 3: Alice processes Reply and responds with Final.");

            // Bob processes Final step (Mutual handshake completes)
            let none_res = auth.process_handshake(&mut handshake_bob, token_final)?;
            assert!(none_res.is_none());
            println!("  -> Step 4: Bob processes Final. Mutual verification complete!");

            // Extract shared secrets (Derived via ECDH)
            let secret_alice = auth.get_shared_secret(&handshake_alice)?;
            let secret_bob = auth.get_shared_secret(&handshake_bob)?;
            assert_eq!(secret_alice.0, secret_bob.0);
            println!("  -> Shared Session Key derived successfully on both sides.");

            println!("\n[3/5] Performing Access Control Permissions Checks...");
            let alice_perms_token = PermissionsToken {
                class_id: "DDS:Access:Permissions:1.0".to_string(),
                properties: vec![("allow_topic".to_string(), "SecureTelemetry".to_string())],
            };
            let bob_perms_token = PermissionsToken {
                class_id: "DDS:Access:Permissions:1.0".to_string(),
                properties: vec![("allow_topic".to_string(), "SecureTelemetry".to_string())],
            };

            let alice_perms =
                access.validate_remote_permissions(&alice_id, &alice_id, alice_perms_token)?;
            let bob_perms = access.validate_remote_permissions(&bob_id, &bob_id, bob_perms_token)?;

            let can_alice_write_telemetry = access.check_create_writer(&alice_perms, "SecureTelemetry")?;
            let can_alice_write_commands = access.check_create_writer(&alice_perms, "CommandTelemetry")?;
            println!(
                "  -> Alice authorized to write 'SecureTelemetry'? {}",
                can_alice_write_telemetry
            );
            println!(
                "  -> Alice authorized to write 'CommandTelemetry'? {}",
                can_alice_write_commands
            );
            assert!(can_alice_write_telemetry);
            assert!(!can_alice_write_commands);

            let can_bob_read_telemetry = access.check_create_reader(&bob_perms, "SecureTelemetry")?;
            println!(
                "  -> Bob authorized to read 'SecureTelemetry'? {}",
                can_bob_read_telemetry
            );
            assert!(can_bob_read_telemetry);

            println!("\n[4/5] Registering Cryptographic Handlers...");
            let alice_crypto = crypto.register_local_participant(&alice_id)?;
            let bob_crypto =
                crypto.register_matched_remote_participant(&alice_crypto, &bob_id, &secret_alice)?;
            println!("  -> Cryptographic channels established.");

            println!("\n[5/5] Encrypting & Decrypting published payload (AES-128-GCM)...");
            let original = SecureMessage {
                id: 777,
                content: "Top Secret: Radar tracking data".to_string(),
            };

            let serialized_plain = dds::cdr::serialize_to_bytes(&original, Endianness::LittleEndian)?;
            println!("  -> Original Payload: {:?}", original);
            println!(
                "  -> Plaintext bytes (len = {}): {:02x?}",
                serialized_plain.len(),
                serialized_plain
            );

            let (ciphertext, header, footer) =
                crypto.encrypt_payload(&serialized_plain, &alice_crypto, &bob_crypto)?;
            println!(
                "  -> Ciphertext bytes (len = {}): {:02x?}",
                ciphertext.len(),
                ciphertext
            );
            println!(
                "  -> CryptoHeader envelope: session_id = {:02x?}, IV = {:02x?}",
                header.session_id, header.initialization_vector
            );
            println!(
                "  -> CryptoFooter envelope: mac_tag = {:02x?}",
                footer.mac_tag
            );

            let decrypted_plain =
                crypto.decrypt_payload(&ciphertext, &header, &footer, &alice_crypto, &bob_crypto)?;
            let decoded: SecureMessage =
                dds::cdr::deserialize_from_slice(&decrypted_plain, Endianness::LittleEndian)?;
            println!(
                "  -> Decrypted Plaintext bytes (len = {}): {:02x?}",
                decrypted_plain.len(),
                decrypted_plain
            );
            println!("  -> Decoded Payload: {:?}", decoded);

            assert_eq!(original, decoded);
            println!("\nDDS Security pipeline roundtrip completed successfully!");
            println!("==============================================================");
        }
        Some(other) => {
            println!("Unknown mode: {}. Expected 'pub' or 'sub' or no arguments.", other);
        }
    }

    Ok(())
}
