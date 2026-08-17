//! # Locator — Network transport addresses for RTPS
//!
//! A `Locator` encodes a transport-specific address: a kind (UDP, TCP, etc.),
//! a port number, and a 16-byte address field (IPv4 uses the last 4 bytes).
//!
//! Reference: RTPS §8.2.4.3 — `Locator_t`.

use core::fmt;
use core::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

// ──────────────────────────────────────────────────────────────────────────────
// Locator Kind constants (RTPS §8.2.4.3)
// ──────────────────────────────────────────────────────────────────────────────

/// Transport kind for the locator. Values defined by the RTPS spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
#[non_exhaustive]
pub enum Kind {
    /// Invalid/unknown locator.
    Invalid = -1,
    /// UDP over IPv4 transport.
    UdpV4 = 1,
    /// UDP over IPv6 transport.
    UdpV6 = 2,
}

impl Kind {
    /// Parse a locator kind from its wire representation.
    #[must_use]
    #[inline]
    pub const fn from_i32(value: i32) -> Self {
        match value {
            1 => return Self::UdpV4,
            2 => return Self::UdpV6,
            _ => return Self::Invalid,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Locator — Transport address (RTPS §8.2.4.3)
// ──────────────────────────────────────────────────────────────────────────────

/// A transport-level address for an RTPS endpoint.
///
/// The 16-byte `address` field encodes:
/// - For `UDPv4`: bytes 12..16 hold the IPv4 address, bytes 0..12 are zero
/// - For `UDPv6`: all 16 bytes hold the IPv6 address
///
/// Reference: RTPS §8.2.4.3.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Locator {
    /// The 16-byte address field.
    pub address: [u8; 16],
    /// The transport kind (`UDPv4`, `UDPv6`, etc.).
    pub kind: Kind,
    /// The port number. 0 indicates "not specified".
    pub port: u32,
}

impl Locator {
    /// An invalid/unset locator — sentinel value.
    pub const INVALID: Self = Self {
        address: [0; 16],
        kind: Kind::Invalid,
        port: 0,
    };

    /// Create a locator from wire-format fields.
    #[must_use]
    #[inline]
    pub const fn from_raw(kind: Kind, port: u32, address: [u8; 16]) -> Self {
        return Self { address, kind, port };
    }

    /// Create a Locator from a `std::net::SocketAddr`.
    #[must_use]
    #[inline]
    pub fn from_socket_addr(addr: SocketAddr) -> Self {
        match addr {
            SocketAddr::V4(v4) => return Self::udpv4(*v4.ip(), u32::from(v4.port())),
            SocketAddr::V6(v6) => return Self::udpv6(*v6.ip(), u32::from(v6.port())),
        }
    }
    /// Extract the IPv4 address if this is a `UDPv4` locator.
    #[must_use]
    #[inline]
    pub fn to_ipv4(&self) -> Option<Ipv4Addr> {
        if self.kind != Kind::UdpV4 {
            return None;
        }
        return Some(Ipv4Addr::new(
            self.address[12],
            self.address[13],
            self.address[14],
            self.address[15],
        ))
    }

    /// Extract the IPv6 address if this is a `UDPv6` locator.
    #[must_use]
    #[inline]
    pub fn to_ipv6(&self) -> Option<Ipv6Addr> {
        if self.kind != Kind::UdpV6 {
            return None;
        }
        return Some(Ipv6Addr::from(self.address))
    }

    /// Convert to a `std::net::SocketAddr` if possible.
    #[must_use]
    #[inline]
    pub fn to_socket_addr(&self) -> Option<SocketAddr> {
        let port = match u16::try_from(self.port) {
            Ok(value) => value,
            Err(_) => return None,
        };
        match self.kind {
            Kind::UdpV4 => {
                let ip = match self.to_ipv4() {
                    Some(value) => value,
                    None => return None,
                };
                return Some(SocketAddr::V4(SocketAddrV4::new(ip, port)));
            }
            Kind::UdpV6 => {
                let ip = match self.to_ipv6() {
                    Some(value) => value,
                    None => return None,
                };
                return Some(SocketAddr::V6(SocketAddrV6::new(ip, port, 0, 0)));
            }
            Kind::Invalid => return None,
        }
    }

    /// Create a `UDPv4` locator from an IPv4 address and port.
    ///
    /// The IPv4 address is stored in bytes 12..16 of the address field,
    /// per the RTPS spec convention.
    #[must_use]
    #[inline]
    pub fn udpv4(addr: Ipv4Addr, port: u32) -> Self {
        let mut address = [u8::default(); 16];
        let octets = addr.octets();
        address[12..16].copy_from_slice(&octets);
        return Self {
            address,
            kind: Kind::UdpV4,
            port,
        }
    }

    /// Create a `UDPv6` locator from an IPv6 address and port.
    #[must_use]
    #[inline]
    pub const fn udpv6(addr: Ipv6Addr, port: u32) -> Self {
        return Self {
            address: addr.octets(),
            kind: Kind::UdpV6,
            port,
        }
    }

}

impl fmt::Debug for Locator {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            Kind::UdpV4 => {
                if let Some(ip) = self.to_ipv4() {
                    return write!(f, "Locator(UDPv4 {ip}:{})", self.port);
                }
                return write!(f, "Locator(UDPv4 ???:{})", self.port);
            }
            Kind::UdpV6 => {
                if let Some(ip) = self.to_ipv6() {
                    return write!(f, "Locator(UDPv6 [{ip}]:{})", self.port);
                }
                return write!(f, "Locator(UDPv6 ???:{})", self.port);
            }
            Kind::Invalid => return write!(f, "Locator(INVALID)"),
        }
    }
}

impl fmt::Display for Locator {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_socket_addr() {
            Some(addr) => return write!(f, "{addr}"),
            None => return write!(f, "INVALID"),
        }
    }
}

pub type LocatorKind = Kind;

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locator_invalid_sentinel() {
        let loc = Locator::INVALID;
        assert_eq!(loc.kind, Kind::Invalid);
        assert!(loc.to_socket_addr().is_none());
    }

    #[test]
    fn locator_udpv4_round_trip() {
        let addr = Ipv4Addr::new(192, 168, 1, 100);
        let loc = Locator::udpv4(addr, 7400);
        assert_eq!(loc.kind, Kind::UdpV4);
        assert_eq!(loc.port, 7400);
        assert_eq!(loc.to_ipv4(), Some(addr));
        assert!(loc.to_ipv6().is_none());
    }

    #[test]
    fn locator_udpv6_round_trip() {
        let addr = Ipv6Addr::LOCALHOST;
        let loc = Locator::udpv6(addr, 7401);
        assert_eq!(loc.kind, Kind::UdpV6);
        assert_eq!(loc.to_ipv6(), Some(addr));
        assert!(loc.to_ipv4().is_none());
    }

    #[test]
    fn locator_to_socket_addr_v4() {
        let loc = Locator::udpv4(Ipv4Addr::LOCALHOST, 8080);
        let sock = loc.to_socket_addr().unwrap();
        assert_eq!(sock.to_string(), "127.0.0.1:8080");
    }

    #[test]
    fn locator_to_socket_addr_v6() {
        let loc = Locator::udpv6(Ipv6Addr::LOCALHOST, 8080);
        let sock = loc.to_socket_addr().unwrap();
        assert_eq!(sock.to_string(), "[::1]:8080");
    }

    #[test]
    fn locator_from_socket_addr_v4() {
        let sock: SocketAddr = "10.0.0.1:5000".parse().unwrap();
        let loc = Locator::from_socket_addr(sock);
        assert_eq!(loc.kind, Kind::UdpV4);
        assert_eq!(loc.to_ipv4(), Some(Ipv4Addr::new(10, 0, 0, 1)));
        assert_eq!(loc.port, 5000);
    }

    #[test]
    fn locator_from_socket_addr_v6() {
        let sock: SocketAddr = "[::1]:5000".parse().unwrap();
        let loc = Locator::from_socket_addr(sock);
        assert_eq!(loc.kind, Kind::UdpV6);
        assert_eq!(loc.to_ipv6(), Some(Ipv6Addr::LOCALHOST));
    }

    #[test]
    fn locator_ipv4_address_byte_layout() {
        // RTPS spec: IPv4 stored in bytes 12..16, rest is zero
        let loc = Locator::udpv4(Ipv4Addr::new(10, 20, 30, 40), 0);
        assert_eq!(&loc.address[..12], &[u8::default(); 12]);
        assert_eq!(&loc.address[12..], &[10, 20, 30, 40]);
    }

    #[test]
    fn locator_debug_format_v4() {
        let loc = Locator::udpv4(Ipv4Addr::new(192, 168, 0, 1), 7400);
        let debug = format!("{loc:?}");
        assert!(debug.contains("UDPv4"));
        assert!(debug.contains("192.168.0.1"));
        assert!(debug.contains("7400"));
    }

    #[test]
    fn locator_kind_from_i32() {
        assert_eq!(Kind::from_i32(1), Kind::UdpV4);
        assert_eq!(Kind::from_i32(2), Kind::UdpV6);
        assert_eq!(Kind::from_i32(99), Kind::Invalid);
        assert_eq!(Kind::from_i32(-1), Kind::Invalid);
    }

    #[test]
    fn locator_equality() {
        let a = Locator::udpv4(Ipv4Addr::LOCALHOST, 7400);
        let b = Locator::udpv4(Ipv4Addr::LOCALHOST, 7400);
        let c = Locator::udpv4(Ipv4Addr::LOCALHOST, 7401);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
