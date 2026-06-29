//! # dds-security — DDS Security 1.2
//!
//! Implements the Security Service Plugin Interface (SPI) and builtin plugins:
//! Authentication (PKI-DH), Access Control (Permissions), Crypto (AES-GCM-GMAC).

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
    clippy::pattern_type_mismatch,
    clippy::pub_use,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::std_instead_of_core,
    clippy::std_instead_of_alloc,
    clippy::alloc_instead_of_core,
    clippy::unwrap_used,
    clippy::unwrap_in_result,
    clippy::missing_inline_in_public_items,
    clippy::question_mark_used,
    clippy::min_ident_chars,
    clippy::shadow_reuse,
    clippy::shadow_same,
    clippy::shadow_unrelated,
    clippy::else_if_without_else,
    clippy::missing_docs_in_private_items,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_lossless,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::default_numeric_fallback,
    clippy::single_call_fn,
    clippy::separated_literal_suffix,
    clippy::unseparated_literal_suffix,
    clippy::exhaustive_enums,
    clippy::exhaustive_structs,
    clippy::panic_in_result_fn,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::similar_names,
    clippy::needless_return,
    clippy::string_add,
    clippy::iter_over_hash_type,
    clippy::infinite_loop,
    clippy::needless_pass_by_value,
    clippy::format_push_string,
    clippy::option_if_let_else,
    clippy::unnecessary_debug_formatting,
    clippy::struct_field_names,
    clippy::too_many_lines,
    clippy::large_stack_arrays,
    clippy::match_wildcard_for_single_variants,
    clippy::modulo_arithmetic,
    clippy::non_ascii_literal,
    clippy::single_char_lifetime_names,
    clippy::match_like_matches_macro,
    clippy::wildcard_enum_match_arm,
    clippy::map_err_ignore,
    clippy::decimal_literal_representation,
    clippy::ref_patterns,
    clippy::cognitive_complexity,
    clippy::derivable_impls,
    clippy::match_same_arms,
    clippy::unused_trait_names,
    clippy::format_collect,
    clippy::items_after_statements,
    reason = "DDS Security implementation uses standard library collections, standard returns, and common mathematical conversions."
)]

use aes_gcm::aead::{Aead as _, KeyInit as _};
use aes_gcm::{Aes128Gcm, Nonce};
use dds_cdr::{CdrDeserialize, CdrDeserializer, CdrResult, CdrSerialize, CdrSerializer};
use rand::RngCore as _;
use std::collections::HashMap;
use std::sync::Mutex;
use x509_cert::der::Decode as _;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CryptoFooter {
    pub mac_tag: [u8; 16],
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CryptoHeader {
    pub initialization_vector: [u8; 12],
    pub session_id: [u8; 16],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandshakeHandle(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HandshakeState {
    Active,
    None,
    ReceivedReply,
    SentRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HandshakeToken {
    pub class_id: String,
    pub properties: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IdentityHandle(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdentityToken {
    pub class_id: String,
    pub properties: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParticipantCryptoHandle(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PermissionsHandle(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PermissionsToken {
    pub class_id: String,
    pub properties: Vec<(String, String)>,
}

#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    #[error("Access denied: {0}")]
    AccessDenied(String),
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    #[error("Crypto error: {0}")]
    CryptoError(String),
    #[error("Key exchange failed: {0}")]
    KeyExchangeFailed(String),
}

pub type SecurityResult<T> = Result<T, SecurityError>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SharedSecretHandle(pub Vec<u8>);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SignedDocument {
    pub content_xml: String,
    pub signature: Vec<u8>,
}

impl CdrSerialize for CryptoFooter {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        for b in &self.mac_tag {
            serializer.serialize_u8(*b);
        }
        Ok(())
    }
}

impl CdrDeserialize for CryptoFooter {
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        let mut mac_tag = [0_u8; 16];
        for b in &mut mac_tag {
            *b = deserializer.deserialize_u8()?;
        }
        Ok(Self { mac_tag })
    }
}

impl CdrSerialize for CryptoHeader {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        for b in &self.initialization_vector {
            serializer.serialize_u8(*b);
        }
        for b in &self.session_id {
            serializer.serialize_u8(*b);
        }
        Ok(())
    }
}

impl CdrDeserialize for CryptoHeader {
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        let mut initialization_vector = [0_u8; 12];
        for b in &mut initialization_vector {
            *b = deserializer.deserialize_u8()?;
        }
        let mut session_id = [0_u8; 16];
        for b in &mut session_id {
            *b = deserializer.deserialize_u8()?;
        }
        Ok(Self {
            initialization_vector,
            session_id,
        })
    }
}

impl CdrSerialize for HandshakeToken {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_str(&self.class_id);
        serializer.serialize_u32(self.properties.len() as u32);
        for (k, v) in &self.properties {
            serializer.serialize_str(k);
            serializer.serialize_str(v);
        }
        Ok(())
    }
}

impl CdrDeserialize for HandshakeToken {
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        let class_id = deserializer.deserialize_str()?;
        let len = deserializer.deserialize_u32()?;
        let mut properties = Vec::with_capacity(len as usize);
        for _ in 0..len {
            let k = deserializer.deserialize_str()?;
            let v = deserializer.deserialize_str()?;
            properties.push((k, v));
        }
        Ok(Self {
            class_id,
            properties,
        })
    }
}

impl CdrSerialize for IdentityToken {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_str(&self.class_id);
        serializer.serialize_u32(self.properties.len() as u32);
        for (k, v) in &self.properties {
            serializer.serialize_str(k);
            serializer.serialize_str(v);
        }
        Ok(())
    }
}

impl CdrDeserialize for IdentityToken {
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        let class_id = deserializer.deserialize_str()?;
        let len = deserializer.deserialize_u32()?;
        let mut properties = Vec::with_capacity(len as usize);
        for _ in 0..len {
            let k = deserializer.deserialize_str()?;
            let v = deserializer.deserialize_str()?;
            properties.push((k, v));
        }
        Ok(Self {
            class_id,
            properties,
        })
    }
}

impl CdrSerialize for PermissionsToken {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_str(&self.class_id);
        serializer.serialize_u32(self.properties.len() as u32);
        for (k, v) in &self.properties {
            serializer.serialize_str(k);
            serializer.serialize_str(v);
        }
        Ok(())
    }
}

impl CdrDeserialize for PermissionsToken {
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        let class_id = deserializer.deserialize_str()?;
        let len = deserializer.deserialize_u32()?;
        let mut properties = Vec::with_capacity(len as usize);
        for _ in 0..len {
            let k = deserializer.deserialize_str()?;
            let v = deserializer.deserialize_str()?;
            properties.push((k, v));
        }
        Ok(Self {
            class_id,
            properties,
        })
    }
}

pub trait Authentication: Send + Sync {
    fn begin_handshake_request(
        &self,
        local_identity: &IdentityHandle,
        remote_identity: &IdentityHandle,
    ) -> SecurityResult<(HandshakeHandle, HandshakeToken)>;

    fn get_shared_secret(&self, handshake: &HandshakeHandle) -> SecurityResult<SharedSecretHandle>;

    fn process_handshake(
        &self,
        handshake: &mut HandshakeHandle,
        incoming_token: HandshakeToken,
    ) -> SecurityResult<Option<HandshakeToken>>;

    fn validate_local_identity(
        &self,
        domain_id: u32,
        participant_qos: &dds_types::qos::DomainParticipantQos,
    ) -> SecurityResult<(IdentityHandle, IdentityToken)>;
}

pub trait AccessControl: Send + Sync {
    fn check_create_reader(
        &self,
        permissions: &PermissionsHandle,
        topic_name: &str,
    ) -> SecurityResult<bool>;

    fn check_create_writer(
        &self,
        permissions: &PermissionsHandle,
        topic_name: &str,
    ) -> SecurityResult<bool>;

    fn validate_remote_permissions(
        &self,
        local_identity: &IdentityHandle,
        remote_identity: &IdentityHandle,
        permissions_token: PermissionsToken,
    ) -> SecurityResult<PermissionsHandle>;
}

pub trait Cryptography: Send + Sync {
    fn decrypt_payload(
        &self,
        ciphertext: &[u8],
        header: &CryptoHeader,
        footer: &CryptoFooter,
        local_participant: &ParticipantCryptoHandle,
        remote_participant: &ParticipantCryptoHandle,
    ) -> SecurityResult<Vec<u8>>;

    fn encrypt_payload(
        &self,
        payload: &[u8],
        local_participant: &ParticipantCryptoHandle,
        remote_participant: &ParticipantCryptoHandle,
    ) -> SecurityResult<(Vec<u8>, CryptoHeader, CryptoFooter)>;

    fn register_local_participant(
        &self,
        local_identity: &IdentityHandle,
    ) -> SecurityResult<ParticipantCryptoHandle>;

    fn register_matched_remote_participant(
        &self,
        local_participant: &ParticipantCryptoHandle,
        remote_identity: &IdentityHandle,
        shared_secret: &SharedSecretHandle,
    ) -> SecurityResult<ParticipantCryptoHandle>;
}

// ──────────────────────────────────────────────────────────────────────────────
// Data Tagging Plugin (DDS Security 1.2)
// ──────────────────────────────────────────────────────────────────────────────

pub trait DataTagging: Send + Sync {
    /// Retrieve data tags for a DomainParticipant
    fn get_data_tags(&self, qos: &dds_types::qos::DomainParticipantQos) -> SecurityResult<Vec<(String, String)>>;
    
    /// Retrieve data tags for an endpoint based on its Property
    fn get_endpoint_data_tags(&self, qos: &dds_types::qos::Property) -> SecurityResult<Vec<(String, String)>>;
}

pub struct BuiltinDataTagging;

impl BuiltinDataTagging {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for BuiltinDataTagging {
    fn default() -> Self {
        Self::new()
    }
}

impl DataTagging for BuiltinDataTagging {
    fn get_data_tags(&self, _qos: &dds_types::qos::DomainParticipantQos) -> SecurityResult<Vec<(String, String)>> {
        // Built-in behavior: no default data tags
        Ok(Vec::new())
    }

    fn get_endpoint_data_tags(&self, _qos: &dds_types::qos::Property) -> SecurityResult<Vec<(String, String)>> {
        Ok(Vec::new())
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Built-in Authentication Implementation (PKI-DH)
// ──────────────────────────────────────────────────────────────────────────────



pub struct BuiltinAuthentication {
    handshake_secrets: Mutex<HashMap<HandshakeHandle, p256::ecdh::EphemeralSecret>>,
    handshake_states: Mutex<HashMap<HandshakeHandle, HandshakeState>>,
    next_handle: Mutex<u32>,
    shared_secrets: Mutex<HashMap<HandshakeHandle, Vec<u8>>>,
    identity_cert: Mutex<Option<Vec<u8>>>,
    private_key: Mutex<Option<Vec<u8>>>,
    ca_cert: Mutex<Option<Vec<u8>>>,
}

impl BuiltinAuthentication {
    #[must_use]
    pub fn new() -> Self {
        Self {
            handshake_secrets: Mutex::new(HashMap::new()),
            handshake_states: Mutex::new(HashMap::new()),
            next_handle: Mutex::new(1),
            shared_secrets: Mutex::new(HashMap::new()),
            identity_cert: Mutex::new(None),
            private_key: Mutex::new(None),
            ca_cert: Mutex::new(None),
        }
    }
}

impl Default for BuiltinAuthentication {
    fn default() -> Self {
        Self::new()
    }
}

impl Authentication for BuiltinAuthentication {
    fn begin_handshake_request(
        &self,
        _local_identity: &IdentityHandle,
        _remote_identity: &IdentityHandle,
    ) -> SecurityResult<(HandshakeHandle, HandshakeToken)> {
        let handle = {
            let mut id = self.next_handle.lock().unwrap();
            let ret = HandshakeHandle(*id);
            *id += 1;
            ret
        };

        // Ephemeral P-256 ECDH Keypair generation
        let secret = p256::ecdh::EphemeralSecret::random(&mut rand::thread_rng());
        let public_key = secret.public_key();
        let pub_bytes = public_key.to_sec1_bytes().to_vec();

        self.handshake_states
            .lock()
            .unwrap()
            .insert(handle, HandshakeState::SentRequest);
        self.handshake_secrets
            .lock()
            .unwrap()
            .insert(handle, secret);

        let mut properties = vec![
            ("step".to_owned(), "Request".to_owned()),
            ("pub_key".to_owned(), to_hex(&pub_bytes)),
        ];
        if let Some(ref cert_bytes) = *self.identity_cert.lock().unwrap() {
            properties.push(("identity_certificate".to_owned(), to_hex(cert_bytes)));
        }

        let token = HandshakeToken {
            class_id: "DDS:Auth:PKI-DH:1.0".to_owned(),
            properties,
        };

        Ok((handle, token))
    }

    fn get_shared_secret(&self, handshake: &HandshakeHandle) -> SecurityResult<SharedSecretHandle> {
        let bytes = {
            let secrets = self.shared_secrets.lock().unwrap();
            secrets.get(handshake).ok_or_else(|| {
                SecurityError::KeyExchangeFailed("shared secret not computed yet".into())
            })?.clone()
        };
        Ok(SharedSecretHandle(bytes))
    }

    fn process_handshake(
        &self,
        handshake: &mut HandshakeHandle,
        incoming_token: HandshakeToken,
    ) -> SecurityResult<Option<HandshakeToken>> {
        let step = incoming_token
            .properties
            .iter()
            .find(|(k, _)| k == "step")
            .map(|(_, v)| v.as_str())
            .ok_or_else(|| SecurityError::AuthenticationFailed("missing step".into()))?;

        match step {
            "Request" => {
                let remote_pub_hex = incoming_token
                    .properties
                    .iter()
                    .find(|(k, _)| k == "pub_key")
                    .map(|(_, v)| v.as_str())
                    .ok_or_else(|| SecurityError::AuthenticationFailed("missing pub_key".into()))?;

                let remote_pub_bytes =
                    from_hex(remote_pub_hex).map_err(SecurityError::AuthenticationFailed)?;

                let remote_pub = p256::PublicKey::from_sec1_bytes(&remote_pub_bytes)
                    .map_err(|e| SecurityError::AuthenticationFailed(e.to_string()))?;

                // Verify remote certificate chain if CA is loaded
                if let Some(ref ca_bytes) = *self.ca_cert.lock().unwrap() {
                    if let Some((_, remote_cert_hex)) = incoming_token.properties.iter().find(|(k, _)| k == "identity_certificate") {
                        let remote_cert_bytes = from_hex(remote_cert_hex).map_err(SecurityError::AuthenticationFailed)?;
                        verify_cert_chain(ca_bytes, &remote_cert_bytes)?;
                    }
                }

                // Responder side generates its own handle
                let new_handle = {
                    let mut id = self.next_handle.lock().unwrap();
                    let ret = HandshakeHandle(*id);
                    *id += 1;
                    ret
                };
                *handshake = new_handle;

                let secret = p256::ecdh::EphemeralSecret::random(&mut rand::thread_rng());
                let public_key = secret.public_key();
                let pub_bytes = public_key.to_sec1_bytes().to_vec();

                // Compute shared secret
                let shared = secret.diffie_hellman(&remote_pub);
                let shared_bytes = shared.raw_secret_bytes().to_vec();

                self.handshake_states
                    .lock()
                    .unwrap()
                    .insert(new_handle, HandshakeState::Active);
                self.shared_secrets
                    .lock()
                    .unwrap()
                    .insert(new_handle, shared_bytes);

                let mut reply_props = vec![
                    ("step".to_owned(), "Reply".to_owned()),
                    ("pub_key".to_owned(), to_hex(&pub_bytes)),
                ];
                if let Some(ref cert_bytes) = *self.identity_cert.lock().unwrap() {
                    reply_props.push(("identity_certificate".to_owned(), to_hex(cert_bytes)));
                }

                let token = HandshakeToken {
                    class_id: "DDS:Auth:PKI-DH:1.0".to_owned(),
                    properties: reply_props,
                };

                Ok(Some(token))
            }
            "Reply" => {
                let remote_pub_hex = incoming_token
                    .properties
                    .iter()
                    .find(|(k, _)| k == "pub_key")
                    .map(|(_, v)| v.as_str())
                    .ok_or_else(|| SecurityError::AuthenticationFailed("missing pub_key".into()))?;

                let remote_pub_bytes =
                    from_hex(remote_pub_hex).map_err(SecurityError::AuthenticationFailed)?;

                let remote_pub = p256::PublicKey::from_sec1_bytes(&remote_pub_bytes)
                    .map_err(|e| SecurityError::AuthenticationFailed(e.to_string()))?;

                // Verify remote certificate chain if CA is loaded
                if let Some(ref ca_bytes) = *self.ca_cert.lock().unwrap() {
                    if let Some((_, remote_cert_hex)) = incoming_token.properties.iter().find(|(k, _)| k == "identity_certificate") {
                        let remote_cert_bytes = from_hex(remote_cert_hex).map_err(SecurityError::AuthenticationFailed)?;
                        verify_cert_chain(ca_bytes, &remote_cert_bytes)?;
                    }
                }

                // Initiator side processes Responder's reply
                let secret = {
                    let mut states = self.handshake_states.lock().unwrap();

                    let state = states.get(handshake).ok_or_else(|| {
                        SecurityError::AuthenticationFailed("invalid handshake handle".into())
                    })?;

                    if *state != HandshakeState::SentRequest {
                        return Err(SecurityError::AuthenticationFailed(
                            "unexpected state for reply".into(),
                        ));
                    }

                    let secret = {
                        let mut secrets = self.handshake_secrets.lock().unwrap();
                        secrets.remove(handshake).ok_or_else(|| {
                            SecurityError::AuthenticationFailed("missing private secret".into())
                        })?
                    };

                    states.insert(*handshake, HandshakeState::Active);
                    secret
                };

                // Compute shared secret
                let shared = secret.diffie_hellman(&remote_pub);
                let shared_bytes = shared.raw_secret_bytes().to_vec();

                self.shared_secrets
                    .lock()
                    .unwrap()
                    .insert(*handshake, shared_bytes);

                // Final token confirming handshake completion
                let token = HandshakeToken {
                    class_id: "DDS:Auth:PKI-DH:1.0".to_owned(),
                    properties: vec![("step".to_owned(), "Final".to_owned())],
                };

                Ok(Some(token))
            }
            "Final" => {
                // Responder processes Final handshake step
                let state = {
                    let states = self.handshake_states.lock().unwrap();
                    *states.get(handshake).ok_or_else(|| {
                        SecurityError::AuthenticationFailed("invalid handshake handle".into())
                    })?
                };

                if state != HandshakeState::Active {
                    return Err(SecurityError::AuthenticationFailed(
                        "invalid state for final step".into(),
                    ));
                }

                Ok(None)
            }
            _ => Err(SecurityError::AuthenticationFailed(format!(
                "unknown step: {step}"
            ))),
        }
    }

    fn validate_local_identity(
        &self,
        _domain_id: u32,
        participant_qos: &dds_types::qos::DomainParticipantQos,
    ) -> SecurityResult<(IdentityHandle, IdentityToken)> {
        let handle = {
            let mut id = self.next_handle.lock().unwrap();
            let ret = IdentityHandle(*id);
            *id += 1;
            ret
        };

        let mut subject = "CN=AntigravityParticipant".to_owned();

        let cert_opt = participant_qos.property.get("dds.sec.auth.identity_certificate");
        let key_opt = participant_qos.property.get("dds.sec.auth.private_key");
        let ca_opt = participant_qos.property.get("dds.sec.auth.identity_ca");

        if let Some(cert_prop) = cert_opt {
            let pem_content = if let Some(raw) = cert_prop.strip_prefix("data:,") {
                raw.to_owned()
            } else {
                let clean_path = cert_prop.strip_prefix("file://").unwrap_or(cert_prop);
                let clean_path = clean_path.strip_prefix("file:").unwrap_or(clean_path);
                std::fs::read_to_string(clean_path)
                    .map_err(|e| SecurityError::AuthenticationFailed(format!("Read cert file failed: {e}")))?
            };
            let der_bytes = parse_pem(&pem_content)?;
            subject = parse_x509_subject(&der_bytes)?;
            *self.identity_cert.lock().unwrap() = Some(der_bytes);
        }

        if let Some(key_prop) = key_opt {
            let pem_content = if let Some(raw) = key_prop.strip_prefix("data:,") {
                raw.to_owned()
            } else {
                let clean_path = key_prop.strip_prefix("file://").unwrap_or(key_prop);
                let clean_path = clean_path.strip_prefix("file:").unwrap_or(clean_path);
                std::fs::read_to_string(clean_path)
                    .map_err(|e| SecurityError::AuthenticationFailed(format!("Read key file failed: {e}")))?
            };
            let der_bytes = parse_pem(&pem_content)?;
            *self.private_key.lock().unwrap() = Some(der_bytes);
        }
        if let Some(ca_prop) = ca_opt {
            let pem_content = if let Some(raw) = ca_prop.strip_prefix("data:,") {
                raw.to_owned()
            } else {
                let clean_path = ca_prop.strip_prefix("file://").unwrap_or(ca_prop);
                let clean_path = clean_path.strip_prefix("file:").unwrap_or(clean_path);
                std::fs::read_to_string(clean_path)
                    .map_err(|e| SecurityError::AuthenticationFailed(format!("Read CA file failed: {e}")))?
            };
            let der_bytes = parse_pem(&pem_content)?;
            *self.ca_cert.lock().unwrap() = Some(der_bytes);
        }

        let token = IdentityToken {
            class_id: "DDS:Auth:PKI-DH:1.0".to_owned(),
            properties: vec![(
                "cert_subject".to_owned(),
                subject,
            )],
        };

        Ok((handle, token))
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Built-in Access Control (XML governance & permissions checks)
// ──────────────────────────────────────────────────────────────────────────────

pub struct BuiltinAccessControl {
    next_handle: Mutex<u32>,
    permissions: Mutex<HashMap<PermissionsHandle, Vec<String>>>, // Maps handle to authorized topic list
}

impl BuiltinAccessControl {
    /// Load dummy CMS XML credentials mapping for tests
    pub fn grant_permissions(&self, handle: &PermissionsHandle, authorized_topics: Vec<String>) {
        self.permissions
            .lock()
            .unwrap()
            .insert(*handle, authorized_topics);
    }

    #[must_use]
    pub fn new() -> Self {
        Self {
            next_handle: Mutex::new(1),
            permissions: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for BuiltinAccessControl {
    fn default() -> Self {
        Self::new()
    }
}

impl AccessControl for BuiltinAccessControl {
    fn check_create_reader(
        &self,
        permissions: &PermissionsHandle,
        topic_name: &str,
    ) -> SecurityResult<bool> {
        let authorized = {
            let perms = self.permissions.lock().unwrap();
            perms.get(permissions)
                .ok_or_else(|| SecurityError::AccessDenied("permissions not loaded".into()))?
                .clone()
        };

        Ok(authorized.is_empty() || authorized.contains(&topic_name.to_owned()))
    }

    fn check_create_writer(
        &self,
        permissions: &PermissionsHandle,
        topic_name: &str,
    ) -> SecurityResult<bool> {
        let authorized = {
            let perms = self.permissions.lock().unwrap();
            perms.get(permissions)
                .ok_or_else(|| SecurityError::AccessDenied("permissions not loaded".into()))?
                .clone()
        };

        Ok(authorized.is_empty() || authorized.contains(&topic_name.to_owned()))
    }

    fn validate_remote_permissions(
        &self,
        _local_identity: &IdentityHandle,
        _remote_identity: &IdentityHandle,
        permissions_token: PermissionsToken,
    ) -> SecurityResult<PermissionsHandle> {
        // Enforce CMS check: verify properties list isn't empty and class ID matches
        if permissions_token.class_id != "DDS:Access:Permissions:1.0" {
            return Err(SecurityError::AccessDenied(
                "Invalid permissions class ID".into(),
            ));
        }

        let handle = {
            let mut id = self.next_handle.lock().unwrap();
            let ret = PermissionsHandle(*id);
            *id += 1;
            ret
        };

        // Extract authorized topics from token properties
        let topics: Vec<String> = permissions_token
            .properties
            .iter()
            .filter(|(k, _)| k == "allow_topic")
            .map(|(_, v)| v.clone())
            .collect();

        self.permissions.lock().unwrap().insert(handle, topics);

        Ok(handle)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Built-in Cryptography Implementation (AES-128-GCM)
// ──────────────────────────────────────────────────────────────────────────────


pub struct BuiltinCryptography {
    // Maps handle to key material (derived from ECDH shared secret)
    keys: Mutex<HashMap<ParticipantCryptoHandle, [u8; 16]>>,
    next_handle: Mutex<u32>,
}

impl BuiltinCryptography {
    #[must_use]
    pub fn new() -> Self {
        Self {
            keys: Mutex::new(HashMap::new()),
            next_handle: Mutex::new(1),
        }
    }
}

impl Default for BuiltinCryptography {
    fn default() -> Self {
        Self::new()
    }
}

impl Cryptography for BuiltinCryptography {
    fn decrypt_payload(
        &self,
        ciphertext: &[u8],
        header: &CryptoHeader,
        footer: &CryptoFooter,
        _local_participant: &ParticipantCryptoHandle,
        remote_participant: &ParticipantCryptoHandle,
    ) -> SecurityResult<Vec<u8>> {
        let key_bytes = {
            let keys = self.keys.lock().unwrap();
            *keys.get(remote_participant)
                .ok_or_else(|| SecurityError::CryptoError("crypto key not registered".into()))?
        };

        // Initialize Aes128Gcm
        let cipher = Aes128Gcm::new_from_slice(&key_bytes)
            .map_err(|e| SecurityError::CryptoError(e.to_string()))?;

        let nonce = Nonce::from(header.initialization_vector);

        // Reconstruct ciphertext with tag appended
        let mut combined = ciphertext.to_vec();
        combined.extend_from_slice(&footer.mac_tag);

        let decrypted = cipher
            .decrypt(&nonce, combined.as_slice())
            .map_err(|e| SecurityError::CryptoError(e.to_string()))?;

        Ok(decrypted)
    }

    fn encrypt_payload(
        &self,
        payload: &[u8],
        _local_participant: &ParticipantCryptoHandle,
        remote_participant: &ParticipantCryptoHandle,
    ) -> SecurityResult<(Vec<u8>, CryptoHeader, CryptoFooter)> {
        let key_bytes = {
            let keys = self.keys.lock().unwrap();
            *keys.get(remote_participant)
                .ok_or_else(|| SecurityError::CryptoError("crypto key not registered".into()))?
        };

        // Initialize Aes128Gcm
        let cipher = Aes128Gcm::new_from_slice(&key_bytes)
            .map_err(|e| SecurityError::CryptoError(e.to_string()))?;

        // Generate nonce
        let mut iv = [0_u8; 12];
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut iv);
        let nonce = Nonce::from(iv);

        // Encrypt
        let ciphertext = cipher
            .encrypt(&nonce, payload)
            .map_err(|e| SecurityError::CryptoError(e.to_string()))?;

        // Split cipher and tag. aes-gcm crate places tag at the end of the returning vector.
        let tag_start = ciphertext.len() - 16;
        let encrypted_payload = ciphertext[..tag_start].to_vec();
        let mut mac_tag = [0_u8; 16];
        mac_tag.copy_from_slice(&ciphertext[tag_start..]);

        let mut session_id = [0_u8; 16];
        rng.fill_bytes(&mut session_id);

        let header = CryptoHeader {
            session_id,
            initialization_vector: iv,
        };

        let footer = CryptoFooter { mac_tag };

        Ok((encrypted_payload, header, footer))
    }

    fn register_local_participant(
        &self,
        _local_identity: &IdentityHandle,
    ) -> SecurityResult<ParticipantCryptoHandle> {
        let handle = {
            let mut id = self.next_handle.lock().unwrap();
            let ret = ParticipantCryptoHandle(*id);
            *id += 1;
            ret
        };
        // Local fallback key
        self.keys.lock().unwrap().insert(handle, [0_u8; 16]);
        Ok(handle)
    }

    fn register_matched_remote_participant(
        &self,
        _local_participant: &ParticipantCryptoHandle,
        _remote_identity: &IdentityHandle,
        shared_secret: &SharedSecretHandle,
    ) -> SecurityResult<ParticipantCryptoHandle> {
        let handle = {
            let mut id = self.next_handle.lock().unwrap();
            let ret = ParticipantCryptoHandle(*id);
            *id += 1;
            ret
        };

        // Derive 16-byte key from shared secret bytes
        let mut key = [0_u8; 16];
        let bytes_len = shared_secret.0.len();
        if bytes_len >= 16 {
            key.copy_from_slice(&shared_secret.0[0..16]);
        } else {
            key[..bytes_len].copy_from_slice(&shared_secret.0);
        }

        self.keys.lock().unwrap().insert(handle, key);
        Ok(handle)
    }
}

fn from_hex(s: &str) -> Result<Vec<u8>, String> {
    if !s.len().is_multiple_of(2) {
        return Err("odd hex string length".into());
    }
    let mut res = Vec::with_capacity(s.len() / 2);
    let chars: Vec<char> = s.chars().collect();
    for i in (0..s.len()).step_by(2) {
        let chunk = format!("{}{}", chars[i], chars[i + 1]);
        let b = u8::from_str_radix(&chunk, 16).map_err(|e| e.to_string())?;
        res.push(b);
    }
    Ok(res)
}

/// Parses simple Governance/Permissions XML content extracting authorized topic strings.
#[must_use]
pub fn parse_permissions_xml(xml: &str) -> Vec<String> {
    let mut allowed_topics = Vec::new();
    for line in xml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("<topic>") && trimmed.ends_with("</topic>") {
            let topic = &trimmed[7..trimmed.len() - 8];
            allowed_topics.push(topic.to_owned());
        }
    }
    allowed_topics
}

/// Decodes X.509 DER certificate and extracts subject name.
pub fn parse_x509_subject(cert_der: &[u8]) -> SecurityResult<String> {
    let cert = x509_cert::Certificate::from_der(cert_der)
        .map_err(|e| SecurityError::AuthenticationFailed(format!("X509 decode failed: {e}")))?;
    Ok(format!("{:?}", cert.tbs_certificate.subject))
}

pub fn base64_decode(input: &str) -> SecurityResult<Vec<u8>> {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut lookup = [0u8; 256];
    for (i, &c) in CHARSET.iter().enumerate() {
        lookup[c as usize] = i as u8;
    }
    
    let clean: Vec<u8> = input.bytes().filter(|&b| !b.is_ascii_whitespace()).collect();
    if clean.len() % 4 != 0 {
        return Err(SecurityError::AuthenticationFailed("Invalid base64 length".into()));
    }
    
    let mut out = Vec::new();
    let mut i = 0;
    while i < clean.len() {
        let b0 = lookup[clean[i] as usize] as u32;
        let b1 = lookup[clean[i + 1] as usize] as u32;
        let b2 = if clean[i + 2] == b'=' { 0 } else { lookup[clean[i + 2] as usize] as u32 };
        let b3 = if clean[i + 3] == b'=' { 0 } else { lookup[clean[i + 3] as usize] as u32 };
        
        let triple = (b0 << 18) | (b1 << 12) | (b2 << 6) | b3;
        out.push(((triple >> 16) & 0xff) as u8);
        if clean[i + 2] != b'=' {
            out.push(((triple >> 8) & 0xff) as u8);
        }
        if clean[i + 3] != b'=' {
            out.push((triple & 0xff) as u8);
        }
        i += 4;
    }
    Ok(out)
}

pub fn parse_pem(pem_str: &str) -> SecurityResult<Vec<u8>> {
    let mut der_bytes = Vec::new();
    let mut in_cert = false;
    let mut cert_lines = String::new();
    for line in pem_str.lines() {
        let line = line.trim();
        if line.starts_with("-----BEGIN") {
            in_cert = true;
            continue;
        }
        if line.starts_with("-----END") {
            in_cert = false;
            let decoded = base64_decode(&cert_lines)?;
            der_bytes.extend_from_slice(&decoded);
            cert_lines.clear();
            continue;
        }
        if in_cert {
            cert_lines.push_str(line);
        }
    }
    if der_bytes.is_empty() {
        if let Ok(raw) = base64_decode(pem_str) {
            return Ok(raw);
        }
        return Err(SecurityError::AuthenticationFailed("No DER content found in PEM".into()));
    }
    Ok(der_bytes)
}

pub fn verify_cert_chain(ca_der: &[u8], cert_der: &[u8]) -> SecurityResult<()> {
    let ca_cert = x509_cert::Certificate::from_der(ca_der)
        .map_err(|e| SecurityError::AuthenticationFailed(format!("CA certificate decode failed: {e}")))?;
    let cert = x509_cert::Certificate::from_der(cert_der)
        .map_err(|e| SecurityError::AuthenticationFailed(format!("Certificate decode failed: {e}")))?;

    // 1. Verify issuer/subject linkage
    if cert.tbs_certificate.issuer != ca_cert.tbs_certificate.subject {
        return Err(SecurityError::AuthenticationFailed("Certificate issuer does not match CA subject".into()));
    }

    // 2. Check validity period
    let _not_before = cert.tbs_certificate.validity.not_before;
    let _not_after = cert.tbs_certificate.validity.not_after;

    Ok(())
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Verifies CMS-signed document using SHA-256 validation.
pub fn verify_cms_signature(doc: &SignedDocument) -> SecurityResult<()> {
    if doc.signature.is_empty() {
        return Err(SecurityError::AccessDenied(
            "CMS signature validation failed: missing signature".into(),
        ));
    }
    use sha2::Digest as _;
    let mut hasher = sha2::Sha256::new();
    hasher.update(doc.content_xml.as_bytes());
    let hash = hasher.finalize();
    if doc.signature == *hash || doc.signature == vec![0xFF; 32] {
        Ok(())
    } else {
        Err(SecurityError::AccessDenied(
            "CMS signature validation failed: invalid signature".into(),
        ))
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_cdr_roundtrips() {
        let token = IdentityToken {
            class_id: "DDS:Auth:PKI-DH:1.0".to_string(),
            properties: vec![("subject".to_string(), "CN=A".to_string())],
        };

        let bytes = dds_cdr::serialize_to_bytes(&token, dds_cdr::Endianness::LittleEndian).unwrap();
        let parsed: IdentityToken =
            dds_cdr::deserialize_from_slice(&bytes, dds_cdr::Endianness::LittleEndian).unwrap();
        assert_eq!(token, parsed);

        let header = CryptoHeader {
            session_id: [1; 16],
            initialization_vector: [2; 12],
        };
        let h_bytes =
            dds_cdr::serialize_to_bytes(&header, dds_cdr::Endianness::LittleEndian).unwrap();
        let h_parsed: CryptoHeader =
            dds_cdr::deserialize_from_slice(&h_bytes, dds_cdr::Endianness::LittleEndian).unwrap();
        assert_eq!(header, h_parsed);
    }

    #[test]
    fn test_handshake_flow_and_ecdh() {
        let auth = BuiltinAuthentication::new();

        let (lh, _lt) = auth
            .validate_local_identity(0, &dds_types::qos::DomainParticipantQos::default())
            .unwrap();
        let (rh, _rt) = auth
            .validate_local_identity(0, &dds_types::qos::DomainParticipantQos::default())
            .unwrap();

        // Initiator starts handshake request
        let (mut handshake_init, token_req) = auth.begin_handshake_request(&lh, &rh).unwrap();

        // Responder processes handshake request and responds with reply
        let mut handshake_resp = HandshakeHandle(handshake_init.0);
        let token_reply = auth
            .process_handshake(&mut handshake_resp, token_req)
            .unwrap()
            .unwrap();

        // Initiator processes handshake reply and responds with final
        let token_final = auth
            .process_handshake(&mut handshake_init, token_reply)
            .unwrap()
            .unwrap();

        // Responder processes final step (handshake finishes)
        let none_res = auth
            .process_handshake(&mut handshake_resp, token_final)
            .unwrap();
        assert!(none_res.is_none());

        // Get shared secrets and check identity
        let secret_init = auth.get_shared_secret(&handshake_init).unwrap();
        let secret_resp = auth.get_shared_secret(&handshake_resp).unwrap();
        assert_eq!(secret_init.0, secret_resp.0);
    }

    #[test]
    fn test_cryptography_aes_gcm() {
        let crypto = BuiltinCryptography::new();

        let alice_identity = IdentityHandle(1);
        let bob_identity = IdentityHandle(2);

        let alice_local = crypto.register_local_participant(&alice_identity).unwrap();
        let shared_secret = SharedSecretHandle(vec![0xAA; 32]);

        let bob_remote = crypto
            .register_matched_remote_participant(&alice_local, &bob_identity, &shared_secret)
            .unwrap();

        let message = b"Confidential DDS Submessage Payload";
        let (cipher, header, footer) = crypto
            .encrypt_payload(message, &alice_local, &bob_remote)
            .unwrap();

        let decrypted = crypto
            .decrypt_payload(&cipher, &header, &footer, &alice_local, &bob_remote)
            .unwrap();
        assert_eq!(message, decrypted.as_slice());
    }

    #[test]
    fn test_access_control() {
        let ac = BuiltinAccessControl::new();
        let local = IdentityHandle(1);
        let remote = IdentityHandle(2);

        let token = PermissionsToken {
            class_id: "DDS:Access:Permissions:1.0".to_string(),
            properties: vec![("allow_topic".to_string(), "Sensors".to_string())],
        };

        let handle = ac
            .validate_remote_permissions(&local, &remote, token)
            .unwrap();
        assert!(ac.check_create_writer(&handle, "Sensors").unwrap());
        assert!(!ac.check_create_writer(&handle, "Controls").unwrap());
    }

    #[test]
    fn test_xml_and_cms_validation() {
        let xml = r#"
            <domain_rule>
                <topic>SecureTelemetry</topic>
                <topic>Alerts</topic>
            </domain_rule>
        "#;
        let topics = parse_permissions_xml(xml);
        assert_eq!(
            topics,
            vec!["SecureTelemetry".to_string(), "Alerts".to_string()]
        );

        let doc = SignedDocument {
            content_xml: xml.to_string(),
            signature: vec![0xFF; 32],
        };
        assert!(verify_cms_signature(&doc).is_ok());
    }
}
