//! # Return Codes and Error Types for DDS
//!
//! Defines `DdsError` — the unified error type for all DDS operations,
//! and `DdsResult<T>` — the standard Result alias.
//!
//! Each variant maps to a return code from the DDS specification.
//!
//! Reference: DCPS §2.2.1.1 — Return Codes

use std::fmt;

/// Alias for `Result<T, DdsError>` — used throughout the DDS stack.
pub type DdsResult<T> = Result<T, DdsError>;

/// DDS operation error codes, matching the spec's ReturnCode_t values.
///
/// Successful operations return `Ok(T)` rather than a `ReturnCode::OK`
/// variant — we use Rust's `Result` idiom instead.
///
/// Reference: DCPS §2.2.1.1, Table 2.1
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DdsError {
    /// Generic, unspecified error.
    #[error("unspecified error: {0}")]
    Error(String),

    /// Unsupported operation or QoS policy.
    #[error("operation not supported: {0}")]
    Unsupported(String),

    /// Invalid parameter passed to a DDS operation.
    #[error("bad parameter: {0}")]
    BadParameter(String),

    /// A precondition for the operation was not met.
    /// E.g., attempting to delete an entity that still has dependents.
    #[error("precondition not met: {0}")]
    PreconditionNotMet(String),

    /// The middleware ran out of a resource governed by QoS
    /// (e.g., `ResourceLimits` exceeded).
    #[error("out of resources: {0}")]
    OutOfResources(String),

    /// The entity has not been enabled yet (see `EntityFactory` QoS).
    #[error("entity not enabled")]
    NotEnabled,

    /// An immutable QoS policy was changed, or an incompatible
    /// QoS change was attempted on an enabled entity.
    #[error("immutable QoS policy violation: {0}")]
    ImmutablePolicy(String),

    /// The requested QoS policies are internally inconsistent.
    /// E.g., `Deadline.period < TimeBasedFilter.minimum_separation`.
    #[error("inconsistent QoS policy: {0}")]
    InconsistentPolicy(String),

    /// The data was not available (e.g., `read()`/`take()` with no samples).
    #[error("no data available")]
    NoData,

    /// An operation timed out (e.g., `wait_for_acknowledgments`).
    #[error("timeout")]
    Timeout,

    /// An illegal operation was attempted (programming error).
    #[error("illegal operation: {0}")]
    IllegalOperation(String),

    /// A security-related error (DDS-Security §8).
    #[error("security error: {0}")]
    NotAllowedBySecurity(String),
}

/// Numeric return code values matching the OMG spec's `ReturnCode_t`.
/// Provided for interoperability and debugging; the Rust API uses `Result`.
///
/// Reference: DCPS §2.2.1.1
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum ReturnCode {
    /// Operation completed successfully.
    Ok = 0,
    /// Generic error.
    Error = 1,
    /// Unsupported operation.
    Unsupported = 2,
    /// Bad parameter.
    BadParameter = 3,
    /// Precondition not met.
    PreconditionNotMet = 4,
    /// Out of resources.
    OutOfResources = 5,
    /// Not enabled.
    NotEnabled = 6,
    /// Immutable policy.
    ImmutablePolicy = 7,
    /// Inconsistent policy.
    InconsistentPolicy = 8,
    /// Already deleted.
    AlreadyDeleted = 9,
    /// Timeout.
    Timeout = 10,
    /// No data.
    NoData = 11,
    /// Illegal operation.
    IllegalOperation = 12,
    /// Security violation.
    NotAllowedBySecurity = 13,
}

impl fmt::Display for ReturnCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ok => write!(f, "OK"),
            Self::Error => write!(f, "ERROR"),
            Self::Unsupported => write!(f, "UNSUPPORTED"),
            Self::BadParameter => write!(f, "BAD_PARAMETER"),
            Self::PreconditionNotMet => write!(f, "PRECONDITION_NOT_MET"),
            Self::OutOfResources => write!(f, "OUT_OF_RESOURCES"),
            Self::NotEnabled => write!(f, "NOT_ENABLED"),
            Self::ImmutablePolicy => write!(f, "IMMUTABLE_POLICY"),
            Self::InconsistentPolicy => write!(f, "INCONSISTENT_POLICY"),
            Self::AlreadyDeleted => write!(f, "ALREADY_DELETED"),
            Self::Timeout => write!(f, "TIMEOUT"),
            Self::NoData => write!(f, "NO_DATA"),
            Self::IllegalOperation => write!(f, "ILLEGAL_OPERATION"),
            Self::NotAllowedBySecurity => write!(f, "NOT_ALLOWED_BY_SECURITY"),
        }
    }
}

impl From<&DdsError> for ReturnCode {
    fn from(err: &DdsError) -> Self {
        match err {
            DdsError::Error(_) => Self::Error,
            DdsError::Unsupported(_) => Self::Unsupported,
            DdsError::BadParameter(_) => Self::BadParameter,
            DdsError::PreconditionNotMet(_) => Self::PreconditionNotMet,
            DdsError::OutOfResources(_) => Self::OutOfResources,
            DdsError::NotEnabled => Self::NotEnabled,
            DdsError::ImmutablePolicy(_) => Self::ImmutablePolicy,
            DdsError::InconsistentPolicy(_) => Self::InconsistentPolicy,
            DdsError::NoData => Self::NoData,
            DdsError::Timeout => Self::Timeout,
            DdsError::IllegalOperation(_) => Self::IllegalOperation,
            DdsError::NotAllowedBySecurity(_) => Self::NotAllowedBySecurity,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dds_error_display_messages() {
        let err = DdsError::BadParameter("null pointer".into());
        assert_eq!(err.to_string(), "bad parameter: null pointer");
    }

    #[test]
    fn dds_error_no_data_display() {
        assert_eq!(DdsError::NoData.to_string(), "no data available");
    }

    #[test]
    fn dds_error_timeout_display() {
        assert_eq!(DdsError::Timeout.to_string(), "timeout");
    }

    #[test]
    fn dds_error_not_enabled_display() {
        assert_eq!(DdsError::NotEnabled.to_string(), "entity not enabled");
    }

    #[test]
    fn return_code_from_error() {
        assert_eq!(ReturnCode::from(&DdsError::NoData), ReturnCode::NoData);
        assert_eq!(ReturnCode::from(&DdsError::Timeout), ReturnCode::Timeout);
        assert_eq!(
            ReturnCode::from(&DdsError::BadParameter("x".into())),
            ReturnCode::BadParameter
        );
        assert_eq!(
            ReturnCode::from(&DdsError::NotAllowedBySecurity("x".into())),
            ReturnCode::NotAllowedBySecurity
        );
    }

    #[test]
    fn return_code_numeric_values() {
        // These values are mandated by the spec (DCPS §2.2.1.1)
        assert_eq!(ReturnCode::Ok as i32, 0);
        assert_eq!(ReturnCode::Error as i32, 1);
        assert_eq!(ReturnCode::Unsupported as i32, 2);
        assert_eq!(ReturnCode::BadParameter as i32, 3);
        assert_eq!(ReturnCode::PreconditionNotMet as i32, 4);
        assert_eq!(ReturnCode::OutOfResources as i32, 5);
        assert_eq!(ReturnCode::NotEnabled as i32, 6);
        assert_eq!(ReturnCode::ImmutablePolicy as i32, 7);
        assert_eq!(ReturnCode::InconsistentPolicy as i32, 8);
        assert_eq!(ReturnCode::AlreadyDeleted as i32, 9);
        assert_eq!(ReturnCode::Timeout as i32, 10);
        assert_eq!(ReturnCode::NoData as i32, 11);
        assert_eq!(ReturnCode::IllegalOperation as i32, 12);
        assert_eq!(ReturnCode::NotAllowedBySecurity as i32, 13);
    }

    #[test]
    fn return_code_display() {
        assert_eq!(ReturnCode::Ok.to_string(), "OK");
        assert_eq!(ReturnCode::NoData.to_string(), "NO_DATA");
        assert_eq!(
            ReturnCode::NotAllowedBySecurity.to_string(),
            "NOT_ALLOWED_BY_SECURITY"
        );
    }

    #[test]
    fn dds_result_ok_variant() {
        let result: DdsResult<i32> = Ok(42);
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn dds_result_err_variant() {
        let result: DdsResult<i32> = Err(DdsError::Timeout);
        assert!(result.is_err());
    }

    #[test]
    fn dds_error_equality() {
        assert_eq!(DdsError::Timeout, DdsError::Timeout);
        assert_eq!(DdsError::NoData, DdsError::NoData);
        assert_ne!(DdsError::Timeout, DdsError::NoData);
    }

    #[test]
    fn dds_error_clone() {
        let err = DdsError::Error("test".into());
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }
}
