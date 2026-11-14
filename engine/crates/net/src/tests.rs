//! Virtual-time protocol tests: codecs, the reliable channel, handshakes,
//! long lossy soaks with pacing, joins, timeouts, desyncs, and one real-UDP
//! loopback smoke test. Everything but the UDP test runs on [`LinkSim`]
//! with fixed seeds.
//!
//! [`LinkSim`]: crate::LinkSim

mod codec;
mod harness;
mod reliable;
mod session;
mod soak;
mod udp;
