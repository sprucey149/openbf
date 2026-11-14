//! Encoding round trips and hostile-input checks.

use crate::bundle::*;
use crate::linksim::Lcg;
use crate::msg::{Msg, RejectReason, RosterEntry};
use crate::packet::*;
use crate::wire::{Reader, Writer};
use core::sim::{PadState, ScreenPick};

fn random_pad(r: &mut Lcg) -> PadState {
    let pick = match r.upto(5) {
        0 => Some(ScreenPick::Side(r.upto(3) as u8)),
        1 => Some(ScreenPick::Post(r.next_u64() as u16)),
        2 => Some(ScreenPick::Walker {
            index: r.next_u64() as u32,
            gen: r.next_u64() as u32,
        }),
        _ => None,
    };
    let small = r.unit() < 0.5;
    PadState {
        lx: r.next_u64() as i16,
        ly: if small { 0 } else { r.next_u64() as i16 },
        rx: r.next_u64() as i16,
        ry: r.next_u64() as i16,
        buttons: if small {
            r.upto(3) as u32
        } else {
            r.next_u64() as u32
        },
        mx: if small { 0 } else { r.next_u64() as i32 },
        my: r.next_u64() as i32,
        fire_lag: if small { 0 } else { r.next_u64() as u8 },
        pick,
        view_lag: if small { 0 } else { r.upto(8) as u8 },
    }
}

fn random_bundle(r: &mut Lcg, tick: u32) -> TickBundle {
    let mut pads = Vec::new();
    let mut seqs = Vec::new();
    for slot in 0..MAX_SLOTS as u8 {
        if r.unit() < 0.3 {
            pads.push((slot, random_pad(r)));
            seqs.push(r.next_u64() as u32 >> r.upto(31));
        }
    }
    let mut commands = Vec::new();
    for _ in 0..r.upto(3) {
        commands.push(match r.upto(3) {
            0 => SessionCommand::AddHuman {
                slot: r.upto(31) as u8,
                team: r.upto(255) as u8,
                profile: vec![r.next_u64() as u8; r.upto(40) as usize],
            },
            1 => SessionCommand::RemoveHuman {
                slot: r.upto(31) as u8,
            },
            2 => SessionCommand::Game(vec![7; r.upto(100) as usize]),
            _ => SessionCommand::EndRound,
        });
    }
    TickBundle {
        tick,
        pads,
        commands,
        seqs,
    }
}

#[test]
fn codec_random_pads_round_trip_against_random_bases() {
    let mut r = Lcg(1);
    for _ in 0..5_000 {
        let base = random_pad(&mut r);
        let pad = if r.unit() < 0.3 {
            base
        } else {
            random_pad(&mut r)
        };
        let mut w = Writer::new();
        write_pad(&mut w, &pad, &base);
        let mut rd = Reader::new(&w.buf);
        assert_eq!(read_pad(&mut rd, &base).unwrap(), pad);
        assert!(rd.is_done());
        if pad == base {
            assert_eq!(w.buf.len(), 1);
        }
    }
}

#[test]
fn codec_bundle_chains_and_logs_round_trip() {
    let mut r = Lcg(2);
    let bundles: Vec<_> = (0..300).map(|t| random_bundle(&mut r, 100 + t)).collect();
    let base = random_bundle(&mut r, 99).pads;
    let mut w = Writer::new();
    write_chain(&mut w, &bundles, &base);
    let mut rd = Reader::new(&w.buf);
    assert_eq!(read_chain(&mut rd, 100, 300, &base).unwrap(), bundles);
    let z = compress_log(&bundles);
    assert_eq!(decompress_log(&z, 100, 300).unwrap(), bundles);
    assert!(decompress_log(&z, 100, 301).is_err());
    // Packet-sized bundle runs stay under the datagram limit.
    let (body, n) = encode_bundles(3, 100, Some(&base), &bundles, 1100);
    assert!(n > 0 && body.len() <= 1100);
    let (head, rest) = decode_bundles_head(&body).unwrap();
    assert_eq!(
        (head.stream, head.first_tick, head.has_base),
        (3, 100, true)
    );
    assert_eq!(decode_bundles_rest(rest, 100, &base).unwrap(), bundles[..n]);
}

#[test]
fn codec_inputs_round_trip() {
    let mut r = Lcg(3);
    let base = random_pad(&mut r);
    // View ticks that go up, hold and (a reordered frame) step back.
    let pads: Vec<_> = (0..40u32)
        .map(|i| (random_pad(&mut r), 1000 + i - (i % 7 == 3) as u32 * 2))
        .collect();
    let (body, n) = encode_inputs(17, &base, &pads, 600);
    assert!(n > 5 && n < 40);
    let (seq, got) = decode_inputs(&body, |s| (s == 17).then_some(base))
        .unwrap()
        .unwrap();
    assert_eq!((seq, &got[..]), (17, &pads[..n]));
    assert_eq!(decode_inputs(&body, |_| None).unwrap(), None);
}

#[test]
fn codec_messages_round_trip() {
    let msgs = vec![
        Msg::Hello {
            protocol: PROTOCOL,
            build: "abc+dirty".into(),
            name: "Pëte".into(),
            token: u64::MAX,
            content: vec![1; 3000],
            profile: vec![2; 40],
        },
        Msg::Welcome {
            slot: 3,
            max_players: 16,
        },
        Msg::Reject {
            reason: RejectReason::Full,
            detail: "x".into(),
        },
        Msg::Lobby {
            version: 9,
            state: vec![3; 5000],
        },
        Msg::LoadRound {
            round_id: 2,
            stream: 5,
            joining: true,
            log_len: 18_000,
            setup: vec![4; 100],
            roster: vec![RosterEntry {
                slot: 0,
                name: "Host".into(),
                profile: vec![5],
            }],
        },
        Msg::LogChunk {
            stream: 5,
            first_tick: 2048,
            count: 2048,
            data: vec![6; 70_000],
        },
        Msg::Loaded {
            round_id: 2,
            static_digest: 7,
            hash0: 8,
        },
        Msg::Start { round_id: 2 },
        Msg::HashReport { tick: 30, hash: 9 },
        Msg::Desync {
            tick: 30,
            host_hash: 0xfeed,
            build: "abc".into(),
            target: "linux-x86_64".into(),
        },
        Msg::Leave,
        Msg::Ready { round_id: 2 },
        Msg::Request(vec![1, 2]),
        Msg::Notice(vec![3]),
        Msg::Chat {
            slot: 2,
            text: "hi".into(),
        },
        Msg::CaughtUp {
            round_id: 2,
            tick: 900,
        },
        Msg::Resync,
        Msg::DesyncHashes {
            tick: 60,
            build: "abc".into(),
            target: "windows-x86_64".into(),
            first_tick: 1,
            width: 6,
            values: (0..360).collect(),
        },
    ];
    for m in msgs {
        let b = m.encode();
        assert_eq!(Msg::decode(&b).unwrap(), m);
        for cut in [1, b.len() / 2, b.len() - 1] {
            if cut < b.len() && cut > 0 {
                assert!(Msg::decode(&b[..cut]).is_err() || b.len() == 1);
            }
        }
    }
    assert!(Msg::decode(&[0]).is_err());
    assert!(Msg::decode(&[99]).is_err());
    // Desync rows: a width outside 1..=8 or more than 128 rows fail.
    let rows = |width: u8, rows: u64| {
        let mut w = crate::wire::Writer::new();
        w.u8(18);
        w.var(60);
        w.str("abc");
        w.str("t");
        w.var(1);
        w.u8(width);
        w.var(rows);
        for _ in 0..rows * u64::from(width) {
            w.u64(7);
        }
        Msg::decode(&w.buf)
    };
    assert!(rows(6, 128).is_ok());
    assert!(rows(0, 0).is_err());
    assert!(rows(9, 1).is_err());
    assert!(rows(1, 129).is_err());
    assert!(crate::msg::valid_name("Stormtrooper TK-421"));
    let taken = [
        "the maintainer".to_string(),
        "playtest_1".into(),
        "x".repeat(24),
        format!("{}_1", "x".repeat(22)),
    ];
    let free =
        |n: &str| crate::msg::unique_name(n, |c| taken.iter().any(|t| t.eq_ignore_ascii_case(c)));
    assert_eq!(free("Ann"), "Ann");
    assert_eq!(free("THE MAINTAINER"), "playtest_2");
    let long = free(&"x".repeat(24));
    assert_eq!(long, format!("{}_2", "x".repeat(22)));
    assert!(crate::msg::valid_name(&long));
    assert!(!crate::msg::valid_name("   "));
    assert!(!crate::msg::valid_name(&"x".repeat(25)));
}

#[test]
fn codec_hostile_bytes_never_panic() {
    let mut r = Lcg(4);
    let mut pw = PacketWriter::new(&Header {
        protocol: PROTOCOL,
        conn: 5,
        seq: 1,
        ack: 2,
        ack_bits: 3,
    });
    pw.ack_delay(4);
    pw.host_status(1, 2, 3, Some(-4));
    pw.reliable(0, true, b"hello");
    let good = pw.finish();
    let (h, body) = parse_header(&good).unwrap();
    assert_eq!(h.conn, 5);
    assert_eq!(parse_chunks(body).unwrap().len(), 3);
    for _ in 0..20_000 {
        let mut b = good.clone();
        match r.upto(3) {
            0 => {
                let n = r.upto(b.len() as u64) as usize;
                b.truncate(n);
            }
            1 => {
                let i = r.upto(b.len() as u64 - 1) as usize;
                b[i] = r.next_u64() as u8;
            }
            2 => b = (0..r.upto(64)).map(|_| r.next_u64() as u8).collect(),
            _ => b.extend((0..r.upto(30)).map(|_| r.next_u64() as u8)),
        }
        if let Ok((_, body)) = parse_header(&b) {
            let _ = parse_chunks(body);
            let _ = decode_inputs(body, |_| Some(PadState::default()));
            if let Ok((head, rest)) = decode_bundles_head(body) {
                let _ = decode_bundles_rest(rest, head.first_tick, &[]);
            }
        }
        let _ = Msg::decode(&b);
        let _ = decompress_log(&b, 0, 10);
    }
    assert_eq!(parse_header(&[0u8; 1201]), Err(DropReason::TooLong));
    assert_eq!(
        parse_header(b"XXXX0123456789abcdef"),
        Err(DropReason::BadMagic)
    );
}

/// The bundle size bounds: the widest pad is exactly [`MAX_PAD_BYTES`], and
/// a live packet with the largest header, ack delay, host status and bundles
/// head still has [`BUNDLE_BUDGET`] bytes for its first bundle.
#[test]
fn codec_bundle_bounds_hold_at_their_extremes() {
    let widest = PadState {
        lx: i16::MIN,
        ly: i16::MIN,
        rx: i16::MIN,
        ry: i16::MIN,
        buttons: u32::MAX,
        mx: i32::MIN,
        my: i32::MIN,
        fire_lag: 1,
        pick: Some(ScreenPick::Walker {
            index: u32::MAX,
            gen: u32::MAX,
        }),
        view_lag: 1,
    };
    assert_eq!(
        pad_wire_len(&widest, u32::MAX, &PadState::default()),
        MAX_PAD_BYTES
    );
    let mut r = Lcg(8);
    for _ in 0..2_000 {
        let (a, b) = (random_pad(&mut r), random_pad(&mut r));
        assert!(pad_wire_len(&a, r.next_u64() as u32, &b) <= MAX_PAD_BYTES);
    }
    let mut pw = PacketWriter::new(&Header {
        protocol: PROTOCOL,
        conn: u32::MAX,
        seq: u16::MAX,
        ack: u16::MAX,
        ack_bits: u32::MAX,
    });
    pw.ack_delay(u16::MAX);
    pw.host_status(u32::MAX, u8::MAX, u32::MAX, Some(i64::MIN));
    let (head, _) = encode_bundles(u32::MAX, u32::MAX, Some(&[]), &[], 2_000);
    assert_eq!(pw.room() - CHUNK_OVERHEAD - head.len(), BUNDLE_BUDGET);
    // The most humans' widest pads alone fit.
    let max = usize::from(crate::host::MAX_PLAYERS);
    assert!(2 + max * MAX_PAD_BYTES <= BUNDLE_BUDGET);
    let full = TickBundle {
        tick: 0,
        pads: (0..max as u8).map(|s| (s, widest)).collect(),
        commands: Vec::new(),
        seqs: vec![u32::MAX; max],
    };
    let mut w = Writer::new();
    full.write(&mut w, &[]);
    assert_eq!(w.len(), 2 + max * MAX_PAD_BYTES);
}
