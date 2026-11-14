//! A real-UDP loopback smoke test through [`crate::NetSession`].

use crate::*;
use std::time::{Duration, Instant};

#[test]
fn udp_loopback_host_and_client_exchange_bundles() {
    let host_t = UdpTransport::bind("127.0.0.1:0").expect("bind host");
    let host_addr = host_t.local_addr().unwrap();
    let host = NetSession::host(HostConfig::new("udp-test", "Host"), host_t);
    let client_t = UdpTransport::bind("127.0.0.1:0").expect("bind client");
    let client = NetSession::join(
        ClientConfig::new("udp-test", "Guest", 5),
        host_addr,
        client_t,
    );

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut host_bundles = Vec::new();
    let mut client_bundles = Vec::new();
    let mut started: Option<Instant> = None;
    while Instant::now() < deadline {
        for e in host.drain_events() {
            match e {
                NetEvent::PlayerJoined { slot, .. } => host.send(HostCommand::LoadRound {
                    setup: b"s".to_vec(),
                    humans: vec![0, slot],
                }),
                NetEvent::AllLoaded { .. } => host.send(HostCommand::StartRound),
                NetEvent::RoundStarted { .. } => started = Some(Instant::now()),
                NetEvent::Bundle(b) => host_bundles.push(b),
                _ => {}
            }
        }
        for e in client.drain_events() {
            match e {
                ClientEvent::LoadRound { round_id, .. } => client.send(ClientCommand::Loaded {
                    round_id,
                    static_digest: 1,
                    hash0: 2,
                }),
                ClientEvent::Bundle(b) => {
                    client.send(ClientCommand::SubmitInput(
                        core::sim::PadState {
                            mx: 1,
                            ..Default::default()
                        },
                        b.tick + 1,
                    ));
                    client_bundles.push(b);
                }
                _ => {}
            }
        }
        if started.is_some_and(|t| t.elapsed() >= Duration::from_secs(1)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(started.is_some(), "round never started");
    assert!(
        host_bundles.len() >= 20,
        "host closed {}",
        host_bundles.len()
    );
    assert!(
        client_bundles.len() >= 20,
        "client got {}",
        client_bundles.len()
    );
    assert_eq!(client_bundles[..], host_bundles[..client_bundles.len()]);
    assert!(host_bundles
        .iter()
        .any(|b| b.pad(1).is_some_and(|p| p.mx == 1)));
    let st = client.stats();
    assert!(st.link.packets_recv > 20);
    client.shutdown();
    host.shutdown();
}
