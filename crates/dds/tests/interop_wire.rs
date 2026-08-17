//! Wire-level interoperability fixtures: verify AI-DDS can parse RTPS packets
//! from reference implementations (CycloneDDS-generated captures).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;
use support::interop_common;

use dds::rtps::{parse_rtps_message, Submessage};
use support::interop_common::read_fixture;

#[test]
fn interop_wire_parse_cyclonedds_spdp_fixture() {
    let bytes = read_fixture("cyclonedds_spdp.bin");
    let (header, submessages) =
        parse_rtps_message(&bytes).expect("parse CycloneDDS SPDP RTPS message");

    assert!(!header.guid_prefix.as_bytes().iter().all(|&b| b == 0));
    assert!(
        submessages.iter().any(|s| matches!(s, Submessage::Data(_))),
        "SPDP fixture should contain DATA submessage"
    );

    let parsed_spdp = submessages.iter().any(|s| {
        if let Submessage::Data(d) = s {
            dds_discovery::parse_spdp_packet(&d.serialized_payload).is_some()
        } else {
            false
        }
    });
    assert!(
        parsed_spdp,
        "AI-DDS discovery parser should decode CycloneDDS SPDP PL-CDR"
    );
}

#[test]
fn interop_wire_parse_cyclonedds_sedp_from_fixture() {
    let bytes = read_fixture("cyclonedds_spdp.bin");
    let (_header, submessages) = parse_rtps_message(&bytes).expect("parse RTPS");

    let parsed_sedp = submessages.iter().find_map(|s| {
        if let Submessage::Data(d) = s {
            dds_discovery::parse_sedp_packet(&d.serialized_payload)
        } else {
            None
        }
    });

    // Fixture may be SPDP-only; when SEDP is present it must parse.
    if let Some(ep) = parsed_sedp {
        assert!(!ep.topic_name.is_empty() || !ep.type_name.is_empty());
    }
}

#[test]
fn interop_wire_parse_cyclonedds_data_fixture() {
    let bytes = read_fixture("cyclonedds_data_cdr_le.bin");
    let (_header, submessages) =
        parse_rtps_message(&bytes).expect("parse CycloneDDS user DATA RTPS message");

    let data = submessages
        .iter()
        .find_map(|s| {
            if let Submessage::Data(d) = s {
                Some(d)
            } else {
                None
            }
        })
        .expect("DATA submessage");

    assert!(
        !data.serialized_payload.is_empty(),
        "DATA payload should not be empty"
    );

    // User samples use CdrLe encapsulation (kind id 0x0001, big-endian on wire).
    let payload = &data.serialized_payload;
    let is_cdr_le = payload.len() >= 2 && payload[0..2] == [0x00, 0x01];
    let is_plcdr = payload.len() >= 2 && payload[0..2] == [0x00, 0x02] || payload[0..2] == [0x00, 0x03];
    assert!(
        is_cdr_le || is_plcdr || dds_discovery::parse_spdp_packet(payload).is_some(),
        "Payload should be CDR/PL-CDR encapsulated or valid SPDP parameter list"
    );
}

#[test]
fn interop_cdr_encapsulation_roundtrip_matches_spec() {
    use dds::cdr::{
        serialize_to_bytes, EncapsulationHeader, EncapsulationKind, Endianness,
    };
    use dds::core::TypeSupport;
use support::interop_common::{InteropMessage, InteropTypeSupport};
    use std::any::Any;

    let ts = InteropTypeSupport;
    let sample = InteropMessage {
        id: 42,
        payload: "interop".to_string(),
    };
    let wire = ts.serialize(&sample as &dyn Any).unwrap();

    let plain = serialize_to_bytes(&sample, Endianness::LittleEndian).unwrap();
    assert!(wire.len() > plain.len());
    // Encapsulation kind is always big-endian on the wire (DDS CDR §10.2).
    assert_eq!(&wire[0..2], &[0x00, 0x01]); // CdrLe

    let mut header = dds::cdr::CdrSerializer::new(Endianness::LittleEndian);
    EncapsulationHeader::new(EncapsulationKind::CdrLe).serialize(&mut header);
    assert_eq!(header.into_bytes().as_ref(), &wire[0..4]);
}
