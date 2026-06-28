//! DDS Secure Publisher process.
//!
//! This example runs as the publisher (handshake initiator) in a multi-process
//! secure DDS setup. It performs the 3-step handshake with secure_subscriber,
//! derives the key, encrypts a payload using AES-128-GCM, and transmits it.
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

use dds::cdr::{CdrSerialize, Endianness};
use dds::security::{
    Authentication as _, BuiltinAuthentication, BuiltinCryptography, Cryptography as _,
    HandshakeToken, IdentityHandle,
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

impl CdrSerialize for SecureMessage {
    fn serialize(&self, serializer: &mut dds::cdr::CdrSerializer) -> dds::cdr::CdrResult<()> {
        serializer.serialize_u32(self.id);
        serializer.serialize_str(&self.content);
        Ok(())
    }
}

// Custom wrapper to hold the encrypted payload + CryptoHeader + CryptoFooter.
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("==============================================================");
    println!("DDS SECURE PUBLISHER PROCESS (INITIATOR)");
    println!("==============================================================");

    // 1. Bind local UDP socket for communication.
    let socket = UdpSocket::bind("127.0.0.1:7910")?;
    socket.set_read_timeout(Some(Duration::from_secs(30)))?;
    println!("[Publisher] Ephemeral socket bound to 127.0.0.1:7910");

    // 2. Initialize security plugins.
    let auth = BuiltinAuthentication::new();
    let crypto = BuiltinCryptography::new();

    // 3. Validate local participant identity.
    let qos = DomainParticipantQos::default();
    let (alice_id, _) = auth.validate_local_identity(0, &qos)?;
    let bob_id = IdentityHandle(2); // Mock remote identity handle for Subscriber.
    println!("[Publisher] Local identity validated successfully.");

    // Delay slightly to ensure the subscriber is bound and listening on port 7911.
    std::thread::sleep(Duration::from_millis(500));

    // 4. STEP 1: Generate Handshake Request.
    let (mut handshake_alice, token_req) = auth.begin_handshake_request(&alice_id, &bob_id)?;
    println!("[Publisher] Step 1: Handshake request token generated.");

    // Serialize and send the request token to the Subscriber.
    let req_bytes = dds::cdr::serialize_to_bytes(&token_req, Endianness::LittleEndian)?;
    socket.send_to(&req_bytes, "127.0.0.1:7911")?;
    println!("[Publisher] Step 1: Request token sent to subscriber (127.0.0.1:7911).");

    // 5. STEP 2: Receive Handshake Reply.
    let mut buf = [0_u8; 4096];
    let (len, _) = socket.recv_from(&mut buf)?;
    let token_reply: HandshakeToken =
        dds::cdr::deserialize_from_slice(&buf[..len], Endianness::LittleEndian)?;
    println!("[Publisher] Step 2: Handshake reply token received from subscriber.");

    // 6. STEP 3: Process Reply and Generate Final.
    let token_final = auth
        .process_handshake(&mut handshake_alice, token_reply)?
        .ok_or("Expected final handshake token")?;
    println!("[Publisher] Step 3: Handshake final token generated.");

    // Send the final handshake token to the Subscriber.
    let final_bytes = dds::cdr::serialize_to_bytes(&token_final, Endianness::LittleEndian)?;
    socket.send_to(&final_bytes, "127.0.0.1:7911")?;
    println!("[Publisher] Step 3: Final token sent to subscriber.");

    // 7. Derive shared secret and register crypto participants.
    let secret = auth.get_shared_secret(&handshake_alice)?;
    println!("[Publisher] Cryptographic handshake completed successfully.");

    let local_handle = crypto.register_local_participant(&alice_id)?;
    let remote_handle = crypto.register_matched_remote_participant(
        &local_handle,
        &bob_id,
        &secret,
    )?;

    // 8. Prepare and encrypt payload.
    let message = SecureMessage {
        id: 42,
        content: "Top Secret: Multi-process secure DDS payload".to_string(),
    };
    println!("\n[Publisher] Original Payload: {:?}", message);

    // Serialize message to plaintext using standard CDR.
    let payload = dds::cdr::serialize_to_bytes(&message, Endianness::LittleEndian)?;

    let (ciphertext, header, footer) =
        crypto.encrypt_payload(&payload, &local_handle, &remote_handle)?;
    println!("[Publisher] Payload encrypted via AES-128-GCM.");

    // 9. Package into SecurePacket and send to Subscriber.
    let packet = SecurePacket {
        header,
        ciphertext,
        footer,
    };
    let packet_bytes = dds::cdr::serialize_to_bytes(&packet, Endianness::LittleEndian)?;
    socket.send_to(&packet_bytes, "127.0.0.1:7911")?;
    println!("[Publisher] Encrypted SecurePacket sent to subscriber.");

    println!("==============================================================");
    println!("Publisher completed transmission successfully.");
    println!("==============================================================");
    Ok(())
}
