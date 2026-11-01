//! Offline bake tool: ISO/LVL -> glTF/KTX2/OGG/RON.
//!
//! `map`, `common`, `soldier`, `weapon` and `vehicle` take an optional trailing PC-format
//! LVL: eligible larger PC textures become optional tier-2 siblings beside
//! the PS2 base (see `bake::tier`). Omit it and the bake is PS2-only.
//!
//! Run `bake` with no arguments for the full usage table (tex-dump, pc-tex,
//! map, common, classes, loadscreen, sounds, sound-names, global-sound, beep,
//! specular, omniproj, soldier, weapon, vehicle, effects, locl, fonts). scripts/rebake.sh runs all of them
//! over the extracted discs; `extract-iso` makes those (scripts/pack.py).

use anyhow::{bail, Context, Result};
use std::io::BufWriter;
use std::path::Path;

/// Every file under the disc's `DATA/_LVL_PS2/` but its FMV (`MOVIES/`, 1-1.2
/// GB a disc) to `<out>/DATA/_LVL_PS2/...` (card PACK: the user's own ISO is
/// the only input) - save [`KEPT_MOVIES`], which the shell bake reads. A file
/// already there at the same size is left alone, so a second run writes
/// nothing.
/// The movies the port plays: BF2's shell background (card BSHLA, 260 MB;
/// `bake shell`).
const KEPT_MOVIES: [&str; 1] = ["DATA/_LVL_PS2/MOVIES/SHELL.MVS"];

fn extract_iso(iso: &str, out: &str) -> Result<()> {
    let f = std::fs::File::open(iso).with_context(|| iso.to_string())?;
    let mut disc = mungefs::iso::Iso::open(std::io::BufReader::new(f))
        .map_err(|e| anyhow::anyhow!("{iso}: not an ISO9660 image ({e})"))?;
    let want: Vec<usize> = (0..disc.files.len())
        .filter(|&i| {
            let p = &disc.files[i].path;
            p.starts_with("DATA/_LVL_PS2/")
                && (!p.starts_with("DATA/_LVL_PS2/MOVIES/")
                    || KEPT_MOVIES.iter().any(|m| p.eq_ignore_ascii_case(m)))
        })
        .collect();
    if want.is_empty() {
        bail!("{iso}: no DATA/_LVL_PS2 directory (not a PS2 Battlefront disc?)");
    }
    let (mut wrote, mut kept, mut bytes) = (0, 0, 0u64);
    for i in want {
        let rel = disc.files[i].path.clone();
        if rel
            .split('/')
            .any(|c| c.is_empty() || c == "." || c == "..")
        {
            bail!("{iso}: refusing the path {rel:?}");
        }
        let dst = Path::new(out).join(&rel);
        let size = disc.files[i].size as u64;
        if std::fs::metadata(&dst).is_ok_and(|m| m.len() == size) {
            kept += 1;
            continue;
        }
        let entry = mungefs::iso::IsoFile {
            path: rel.clone(),
            lba: disc.files[i].lba,
            size: disc.files[i].size,
        };
        let data = disc
            .read_file(&entry)
            .map_err(|e| anyhow::anyhow!("{rel}: {e}"))?;
        std::fs::create_dir_all(dst.parent().unwrap())?;
        std::fs::write(&dst, &data).with_context(|| dst.display().to_string())?;
        wrote += 1;
        bytes += size;
    }
    println!(
        "extract-iso {iso}: {wrote} files written ({} MB), {kept} already there -> {out}",
        bytes >> 20
    );
    Ok(())
}

fn ci(s: &str) -> String {
    bake::resolve_ci(s)
}

fn ci_csv(s: &str) -> String {
    s.split(',')
        .map(bake::resolve_ci)
        .collect::<Vec<_>>()
        .join(",")
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("extract-iso") if args.len() == 4 => extract_iso(&ci(&args[2]), &args[3]),
        Some("heroes") if args.len() == 4 => {
            let p2 = ci(&args[2]);
            let result = bake::heroes::bake_heroes(Path::new(&p2), Path::new(&args[3]))?;
            println!(
                "baked {} BF2 hero classes: {} bodies, {} weapon/HUD models, {} extra textures",
                result.heroes.len(),
                result.soldiers.len(),
                result.weapons.len(),
                result.textures.len()
            );
            Ok(())
        }
        Some("combos") if args.len() == 4 => {
            bake::combo::bake_combos(&ci(&args[2]), &args[3])?;
            Ok(())
        }
        Some("tex-dump") if args.len() == 4 => tex_dump(&ci(&args[2]), &args[3]),
        Some("pc-tex") if args.len() == 4 => pc_tex(&ci(&args[2]), &args[3]),
        Some("soldier") if (5..=7).contains(&args.len()) => {
            let lvl = ci(&args[2]);
            let anims = args.get(5).map(|s| ci(s));
            let pc = args.get(6).map(|s| ci(s));
            for model in args[3].split(',') {
                let b = bake::soldier::bake_soldier_anims(
                    &lvl,
                    model,
                    anims.as_deref(),
                    &args[4],
                    pc.as_deref(),
                )?;
                println!(
                    "baked soldier {}: {} bones, {} verts, {} tris, {} clips (missing {:?}), textures {:?}, cloths {:?}, extents {:?}..{:?} -> {}/soldiers",
                    b.name, b.bones, b.vertices, b.triangles, b.clips, b.missing_clips, b.textures, b.cloths, b.aabb.0, b.aabb.1, args[4]
                );
            }
            Ok(())
        }
        Some("droid-chunks") if args.len() == 6 || args.len() == 7 => {
            let pc = args.get(6).map(|s| ci(s));
            bake::droid_chunks::bake(
                &ci(&args[2]),
                &ci(&args[3]),
                &ci(&args[4]),
                Path::new(&args[5]),
                pc.as_deref(),
            )
        }
        Some("weapon") if args.len() == 5 || args.len() == 6 => {
            let lvl = ci_csv(&args[2]);
            let pc = args.get(5).map(|s| ci(s));
            for model in args[3].split(',') {
                let w = bake::soldier::bake_weapon(&lvl, model, &args[4], pc.as_deref())?;
                println!(
                    "baked weapon {}: {} verts, {} tris, {} skel nodes, textures {:?} -> {}/weapons",
                    w.name, w.vertices, w.triangles, w.nodes, w.textures, args[4]
                );
            }
            Ok(())
        }
        Some("vehicle") if args.len() == 4 || args.len() == 5 => {
            let pc = args.get(4).map(|s| ci(s));
            let v = bake::vehicle::bake_vehicles(&ci(&args[2]), &args[3], pc.as_deref())?;
            println!(
                "baked vehicles '{}': {} classes, {} geometries, {} models, {} textures, {} collision rows -> {}",
                v.source,
                v.vehicles.len(),
                v.geometries.len(),
                v.models.len(),
                v.textures.len(),
                v.collision.len(),
                args[3]
            );
            Ok(())
        }
        Some("beep") if args.len() == 4 => {
            let p2 = ci(&args[2]);
            let core = std::fs::read(&p2).with_context(|| args[2].clone())?;
            let b = bake::beep::bake_beep_with_path(Some(Path::new(&p2)), &core, &args[3])?;
            println!(
                "baked {} beep renders to {} (ogg: {})",
                b.outputs.len(),
                args[3],
                b.ogg_written
            );
            Ok(())
        }
        Some("common") if args.len() == 4 || args.len() == 5 => {
            let pc = args.get(4).map(|s| ci(s));
            let c = bake::map::bake_common(&ci(&args[2]), &args[3], pc.as_deref())?;
            println!(
                "baked common '{}': {} models, {} textures, {} classes -> {}",
                c.source,
                c.models.len(),
                c.textures.len(),
                c.classes.len(),
                args[3]
            );
            Ok(())
        }
        Some("effects") if args.len() == 5 && matches!(args[4].as_str(), "bf1" | "bf2") => {
            let set = bake::effects::bake_side(&ci(&args[2]), &args[3], &args[4])?;
            println!(
                "baked side effects '{}': {} effect(s) -> {}",
                args[2],
                set.map_or(0, |s| s.effects.len()),
                args[3]
            );
            Ok(())
        }
        Some("fonts") if args.len() == 5 => {
            let p2 = ci(&args[2]);
            let fonts = bake::hud::bake_fonts(
                std::path::Path::new(&p2),
                std::path::Path::new(&args[3]),
                &args[4],
            )?;
            println!("baked {} font(s) -> {}/fonts", fonts.len(), args[3]);
            Ok(())
        }
        Some("locl") if args.len() == 4 => {
            let l = bake::hud::bake_locl(&ci(&args[2]), &args[3])?;
            println!(
                "baked {} {} text entries -> {}/locl",
                l.entries.len(),
                l.language,
                args[3]
            );
            Ok(())
        }
        Some("specular") if args.len() == 4 => {
            bake::spec::bake_table(&ci(&args[2]), &args[3])?;
            println!("baked the specular table -> {}/specular.ktx2", args[3]);
            Ok(())
        }
        Some("omniproj") if args.len() == 4 => {
            let name = bake::spec::OMNIPROJ_NAME;
            bake::spec::bake_core_texture(&ci(&args[2]), name, &args[3])?;
            println!("baked the terrain light texture -> {}/{name}.ktx2", args[3]);
            Ok(())
        }
        Some("shell") if args.len() == 7 => {
            // <SHELL.LVL> <MOVIES/SHELL.MVS> <SOUND/SHELL.LVL> <CORE.LVL> <out>
            let s = bake::shell::bake_shell(
                &ci(&args[2]),
                &ci(&args[3]),
                &ci(&args[4]),
                &ci(&args[5]),
                &args[6],
            )?;
            println!(
                "baked shell: {} movies ({} segments), textures {:?}, music {} -> {}/shell",
                s.movies.len(),
                s.movies.iter().map(|m| m.segments.len()).sum::<usize>(),
                s.textures,
                s.music,
                args[6]
            );
            Ok(())
        }
        Some("sounds") if args.len() == 4 || args.len() == 5 => {
            // <sound lvl> <mapdir> [the game's CORE.LVL: the Inherit templates]
            let core = args.get(4).map(|s| ci(s));
            let r = bake::sounds::bake_sounds(&ci(&args[2]), &args[3], core.as_deref())?;
            println!(
                "baked sounds '{}': {} banks, {} samples, {} properties \
                 ({} odd frequency) -> {}/sounds",
                r.source,
                r.banks.len(),
                r.samples.len(),
                r.props.len(),
                r.odd_frequency,
                args[3]
            );
            Ok(())
        }
        Some("sound-names") if args.len() >= 4 => {
            // <classes dir> <lua root[,lua root]> <mapdir>...
            let luas: Vec<&Path> = args[3].split(',').map(Path::new).collect();
            let dict = bake::sound_names::build_dictionary(Path::new(&args[2]), &luas)?;
            for mapdir in &args[4..] {
                let (named, total) = bake::sound_names::write_names(mapdir, &dict)?;
                println!(
                    "sound-names {mapdir}: {named}/{total} hashes named \
                     ({} candidates)",
                    dict.len()
                );
            }
            Ok(())
        }
        Some("global-sound") if args.len() == 4 || args.len() == 5 => {
            let sources: Vec<String> = ci_csv(&args[2]).split(',').map(str::to_string).collect();
            let refs: Vec<String> = args
                .get(4)
                .map(|r| ci_csv(r).split(',').map(str::to_string).collect())
                .unwrap_or_default();
            let opts = bake::global_sound::Options::default();
            let g = bake::global_sound::bake_global_sound(&sources, &args[3], &refs, &opts)?;
            let music = g.streams.iter().filter(|s| !s.baked).count();
            let segs: usize = g.streams.iter().map(|s| s.segments.len()).sum();
            println!(
                "baked global-sound {:?}: {} streams ({music} music, ADPCM bodies copied), \
                 {segs} segments, {} OGG written ({} referenced, {} unreferenced) -> {}",
                g.sources,
                g.streams.len(),
                g.files.len(),
                g.referenced,
                g.unreferenced,
                args[3]
            );
            Ok(())
        }
        Some("loadscreen") if args.len() == 4 => {
            let ls = bake::loadscreen::bake_loadscreen(&ci(&args[2]), &args[3])?;
            println!(
                "baked load screen: {} steps + {} extras -> {}/load",
                ls.sequence.len(),
                ls.extras.len(),
                args[3]
            );
            Ok(())
        }
        Some("classes") if args.len() == 4 => {
            let c = bake::classes::bake_classes(&ci(&args[2]), &args[3])?;
            println!("baked {} classes -> {}", c.len(), args[3]);
            Ok(())
        }
        Some("donor-hints") if args.len() == 3 => {
            for r in bake::donor::donate_all(std::path::Path::new(&args[2]))? {
                println!(
                    "donor hints {} <- {}: hubs {}/{} matched; cover offered {} -> added {} \
                     (in a barrier {}, off the graph {}, on its own {}); its own cover {}",
                    r.target,
                    r.donor,
                    r.hubs_matched,
                    r.hubs,
                    r.offered,
                    r.added,
                    r.in_barrier,
                    r.off_graph,
                    r.same,
                    r.own
                );
            }
            Ok(())
        }
        Some("map") if args.len() == 4 || args.len() == 5 => {
            let pc = args.get(4).map(|s| ci(s));
            let m = bake::map::bake_map(&ci(&args[2]), &args[3], pc.as_deref())?;
            // Since item 11 (`two-tier-ktx2`) every base texture is the PS2
            // disc's own, and `texture_tiers` lists only the textures that
            // ALSO got a `<name>.tier2.ktx2` — so this counts classic-plus
            // overrides written beside the PS2 pixels, not textures that
            // replaced them (`bake::tier`, manifest `texture_tiers`).
            let tier2 = m.texture_tiers.iter().filter(|(_, t)| t == "pc").count();
            println!(
                "baked map '{}': {} models, {} textures ({tier2} with a PC tier 2), \
                 {} instances, {} regions -> {}",
                m.map,
                m.models.len(),
                m.textures.len(),
                m.instances.len(),
                m.regions.len(),
                args[3]
            );
            Ok(())
        }
        _ => {
            eprintln!(
                "usage:\n\
                 \x20 bake extract-iso <disc.iso>  <outdir>       the disc's DATA/_LVL_PS2 tree, as files\n\
                 \x20 bake map        <map.lvl>    <outdir> [pc.lvl]  models/textures/world manifest\n\
                 \x20 bake donor-hints <assets_baked>             BF2 cover hints onto the BF1 maps of bake::donor::DONORS\n\
                 \x20 bake common     <common.lvl> <outdir> [pc.lvl]  shared models/textures/classes\n\
                 \x20 bake classes    <file.lvl>   <out.ron>      entc/wpnc/ordc/expc props\n\
                 \x20 bake loadscreen <load.lvl>   <mapdir>       load screen sequence/pool\n\
                 \x20 bake sounds     <sound.lvl>  <mapdir> [CORE.LVL]  sample banks + snd_ props (templates from CORE)\n\
                 \x20 bake shell     <SHELL.LVL> <SHELL.MVS> <SOUND/SHELL.LVL> <CORE.LVL> <outdir>  BF2 shell logo, menu movie, music\n\
                 \x20 bake sound-names <classes dir> <lua roots> <mapdir>...  names.ron\n\
                 \x20 bake global-sound <lvl[,lvl]> <outdir> [ref.lvl,...]  stream VO to OGG\n\
                 \x20 bake beep       <core.lvl>   <outdir>       BF1 loading beep renders\n\
                 \x20 bake specular   <core.lvl>   <outdir>       the specular pass's lookup table\n\
                 \x20 bake omniproj   <core.lvl>   <outdir>       BF2's terrain light texture\n\
                 \x20 bake locl       <core.lvl>   <outdir>       the English text table\n\
                 \x20 bake fonts      <core.lvl>   <outdir> <prefix>  the bitmap fonts, named <prefix><font>\n\
                 \x20 bake tex-dump   <file.lvl>   <outdir>       every tex_ to PNG (mip 0)\n\
                 \x20 bake pc-tex     <file.lvl|-> <outdir>       PC-format textures to KTX2\n\
                 \x20 bake soldier    <side.lvl> <model[,model]> <outdir> [anim.lvl] [pc.lvl]\n\
                 \x20 bake heroes     <BF2 DATA/_LVL_PS2> <side_bf2 outdir>\n\
                 \x20 bake combos     <side.lvl> <out.ron>\n\
                 \x20 bake weapon     <side.lvl[,lvl]> <model[,model]> <outdir> [pc.lvl]\n\
                 \x20 bake droid-chunks <bf1-cis.lvl> <bf2-cis.lvl> <bf1-common.lvl> <outroot> [pc-cis.lvl]\n\
                 \x20 bake vehicle    <side.lvl> <outdir> [pc.lvl]     every vehicle class: models, chunks, collision\n\
                 \x20 bake effects    <side.lvl> <outdir> bf1|bf2   fx__ particle effects + their textures/models\n\
                 \nscripts/rebake.sh runs all of these over the extracted discs."
            );
            bail!("bad arguments");
        }
    }
}

/// Dump a PC (`_LVL_PC`) LVL's textures to KTX2 — an inspection dumper only.
/// The shipped override path is `bake::tier` (PC stock where larger AND the
/// same art), applied by `bake map/common/soldier/weapon`; see `bake::tier`'s
/// header. Reads stdin when `lvl_path` is `-`, so a mod archive member can be
/// streamed:
///   unzip -p pack.zip BS1/data/_LVL_PC/SIDE/bes.lvl | bake pc-tex - out/pc_bes
fn pc_tex(lvl_path: &str, outdir: &str) -> Result<()> {
    use std::io::Read;
    let buf = if lvl_path == "-" {
        let mut v = Vec::new();
        std::io::stdin().read_to_end(&mut v)?;
        v
    } else {
        std::fs::read(lvl_path).with_context(|| lvl_path.to_string())?
    };
    let inner = mungefs::open(&buf).map_err(|e| anyhow::anyhow!("{e}"))?;
    let dir = Path::new(outdir).join("textures");
    std::fs::create_dir_all(&dir)?;

    let mut index = String::from(
        "// bake pc-tex: PC LVL textures decoded to KTX2 — an inspection dumper;\n\
         // the shipped override path is bake::tier (PC stock where larger AND\n\
         // the same art), applied by bake map/common/soldier/weapon. `wrote` is\n\
         // the surface picked by Texture::best() (closest to the source art),\n\
         // which is not always the one the PC game renders\n\
         // (Texture::preferred()).\n\
         // Everything lands in the KTX2 sRGB colour format, so `_bump` normal\n\
         // maps here are for looking at, not for shading.\n[\n",
    );
    let (mut written, mut skipped) = (0usize, 0usize);
    for t in mungefs::texture_pc::find_textures(inner) {
        let Some(s) = t.best() else {
            skipped += 1;
            continue;
        };
        let Some(face) = s.faces.first() else {
            skipped += 1;
            continue;
        };
        let name = t.name.to_ascii_lowercase();
        // Only mips whose size matches the RGBA8 chain the writer expects.
        let mips: Vec<Vec<u8>> = face
            .mips
            .iter()
            .enumerate()
            .take_while(|(i, m)| {
                let (w, h) = s.mip_dims(*i);
                m.rgba.len() == (w * h * 4) as usize
            })
            .map(|(_, m)| m.rgba.clone())
            .collect();
        if mips.is_empty() {
            skipped += 1;
            continue;
        }
        std::fs::write(
            dir.join(format!("{name}.ktx2")),
            bake::ktx2w::write_rgba8(s.width, s.height, &mips),
        )?;
        let formats: Vec<&str> = t.surfaces.iter().map(|x| x.format.name()).collect();
        index.push_str(&format!(
            "    (name: \"{name}\", w: {}, h: {}, mips: {}, wrote: \"{}\", formats: {formats:?}, cube: {}),\n",
            s.width,
            s.height,
            mips.len(),
            s.format.name(),
            s.is_cube()
        ));
        written += 1;
    }
    index.push_str("]\n");
    std::fs::write(Path::new(outdir).join("pc_textures.ron"), index)?;
    println!("pc-tex: {written} textures -> {outdir}/textures ({skipped} skipped)");
    Ok(())
}

fn tex_dump(lvl_path: &str, outdir: &str) -> Result<()> {
    let buf = std::fs::read(lvl_path).with_context(|| lvl_path.to_string())?;
    let inner = mungefs::open(&buf).map_err(|e| anyhow::anyhow!("{e}"))?;
    std::fs::create_dir_all(outdir)?;
    let mut n = 0;
    // content_chunks descends lvl_ sublevels (SIDE/*.LVL keep textures there).
    for c in mungefs::content_chunks(inner).map_err(|e| anyhow::anyhow!("{e}"))? {
        if &c.tag != b"tex_" {
            continue;
        }
        let tex = match mungefs::texture::Texture::parse(&c) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("skip tex_ at 0x{:X}: {e}", c.offset);
                continue;
            }
        };
        let (w, h) = tex.mip_dims(0);
        println!(
            "{:32} {:>4}x{:<4} {:>2}bpp mips {} detail {}",
            tex.name, w, h, tex.info.bpp, tex.info.mip_count, tex.info.detail_compressed
        );
        let rgba = tex.rgba8(0);
        let out = Path::new(outdir).join(format!("{}.png", tex.name));
        let file = std::fs::File::create(&out)?;
        let mut enc = png::Encoder::new(BufWriter::new(file), w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()?.write_image_data(&rgba)?;
        n += 1;
    }
    println!("wrote {n} textures to {outdir}");
    Ok(())
}
