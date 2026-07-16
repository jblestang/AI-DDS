//! Shared types and helpers for interoperability testing against external DDS
//! implementations (CycloneDDS, etc.).

use dds::cdr::{
    CdrDeserialize, CdrDeserializer, CdrResult, CdrSerialize, CdrSerializer, EncapsulationHeader,
    EncapsulationKind, Endianness,
};
use dds::core::TypeSupport;
use std::any::Any;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Duration;

pub const INTEROP_DOMAIN: u32 = 70;
pub const INTEROP_TOPIC: &str = "AiDdsInteropMessage";
pub const INTEROP_TYPE: &str = "AiDdsInterop::Message";

/// XCDR2 TypeInformation for `AiDdsInterop::Message`, from Cyclone IDLC output.
pub const INTEROP_MESSAGE_TYPE_INFORMATION: &[u8] = &[
    0x60, 0x00, 0x00, 0x00, 0x01, 0x10, 0x00, 0x40, 0x28, 0x00, 0x00, 0x00, 0x24, 0x00, 0x00, 0x00,
    0x14, 0x00, 0x00, 0x00, 0xf1, 0x8a, 0xdc, 0xac, 0xb5, 0x9d, 0x5f, 0xe9, 0x25, 0x41, 0x6c, 0xc0,
    0x38, 0x92, 0x17, 0x00, 0x38, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x02, 0x10, 0x00, 0x40, 0x28, 0x00, 0x00, 0x00, 0x24, 0x00, 0x00, 0x00,
    0x14, 0x00, 0x00, 0x00, 0xf2, 0x16, 0x24, 0x23, 0x8c, 0x7b, 0xbf, 0x6a, 0xf0, 0x05, 0xea, 0x3f,
    0x86, 0xf8, 0x87, 0x00, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
];

/// Interop payload matching `interop/idl/InteropMessage.idl`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteropMessage {
    pub id: u32,
    pub payload: String,
}

impl CdrSerialize for InteropMessage {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_u32(self.id);
        serializer.serialize_str(&self.payload);
        Ok(())
    }
}

impl CdrDeserialize for InteropMessage {
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        Ok(Self {
            id: deserializer.deserialize_u32()?,
            payload: deserializer.deserialize_str()?,
        })
    }
}

/// TypeSupport using standard CDR encapsulation (`CdrLe`) for wire compatibility.
pub struct InteropTypeSupport;

impl TypeSupport for InteropTypeSupport {
    fn get_type_name(&self) -> &str {
        INTEROP_TYPE
    }

    fn serialize(&self, value: &dyn Any) -> dds::types::return_code::DdsResult<Vec<u8>> {
        let msg = value
            .downcast_ref::<InteropMessage>()
            .ok_or_else(|| dds::types::return_code::DdsError::BadParameter("cast failed".into()))?;
        let mut ser = CdrSerializer::new(Endianness::LittleEndian);
        EncapsulationHeader::new(EncapsulationKind::CdrLe).serialize(&mut ser);
        msg.serialize(&mut ser)
            .map_err(|e| dds::types::return_code::DdsError::Error(e.to_string()))?;
        Ok(ser.into_bytes().to_vec())
    }

    fn deserialize(&self, bytes: &[u8]) -> dds::types::return_code::DdsResult<Box<dyn Any>> {
        let payload = if bytes.len() >= 4 {
            let be_kind = u16::from_be_bytes([bytes[0], bytes[1]]);
            let has_encapsulation = matches!(
                be_kind,
                0x0000 | 0x0001 | 0x0002 | 0x0003 | 0x0010 | 0x0011 | 0x0012 | 0x0013
            );
            if has_encapsulation {
                let mut de = CdrDeserializer::new(bytes, Endianness::LittleEndian);
                let header = EncapsulationHeader::deserialize(&mut de)
                    .map_err(|e| dds::types::return_code::DdsError::Error(e.to_string()))?;
                let mut body_de = CdrDeserializer::new(&bytes[de.offset()..], header.kind.endianness());
                let msg = InteropMessage::deserialize(&mut body_de)
                    .map_err(|e| dds::types::return_code::DdsError::Error(e.to_string()))?;
                return Ok(Box::new(msg));
            }
            bytes
        } else {
            bytes
        };
        let mut de = CdrDeserializer::new(payload, Endianness::LittleEndian);
        let msg = InteropMessage::deserialize(&mut de)
            .map_err(|e| dds::types::return_code::DdsError::Error(e.to_string()))?;
        Ok(Box::new(msg))
    }

    fn get_key_hash(
        &self,
        _value: &dyn Any,
    ) -> dds::types::return_code::DdsResult<dds::types::instance::InstanceHandle> {
        Ok(dds::types::instance::InstanceHandle::NIL)
    }

    fn is_keyless(&self) -> bool {
        true
    }

    fn type_information_wire(&self) -> Option<&[u8]> {
        Some(INTEROP_MESSAGE_TYPE_INFORMATION)
    }
}

#[cfg(test)]
mod interop_type_tests {
    use super::*;

    #[test]
    fn deserialize_cyclonedds_payload_without_encapsulation_header() {
        let ts = InteropTypeSupport;
        let mut ser = CdrSerializer::new(Endianness::LittleEndian);
        InteropMessage {
            id: 9001,
            payload: "from-cyclonedds".to_string(),
        }
        .serialize(&mut ser)
        .unwrap();
        let raw = ser.into_bytes();
        let boxed = ts.deserialize(&raw).expect("deserialize raw CDR");
        let msg = boxed.downcast_ref::<InteropMessage>().unwrap();
        assert_eq!(msg.id, 9001);
        assert_eq!(msg.payload, "from-cyclonedds");

        // Wire bytes captured from CycloneDDS interop publisher (no encapsulation header).
        let wire = [
            0x29, 0x23, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x66, 0x72, 0x6f, 0x6d, 0x2d, 0x63,
            0x79, 0x63, 0x6c, 0x6f, 0x6e, 0x65, 0x64, 0x64, 0x73, 0x00,
        ];
        let boxed = ts.deserialize(&wire).expect("deserialize cyclone wire");
        let msg = boxed.downcast_ref::<InteropMessage>().unwrap();
        assert_eq!(msg.id, 9001);
        assert_eq!(msg.payload, "from-cyclonedds");
    }
}

/// Locate CycloneDDS interop binaries built by `interop/scripts/build-cyclonedds-apps.sh`.
pub fn cyclonedds_bin_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("AIDDS_INTEROP_BIN") {
        let path = PathBuf::from(dir);
        if path.join("interop_publisher").exists() {
            return Some(path);
        }
    }

    let candidates = [
        PathBuf::from("target/interop-cyclonedds"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/interop-cyclonedds"),
    ];
    for dir in candidates {
        if dir.join("interop_publisher").exists() {
            return Some(dir);
        }
    }
    None
}

pub fn cyclonedds_available() -> bool {
    cyclonedds_bin_dir().is_some()
}

pub fn cyclonedds_publisher() -> Option<PathBuf> {
    cyclonedds_bin_dir().map(|d| d.join("interop_publisher"))
}

pub fn cyclonedds_subscriber() -> Option<PathBuf> {
    cyclonedds_bin_dir().map(|d| d.join("interop_subscriber"))
}

pub fn spawn_cyclonedds_publisher(
    domain: u32,
    sample_id: u32,
    payload: &str,
) -> std::io::Result<std::process::Child> {
    let bin = cyclonedds_publisher().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "CycloneDDS interop_publisher not found; run interop/scripts/build-cyclonedds-apps.sh",
        )
    })?;
    Command::new(bin)
        .arg(sample_id.to_string())
        .arg(payload)
        .env("AIDDS_INTEROP_DOMAIN", domain.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
}

pub fn spawn_cyclonedds_subscriber(expect_id: Option<u32>) -> std::io::Result<std::process::Child> {
    let bin = cyclonedds_subscriber().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "CycloneDDS interop_subscriber not found",
        )
    })?;
    let mut cmd = Command::new(bin);
    if let Some(id) = expect_id {
        cmd.arg(id.to_string());
    }
    cmd.env("AIDDS_INTEROP_DOMAIN", INTEROP_DOMAIN.to_string())
        .env("AIDDS_INTEROP_TIMEOUT_MS", "12000")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
}

pub fn wait_output(mut child: std::process::Child, timeout: Duration) -> std::io::Result<Output> {
    let start = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return child.wait_with_output().map(|mut o| {
                o.status = status;
                o
            });
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.try_wait();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "child process timed out",
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub fn output_contains_interop_receive(output: &Output, id: u32, payload: &str) -> bool {
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.contains("INTEROP_RECEIVE")
        && stdout.contains(&format!("id={id}"))
        && stdout.contains(payload)
}

pub fn read_fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../interop/wire")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("missing interop fixture {}: {e}", path.display()))
}
