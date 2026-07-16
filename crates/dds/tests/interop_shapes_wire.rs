//! Wire-level tests for the OMG DDS Shapes Demo `ShapeType` (no external deps).

mod interop_common;
mod shapes_common;

use dds::cdr::{CdrSerialize, EncapsulationHeader, EncapsulationKind, Endianness};
use dds::core::TypeSupport;
use shapes_common::{ShapeType, ShapeTypeSupport, SHAPES_TOPICS, SHAPES_TYPE};
use std::any::Any;

#[test]
fn shapes_type_name_matches_omg_demo() {
    assert_eq!(SHAPES_TYPE, "org::omg::dds::demo::ShapeType");
    assert_eq!(SHAPES_TOPICS, &["Square", "Circle", "Triangle"]);
}

#[test]
fn shapes_cdr_encapsulation_matches_interop_convention() {
    let ts = ShapeTypeSupport;
    let sample = ShapeType {
        color: "PURPLE".to_string(),
        x: 1,
        y: 2,
        shapesize: 40,
    };
    let wire = ts.serialize(&sample as &dyn Any).unwrap();
    assert_eq!(&wire[0..2], &[0x00, 0x01]); // CdrLe kind id (big-endian on wire)

    let mut header = dds::cdr::CdrSerializer::new(Endianness::LittleEndian);
    EncapsulationHeader::new(EncapsulationKind::CdrLe).serialize(&mut header);
    assert_eq!(header.into_bytes().as_ref(), &wire[0..4]);
}

#[test]
fn shapes_deserialize_accepts_cdr_be_from_vendors() {
    let ts = ShapeTypeSupport;
    let mut ser = dds::cdr::CdrSerializer::new(Endianness::BigEndian);
    EncapsulationHeader::new(EncapsulationKind::CdrBe).serialize(&mut ser);
    ShapeType {
        color: "ORANGE".to_string(),
        x: 5,
        y: 6,
        shapesize: 15,
    }
    .serialize(&mut ser)
    .unwrap();
    let boxed = ts.deserialize(ser.bytes()).expect("deserialize CdrBe");
    let shape = boxed.downcast_ref::<ShapeType>().unwrap();
    assert_eq!(shape.color, "ORANGE");
    assert_eq!(shape.shapesize, 15);
}

#[test]
fn shapes_deserialize_fastdds_cdr_fixture() {
    let ts = ShapeTypeSupport;
    // Captured from `dump_shape_cdr` (Fast DDS XCDR plain CDR / CdrLe).
    let wire: &[u8] = &[
        0x00, 0x01, 0x00, 0x00, 0x0f, 0x00, 0x00, 0x00, 0x53, 0x71, 0x75, 0x61, 0x72, 0x65,
        0x2d, 0x66, 0x61, 0x73, 0x74, 0x64, 0x64, 0x73, 0x00, 0x00, 0x0a, 0x00, 0x00, 0x00,
        0x14, 0x00, 0x00, 0x00, 0x1e, 0x00, 0x00, 0x00,
    ];
    let boxed = ts.deserialize(wire).expect("deserialize FastDDS ShapeType wire");
    let shape = boxed.downcast_ref::<ShapeType>().unwrap();
    assert_eq!(shape.color, "Square-fastdds");
    assert_eq!(shape.x, 10);
    assert_eq!(shape.y, 20);
    assert_eq!(shape.shapesize, 30);
}
