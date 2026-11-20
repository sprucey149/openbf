//! Dump one `modl`'s vertex rings, COL0 values and UVs straight off the disc.
//! Written for the ground CTF marker's height fade: the model is seven
//! vertex rings whose COL0 is an alpha-only ramp, which the common bake now
//! exports as `COLOR_0` (`analysis/re-ctf.md` §8).
//!
//! ```text
//! cargo run --release -p mungefs --example ctfdump -- <LVL> <model name>
//! ```

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: ctfdump <LVL> <model name>");
        std::process::exit(2);
    }
    let buf = std::fs::read(&args[1])?;
    let want = &args[2];
    for c in mungefs::content_chunks(mungefs::open(&buf)?)? {
        if &c.tag != b"modl" {
            continue;
        }
        let m = match mungefs::model::Model::parse(&c) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !m.name.eq_ignore_ascii_case(want) {
            continue;
        }
        println!("modl {} segments={}", m.name, m.segments.len());
        for (i, s) in m.segments.iter().enumerate() {
            println!(
                "  seg{i}: vtx={} idx={} flags=0x{:08X} rtyp={:?} tex={:?} group={:?}",
                s.positions.len(),
                s.index_count,
                s.material_flags,
                s.render_type,
                s.textures
                    .iter()
                    .map(|(_, n)| n.as_str())
                    .collect::<Vec<_>>(),
                s.group,
            );
            match &s.colors {
                Some(c) => println!(
                    "    colors: n={} a=[{}, {}] rgb min={:?} max={:?} unique={:?}",
                    c.len(),
                    c.iter().map(|v| v[3]).min().unwrap_or(0),
                    c.iter().map(|v| v[3]).max().unwrap_or(0),
                    (0..3)
                        .map(|k| c.iter().map(|v| v[k]).min().unwrap_or(0))
                        .collect::<Vec<_>>(),
                    (0..3)
                        .map(|k| c.iter().map(|v| v[k]).max().unwrap_or(0))
                        .collect::<Vec<_>>(),
                    {
                        let mut u = c.iter().map(|v| [v[0], v[1], v[2]]).collect::<Vec<_>>();
                        u.sort();
                        u.dedup();
                        u
                    },
                ),
                None => println!("    colors: none"),
            }
            // Vertex rings by height, with the COL0 that rides each ring.
            let mut rows: Vec<(i32, Vec<[u8; 4]>)> = Vec::new();
            for (vi, p) in s.positions.iter().enumerate() {
                let key = (p[1] * 100.0).round() as i32;
                let slot = match rows.iter().position(|(k, _)| *k == key) {
                    Some(at) => at,
                    None => {
                        rows.push((key, Vec::new()));
                        rows.len() - 1
                    }
                };
                rows[slot]
                    .1
                    .push(s.colors.as_ref().map_or([0, 0, 0, 0], |c| c[vi]));
            }
            rows.sort_by_key(|(k, _)| *k);
            for (k, cols) in rows {
                println!(
                    "    y={:.2} n={} rgba={:?}",
                    k as f32 / 100.0,
                    cols.len(),
                    cols.first(),
                );
            }
            let (mut vlo, mut vhi, mut ulo, mut uhi) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
            for uv in &s.uvs {
                vlo = vlo.min(uv[1]);
                vhi = vhi.max(uv[1]);
                ulo = ulo.min(uv[0]);
                uhi = uhi.max(uv[0]);
            }
            println!("    u=[{ulo:.4}, {uhi:.4}] v=[{vlo:.4}, {vhi:.4}]");
        }
    }
    Ok(())
}
