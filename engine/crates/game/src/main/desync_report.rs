//! `game --desync-report FILE [FILE]` (issue #168, LAN §4.7): reads one or
//! two desync reports (`game::netplay::desync`), prints the mismatch, the
//! first tick and state group where the two sides' rows differ and, given
//! the report's round setup and bundles, re-simulates the round with this
//! build to say which side it agrees with.
//!
//!   --desync-report FILE [FILE] [--assets DIR] [--missions DIR] [--no-resim]
//!
//! With two files (the host's and the guest's of one desync) each side's
//! rows come from whichever file has them, and the static digests are
//! compared group by group. The re-simulation needs the map baked under
//! `--assets` (default `assets_baked`).

use anyhow::{bail, Context, Result};
use game::netplay::desync::{self, Agreement, Row};
use game::netplay::DesyncReport;
use std::path::PathBuf;

pub(super) fn run(args: &[String]) -> Result<()> {
    let mut files = Vec::new();
    let mut assets = PathBuf::from("assets_baked");
    let mut missions = PathBuf::from("missions");
    let mut resim = true;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--assets" => assets = it.next().context("--assets DIR")?.into(),
            "--missions" => missions = it.next().context("--missions DIR")?.into(),
            "--no-resim" => resim = false,
            f if !f.starts_with("--") => files.push(PathBuf::from(f)),
            other => bail!("unknown --desync-report flag: {other}"),
        }
    }
    anyhow::ensure!(
        (1..=2).contains(&files.len()),
        "--desync-report FILE [FILE] [--assets DIR] [--missions DIR] [--no-resim]"
    );
    let reports: Vec<DesyncReport> = files
        .iter()
        .map(|f| DesyncReport::read(f).with_context(|| f.display().to_string()))
        .collect::<Result<_>>()?;
    let r = &reports[0];
    if let Some(o) = reports.get(1) {
        if (o.session.as_str(), o.slot, o.tick) != (r.session.as_str(), r.slot, r.tick) {
            println!(
                "WARNING: the files are of different desyncs ({} s{} t{} / {} s{} t{})",
                r.session, r.slot, r.tick, o.session, o.slot, o.tick
            );
        }
    }
    let pick = |f: fn(&DesyncReport) -> &desync::Side| {
        reports
            .iter()
            .map(f)
            .find(|s| !s.rows.is_empty())
            .or_else(|| reports.iter().map(f).find(|s| !s.build.is_empty()))
            .cloned()
            .unwrap_or_default()
    };
    let (host, guest) = (pick(|r| &r.host), pick(|r| &r.guest));
    println!(
        "desync: session {} ({}), guest slot {}, hash check at tick {}",
        r.session, r.setup_summary, r.slot, r.tick
    );
    for (name, s) in [("host", &host), ("guest", &guest)] {
        println!(
            "  {name:5} hash {:16}  build {}  {}  ({} rows)",
            if s.hash.is_empty() { "?" } else { &s.hash },
            if s.build.is_empty() { "?" } else { &s.build },
            s.target,
            s.rows.len()
        );
    }
    if host.build != guest.build && !host.build.is_empty() && !guest.build.is_empty() {
        println!("  WARNING: the builds differ");
    }
    if let Some(o) = reports.get(1) {
        let differ: Vec<&str> = r
            .static_groups
            .iter()
            .filter(|(n, d)| o.static_groups.iter().any(|(m, e)| m == n && e != d))
            .map(|(n, _)| n.as_str())
            .collect();
        if r.static_digest == o.static_digest {
            println!("static digest: equal ({})", r.static_digest);
        } else {
            println!(
                "static digest: {} / {} differ in {differ:?}",
                r.static_digest, o.static_digest
            );
        }
    }
    let hr = desync::parse_rows(&host.rows)?;
    let gr = desync::parse_rows(&guest.rows)?;
    let span = |rows: &[(u32, Row)]| {
        rows.first()
            .zip(rows.last())
            .map_or_else(|| "none".to_owned(), |(a, b)| format!("{}..={}", a.0, b.0))
    };
    if hr.is_empty() || gr.is_empty() {
        println!(
            "rows: host {}, guest {} - both sides' rows are needed to compare (give both files)",
            span(&hr),
            span(&gr)
        );
    } else {
        match desync::first_difference(&hr, &gr) {
            Some((tick, cols)) => {
                let groups: Vec<&str> = cols.iter().copied().filter(|c| *c != "hash").collect();
                println!(
                    "first difference: tick {tick}, {}",
                    if groups.is_empty() {
                        "the whole-state hash only (state outside the five groups: vehicles, bolts, AI, ...)".to_owned()
                    } else {
                        format!("group {}", groups.join(", "))
                    }
                );
            }
            None => println!(
                "the rows agree over ticks {} / {}: the difference is earlier than the rows",
                span(&hr),
                span(&gr)
            ),
        }
    }
    if !resim {
        return Ok(());
    }
    if r.setup.is_empty() {
        println!("re-simulation: the report has no round setup");
        return Ok(());
    }
    let setup = r.round_setup()?;
    let dir = assets.join(&setup.map);
    let started = std::time::Instant::now();
    let (scene, load) = match game::assets::load_battle_scene(&dir, &setup.mode) {
        Ok(x) => x,
        Err(e) => {
            println!("re-simulation: cannot load {}: {e:#}", dir.display());
            return Ok(());
        }
    };
    if let Some((path, Err(error))) = load.common {
        bail!("common assets at {} unreadable: {error:#}", path.display());
    }
    let mut walk =
        super::round_setup::build_round(&scene, &setup, &missions, &assets, r.local_slot)?;
    let mut feed =
        game::event_feed::EventFeed::for_walk(&assets, &scene.manifest.source_game, &walk);
    let digest = format!("{:016x}", walk.sim.static_digest());
    if digest != r.static_digest {
        println!(
            "WARNING: this copy builds static digest {digest}, the report's is {}",
            r.static_digest
        );
    }
    let (stepped, with_host, with_guest) =
        desync::resimulate(r, &hr, &gr, &mut walk, Some(&mut feed))?;
    let say = |a: &Agreement| match &a.differs {
        _ if a.compared == 0 => "no rows to compare".to_owned(),
        None => format!("agrees ({} rows)", a.compared),
        Some((t, c)) => format!("differs from tick {t} ({})", c.join(", ")),
    };
    println!(
        "re-simulation ({stepped} ticks, {:.1} s): host {}; guest {}",
        started.elapsed().as_secs_f64(),
        say(&with_host),
        say(&with_guest)
    );
    let verdict = match (
        with_host.compared > 0 && with_host.differs.is_none(),
        with_guest.compared > 0 && with_guest.differs.is_none(),
    ) {
        (true, false) => "this build agrees with the HOST",
        (false, true) => "this build agrees with the GUEST",
        (true, true) => "this build agrees with both (the rows do not show the difference)",
        (false, false) => "this build agrees with neither side",
    };
    println!("verdict: {verdict}");
    Ok(())
}
