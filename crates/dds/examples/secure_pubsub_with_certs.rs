//! DDS Secure Publish/Subscribe using certificates loaded from disk.
//!
//! This example demonstrates how to read PEM‑encoded certificates and private keys
//! from files and use them with the BuiltinAuthentication plugin. The current
//! BuiltinAuthentication implementation does not yet consume the certificate data,
//! but the example shows how you would load them for a future implementation.

#![forbid(unsafe_code)]
#![warn(
    rust_2018_idioms,
    nonstandard_style,
    future_incompatible,
    clippy::all,
    clippy::restriction,
    clippy::pedantic,
    clippy::nursery,
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
    AccessControl, Authentication, BuiltinAccessControl, BuiltinAuthentication, BuiltinCryptography,
    Cryptography, HandshakeHandle, HandshakeToken, IdentityHandle, PermissionsToken,
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

#[derive(Debug, Clone, PartialEq, Eq)]
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
    // ---------------------------------------------------------------------
    let alice_cert = std::fs::read_to_string("certs/alice_cert.pem")?;
    let alice_key = std::fs::read_to_string("certs/alice_key.pem")?;
    let bob_cert = std::fs::read_to_string("certs/bob_cert.pem")?;
    let bob_key = std::fs::read_to_string("certs/bob_key.pem")?;
    let ca_cert = std::fs::read_to_string("certs/ca_cert.pem")?;

    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str);

    match mode {
        Some("pub") => {
            println!("=== DDS SECURITY PUBLISHER (certificates) ===");
            let socket = UdpSocket::bind("127.0.0.1:7928")?;
            socket.set_read_timeout(Some(Duration::from_secs(30)))?;
            let auth = BuiltinAuthentication::new();
            let crypto = BuiltinCryptography::new();
            let mut qos = DomainParticipantQos::default();
            qos.property.value.push(("dds.sec.auth.identity_ca".to_owned(), format!("data:,{}", ca_cert)));
            qos.property.value.push(("dds.sec.auth.identity_certificate".to_owned(), format!("data:,{}", alice_cert)));
            qos.property.value.push(("dds.sec.auth.private_key".to_owned(), format!("data:,{}", alice_key)));
            let (alice_id, _) = auth.validate_local_identity(0, &qos)?;
            let bob_id = IdentityHandle(2);
            std::thread::sleep(Duration::from_millis(500));
            let (mut handshake_alice, token_req) = auth.begin_handshake_request(&alice_id, &bob_id)?;
            let req_bytes = dds::cdr::serialize_to_bytes(&token_req, Endianness::LittleEndian)?;
            socket.send_to(&req_bytes, "127.0.0.1:7929")?;

            #[allow(clippy::unseparated_literal_suffix, clippy::allow_attributes, reason = "Buffer literal format required")]
            let mut buf = [0u8; 4096];
            let (len_reply, _) = socket.recv_from(&mut buf)?;
            let token_reply: HandshakeToken =
                dds::cdr::deserialize_from_slice(&buf[..len_reply], Endianness::LittleEndian)?;
            let token_final = auth
                .process_handshake(&mut handshake_alice, token_reply)?
                .ok_or("Expected final handshake token")?;
            let final_bytes = dds::cdr::serialize_to_bytes(&token_final, Endianness::LittleEndian)?;
            socket.send_to(&final_bytes, "127.0.0.1:7929")?;

            let secret = auth.get_shared_secret(&handshake_alice)?;
            let local_handle = crypto.register_local_participant(&alice_id)?;
            let remote_handle = crypto.register_matched_remote_participant(
                &local_handle,
                &bob_id,
                &secret,
            )?;

            let message = SecureMessage { id: 777, content: "Top Secret: Disk cert demo".to_string() };
            let payload = dds::cdr::serialize_to_bytes(&message, Endianness::LittleEndian)?;
            let (ciphertext, header, footer) =
                crypto.encrypt_payload(&payload, &local_handle, &remote_handle)?;
            let packet = SecurePacket { header, ciphertext, footer };
            let packet_bytes = dds::cdr::serialize_to_bytes(&packet, Endianness::LittleEndian)?;
            socket.send_to(&packet_bytes, "127.0.0.1:7929")?;
            println!("=== Publisher finished ===");
        }
        Some("sub") => {
            println!("=== DDS SECURITY SUBSCRIBER (certificates) ===");
            let socket = UdpSocket::bind("127.0.0.1:7929")?;
            socket.set_read_timeout(Some(Duration::from_secs(30)))?;
            let auth = BuiltinAuthentication::new();
            let crypto = BuiltinCryptography::new();
            let mut qos = DomainParticipantQos::default();
            qos.property.value.push(("dds.sec.auth.identity_ca".to_owned(), format!("data:,{}", ca_cert)));
            qos.property.value.push(("dds.sec.auth.identity_certificate".to_owned(), format!("data:,{}", bob_cert)));
            qos.property.value.push(("dds.sec.auth.private_key".to_owned(), format!("data:,{}", bob_key)));
            let (bob_id, _) = auth.validate_local_identity(0, &qos)?;
            let alice_id = IdentityHandle(1);

            #[allow(clippy::separated_literal_suffix, clippy::allow_attributes, reason = "Buffer literal format required")]
            let mut buf = [0_u8; 4096];
            let (len_req, _) = socket.recv_from(&mut buf)?;
            let token_req: HandshakeToken =
                dds::cdr::deserialize_from_slice(&buf[..len_req], Endianness::LittleEndian)?;
            let mut handshake_bob = HandshakeHandle(0);
            let token_reply = auth
                .process_handshake(&mut handshake_bob, token_req)?
                .ok_or("Expected handshake reply token")?;
            let reply_bytes = dds::cdr::serialize_to_bytes(&token_reply, Endianness::LittleEndian)?;
            socket.send_to(&reply_bytes, "127.0.0.1:7928")?;
            let (len_final, _) = socket.recv_from(&mut buf)?;
            let token_final: HandshakeToken =
                dds::cdr::deserialize_from_slice(&buf[..len_final], Endianness::LittleEndian)?;
            auth.process_handshake(&mut handshake_bob, token_final)?;
            let secret = auth.get_shared_secret(&handshake_bob)?;
            let local_handle = crypto.register_local_participant(&bob_id)?;
            let remote_handle = crypto.register_matched_remote_participant(
                &local_handle,
                &alice_id,
                &secret,
            )?;
            let (len_pkt, _) = socket.recv_from(&mut buf)?;
            let packet: SecurePacket =
                dds::cdr::deserialize_from_slice(&buf[..len_pkt], Endianness::LittleEndian)?;
            let decrypted = crypto.decrypt_payload(
                &packet.ciphertext,
                &packet.header,
                &packet.footer,
                &local_handle,
                &remote_handle,
            )?;
            let decoded: SecureMessage =
                dds::cdr::deserialize_from_slice(&decrypted, Endianness::LittleEndian)?;
            println!("Received secure message: {:?}", decoded);
            println!("=== Subscriber finished ===");
        }
        None => {
            println!("Run with 'pub' or 'sub' arguments.");
        }
        Some(other) => {
            println!("Unknown mode: {}", other);
        }
    }
    Ok(())
}
