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

/// External DDS implementation used for live interoperability tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteropVendor {
    CycloneDds,
    FastDds,
    OpenDds,
}

impl InteropVendor {
    pub fn slug(self) -> &'static str {
        match self {
            Self::CycloneDds => "cyclonedds",
            Self::FastDds => "fastdds",
            Self::OpenDds => "opendds",
        }
    }

    pub fn build_script(self) -> &'static str {
        match self {
            Self::CycloneDds => "interop/scripts/build-cyclonedds-apps.sh",
            Self::FastDds => "interop/scripts/build-fastdds-apps.sh",
            Self::OpenDds => "interop/scripts/build-opendds-apps.sh",
        }
    }

    fn bin_env_var(self) -> &'static str {
        match self {
            Self::CycloneDds => "AIDDS_INTEROP_BIN_CYCLONEDDS",
            Self::FastDds => "AIDDS_INTEROP_BIN_FASTDDS",
            Self::OpenDds => "AIDDS_INTEROP_BIN_OPENDDS",
        }
    }

    fn default_build_dir(self) -> &'static str {
        match self {
            Self::CycloneDds => "target/interop-cyclonedds",
            Self::FastDds => "target/interop-fastdds",
            Self::OpenDds => "target/interop-opendds",
        }
    }
}

pub fn vendor_display_name(vendor: InteropVendor) -> &'static str {
    match vendor {
        InteropVendor::CycloneDds => "CycloneDDS",
        InteropVendor::FastDds => "Fast DDS",
        InteropVendor::OpenDds => "OpenDDS",
    }
}

/// Base domain id per vendor (tests use +0, +1, +2 offsets).
pub fn vendor_base_domain(vendor: InteropVendor) -> u32 {
    match vendor {
        InteropVendor::CycloneDds => INTEROP_DOMAIN,
        InteropVendor::FastDds => 83,
        InteropVendor::OpenDds => 93,
    }
}

pub fn vendor_bin_dir(vendor: InteropVendor) -> Option<PathBuf> {
    if let Ok(dir) = std::env::var(vendor.bin_env_var()) {
        let path = PathBuf::from(dir);
        if path.join("interop_publisher").exists() {
            return Some(path);
        }
    }

    // Legacy env: only applies to CycloneDDS peer apps.
    if vendor == InteropVendor::CycloneDds {
        if let Ok(dir) = std::env::var("AIDDS_INTEROP_BIN") {
            let path = PathBuf::from(dir);
            if path.join("interop_publisher").exists() {
                return Some(path);
            }
        }
    }

    let rel = PathBuf::from(vendor.default_build_dir());
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../")
        .join(vendor.default_build_dir());
    for dir in [rel, manifest] {
        if dir.join("interop_publisher").exists() {
            return Some(dir);
        }
    }
    None
}

pub fn vendor_available(vendor: InteropVendor) -> bool {
    vendor_bin_dir(vendor).is_some()
}

pub fn vendor_publisher(vendor: InteropVendor) -> Option<PathBuf> {
    vendor_bin_dir(vendor).map(|d| d.join("interop_publisher"))
}

pub fn vendor_subscriber(vendor: InteropVendor) -> Option<PathBuf> {
    vendor_bin_dir(vendor).map(|d| d.join("interop_subscriber"))
}

pub fn opendds_config_file() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("AIDDS_OPENDDS_CONFIG") {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }
    for dir in vendor_bin_dir(InteropVendor::OpenDds).into_iter() {
        let ini = dir.join("rtps.ini");
        if ini.exists() {
            return Some(ini);
        }
    }
    None
}

fn apply_vendor_runtime(cmd: &mut Command, vendor: InteropVendor) {
    if let Ok(ld) = std::env::var("LD_LIBRARY_PATH") {
        cmd.env("LD_LIBRARY_PATH", ld);
    }
    if vendor == InteropVendor::FastDds {
        cmd.env("AIDDS_FASTDDS_NO_PROFILE", "1");
    }
    if vendor == InteropVendor::OpenDds {
        if let Some(ini) = opendds_config_file() {
            cmd.arg("-DCPSConfigFile").arg(ini);
        }
        if std::env::var("AIDDS_OPENDDS_DEBUG").is_ok() {
            let level = std::env::var("AIDDS_OPENDDS_DEBUG_LEVEL").unwrap_or_else(|_| "10".to_string());
            cmd.arg("-DCPSDebugLevel").arg(&level);
            let tlevel = std::env::var("AIDDS_OPENDDS_TRANSPORT_DEBUG").unwrap_or_else(|_| "6".to_string());
            cmd.arg("-DCPSTransportDebugLevel").arg(tlevel);
        }
    }
}

pub fn spawn_vendor_publisher(
    vendor: InteropVendor,
    domain: u32,
    sample_id: u32,
    payload: &str,
) -> std::io::Result<std::process::Child> {
    let bin = vendor_publisher(vendor).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "{} interop_publisher not found; run {}",
                vendor_display_name(vendor),
                vendor.build_script()
            ),
        )
    })?;
    let mut cmd = Command::new(&bin);
    cmd.arg(sample_id.to_string()).arg(payload);
    apply_vendor_runtime(&mut cmd, vendor);
    cmd.env("AIDDS_INTEROP_DOMAIN", domain.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("AIDDS_INTEROP_WAIT_MATCH", "1")
        .spawn()
}

pub fn spawn_vendor_subscriber(
    vendor: InteropVendor,
    domain: u32,
    expect_id: Option<u32>,
) -> std::io::Result<std::process::Child> {
    let bin = vendor_subscriber(vendor).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "{} interop_subscriber not found; run {}",
                vendor_display_name(vendor),
                vendor.build_script()
            ),
        )
    })?;
    let mut cmd = Command::new(&bin);
    if let Some(id) = expect_id {
        cmd.arg(id.to_string());
    }
    apply_vendor_runtime(&mut cmd, vendor);
    cmd.env("AIDDS_INTEROP_DOMAIN", domain.to_string())
        .env("AIDDS_INTEROP_TIMEOUT_MS", "12000")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

pub const INTEROP_DOMAIN: u32 = 73;
pub const INTEROP_TOPIC: &str = "AiDdsInteropMessage";
pub const INTEROP_TYPE: &str = "AiDdsInterop::Message";

/// XCDR2 TypeInformation for `AiDdsInterop::Message`, from OpenDDS IDL output.
pub const INTEROP_MESSAGE_TYPE_INFORMATION: &[u8] = &[
    0x54, 0x00, 0x00, 0x00, 0x01, 0x10, 0x00, 0x40, 0x28, 0x00, 0x00, 0x00, 0x24, 0x00, 0x00, 0x00,
    0x14, 0x00, 0x00, 0x00, 0xf1, 0x8a, 0xdc, 0xac, 0xb5, 0x9d, 0x5f, 0xe9, 0x25, 0x41, 0x6c, 0xc0,
    0x38, 0x92, 0x17, 0x00, 0x38, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0x04, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x02, 0x10, 0x00, 0x40, 0x1c, 0x00, 0x00, 0x00, 0x18, 0x00, 0x00, 0x00,
    0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
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

/// TypeSupport for CycloneDDS interop: plain CDR with encapsulation header on the wire
/// (`CdrLe` or `CdrBe` in RTPS serialized_payload), matching CycloneDDS user samples.
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
        let header = EncapsulationHeader::deserialize(&mut de)
            .map_err(|e| dds::types::return_code::DdsError::Error(e.to_string()))?;
        if !matches!(
            header.kind,
            EncapsulationKind::CdrLe
                | EncapsulationKind::CdrBe
                | EncapsulationKind::PlCdrLe
                | EncapsulationKind::PlCdrBe
        ) {
            return Err(dds::types::return_code::DdsError::Error(format!(
                "unsupported encapsulation kind for interop: {:?}",
                header.kind
            )));
        }
        let mut body_de =
            CdrDeserializer::new(&bytes[de.offset()..], header.kind.endianness());
        let msg = InteropMessage::deserialize(&mut body_de)
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
    fn deserialize_cyclonedds_cdr_le_encapsulated_payload() {
        let ts = InteropTypeSupport;
        let mut ser = CdrSerializer::new(Endianness::LittleEndian);
        EncapsulationHeader::new(EncapsulationKind::CdrLe).serialize(&mut ser);
        InteropMessage {
            id: 9001,
            payload: "from-cyclonedds".to_string(),
        }
        .serialize(&mut ser)
        .unwrap();
        let boxed = ts.deserialize(ser.bytes()).expect("deserialize CdrLe wire");
        let msg = boxed.downcast_ref::<InteropMessage>().unwrap();
        assert_eq!(msg.id, 9001);
        assert_eq!(msg.payload, "from-cyclonedds");
    }

    #[test]
    fn deserialize_cyclonedds_cdr_be_encapsulated_payload() {
        let ts = InteropTypeSupport;
        let mut ser = CdrSerializer::new(Endianness::BigEndian);
        EncapsulationHeader::new(EncapsulationKind::CdrBe).serialize(&mut ser);
        InteropMessage {
            id: 9001,
            payload: "from-cyclonedds".to_string(),
        }
        .serialize(&mut ser)
        .unwrap();
        let boxed = ts.deserialize(ser.bytes()).expect("deserialize CdrBe wire");
        let msg = boxed.downcast_ref::<InteropMessage>().unwrap();
        assert_eq!(msg.id, 9001);
        assert_eq!(msg.payload, "from-cyclonedds");
    }

    #[test]
    fn deserialize_rejects_plain_cdr_without_encapsulation_header() {
        let ts = InteropTypeSupport;
        let mut ser = CdrSerializer::new(Endianness::LittleEndian);
        InteropMessage {
            id: 9001,
            payload: "from-cyclonedds".to_string(),
        }
        .serialize(&mut ser)
        .unwrap();
        assert!(ts.deserialize(ser.bytes()).is_err());
    }

    #[test]
    fn deserialize_accepts_plcdr_encapsulation() {
        let ts = InteropTypeSupport;
        let mut ser = CdrSerializer::new(Endianness::LittleEndian);
        EncapsulationHeader::new(EncapsulationKind::PlCdrLe).serialize(&mut ser);
        InteropMessage {
            id: 9001,
            payload: "from-cyclonedds".to_string(),
        }
        .serialize(&mut ser)
        .unwrap();
        let boxed = ts.deserialize(ser.bytes()).expect("deserialize PlCdrLe wire");
        let msg = boxed.downcast_ref::<InteropMessage>().unwrap();
        assert_eq!(msg.id, 9001);
    }
}

/// Locate CycloneDDS interop binaries built by `interop/scripts/build-cyclonedds-apps.sh`.
pub fn cyclonedds_bin_dir() -> Option<PathBuf> {
    vendor_bin_dir(InteropVendor::CycloneDds)
}

pub fn cyclonedds_available() -> bool {
    vendor_available(InteropVendor::CycloneDds)
}

pub fn cyclonedds_publisher() -> Option<PathBuf> {
    vendor_publisher(InteropVendor::CycloneDds)
}

pub fn cyclonedds_subscriber() -> Option<PathBuf> {
    vendor_subscriber(InteropVendor::CycloneDds)
}

pub fn spawn_cyclonedds_publisher(
    domain: u32,
    sample_id: u32,
    payload: &str,
) -> std::io::Result<std::process::Child> {
    spawn_vendor_publisher(InteropVendor::CycloneDds, domain, sample_id, payload)
}

pub fn spawn_cyclonedds_subscriber(expect_id: Option<u32>) -> std::io::Result<std::process::Child> {
    spawn_vendor_subscriber(InteropVendor::CycloneDds, INTEROP_DOMAIN, expect_id)
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
            let output = child.wait_with_output().unwrap_or_else(|_| Output {
                status: std::process::ExitStatus::default(),
                stdout: Vec::new(),
                stderr: Vec::new(),
            });
            eprintln!(
                "child timed out after {:?}\nstdout: {}\nstderr: {}",
                timeout,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
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
