//! Bake gameplay classes (entc/wpnc/ordc/expc) from any LVL — map or side —
//! to a RON file.
//!
//! Side LVLs (SIDE/ALL.LVL etc.) carry the soldier/vehicle gameplay constants
//! as entc PROPs (MaxSpeed, AimFactorPosture*, CAMERASECTION blocks...) plus
//! the weapon (`wpnc`), ordnance (`ordc`) and explosion (`expc`) classes that
//! drive the fire/damage chain (re-blaster-damage §7-8); this is the runtime's
//! source for P3 movement and P4 weapon tuning. All four chunk kinds share one
//! layout (BASE string, TYPE string, PROP* = [u32 fnv1a(key)][NUL value]), so
//! `EntityClass::parse` reads them all. PROP keys are FNV-1a-32 hashes; values
//! stay raw strings (RE_LOG §Soldier ODF props maps the known keys).

use anyhow::{anyhow, Context, Result};

/// Chunk tags baked into a classes.ron, in file order.
pub const CLASS_TAGS: [&[u8; 4]; 4] = [b"entc", b"wpnc", b"ordc", b"expc"];

/// One record of a classes.ron. The struct is `manifest`'s (queue item CT):
/// `game::classdb` reads the same definition this bake writes, so the two
/// cannot drift. `BakedClass` stays this crate's name for it.
pub use manifest::ClassDef as BakedClass;

/// Property lookups on ONE class record, no BASE chain (that is
/// `game::classdb::ClassDb`). A trait rather than inherent methods because
/// the record is `manifest`'s, which is data only, and the key hash is
/// `mungefs`'s.
pub trait ClassProps {
    /// First value for a property whose key hash is known but whose name is
    /// not (several BF2 ordnance falloff keys — re-blaster-damage §7.3).
    fn prop_hash(&self, key_hash: u32) -> Option<&str>;

    /// First value for a property, by key name (hashed with fnv1a_lowercase).
    fn prop(&self, key: &str) -> Option<&str> {
        self.prop_hash(mungefs::fnv1a_lowercase(key))
    }

    /// First value for a property, parsed as f32 (mission/ODF numbers are f32).
    fn prop_f32(&self, key: &str) -> Option<f32> {
        self.prop(key)?.trim().parse().ok()
    }

    /// f32 for a property known only by key hash.
    fn prop_hash_f32(&self, key_hash: u32) -> Option<f32> {
        self.prop_hash(key_hash)?.trim().parse().ok()
    }
}

impl ClassProps for BakedClass {
    fn prop_hash(&self, key_hash: u32) -> Option<&str> {
        self.props
            .iter()
            .find(|p| p.0 == key_hash)
            .map(|p| p.1.as_str())
    }
}

pub fn bake_classes(lvl_path: &str, out_ron: &str) -> Result<Vec<BakedClass>> {
    let buf = crate::gameplay::read(lvl_path).with_context(|| lvl_path.to_string())?;
    let classes = read_classes(&buf, &CLASS_TAGS, MalformedClass::Strict)?;
    let out = std::path::Path::new(out_ron);
    crate::write_ron(out, &classes)?;
    if let (Some(dir), Some(file)) = (out.parent(), out.file_name()) {
        let order = damage_load_order(&buf)?;
        crate::write_ron(&dir.join(DAMAGE_ORDER_DIR).join(file), &order)?;
    }
    Ok(classes)
}

pub use manifest::DAMAGE_ORDER_DIR;

/// The property keys BF2's damageable setter `FUN_0026a340` writes through
/// its damage-slot pointer `DAT_005fd2c0` (B2:186058-186096, 186129-186167,
/// 186306-186313, 186362-186400): `DamageStartPercent`, `DamageStopPercent`,
/// `DamageEffect`, `DamageEffectScale`, `DamageInheritVelocity`,
/// `DamageAttachPoint`, and two keys known only by hash (slot +0x10 and +0x28).
pub const DAMAGE_SLOT_KEYS: [u32; 8] = [
    0x0618_2385,
    0x37fe_5ca9,
    0xae2b_fdd1,
    0x57ce_6c85,
    0xd06d_0b7c,
    0xca69_8181,
    0xdd5c_94da,
    0x27cd_02e8,
];

/// Every class definition in `buf` that sets a damage-slot key, in file
/// order with repeats, each with the sub-level that holds it (`None` at the
/// top level, loaded with the file) and its damage-slot rows in order. The PS2 creates a file's classes in this
/// order when a mission reads it, so the setter's slot pointer, a global it
/// never resets between classes, can carry from one class into the next
/// (re-hover §13.12).
pub fn damage_load_order(buf: &[u8]) -> Result<Vec<manifest::DamageLoad>> {
    let inner = mungefs::open(buf).map_err(|e| anyhow!("{e}"))?;
    let mut out = Vec::new();
    let mut take = |c: &mungefs::Chunk, sub: Option<u32>| -> Result<()> {
        if !CLASS_TAGS.contains(&&c.tag) {
            return Ok(());
        }
        let e = mungefs::entity::EntityClass::parse(c).map_err(|e| anyhow!("{e}"))?;
        let rows: Vec<(u32, String)> = e
            .props
            .into_iter()
            .filter(|p| DAMAGE_SLOT_KEYS.contains(&p.key_hash))
            .map(|p| (p.key_hash, p.value))
            .collect();
        if !rows.is_empty() {
            out.push(manifest::DamageLoad {
                sub,
                class: e.name,
                rows,
            });
        }
        Ok(())
    };
    for c in mungefs::chunks(inner) {
        let c = c.map_err(|e| anyhow!("{e}"))?;
        if &c.tag != b"lvl_" {
            take(&c, None)?;
            continue;
        }
        for sub in c.children() {
            let sub = sub.map_err(|e| anyhow!("{e}"))?;
            if mungefs::CONTENT_TAGS.contains(&&sub.tag) {
                take(&sub, None)?;
                continue;
            }
            for inner in sub.children() {
                take(&inner.map_err(|e| anyhow!("{e}"))?, Some(sub.tag_u32()))?;
            }
        }
    }
    Ok(out)
}

/// Only selected class parse errors differ; container errors always propagate.
pub(crate) enum MalformedClass {
    Strict,
    Skip,
}

pub(crate) fn read_classes(
    buf: &[u8],
    tags: &[&[u8; 4]],
    policy: MalformedClass,
) -> Result<Vec<BakedClass>> {
    let inner = mungefs::open(buf).map_err(|e| anyhow!("{e}"))?;
    let mut classes: Vec<BakedClass> = Vec::new();
    for c in mungefs::content_chunks(inner).map_err(|e| anyhow!("{e}"))? {
        if !tags.contains(&&c.tag) {
            continue;
        }
        let kind = String::from_utf8_lossy(&c.tag).into_owned();
        let e = match mungefs::entity::EntityClass::parse(&c) {
            Ok(e) => e,
            Err(_) if matches!(policy, MalformedClass::Skip) => continue,
            Err(e) => return Err(anyhow!("{kind} 0x{:X}: {e}", c.offset)),
        };
        // Side LVLs repeat shared classes across per-unit sublevels — first
        // definition wins (they are byte-identical repeats of the same munge).
        // Dedup on (kind, name) so a weapon never shadows a same-named entity.
        if classes.iter().any(|b| b.kind == kind && b.name == e.name) {
            continue;
        }
        classes.push(BakedClass {
            name: e.name,
            kind,
            base: e.base,
            props: e.props.into_iter().map(|p| (p.key_hash, p.value)).collect(),
        });
    }
    Ok(classes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = tag.to_vec();
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(payload);
        out.resize(out.len().next_multiple_of(4), 0);
        out
    }

    fn class(tag: &[u8; 4], name: &str, base: &str) -> Vec<u8> {
        let mut payload = chunk(b"TYPE", format!("{name}\0").as_bytes());
        payload.extend(chunk(b"BASE", format!("{base}\0").as_bytes()));
        chunk(tag, &payload)
    }

    #[test]
    fn class_reader_keeps_order_exact_names_kinds_and_selected_tags() {
        let payload: Vec<u8> = [
            class(b"entc", "Same", "first"),
            class(b"entc", "Same", "duplicate"),
            class(b"wpnc", "Same", "weapon"),
            class(b"entc", "same", "case"),
            class(b"ordc", "ord", "ordnance"),
            class(b"expc", "exp", "explosion"),
        ]
        .concat();
        let lvl = chunk(b"ucfb", &chunk(b"lvl_", &payload));
        let strict = read_classes(&lvl, &CLASS_TAGS, MalformedClass::Strict).unwrap();
        let rows: Vec<_> = strict
            .iter()
            .map(|c| (c.kind.as_str(), c.name.as_str(), c.base.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                ("entc", "Same", "first"),
                ("wpnc", "Same", "weapon"),
                ("entc", "same", "case"),
                ("ordc", "ord", "ordnance"),
                ("expc", "exp", "explosion"),
            ]
        );
        let side = crate::anim_sets::bf2::side_classes(&lvl).unwrap();
        assert_eq!(side, strict[..3]);
    }

    #[test]
    fn class_reader_only_tolerates_selected_class_parse_errors() {
        let malformed = chunk(b"entc", &chunk(b"TYPE", b"Same")); // no NUL
        let good = class(b"entc", "Same", "valid");
        let lvl = chunk(b"ucfb", &[malformed, good.clone()].concat());
        let err = read_classes(&lvl, &CLASS_TAGS, MalformedClass::Strict).unwrap_err();
        assert!(err.to_string().starts_with("entc 0x0:"), "{err}");
        let side = crate::anim_sets::bf2::side_classes(&lvl).unwrap();
        assert_eq!(side.len(), 1);
        assert_eq!(side[0].name, "Same");
        assert_eq!(side[0].base, "valid");

        // An omitted malformed class must never be parsed, even in strict mode.
        let omitted = chunk(b"ucfb", &[chunk(b"ordc", &[]), good].concat());
        assert!(read_classes(&omitted, &[b"entc", b"wpnc"], MalformedClass::Strict).is_ok());
        assert!(read_classes(&omitted, &CLASS_TAGS, MalformedClass::Strict).is_err());
        assert_eq!(
            crate::anim_sets::bf2::side_classes(&omitted).unwrap().len(),
            1
        );

        for lvl in [
            b"bad magic".to_vec(),
            b"ucfb\x10\0\0\0".to_vec(),
            chunk(b"ucfb", b"entc\x20\0\0\0"),
        ] {
            assert!(read_classes(&lvl, &CLASS_TAGS, MalformedClass::Strict).is_err());
            assert!(crate::anim_sets::bf2::side_classes(&lvl).is_err());
        }
    }

    const BF1: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../extracted/bf1/lvl/DATA"
    );
    const BF2: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../extracted/bf2/lvl/DATA"
    );

    /// Bake `lvl` to a temp RON, or `None` when the extracted data is absent
    /// (Tier-1 data gate — CI without the ISO still passes).
    fn bake_or_skip(lvl: &str, tag: &str) -> Option<Vec<BakedClass>> {
        if !std::path::Path::new(lvl).exists() {
            eprintln!("SKIP: {lvl} not present");
            return None;
        }
        static NEXT_SCRATCH: std::sync::atomic::AtomicUsize =
            std::sync::atomic::AtomicUsize::new(0);
        let serial = NEXT_SCRATCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let out = std::env::temp_dir().join(format!(
            "openbf-{tag}-classes-{}-{serial}.ron",
            std::process::id()
        ));
        let classes = bake_classes(lvl, out.to_str().unwrap()).expect("bake");
        drop_scratch(&out);
        Some(classes)
    }

    /// The tests read the returned classes, never the RON, so the scratch file
    /// goes as soon as it is written (queue CLN3: `/tmp` is a tmpfs and ci runs
    /// many times a night). `OPENBF_KEEP_SCRATCH=1` keeps it for triage.
    fn drop_scratch(p: &std::path::Path) {
        if std::env::var_os("OPENBF_KEEP_SCRATCH").is_none() {
            let _ = std::fs::remove_file(p);
        }
    }

    fn find<'a>(cs: &'a [BakedClass], kind: &str, name: &str) -> &'a BakedClass {
        cs.iter()
            .find(|c| c.kind == kind && c.name == name)
            .unwrap_or_else(|| panic!("{kind} {name} not found"))
    }

    /// Dump every prop of a class to the test log — the lead reads these
    /// numbers out of `cargo test -- --nocapture`.
    fn dump(cs: &[BakedClass], kind: &str, name: &str) {
        match cs.iter().find(|c| c.kind == kind && c.name == name) {
            None => eprintln!("--- {kind} {name}: ABSENT"),
            Some(c) => {
                eprintln!(
                    "--- {kind} {} BASE={} ({} props)",
                    c.name,
                    c.base,
                    c.props.len()
                );
                for (h, v) in &c.props {
                    eprintln!("      0x{h:08x} = {v}");
                }
            }
        }
    }

    /// Queue item CT: the writer and the reader are ONE struct now
    /// (`manifest::ClassDef`), so every classes.ron the bake shipped must
    /// survive a parse and a re-serialize UNCHANGED — byte for byte, through
    /// the same `PrettyConfig` `crate::write_ron` uses. A field renamed,
    /// reordered or dropped on either side fails here on real data.
    /// Data-gated on `engine/assets_baked/classes`.
    #[test]
    fn every_shipped_classes_ron_re_serializes_byte_identically() {
        let dir = std::path::Path::new(crate::BAKED_ROOT).join("classes");
        if !dir.exists() {
            eprintln!("SKIP: assets_baked/classes not present");
            return;
        }
        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .expect("classes dir")
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "ron"))
            .collect();
        files.sort();
        let mut classes = 0usize;
        for f in &files {
            let text = std::fs::read_to_string(f).expect("read");
            let parsed: Vec<BakedClass> =
                ron::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
            let again =
                ron::ser::to_string_pretty(&parsed, ron::ser::PrettyConfig::default()).unwrap();
            assert_eq!(again, text, "{} re-serializes differently", f.display());
            classes += parsed.len();
        }
        eprintln!(
            "{} classes.ron, {classes} classes, byte-identical",
            files.len()
        );
        // rebake.sh writes ten: four sides per disc + bf1_common + bf2_ingame.
        assert!(files.len() >= 10, "only {} classes.ron found", files.len());
    }

    /// Data-gated Tier-1: BF1 SIDE/ALL.LVL soldier constants (values verified
    /// by raw-byte dump, RE_LOG §Soldier ODF props).
    #[test]
    fn all_lvl_soldier_classes_bake() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/ALL.LVL"
        );
        if !std::path::Path::new(p).exists() {
            eprintln!("SKIP: extracted ALL.LVL not present");
            return;
        }
        let out =
            std::env::temp_dir().join(format!("openbf-all-classes-{}.ron", std::process::id()));
        let classes = bake_classes(p, out.to_str().unwrap()).expect("bake");
        // 40 entc across sublevels, deduped (shared vanguard/pilot repeats),
        // plus the wpnc/ordc/expc kinds.
        let entc = classes.iter().filter(|c| c.kind == "entc").count();
        assert!((15..=40).contains(&entc), "deduped entc count: {entc}");
        assert!(
            classes.len() > entc,
            "wpnc/ordc/expc missing: {} total",
            classes.len()
        );
        let s = classes
            .iter()
            .find(|c| c.name == "all_inf_soldier")
            .expect("all_inf_soldier");
        assert_eq!(s.base, "soldier");
        assert_eq!(s.prop_f32("MaxSpeed"), Some(6.5));
        assert_eq!(s.prop_f32("MaxStrafeSpeed"), Some(4.5));
        assert_eq!(s.prop_f32("MaxTurnSpeed"), Some(4.0));
        assert_eq!(s.prop_f32("MaxHealth"), Some(300.0));
        assert_eq!(s.prop_f32("AimFactorPostureProne"), Some(160.0));
        assert_eq!(s.prop("UnitType"), Some("Trooper"));
        // Round-trips through RON.
        let back: Vec<BakedClass> =
            ron::from_str(&std::fs::read_to_string(&out).unwrap()).expect("parse");
        drop_scratch(&out);
        assert_eq!(back.len(), classes.len());
    }

    /// Data-gated Tier-1: BF1 SIDE/ALL.LVL rifle weapon + ordnance classes.
    /// Values byte-verified in analysis/re-blaster-damage.md §8.1.
    #[test]
    fn bf1_all_lvl_rifle_weapon_and_ordnance() {
        let Some(cs) = bake_or_skip(&format!("{BF1}/_LVL_PS2/SIDE/ALL.LVL"), "bf1-all") else {
            return;
        };
        let w = find(&cs, "wpnc", "all_weap_inf_rifle");
        assert_eq!(w.base, "cannon");
        assert_eq!(w.prop_f32("ShotDelay"), Some(0.2));
        assert_eq!(w.prop("RoundsPerClip"), Some("50"));
        assert_eq!(w.prop_f32("ReloadTime"), Some(1.5));
        assert_eq!(w.prop_f32("YawSpread"), Some(0.25));
        assert_eq!(w.prop_f32("PitchSpread"), Some(0.25));
        assert_eq!(w.prop_f32("SpreadPerShot"), Some(1.1));
        assert_eq!(w.prop_f32("SpreadRecoverRate"), Some(4.8));
        assert_eq!(w.prop_f32("SpreadThreshold"), Some(1.6));
        assert_eq!(w.prop_f32("SpreadLimit"), Some(6.0));
        assert_eq!(w.prop_f32("SpreadLimit"), Some(6.0));
        assert_eq!(w.prop_f32("LockOnRange"), Some(64.0));
        // Automatic, single-shot salvo (§8.1 "plain auto rifle").
        assert_eq!(w.prop("TriggerSingle"), Some("0"));
        assert_eq!(w.prop("SalvoCount"), Some("1"));
        assert_eq!(w.prop_f32("SalvoDelay"), Some(0.0));
        // Posture spreads all zero: no stance/movement accuracy penalty.
        for k in [
            "StandStillSpread",
            "StandMoveSpread",
            "CrouchStillSpread",
            "CrouchMoveSpread",
            "ProneStillSpread",
            "ProneMoveSpread",
        ] {
            assert_eq!(w.prop_f32(k), Some(0.0), "{k}");
        }
        // The ordnance link key is spelled OrdnanceName (0xf8899c5e), not
        // "Ordnance" (0x399bf05d, the BF1 base-class alias) — §7.1.
        assert_eq!(w.prop("OrdnanceName"), Some("all_weap_inf_rifle_ord"));
        assert_eq!(w.prop("Ordnance"), None);

        let o = find(&cs, "ordc", "all_weap_inf_rifle_ord");
        assert_eq!(o.base, "bolt");
        assert_eq!(o.prop_f32("Velocity"), Some(80.0));
        assert_eq!(o.prop_f32("LifeSpan"), Some(1.5));
        assert_eq!(o.prop_f32("MaxDamage"), Some(60.0));
        assert_eq!(o.prop("Damage"), None); // §7.3 alias unused by this class
        assert_eq!(o.prop_f32("PersonScale"), Some(1.0));
        assert_eq!(o.prop_f32("DroidScale"), Some(1.0));
        assert_eq!(o.prop_f32("VehicleScale"), Some(0.0));
        assert_eq!(o.prop_f32("BuildingScale"), Some(0.1));
        assert_eq!(o.prop_f32("ShieldScale"), Some(1.0));
        // §8.1 lists "Animal 1.0", but the class carries no AnimalScale key
        // (nor the HealthScale combo) — 1.0 is the engine default, not data.
        assert_eq!(o.prop("AnimalScale"), None);
        assert_eq!(o.prop("HealthScale"), None);
        // Dead on PS2 (bolt class forces gravity 0) but present in the data.
        assert_eq!(o.prop_f32("Gravity"), Some(1.0));
        dump(&cs, "wpnc", "all_weap_inf_rifle");
        dump(&cs, "ordc", "all_weap_inf_rifle_ord");
    }

    /// Data-gated Tier-1: BF1 infantry weapon loadout lives on numbered
    /// WeaponName<N>/WeaponAmmo<N> keys, not the unnumbered vehicle keys.
    #[test]
    fn bf1_all_lvl_soldier_weapon_loadout() {
        let Some(cs) = bake_or_skip(&format!("{BF1}/_LVL_PS2/SIDE/ALL.LVL"), "bf1-all") else {
            return;
        };
        let s = find(&cs, "entc", "all_inf_soldier");
        // Unnumbered WeaponName/WeaponAmmo (0xfbf47dba / 0xdc19bd13) are the
        // *vehicle/turret* keys — absent on infantry classes.
        assert_eq!(s.prop_hash(0xFBF4_7DBA), None);
        assert_eq!(s.prop_hash(0xDC19_BD13), None);
        let slots = [
            ("all_weap_inf_rifle", "6"),
            ("all_weap_inf_pistol", "0"),
            ("all_weap_inf_thermaldetonator", "3"),
            ("all_weap_inf_crackgrenade", "3"),
        ];
        for (i, (name, ammo)) in slots.iter().enumerate() {
            let n = i + 1;
            assert_eq!(s.prop(&format!("WeaponName{n}")), Some(*name));
            assert_eq!(s.prop(&format!("WeaponAmmo{n}")), Some(*ammo));
            eprintln!("all_inf_soldier slot {n}: {name} ammo={ammo}");
        }
        assert_eq!(s.prop("WeaponName5"), None);
        // Grenades sit on the secondary channel; rifle/pistol on the primary.
        assert_eq!(s.prop("WeaponChannel1"), None);
        assert_eq!(s.prop("WeaponChannel3"), Some("1"));
        assert_eq!(s.prop("WeaponChannel4"), Some("1"));
    }

    /// Data-gated Tier-1: BF2 SIDE/REP.LVL rifle wpnc->wpnc BASE chain and
    /// the ordnance falloff block (re-blaster-damage §8.2).
    #[test]
    fn bf2_rep_lvl_rifle_chain_and_ordnance() {
        let Some(cs) = bake_or_skip(&format!("{BF2}/_LVL_PS2/SIDE/REP.LVL"), "bf2-rep") else {
            return;
        };
        // Both links of the chain live in SIDE/REP.LVL (com_weap_inf_rifle is
        // NOT in INGAME.LVL; each side LVL carries its own copy of the shared
        // com_ weapon classes).
        let rep = find(&cs, "wpnc", "rep_weap_inf_rifle");
        assert_eq!(rep.base, "com_weap_inf_rifle");
        let com = find(&cs, "wpnc", "com_weap_inf_rifle");
        assert_eq!(com.base, "cannon");
        // Stats sit on the com_ parent; rep_ is a skin (model/sound/ordnance).
        assert_eq!(rep.prop("ShotDelay"), None);
        assert_eq!(com.prop_f32("ShotDelay"), Some(0.2));
        assert_eq!(com.prop_f32("SpreadPerShot"), Some(1.2));
        assert_eq!(com.prop_f32("SpreadLimit"), Some(5.0));
        assert_eq!(com.prop_f32("SpreadRecoverRate"), Some(4.8));
        assert_eq!(com.prop_f32("SpreadThreshold"), Some(1.6));
        assert_eq!(com.prop_f32("LockOnRange"), Some(40.0));
        assert_eq!(com.prop("RoundsPerClip"), Some("50"));
        assert_eq!(com.prop_f32("ReloadTime"), Some(1.5));

        // §8.2 attributes the bolt to com_weap_inf_rifle_ord, but rep_ points
        // at its own ordc, an ordc->ordc reskin whose only props are the blue
        // laser colours — every damage number still comes from the com_ parent.
        assert_eq!(rep.prop("OrdnanceName"), Some("rep_weap_inf_rifle_ord"));
        let ro = find(&cs, "ordc", "rep_weap_inf_rifle_ord");
        assert_eq!(ro.base, "com_weap_inf_rifle_ord");
        assert_eq!(ro.prop("MaxDamage"), None);

        let o = find(&cs, "ordc", "com_weap_inf_rifle_ord");
        assert_eq!(o.base, "bolt");
        assert_eq!(o.prop_f32("Velocity"), Some(160.0));
        assert_eq!(o.prop_f32("LifeSpan"), Some(0.8));
        assert_eq!(o.prop("LifeSpan"), Some("0.80")); // stored with trailing 0
        assert_eq!(o.prop_f32("MaxDamage"), Some(80.0));
        assert_eq!(o.prop_f32("PersonScale"), Some(1.0));
        assert_eq!(o.prop_f32("VehicleScale"), Some(0.1));
        assert_eq!(o.prop_f32("BuildingScale"), Some(0.1));
        // Falloff block — names uncracked, asserted by hash (§7.3).
        assert_eq!(o.prop_hash_f32(0xA745_61B2), Some(0.4)); // falloff start s
        assert_eq!(o.prop_hash_f32(0xEE08_D1BA), Some(0.2)); // falloff duration
        assert_eq!(o.prop_hash_f32(0x955F_71E9), Some(50.0)); // end damage
        assert_eq!(o.prop_hash_f32(0x9748_1656), Some(0.25)); // team bonus scale
        dump(&cs, "wpnc", "com_weap_inf_rifle");
        dump(&cs, "ordc", "com_weap_inf_rifle_ord");
    }

    /// Data-gated Tier-1: the shared command-post / recharge-droid / turret
    /// classes. Values are dumped verbatim (hash + value) — most keys are
    /// still uncracked, so the test pins structure, not names.
    #[test]
    fn common_lvl_gameplay_objects() {
        // BF1 COMMON/ENG/COMMON.LVL and BF2 INGAME.LVL are both flat ucfb
        // (no lvl_ wrappers), unlike the side LVLs.
        if let Some(cs) = bake_or_skip(&format!("{BF1}/_LVL_PS2/COMMON/ENG/COMMON.LVL"), "bf1-com")
        {
            eprintln!("===== BF1 COMMON.LVL =====");
            for n in [
                "com_bldg_controlzone",
                "com_bldg_major_controlzone",
                "com_item_healthrecharge",
                "com_item_weaponrecharge",
                "com_weap_gunturret", // BF1 turret entc (no com_bldg_gunturret)
            ] {
                dump(&cs, "entc", n);
            }
            dump(&cs, "wpnc", "com_weap_bldg_gunturret");
            dump(&cs, "ordc", "com_weap_bldg_gunturret_ord");
            let cp = find(&cs, "entc", "com_bldg_controlzone");
            assert_eq!(cp.base, "commandpost");
            assert_eq!(cp.prop("Label"), Some("Control Zone"));
            assert_eq!(cp.prop("GeometryName"), Some("com_bldg_controlzone"));
            let major = find(&cs, "entc", "com_bldg_major_controlzone");
            assert_eq!(major.base, "commandpost");
            assert_eq!(major.prop("Label"), Some("Major Control Zone"));
            // MaxHealth (0x19971f1b): BF1 recharge droids 80 HP, turret 300.
            assert_eq!(
                find(&cs, "entc", "com_item_healthrecharge").prop_hash(0x1997_1F1B),
                Some("80")
            );
            assert_eq!(
                find(&cs, "entc", "com_weap_gunturret").prop_hash(0x1997_1F1B),
                Some("300.0")
            );
            assert_eq!(
                find(&cs, "entc", "com_weap_gunturret").prop_hash(0xFBF4_7DBA),
                Some("com_weap_bldg_gunturret")
            );
        }
        if let Some(cs) = bake_or_skip(&format!("{BF2}/_LVL_PS2/INGAME.LVL"), "bf2-ingame") {
            eprintln!("===== BF2 INGAME.LVL =====");
            for n in [
                "com_bldg_controlzone",
                "com_bldg_major_controlzone",
                "com_item_healthrecharge",
                "com_item_hero_healthrecharge",
                "com_item_weaponrecharge",
                "com_bldg_inf_defenseturret", // BF2 turret entc
            ] {
                dump(&cs, "entc", n);
            }
            dump(&cs, "wpnc", "com_weap_inf_defenseturret");
            dump(&cs, "ordc", "com_weap_inf_defenseturret_ord");
            let cp = find(&cs, "entc", "com_bldg_controlzone");
            assert_eq!(cp.base, "commandpost");
            assert_eq!(cp.prop("Label"), Some("Control Zone"));
            // BF2 adds CTF / invisible / untakeable CP variants.
            for n in [
                "com_bldg_controlzone_CTF",
                "com_bldg_invisible_controlzone",
                "com_bldg_untakeable_controlzone",
                "com_bldg_ctfbase",
            ] {
                assert!(
                    cs.iter().any(|c| c.kind == "entc" && c.name == n),
                    "missing {n}"
                );
            }
            // Recharge droids: BF2 raises both to 200/80 HP (BF1: 80/80).
            assert_eq!(
                find(&cs, "entc", "com_item_healthrecharge").prop_hash(0x1997_1F1B),
                Some("200")
            );
            assert_eq!(
                find(&cs, "entc", "com_item_weaponrecharge").prop_hash(0x1997_1F1B),
                Some("80")
            );
            let t = find(&cs, "entc", "com_bldg_inf_defenseturret");
            assert_eq!(t.base, "defensegridturret");
            assert_eq!(t.prop_hash(0xFBF4_7DBA), Some("com_weap_inf_defenseturret"));
        }
    }

    /// All four class kinds are baked, deduped per (kind, name), and no name
    /// is reused across kinds — the assumption `ClassDb` keys on
    /// (see game::classdb::ClassDb docs).
    #[test]
    fn all_class_kinds_bake_without_name_collisions() {
        let files = [
            (format!("{BF1}/_LVL_PS2/SIDE/ALL.LVL"), "bf1-all"),
            (format!("{BF1}/_LVL_PS2/SIDE/IMP.LVL"), "bf1-imp"),
            (format!("{BF1}/_LVL_PS2/COMMON/ENG/COMMON.LVL"), "bf1-com"),
            (format!("{BF2}/_LVL_PS2/SIDE/REP.LVL"), "bf2-rep"),
            (format!("{BF2}/_LVL_PS2/SIDE/CIS.LVL"), "bf2-cis"),
            (format!("{BF2}/_LVL_PS2/INGAME.LVL"), "bf2-ingame"),
        ];
        let mut seen: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for (path, tag) in &files {
            let Some(cs) = bake_or_skip(path, tag) else {
                continue;
            };
            let mut n = std::collections::HashMap::new();
            for c in &cs {
                *n.entry(c.kind.clone()).or_insert(0usize) += 1;
                // (kind, name) dedup held.
                assert_eq!(
                    cs.iter()
                        .filter(|d| d.kind == c.kind && d.name == c.name)
                        .count(),
                    1,
                    "duplicate {} {}",
                    c.kind,
                    c.name
                );
                if let Some(prev) = seen.insert(c.name.clone(), c.kind.clone()) {
                    assert_eq!(prev, c.kind, "name {} used by two kinds", c.name);
                }
            }
            eprintln!("{tag}: {} classes {n:?}", cs.len());
            for k in ["entc", "wpnc", "ordc", "expc"] {
                assert!(n.contains_key(k), "{tag}: no {k} chunks baked");
            }
        }
    }
}
