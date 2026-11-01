//! Tier-1 (data-gated, ignored: ~minutes): no step of `scripts/rebake.sh side`
//! rewrites a file another step wrote with different bytes, and no droid chunk
//! sheet shadows a soldier sheet by name. A chunk's `bake weapon` once turned
//! the living battle droid's sheet opaque (`cis_inf_battledroid.ktx2`).
//!
//! Each soldier/weapon model and the droid-chunks step bakes alone into its
//! own directory, then the outputs are compared path by path. Soldiers bake
//! without borrowed clips: textures do not depend on them.

use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

fn files(base: &Path, dir: &Path, out: &mut Vec<(String, u64)>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files(base, &path, out);
        } else {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            std::fs::read(&path).unwrap().hash(&mut h);
            let rel = path
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            out.push((rel, h.finish()));
        }
    }
}

#[test]
#[ignore = "bakes every side soldier and weapon from the extracted discs"]
fn side_steps_never_rewrite_or_shadow_another_steps_file() {
    let root = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.."));
    let lvl = root.join("extracted/bf1/lvl/DATA/_LVL_PS2");
    let lvl2 = root.join("extracted/bf2/lvl/DATA/_LVL_PS2");
    if !lvl.join("SIDE/CIS.LVL").is_file() || !lvl2.join("SIDE/CIS.LVL").is_file() {
        eprintln!("SKIP: extracted discs not present");
        return;
    }
    let pc = root.join("PC_swbf2_exe/Star Wars Battlefront II Classic/GameData/data/_lvl_pc/side");
    let pc_side = |s: &str| {
        let p = pc.join(s);
        p.is_file().then(|| p.to_string_lossy().into_owned())
    };
    let script = std::fs::read_to_string(root.join("scripts/rebake.sh")).unwrap();
    let side = &script
        [script.find("if want side; then").unwrap()..script.find("if want classes; then").unwrap()];
    let tmp = std::env::temp_dir().join(format!("openbf-side-steps-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);

    let mut writers: BTreeMap<String, Vec<(String, u64)>> = BTreeMap::new();
    let mut step = 0;
    // The first-person weapons are a shell loop: spell out what each pass runs.
    let w1st = side
        .lines()
        .find_map(|l| l.trim().strip_prefix("W1ST="))
        .expect("rebake.sh's W1ST list");
    let lines: Vec<String> = side
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .flat_map(|line| -> Vec<String> {
            if !line.starts_with("\"$BAKE\" weapon \"$G/SIDE/") {
                return vec![line.to_string()];
            }
            let mut passes = Vec::new();
            for game in ["1", "2"] {
                for s in ["all", "imp", "rep", "cis"] {
                    let extra = match (s, game) {
                        ("all", _) => ",bowcaster,seeker",
                        ("rep", "1") => ",landmine",
                        ("rep", "2") => ",chaingun",
                        _ => "",
                    };
                    let models = format!("{w1st}{extra}")
                        .split(',')
                        .map(|w| format!("{s}_1st_weap_inf_{w}"))
                        .collect::<Vec<_>>()
                        .join(",");
                    let big = s.to_uppercase();
                    passes.push(format!(
                        "\"$BAKE\" weapon \"$B{game}/SIDE/{big}.LVL\" {models} \"$OUT/side_bf{game}\" ${{DS{game}_{big}:+x}}"
                    ));
                }
            }
            passes
        })
        .collect();
    for line in &lines {
        let Some(at) = line.find("\"$BAKE\" ") else {
            continue;
        };
        let mut args = Vec::new();
        let mut pc_arg = None;
        for t in line[at + 8..].split_whitespace() {
            // ${DS2_REP:+...}: that side's donor. Only BF2 has a PC copy here; an
            // unknown name is a change this parser must learn, not a silent "none".
            if let Some(var) = t.strip_prefix("${") {
                let name = var.split(':').next().unwrap();
                pc_arg = match name.strip_prefix("DS2_") {
                    Some(s) => pc_side(&format!("{}.lvl", s.to_lowercase())),
                    None if name.starts_with("DS1_") => None,
                    None => panic!("unknown donor variable in rebake.sh: {name}"),
                };
                continue;
            }
            args.push(
                t.trim_matches('"')
                    .replace("$B1", &lvl.to_string_lossy())
                    .replace("$B2", &lvl2.to_string_lossy())
                    .replace("$A1", &lvl.join("COMMON/ENG/COMMON.LVL").to_string_lossy()),
            );
        }
        let (cmd, rest) = (args[0].as_str(), &args[1..]);
        let mut run = |label: String, f: &mut dyn FnMut(&Path)| {
            let out = tmp.join(format!("{step:03}"));
            step += 1;
            f(&out);
            let mut found = Vec::new();
            files(&out, &out, &mut found);
            for (rel, h) in found {
                writers.entry(rel).or_default().push((label.clone(), h));
            }
            std::fs::remove_dir_all(&out).unwrap();
        };
        let at_out = |a: &str, out: &Path| a.replace("$OUT", &out.to_string_lossy());
        match cmd {
            "soldier" | "weapon" => {
                for model in rest[1].split(',') {
                    run(format!("{cmd} {model}"), &mut |out| {
                        let dir = at_out(&rest[2], out);
                        if cmd == "soldier" {
                            bake::soldier::bake_soldier_anims(
                                &rest[0],
                                model,
                                None,
                                &dir,
                                pc_arg.as_deref(),
                            )
                            .map(|_| ())
                        } else {
                            bake::soldier::bake_weapon(&rest[0], model, &dir, pc_arg.as_deref())
                                .map(|_| ())
                        }
                        .unwrap_or_else(|e| panic!("{cmd} {model}: {e:#}"));
                    });
                }
            }
            "droid-chunks" => run("droid-chunks".into(), &mut |out| {
                bake::droid_chunks::bake(
                    &rest[0],
                    &rest[1],
                    &rest[2],
                    Path::new(&at_out(&rest[3], out)),
                    pc_arg.as_deref(),
                )
                .unwrap()
            }),
            _ => {} // vehicle/effects: each owns side_<game>/{vehicles,effects}/<side>/
        }
    }
    assert!(step > 100, "parsed only {step} steps from rebake.sh");

    let mut problems = Vec::new();
    let names: HashMap<(String, String), Vec<String>> = {
        // (game dir, file name) -> writers, for the side texture namespace
        let mut m: HashMap<_, Vec<String>> = HashMap::new();
        for (rel, w) in &writers {
            let parts: Vec<_> = rel.split('/').collect();
            if let [game, "textures", file] = parts[..] {
                m.entry((game.to_string(), file.to_string()))
                    .or_default()
                    .extend(w.iter().map(|(l, _)| l.clone()));
            }
        }
        m
    };
    for (rel, w) in &writers {
        if w.iter().any(|(_, h)| *h != w[0].1) {
            problems.push(format!("{rel} rewritten with different bytes by {w:?}"));
        }
        let parts: Vec<_> = rel.split('/').collect();
        if let [game, "droid_chunks", "textures", file] = parts[..] {
            if let Some(owners) = names.get(&(game.to_string(), file.to_string())) {
                problems.push(format!("{rel} shadows the sheet {owners:?} write"));
            }
            if !file.starts_with("openbf_droid_") {
                problems.push(format!("{rel}: a droid chunk sheet under a native name"));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    println!("{step} steps, {} output paths", writers.len());
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
