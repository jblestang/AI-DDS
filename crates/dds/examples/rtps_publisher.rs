//! RTPS Publisher example binary sending RTPS Data submessages over UDP loopback.

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

use dds::rtps::{serialize_rtps_message, Data, Endianness, RtpsHeader, Submessage, UdpTransport};
use dds::types::guid::{EntityId, GuidPrefix, SequenceNumber};
use dds::types::locator::Locator;

struct HelloWorld {
    id: u32,
    msg: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting RTPS Publisher binary...");

    // Bind transport to an ephemeral port
    let transport = UdpTransport::bind(0)?;

    // Define receiver locator (matching the subscriber port)
    let dest = Locator::udpv4(std::net::Ipv4Addr::LOCALHOST, 7905);

    // Prepare message payload
    let hw = HelloWorld {
        id: 101,
        msg: "Antigravity RTPS packet!".to_string(),
    };

    // Serialize payload
    let mut payload = Vec::new();
    payload.extend_from_slice(&hw.id.to_le_bytes());
    payload.extend_from_slice(hw.msg.as_bytes());

    // Prepare RTPS Header
    let prefix = GuidPrefix::new([0x01; 12]);
    let header = RtpsHeader::new(prefix);

    // Prepare RTPS Data submessage
    let data_sub = Data {
        reader_id: EntityId::new([1, 0, 0, 4]),
        writer_id: EntityId::new([2, 0, 0, 3]),
        writer_sn: SequenceNumber(1),
        inline_qos: None,
        serialized_payload: payload.into(),
    };

    let submessages = vec![Submessage::Data(data_sub)];

    // Serialize RTPS message
    let bytes = serialize_rtps_message(&header, &submessages, Endianness::LittleEndian);

    println!("Sending RTPS message to 127.0.0.1:7905...");
    transport.send(&bytes, &dest)?;
    println!("Successfully sent RTPS message!");

    Ok(())
}
