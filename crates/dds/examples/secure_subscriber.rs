//! DDS Secure Subscriber process.
//!
//! This example runs as the subscriber (handshake responder) in a multi-process
//! secure DDS setup. It performs the 3-step handshake with secure_publisher,
//! derives the key, receives the encrypted packet, and decrypts it.
//!
//! Spec Reference: OMG DDS Security 1.2 §8.4

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
    clippy::separated_literal_suffix,
    clippy::unseparated_literal_suffix,
    clippy::missing_asserts_for_indexing,
    reason = "DDS examples require print logging, panic unwraps, and simplified structural configurations for demonstration purposes."
)]

use dds::cdr::{CdrDeserialize, Endianness};
use dds::security::{
    Authentication as _, BuiltinAuthentication, BuiltinCryptography, Cryptography as _,
    HandshakeHandle, HandshakeToken, IdentityHandle,
};
use dds::types::qos::DomainParticipantQos;
use std::net::UdpSocket;
use std::time::Duration;

// 1/3 Comment-to-code ratio.
// Define a sample message to publish securely.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SecureMessage {
    id: u32,
    content: String,
}

impl CdrDeserialize for SecureMessage {
    fn deserialize(deserializer: &mut dds::cdr::CdrDeserializer<'_>) -> dds::cdr::CdrResult<Self> {
        let id = deserializer.deserialize_u32()?;
        let content = deserializer.deserialize_str()?;
        Ok(Self { id, content })
    }
}

// Custom wrapper to hold the encrypted payload + CryptoHeader + CryptoFooter.
struct SecurePacket {
    header: dds::security::CryptoHeader,
    ciphertext: Vec<u8>,
    footer: dds::security::CryptoFooter,
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
    println!("==============================================================");
    println!("DDS SECURE SUBSCRIBER PROCESS (RESPONDER)");
    println!("==============================================================");

    // 1. Bind local UDP socket for communication.
    let socket = UdpSocket::bind("127.0.0.1:7911")?;
    socket.set_read_timeout(Some(Duration::from_secs(30)))?;
    println!("[Subscriber] Socket bound and listening on 127.0.0.1:7911");

    // 2. Initialize security plugins.
    let auth = BuiltinAuthentication::new();
    let crypto = BuiltinCryptography::new();

    // 3. Validate local participant identity.
    let qos = DomainParticipantQos::default();
    let (bob_id, _) = auth.validate_local_identity(0, &qos)?;
    let alice_id = IdentityHandle(1); // Mock remote identity handle for Publisher.
    println!("[Subscriber] Local identity validated successfully.");

    // 4. STEP 1 & 2: Receive Handshake Request & Respond with Handshake Reply.
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

    // Serialize and send the reply token to the Publisher.
    let reply_bytes = dds::cdr::serialize_to_bytes(&token_reply, Endianness::LittleEndian)?;
    socket.send_to(&reply_bytes, "127.0.0.1:7910")?;
    println!("[Subscriber] Step 2: Reply token sent to publisher.");

    // 5. STEP 3: Receive Handshake Final & Complete Handshake.
    let (len_final, _) = socket.recv_from(&mut buf)?;
    let token_final: HandshakeToken =
        dds::cdr::deserialize_from_slice(&buf[..len_final], Endianness::LittleEndian)?;
    println!("[Subscriber] Step 3: Handshake final token received.");

    let none_res = auth.process_handshake(&mut handshake_bob, token_final)?;
    assert!(none_res.is_none());
    println!("[Subscriber] Step 4: Handshake completed successfully.");

    // 6. Derive shared secret and register crypto participants.
    let secret = auth.get_shared_secret(&handshake_bob)?;
    println!("[Subscriber] Shared session key derived successfully.");

    let local_handle = crypto.register_local_participant(&bob_id)?;
    let remote_handle = crypto.register_matched_remote_participant(
        &local_handle,
        &alice_id,
        &secret,
    )?;

    // 7. Receive and decrypt the encrypted packet.
    let (len_pkt, _) = socket.recv_from(&mut buf)?;
    let packet: SecurePacket =
        dds::cdr::deserialize_from_slice(&buf[..len_pkt], Endianness::LittleEndian)?;
    println!("\n[Subscriber] Encrypted SecurePacket received from publisher.");

    // Decrypt the payload.
    let decrypted_payload = crypto.decrypt_payload(
        &packet.ciphertext,
        &packet.header,
        &packet.footer,
        &local_handle,
        &remote_handle,
    )?;
    println!("[Subscriber] Ciphertext decrypted successfully using derived key.");

    // Deserialize decrypted plaintext back to SecureMessage.
    let decoded: SecureMessage =
        dds::cdr::deserialize_from_slice(&decrypted_payload, Endianness::LittleEndian)?;
    println!("[Subscriber] Decoded Payload: {:?}", decoded);

    assert_eq!(decoded.id, 42);
    assert_eq!(decoded.content, "Top Secret: Multi-process secure DDS payload");

    println!("==============================================================");
    println!("Subscriber successfully received and decrypted payload.");
    println!("==============================================================");
    Ok(())
}
