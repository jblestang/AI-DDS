//! Shared types and helpers for the OMG DDS Shapes Demo interoperability tests
//! (Square / Circle / Triangle topics, `ShapeType` keyed by `color`).

use dds::cdr::{
    CdrDeserialize, CdrDeserializer, CdrResult, CdrSerialize, CdrSerializer, EncapsulationHeader,
    EncapsulationKind, Endianness,
};
use dds::core::TypeSupport;
use std::any::Any;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

pub use super::interop_common::{InteropVendor, wait_output};

pub const SHAPES_TYPE: &str = "org::omg::dds::demo::ShapeType";

pub const SHAPES_TOPICS: &[&str] = &["Square", "Circle", "Triangle"];

/// Base domain id per vendor for shapes tests.
/// Individual tests pass `domain_offset`; `all_topics_*` helpers add +0/+1/+2 per topic.
pub fn shapes_base_domain(vendor: InteropVendor) -> u32 {
    match vendor {
        InteropVendor::CycloneDds => 120,
        // Align with proven Fast DDS interop domain block (83 + offset).
        InteropVendor::FastDds => 86,
        InteropVendor::OpenDds => 199,
    }
}

pub fn shapes_bin_dir(vendor: InteropVendor) -> Option<PathBuf> {
    super::interop_common::vendor_bin_dir(vendor).filter(|d| d.join("shapes_publisher").exists())
}

pub fn shapes_available(vendor: InteropVendor) -> bool {
    shapes_bin_dir(vendor).is_some()
}

pub fn shapes_publisher(vendor: InteropVendor) -> Option<PathBuf> {
    shapes_bin_dir(vendor).map(|d| d.join("shapes_publisher"))
}

pub fn shapes_subscriber(vendor: InteropVendor) -> Option<PathBuf> {
    shapes_bin_dir(vendor).map(|d| d.join("shapes_subscriber"))
}

pub fn spawn_shapes_publisher(
    vendor: InteropVendor,
    domain: u32,
    topic: &str,
    shape: &ShapeType,
) -> std::io::Result<std::process::Child> {
    let bin = shapes_publisher(vendor).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "{} shapes_publisher not found; rebuild peer apps",
                super::interop_common::vendor_display_name(vendor)
            ),
        )
    })?;
    let mut cmd = Command::new(&bin);
    cmd.arg(topic)
        .arg(&shape.color)
        .arg(shape.x.to_string())
        .arg(shape.y.to_string())
        .arg(shape.shapesize.to_string())
        .env("AIDDS_SHAPES_DOMAIN", domain.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Ok(ld) = std::env::var("LD_LIBRARY_PATH") {
        cmd.env("LD_LIBRARY_PATH", ld);
    }
    if vendor == InteropVendor::FastDds {
        cmd.env("AIDDS_FASTDDS_NO_PROFILE", "1");
    }
    cmd.env("AIDDS_SHAPES_WAIT_MATCH", "1").spawn()
}

pub fn spawn_shapes_subscriber(
    vendor: InteropVendor,
    domain: u32,
    topic: &str,
    expect_color: Option<&str>,
) -> std::io::Result<std::process::Child> {
    let bin = shapes_subscriber(vendor).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "{} shapes_subscriber not found; rebuild peer apps",
                super::interop_common::vendor_display_name(vendor)
            ),
        )
    })?;
    let mut cmd = Command::new(&bin);
    cmd.arg(topic);
    if let Some(color) = expect_color {
        cmd.arg(color);
    }
    cmd.env("AIDDS_SHAPES_DOMAIN", domain.to_string())
        .env("AIDDS_SHAPES_TIMEOUT_MS", "12000")
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Ok(ld) = std::env::var("LD_LIBRARY_PATH") {
        cmd.env("LD_LIBRARY_PATH", ld);
    }
    if vendor == InteropVendor::FastDds {
        cmd.env("AIDDS_FASTDDS_NO_PROFILE", "1");
    }
    cmd.spawn()
}

/// OMG Shapes Demo `ShapeType` (keyed by `color`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeType {
    pub color: String,
    pub x: i32,
    pub y: i32,
    pub shapesize: i32,
}

impl ShapeType {
    pub fn sample(topic: &str, vendor_slug: &str) -> Self {
        let (x, y, size) = match topic {
            "Square" => (10, 20, 30),
            "Circle" => (50, 60, 25),
            "Triangle" => (80, 40, 35),
            _ => (0, 0, 10),
        };
        Self {
            color: format!("{topic}-{vendor_slug}"),
            x,
            y,
            shapesize: size,
        }
    }
}

impl CdrSerialize for ShapeType {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_str(&self.color);
        serializer.serialize_i32(self.x);
        serializer.serialize_i32(self.y);
        serializer.serialize_i32(self.shapesize);
        Ok(())
    }
}

impl CdrDeserialize for ShapeType {
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        Ok(Self {
            color: deserializer.deserialize_str()?,
            x: deserializer.deserialize_i32()?,
            y: deserializer.deserialize_i32()?,
            shapesize: deserializer.deserialize_i32()?,
        })
    }
}

pub struct ShapeTypeSupport;

impl TypeSupport for ShapeTypeSupport {
    fn get_type_name(&self) -> &str {
        SHAPES_TYPE
    }

    fn serialize(&self, value: &dyn Any) -> dds::types::return_code::DdsResult<Vec<u8>> {
        let shape = value
            .downcast_ref::<ShapeType>()
            .ok_or_else(|| dds::types::return_code::DdsError::BadParameter("cast failed".into()))?;
        let mut ser = CdrSerializer::new(Endianness::LittleEndian);
        EncapsulationHeader::new(EncapsulationKind::CdrLe).serialize(&mut ser);
        shape
            .serialize(&mut ser)
            .map_err(|e| dds::types::return_code::DdsError::Error(e.to_string()))?;
        Ok(ser.into_bytes().to_vec())
    }

    fn deserialize(&self, bytes: &[u8]) -> dds::types::return_code::DdsResult<Box<dyn Any>> {
        let mut de = CdrDeserializer::new(bytes, Endianness::LittleEndian);
        let header = EncapsulationHeader::deserialize(&mut de)
            .map_err(|e| dds::types::return_code::DdsError::Error(e.to_string()))?;
        if !matches!(header.kind, EncapsulationKind::CdrLe | EncapsulationKind::CdrBe) {
            return Err(dds::types::return_code::DdsError::Error(format!(
                "unsupported encapsulation kind for shapes: {:?}",
                header.kind
            )));
        }
        let mut body_de =
            CdrDeserializer::new(&bytes[de.offset()..], header.kind.endianness());
        let shape = ShapeType::deserialize(&mut body_de)
            .map_err(|e| dds::types::return_code::DdsError::Error(e.to_string()))?;
        Ok(Box::new(shape))
    }

    fn get_key_hash(
        &self,
        value: &dyn Any,
    ) -> dds::types::return_code::DdsResult<dds::types::instance::InstanceHandle> {
        let shape = value
            .downcast_ref::<ShapeType>()
            .ok_or_else(|| dds::types::return_code::DdsError::BadParameter("cast failed".into()))?;
        let mut ser = CdrSerializer::new(Endianness::LittleEndian);
        ser.serialize_str(&shape.color);
        Ok(dds::types::instance::InstanceHandle::from_key_bytes(
            ser.bytes(),
        ))
    }

    fn is_keyless(&self) -> bool {
        false
    }
}

pub fn output_contains_shapes_receive(output: &Output, topic: &str, shape: &ShapeType) -> bool {
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.contains("SHAPES_RECEIVE")
        && stdout.contains(&format!("topic={topic}"))
        && stdout.contains(&format!("color={}", shape.color))
        && stdout.contains(&format!("x={}", shape.x))
        && stdout.contains(&format!("y={}", shape.y))
        && stdout.contains(&format!("shapesize={}", shape.shapesize))
}

pub fn output_contains_shapes_publish(output: &Output, topic: &str, shape: &ShapeType) -> bool {
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.contains("SHAPES_PUBLISH")
        && stdout.contains(&format!("topic={topic}"))
        && stdout.contains(&format!("color={}", shape.color))
}

#[cfg(test)]
mod shapes_type_tests {
    use super::*;
    use dds::core::TypeSupport;

    #[test]
    fn shapes_cdr_le_roundtrip_with_encapsulation() {
        let ts = ShapeTypeSupport;
        let sample = ShapeType {
            color: "BLUE".to_string(),
            x: 100,
            y: 200,
            shapesize: 50,
        };
        let wire = ts.serialize(&sample as &dyn Any).unwrap();
        assert_eq!(&wire[0..2], &[0x00, 0x01]); // CdrLe
        let boxed = ts.deserialize(&wire).unwrap();
        assert_eq!(boxed.downcast_ref::<ShapeType>().unwrap(), &sample);
    }

    #[test]
    fn shapes_key_hash_uses_color_field() {
        let ts = ShapeTypeSupport;
        let a = ShapeType {
            color: "RED".to_string(),
            x: 1,
            y: 2,
            shapesize: 3,
        };
        let b = ShapeType {
            color: "RED".to_string(),
            x: 99,
            y: 88,
            shapesize: 77,
        };
        let c = ShapeType {
            color: "GREEN".to_string(),
            x: 1,
            y: 2,
            shapesize: 3,
        };
        let ha = ts.get_key_hash(&a as &dyn Any).unwrap();
        let hb = ts.get_key_hash(&b as &dyn Any).unwrap();
        let hc = ts.get_key_hash(&c as &dyn Any).unwrap();
        assert_eq!(ha, hb);
        assert_ne!(ha, hc);
        assert!(!ts.is_keyless());
    }
}
