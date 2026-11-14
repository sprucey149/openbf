//! Moving datagrams: the [`Transport`] trait, real UDP ([`UdpTransport`]),
//! and the address helpers for joining by name.
//!
//! Rules:
//! - Non-blocking: `recv` returns `Ok(None)` when nothing waits. The network
//!   thread polls; it never blocks inside a transport.
//! - Hosts bind `0.0.0.0:<port>` (default 37450); clients and browsers bind
//!   an ephemeral port. Broadcast is enabled so discovery can send to
//!   `255.255.255.255` and directed broadcasts.
//! - A datagram over [`crate::MAX_DATAGRAM`] is truncated by the buffer and
//!   then rejected by the packet parser; transports do not police content.

use std::io;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};

/// Something that sends and receives datagrams.
pub trait Transport {
    fn send(&mut self, to: SocketAddr, bytes: &[u8]) -> io::Result<()>;
    /// One waiting datagram into `buf`, or `None`.
    fn recv(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>>;
    fn local_addr(&self) -> io::Result<SocketAddr>;
}

/// A non-blocking UDP socket.
#[derive(Debug)]
pub struct UdpTransport {
    socket: UdpSocket,
}

impl UdpTransport {
    /// Binds `addr` (`"0.0.0.0:37450"` to host, `"0.0.0.0:0"` to join).
    pub fn bind(addr: impl ToSocketAddrs) -> io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;
        // Broadcast is only needed for discovery; some sandboxes refuse it.
        let _ = socket.set_broadcast(true);
        Ok(Self { socket })
    }

    /// Binds the host port on every interface.
    pub fn host(port: u16) -> io::Result<Self> {
        Self::bind(("0.0.0.0", port))
    }

    /// Binds an ephemeral port on every interface.
    pub fn client() -> io::Result<Self> {
        Self::bind("0.0.0.0:0")
    }
}

impl Transport for UdpTransport {
    fn send(&mut self, to: SocketAddr, bytes: &[u8]) -> io::Result<()> {
        match self.socket.send_to(bytes, to) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(()),
            Err(e) => Err(e),
        }
    }

    fn recv(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
        match self.socket.recv_from(buf) {
            Ok((n, from)) => Ok(Some((n, from))),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            // Windows reports an ICMP port-unreachable as a reset on the next
            // read; it says nothing about other peers.
            Err(e) if e.kind() == io::ErrorKind::ConnectionReset => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }
}

/// Resolves "host", "host:port", an IPv4/IPv6 literal or a MagicDNS name,
/// filling in `default_port` when none is given. Blocking (DNS); call it off
/// the frame loop.
pub fn resolve(addr: &str, default_port: u16) -> io::Result<SocketAddr> {
    let addr = addr.trim();
    if let Ok(sa) = addr.parse::<SocketAddr>() {
        return Ok(sa);
    }
    if let Ok(ip) = addr.trim_matches(['[', ']']).parse::<std::net::IpAddr>() {
        return Ok(SocketAddr::new(ip, default_port));
    }
    let with_port = if addr
        .rsplit_once(':')
        .is_some_and(|(_, p)| p.parse::<u16>().is_ok())
    {
        addr.to_owned()
    } else {
        format!("{addr}:{default_port}")
    };
    let mut it = with_port.to_socket_addrs()?;
    let all: Vec<SocketAddr> = it.by_ref().collect();
    all.iter()
        .find(|a| a.is_ipv4())
        .or(all.first())
        .copied()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no address"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_resolves_literals_with_default_port() {
        assert_eq!(
            resolve("10.0.0.2", 37450).unwrap(),
            "10.0.0.2:37450".parse().unwrap()
        );
        assert_eq!(
            resolve("10.0.0.2:5000", 37450).unwrap(),
            "10.0.0.2:5000".parse().unwrap()
        );
        assert_eq!(
            resolve("::1", 37450).unwrap(),
            "[::1]:37450".parse().unwrap()
        );
    }
}
