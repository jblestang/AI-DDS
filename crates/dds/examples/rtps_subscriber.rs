//! RTPS Subscriber example binary listening for RTPS Data submessages over UDP loopback.

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
    reason = "DDS examples require print logging, panic unwraps, and simplified structural configurations for demonstration purposes."
)]

use dds::rtps::{parse_rtps_message, Submessage, UdpTransport};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting RTPS Subscriber binary listening on port 7905...");

    // Bind transport to port 7905
    let transport = UdpTransport::bind(7905)?;

    println!("Polling for incoming RTPS packets...");
    loop {
        if let Some((bytes, _src_addr)) = transport.recv()? {
            println!(
                "Received {} bytes on socket. Parsing RTPS message...",
                bytes.len()
            );

            match parse_rtps_message(&bytes) {
                Ok((header, submessages)) => {
                    println!(
                        "Parsed RTPS message header prefix: {:?}",
                        header.guid_prefix
                    );

                    for sub in submessages {
                        if let Submessage::Data(data) = sub {
                            println!("Parsed DATA submessage. writer_sn = {:?}", data.writer_sn);

                            // Deserialize HelloWorld struct
                            let payload = data.serialized_payload;
                            if payload.len() >= 4 {
                                let mut id_bytes = [0_u8; 4];
                                id_bytes.copy_from_slice(&payload[0..4]);
                                let id = u32::from_le_bytes(id_bytes);
                                let msg = String::from_utf8_lossy(&payload[4..]).to_string();

                                println!("Successfully received and decoded HelloWorld struct:");
                                println!("  -> id: {}", id);
                                println!("  -> msg: '{}'", msg);
                                return Ok(());
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("Failed to parse RTPS message: {:?}", e);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
