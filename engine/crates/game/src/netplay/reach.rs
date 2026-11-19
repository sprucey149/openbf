//! Networking readouts (LAN §5b, issues #168, #181): the Host page's
//! LAN/VPN addresses and the Join page's "Test connection".
//!
//! Rules:
//! - This machine's IPv4 addresses come without an interface-list crate:
//!   the address a UDP socket takes when "connected" toward a public
//!   address (the default route's), and toward Tailscale's service address
//!   100.100.100.100 (a Tailscale address is in 100.64.0.0/10). A UDP
//!   connect sends nothing. A ZeroTier address, or a WireGuard tunnel that
//!   is not the default route, is not found this way. The Host page says "Not detected" rather
//!   than guessing a tunnel or a second LAN interface.
//! - A connection test sends the discovery `Find` to the one address,
//!   once a second (`net::LanBrowser`), and reports the first answer's
//!   ping, host name and whether its build matches; no answer within
//!   [`TEST_TIMEOUT`] is "No answer" with what to check. The caller drives
//!   it with its clock and a transport, so a test runs it in loopback.

use super::HostInfo;
use net::{LanBrowser, Micros, Transport};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

/// How long a connection test waits for an answer.
pub const TEST_TIMEOUT: Micros = Micros::from_secs(3);

/// Whether `ip` is a Tailscale address (100.64.0.0/10, CGNAT space).
pub fn is_tailscale(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 100 && (64..128).contains(&o[1])
}

/// The local address a UDP socket takes toward `to` (nothing is sent).
fn route_to(to: &str) -> Option<Ipv4Addr> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect(to).ok()?;
    match s.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_unspecified() && !ip.is_loopback() => Some(ip),
        _ => None,
    }
}

/// This machine's IPv4 addresses (module rules), each with whether it is a
/// Tailscale one; the default route's first.
pub fn local_ipv4s() -> Vec<(Ipv4Addr, bool)> {
    let mut v: Vec<(Ipv4Addr, bool)> = Vec::new();
    for to in ["192.0.2.1:9", "100.100.100.100:9"] {
        if let Some(ip) = route_to(to) {
            if !v.iter().any(|(a, _)| *a == ip) {
                v.push((ip, is_tailscale(ip)));
            }
        }
    }
    v
}

/// Only the Host page's LAN and identifiable VPN endpoints. An invalid
/// port must not advertise the default port in place of the typed value.
pub fn host_addresses(addrs: &[(Ipv4Addr, bool)], port: Option<u16>) -> Vec<String> {
    [("LAN Address", false), ("VPN Address", true)]
        .into_iter()
        .map(|(label, vpn)| {
            let address = match port.filter(|p| *p != 0) {
                None => "Unavailable (invalid UDP port)".to_owned(),
                Some(port) => addrs
                    .iter()
                    .find(|(ip, is_vpn)| {
                        *is_vpn == vpn && !ip.is_unspecified() && !ip.is_loopback()
                    })
                    .map(|(ip, _)| {
                        if port == net::DEFAULT_PORT {
                            ip.to_string()
                        } else {
                            SocketAddr::from((*ip, port)).to_string()
                        }
                    })
                    .unwrap_or_else(|| "Not detected".to_owned()),
            };
            format!("{label}: {address}")
        })
        .collect()
}

/// What a connection test found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TestState {
    Testing,
    Answered {
        name: String,
        ping_ms: u32,
        /// `None`: this build can join; else why not (another build, full).
        refusal: Option<String>,
    },
    NoAnswer,
}

/// The line the Join page shows for `state` of a test of `addr`.
pub fn test_text(addr: &str, state: &TestState) -> String {
    match state {
        TestState::Testing => format!("Testing {addr}\u{2026}"),
        TestState::Answered {
            name,
            ping_ms,
            refusal,
        } => match refusal {
            None => format!("{name} answered in {ping_ms} ms \u{2014} same build, ready to join"),
            Some(why) => format!("{name} answered in {ping_ms} ms \u{2014} cannot join: {why}"),
        },
        TestState::NoAnswer => {
            "No answer \u{2014} check the address, the host's firewall, and that the host has \
             opened the Host page"
                .to_owned()
        }
    }
}

/// One connection test (module rules).
#[derive(Debug)]
pub struct ConnectionTest {
    browser: LanBrowser,
    started: Option<Micros>,
    state: TestState,
}

impl ConnectionTest {
    pub fn new(target: SocketAddr) -> Self {
        Self {
            browser: LanBrowser::new(vec![target]),
            started: None,
            state: TestState::Testing,
        }
    }

    /// Sends the due `Find`, reads the answers; `build` is this build's id.
    /// Once decided, it stays decided.
    pub fn pump(&mut self, now: Micros, t: &mut dyn Transport, build: &str) -> &TestState {
        if self.state != TestState::Testing {
            return &self.state;
        }
        let started = *self.started.get_or_insert(now);
        self.browser.pump(now, t);
        if let Some(h) = self.browser.hosts().into_iter().next() {
            let info = HostInfo::decode(&h.info).unwrap_or_default();
            self.state = TestState::Answered {
                refusal: info.refusal(build),
                name: if info.name.is_empty() {
                    h.addr.to_string()
                } else {
                    info.name
                },
                ping_ms: h.rtt.as_millis_f64().round() as u32,
            };
        } else if now.saturating_sub(started) >= TEST_TIMEOUT {
            self.state = TestState::NoAnswer;
        }
        &self.state
    }

    pub fn state(&self) -> &TestState {
        &self.state
    }
}

#[cfg(test)]
mod host_draw_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use net::{HostConfig, HostCore, UdpTransport};

    #[test]
    fn reach_host_addresses_label_routes_and_custom_ports() {
        let addrs = [
            (Ipv4Addr::new(192, 168, 255, 254), false),
            (Ipv4Addr::new(100, 127, 255, 254), true),
        ];
        assert_eq!(
            host_addresses(&addrs, Some(net::DEFAULT_PORT)),
            [
                "LAN Address: 192.168.255.254",
                "VPN Address: 100.127.255.254",
            ]
        );
        assert_eq!(
            host_addresses(&addrs, Some(65535)),
            [
                "LAN Address: 192.168.255.254:65535",
                "VPN Address: 100.127.255.254:65535",
            ]
        );
        assert_eq!(
            host_addresses(&addrs[..1], Some(40000)),
            [
                "LAN Address: 192.168.255.254:40000",
                "VPN Address: Not detected",
            ]
        );
        assert_eq!(
            host_addresses(&addrs[1..], Some(40000)),
            [
                "LAN Address: Not detected",
                "VPN Address: 100.127.255.254:40000",
            ]
        );
    }

    #[test]
    fn reach_host_addresses_do_not_invent_missing_routes_or_ports() {
        let unknown = ["LAN Address: Not detected", "VPN Address: Not detected"];
        assert_eq!(host_addresses(&[], Some(net::DEFAULT_PORT)), unknown);
        assert_eq!(
            host_addresses(
                &[(Ipv4Addr::LOCALHOST, false), (Ipv4Addr::UNSPECIFIED, true)],
                Some(1)
            ),
            unknown
        );
        for port in [None, Some(0)] {
            assert_eq!(
                host_addresses(&[(Ipv4Addr::new(10, 0, 0, 1), false)], port),
                [
                    "LAN Address: Unavailable (invalid UDP port)",
                    "VPN Address: Unavailable (invalid UDP port)",
                ]
            );
        }
        assert!(is_tailscale(Ipv4Addr::new(100, 64, 0, 1)));
        assert!(!is_tailscale(Ipv4Addr::new(100, 128, 0, 1)));
        assert!(!is_tailscale(Ipv4Addr::new(10, 0, 0, 1)));
        let mine = local_ipv4s();
        assert!(mine.iter().all(|(ip, _)| !ip.is_loopback()));
        assert!(mine.len() <= 2);
    }

    fn host_on_loopback(build: &str) -> (HostCore, UdpTransport) {
        let t = UdpTransport::bind("127.0.0.1:0").expect("loopback socket");
        let mut host = HostCore::new(HostConfig::new(build, "the maintainer"));
        host.set_info(
            HostInfo {
                name: "the maintainer".into(),
                map: String::new(),
                players: 1,
                max_players: 8,
                protocol: net::PROTOCOL,
                build: build.into(),
            }
            .encode(),
        );
        (host, t)
    }

    /// Serves the host's socket once.
    fn serve(host: &mut HostCore, t: &mut UdpTransport, now: Micros) {
        let mut buf = [0u8; net::MAX_DATAGRAM + 1];
        while let Ok(Some((n, from))) = t.recv(&mut buf) {
            host.handle_datagram(now, from, &buf[..n]);
        }
        for (to, b) in host.update(now) {
            let _ = t.send(to, &b);
        }
    }

    fn run(
        test: &mut ConnectionTest,
        host: Option<(&mut HostCore, &mut UdpTransport)>,
        build: &str,
    ) -> TestState {
        let mut me = UdpTransport::bind("127.0.0.1:0").expect("client socket");
        let mut host = host;
        // Virtual time; real sockets on loopback answer within the polls.
        for step in 0..400u64 {
            let now = Micros::from_millis(step * 10);
            test.pump(now, &mut me, build);
            if let Some((h, t)) = host.as_mut() {
                for _ in 0..3 {
                    serve(h, t, now);
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }
            if *test.state() != TestState::Testing {
                break;
            }
        }
        test.state().clone()
    }

    /// A loopback host answers with its name; another build cannot join.
    #[test]
    fn reach_connection_test_with_a_loopback_host() {
        let (mut host, mut t) = host_on_loopback("build-a");
        let at = t.local_addr().unwrap();
        let mut test = ConnectionTest::new(at);
        let got = run(&mut test, Some((&mut host, &mut t)), "build-a");
        let TestState::Answered { name, refusal, .. } = &got else {
            panic!("no answer: {got:?}");
        };
        assert_eq!((name.as_str(), refusal), ("the maintainer", &None));
        assert!(test_text("x", &got).contains("same build"));
        let mut other = ConnectionTest::new(at);
        let got = run(&mut other, Some((&mut host, &mut t)), "build-b");
        assert!(matches!(
            &got,
            TestState::Answered {
                refusal: Some(_),
                ..
            }
        ));
        assert!(test_text("x", &got).contains("cannot join: another openbf build"));
    }

    /// Nobody there: "No answer" after the timeout, with what to check.
    #[test]
    fn reach_connection_test_without_a_host() {
        let quiet = UdpTransport::bind("127.0.0.1:0").unwrap();
        let mut test = ConnectionTest::new(quiet.local_addr().unwrap());
        assert_eq!(
            test_text("10.0.0.5", test.state()),
            "Testing 10.0.0.5\u{2026}"
        );
        assert_eq!(run(&mut test, None, "build-a"), TestState::NoAnswer);
        assert!(test_text("x", test.state()).contains("host's firewall"));
    }
}
