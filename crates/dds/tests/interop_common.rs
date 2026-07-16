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
        let mut de = CdrDeserializer::new(bytes, Endianness::LittleEndian);
        if bytes.len() >= 4 {
            let _ = EncapsulationHeader::deserialize(&mut de)
                .map_err(|e| dds::types::return_code::DdsError::Error(e.to_string()))?;
        }
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

pub fn spawn_cyclonedds_publisher(sample_id: u32, payload: &str) -> std::io::Result<std::process::Child> {
    let bin = cyclonedds_publisher().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "CycloneDDS interop_publisher not found; run interop/scripts/build-cyclonedds-apps.sh",
        )
    })?;
    Command::new(bin)
        .arg(sample_id.to_string())
        .arg(payload)
        .env("AIDDS_INTEROP_DOMAIN", INTEROP_DOMAIN.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
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
        .stderr(Stdio::piped())
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
            let _ = child.wait();
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
