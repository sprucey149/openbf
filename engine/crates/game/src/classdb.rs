//! Runtime entity-class database: baked classes.ron files (from `bake
//! classes`) with BF2-style BASE-chain inheritance.
//!
//! BF2 munges class hierarchies as entc→entc references (RE_LOG §BF2 soldier
//! defaults: all_inf_default_rifleman → all_inf_default → com_inf_default →
//! engine base "soldier"); BF1 classes point straight at engine bases. A
//! property lookup walks the chain child-first, so a subclass overrides its
//! parent. Multiple files load into one db (SIDE + INGAME commons).

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

/// FNV-1a-32 over `byte | 0x20` — the PROP key / chunk-tag hash used
/// throughout the munged data (the engine's FUN_00444b30; one implementation,
/// `core::hero_weapon::sound_hash`).
///
/// Note `| 0x20` is NOT `to_ascii_lowercase`: it also flips punctuation and
/// digits, so the two disagree on any key with an underscore
/// ("ValueBleed_Alliance" → 0xFDE7C9D2 here vs 0x60FFCE72 lowercased).
pub use core::hero_weapon::sound_hash as fnv1a_lowercase;

// One record of a classes.ron — `manifest`'s struct, the same one
// `bake::classes` writes (queue item CT), so the reader cannot drift.
pub use manifest::ClassDef;
use manifest::DAMAGE_ORDER_DIR;

/// A map's own classes file, beside its `manifest.ron` (`bake::map::bake_map`
/// writes it).
pub const MAP_CLASSES_FILE: &str = "classes.ron";

/// The slot arrays of a soldier class are 8 long on both discs
/// (`analysis/re-grenades.md` §2.1).
pub const MAX_WEAPON_SLOTS: usize = 8;

/// The slots of a soldier class's drop table, both discs
/// (`analysis/re-powerups.md` §3.1).
pub const MAX_DROP_SLOTS: usize = 8;

/// One weapon slot of a soldier class ([`ClassDb::weapon_slots`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponSlot {
    /// The PS2's slot index (`WeaponName1` / `WEAPONSECTION 1` = 0).
    pub slot: u8,
    /// The weapon class, as the disc spells it.
    pub weapon: String,
    /// `WeaponAmmo`, in clips: `Some(0)` = infinite (a written value < 1),
    /// `None` = the chain authors none for this slot.
    pub ammo: Option<u32>,
    /// `WeaponChannel`: 0 = the primary trigger's, 1 = the secondary's.
    pub channel: u8,
}

/// Class store keyed by **name only**, with the source `kind` kept on each
/// def. Assumption (checked against BF1 ALL/IMP/COMMON and BF2 REP/CIS/INGAME:
/// zero collisions): class names are unique across entc/wpnc/ordc/expc, so a
/// weapon and an entity never contend for the same key. `prop` therefore
/// ignores kind and BASE chains resolve within a kind naturally
/// (wpnc rep_weap_inf_rifle -> wpnc com_weap_inf_rifle). `bake classes` still
/// dedups on (kind, name), so a future collision arrives here as a first-wins
/// drop, not a silent merge — use [`ClassDb::get_kind`] to assert the kind.
#[derive(Default)]
pub struct ClassDb {
    pub(crate) donor_weapons: Option<Box<ClassDb>>,
    classes: HashMap<String, ClassDef>,
    /// A `bf2_*` class file was loaded: the db is a BF2 disc's.
    bf2: bool,
    /// Asset root of the loaded class files, also used for companion hero data.
    assets_root: Option<std::path::PathBuf>,
    /// Keep every class as its disc wrote it ([`ClassDb::disc`]).
    disc: bool,
    /// Damage-slot rows a later class wrote into this one's slots, keyed by
    /// lowercase class name: (the class's own `DamageStartPercent` ordinal,
    /// key hash, raw value) ([`ClassDb::replay_damage_loads`]).
    damage_patches: HashMap<String, Vec<(usize, u32, String)>>,
}

impl ClassDb {
    /// A db that keeps BF1's classes as the disc wrote them, skipping the
    /// shared health and damage numbers ([`crate::normalize`]). For reading
    /// the original data; the game plays from [`ClassDb::default`].
    pub fn disc() -> Self {
        Self {
            disc: true,
            ..Self::default()
        }
    }

    /// Load one classes.ron into the db. First definition of a name wins
    /// (load order: map-specific before shared commons, mirroring the game's
    /// level-stacking).
    pub fn load(&mut self, path: &Path) -> Result<usize> {
        self.load_file(path, false)
    }

    /// [`ClassDb::load`]; `bf1_map` marks a BF1 map's own class file.
    fn load_file(&mut self, path: &Path, bf1_map: bool) -> Result<usize> {
        let defs: Vec<ClassDef> =
            ron::from_str(&pak::read_to_string(path).with_context(|| path.display().to_string())?)
                .context("parse classes.ron")?;
        if self.assets_root.is_none() {
            self.assets_root = path.parent().and_then(Path::parent).map(Path::to_path_buf);
        }
        let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        self.bf2 |= file.starts_with("bf2_");
        // BF1's soldiers and their weapons take the shared numbers here.
        let normalize = !self.disc && (bf1_map || crate::normalize::covers(file));
        let mut added = 0;
        for mut d in defs {
            if !self.classes.contains_key(&d.name) {
                if normalize {
                    if let Some(mut variant) = crate::normalize::vehicles::variant(&d) {
                        crate::normalize::apply(&mut variant);
                        self.classes.entry(variant.name.clone()).or_insert(variant);
                    }
                    crate::normalize::apply(&mut d);
                }
                self.classes.insert(d.name.clone(), d);
                added += 1;
            }
        }
        Ok(added)
    }

    /// [ours] Use the baked BF1 SBD profile for SBDs and generated Magnaguard
    /// pieces on every map. Match the resolved geometry, including subclasses.
    pub(crate) fn load_droid_chunks(&mut self, path: &Path) -> Result<()> {
        if !pak::is_file(path) {
            return Ok(());
        }
        let profiles: Vec<ClassDef> =
            ron::from_str(&pak::read_to_string(path)?).context("parse droid chunk profiles")?;
        let mut patches = Vec::new();
        for name in self.names() {
            let Some(geometry) = self.prop(name, "GeometryName") else {
                continue;
            };
            if !self
                .chain(name)
                .iter()
                .any(|d| d.base.eq_ignore_ascii_case("soldier"))
            {
                continue;
            }
            if let Some(p) = profiles
                .iter()
                .find(|p| p.kind == "chunk_profile" && p.name.eq_ignore_ascii_case(geometry))
            {
                patches.push((name.to_owned(), p.props.clone()));
            }
        }
        for (name, props) in patches {
            let d = self.classes.get_mut(&name).expect("existing class");
            let keys: Vec<_> = props.iter().map(|p| p.0).collect();
            d.props.retain(|p| !keys.contains(&p.0));
            // Keep chunk sections last, after all ordinary class properties.
            d.props.extend(props);
        }
        for p in profiles.into_iter().filter(|p| p.kind != "chunk_profile") {
            self.classes.entry(p.name.clone()).or_insert(p);
        }
        Ok(())
    }

    /// Load a map's own classes (`<map_dir>/classes.ron`) — the map-local
    /// vehicles and props no side LVL carries (TAT1's `tat_hover_skiff`,
    /// BF1 BES1's cloud car). Call it BEFORE the side files, the order
    /// [`ClassDb::load`] names. `Ok(0)` when the map's bake predates the file.
    /// A BF1 map's turret guns and blasts take the shared numbers as BF1's
    /// side files do ([`crate::normalize`]); the manifest names the game.
    pub fn load_map(&mut self, map_dir: &Path) -> Result<usize> {
        let p = map_dir.join(MAP_CLASSES_FILE);
        if !pak::exists(&p) {
            return Ok(0);
        }
        let bf1_map = !self.disc
            && pak::read_to_string(map_dir.join("manifest.ron"))
                .ok()
                .and_then(|txt| ron::from_str::<manifest::ManifestHead>(&txt).ok())
                .is_some_and(|h| crate::normalize::covers_map(&h.source_game));
        self.load_file(&p, bf1_map)
    }

    /// Replay how the PS2 creates a BF2 mission's classes, to find damage
    /// rows that land in another class's slots (re-hover §13.12). The
    /// damageable setter `FUN_0026a340` points its global `DAT_005fd2c0` at
    /// the slot each `DamageStartPercent` allocates (B2:186058-186096) and
    /// writes every other damage row through it (B2:186129-186167,
    /// 186306-186313, 186362-186400); nothing resets it between classes, so a
    /// class whose damage rows come before any Start (BF2's guided-rocket
    /// ordnance) writes into the last slot of the class created before it.
    /// Order: `ingame.lvl` whole, then each run of `loads` on one side file
    /// as one `ReadDataFile` call, its requested sub-levels and top level in
    /// file order; a class already created is not created again. Checked on
    /// a KAS2 save state: the BARC's 20 % flame holds scale 1.0 there, and
    /// the STAP and snail tank loaded after it keep theirs.
    pub fn replay_damage_loads(
        &mut self,
        classes_dir: &Path,
        game: &str,
        loads: &[crate::mission::SideClassLoad],
    ) {
        self.damage_patches.clear();
        if game != "bf2" || loads.is_empty() {
            return;
        }
        let read = |file: &str| -> Vec<manifest::DamageLoad> {
            let p = classes_dir.join(DAMAGE_ORDER_DIR).join(file);
            pak::read_to_string(&p)
                .ok()
                .and_then(|t| ron::from_str(&t).ok())
                .unwrap_or_default()
        };
        let mut reads: Vec<(Vec<manifest::DamageLoad>, Option<Vec<u32>>)> =
            vec![(read("bf2_ingame.ron"), None)];
        for run in loads.chunk_by(|a, b| a.side.eq_ignore_ascii_case(&b.side)) {
            let file = format!("bf2_{}.ron", run[0].side.to_ascii_lowercase());
            let wanted = run.iter().map(|l| fnv1a_lowercase(&l.request)).collect();
            reads.push((read(&file), Some(wanted)));
        }
        let start = fnv1a_lowercase("DamageStartPercent");
        let mut created = std::collections::HashSet::new();
        let mut starts: HashMap<String, usize> = HashMap::new();
        let mut cur: Option<(String, usize)> = None;
        for (order, wanted) in &reads {
            for d in order {
                let loaded = match (d.sub, wanted) {
                    (Some(sub), Some(w)) => w.contains(&sub),
                    _ => true,
                };
                let name = d.class.to_ascii_lowercase();
                if !loaded || !created.insert(name.clone()) {
                    continue;
                }
                for (k, v) in &d.rows {
                    if *k == start {
                        let n = starts.entry(name.clone()).or_default();
                        cur = (*n < 10).then(|| (name.clone(), *n));
                        *n += 1;
                    } else if let Some((owner, i)) = cur.as_ref().filter(|c| c.0 != name) {
                        self.damage_patches
                            .entry(owner.clone())
                            .or_default()
                            .push((*i, *k, v.clone()));
                    }
                }
            }
        }
    }

    /// The damage rows later classes wrote into `class`'s own slots
    /// ([`ClassDb::replay_damage_loads`]).
    pub fn damage_patches(&self, class: &str) -> &[(usize, u32, String)] {
        self.damage_patches
            .get(&class.to_ascii_lowercase())
            .map_or(&[], Vec::as_slice)
    }

    /// `class` and its BASE chain, child first, down to the last def the db
    /// holds (an engine base such as "hover" has none). Cycle-guarded.
    pub fn chain(&self, class: &str) -> Vec<&ClassDef> {
        let mut out = Vec::new();
        let mut cur = class;
        while let Some(def) = self.classes.get(cur) {
            if out.len() == 16 {
                break;
            }
            out.push(def);
            cur = &def.base;
        }
        out
    }

    /// The db was loaded from a BF2 disc's class files (`bf2_*.ron`) — the
    /// game whose engine rules its classes take where the two differ (the
    /// muzzle flash's model, `core::flash`).
    /// BF2's class data a BF1 side loads beside its own, if any: its classes'
    /// award weapons (`roster::awards`) and a `roster::WEAPON_DONORS` class's
    /// whole kit come from it.
    pub fn donor_classdb(&self) -> Option<&ClassDb> {
        self.donor_weapons.as_deref()
    }

    pub fn is_bf2(&self) -> bool {
        self.bf2
    }

    pub fn assets_root(&self) -> Option<&Path> {
        self.assets_root.as_deref()
    }

    /// Set the companion model/animation root when class RONs are loaded from
    /// a separate directory. This explicit root survives later class loads.
    pub fn set_assets_root(&mut self, root: &Path) {
        self.assets_root = Some(root.to_path_buf());
    }

    /// Every class name the db holds, in no particular order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.classes.keys().map(String::as_str)
    }

    pub fn get(&self, class: &str) -> Option<&ClassDef> {
        self.classes.get(class)
    }

    /// The stored name of `class`, matched exactly or else ignoring ASCII
    /// case — the PS2 finds a class by its lower-cased fnv hash, so an
    /// instance may spell it differently (BF1 BES1 places
    /// `bes_bldg_aaa_turret`, its classes define `bes_bldg_AAA_turret`).
    pub fn resolve<'a>(&'a self, class: &'a str) -> Option<&'a str> {
        if self.classes.contains_key(class) {
            return Some(class);
        }
        self.classes
            .keys()
            .filter(|k| k.eq_ignore_ascii_case(class))
            .min()
            .map(String::as_str)
    }

    /// `get`, but only if the stored def came from the given chunk kind
    /// ("entc" | "wpnc" | "ordc" | "expc").
    pub fn get_kind(&self, kind: &str, class: &str) -> Option<&ClassDef> {
        self.classes.get(class).filter(|d| d.kind == kind)
    }

    /// First value for `key` on `class` or its BASE chain (child overrides
    /// parent). The chain ends at an engine base ("soldier", "hover", ...)
    /// that has no entc of its own. Cycle-guarded.
    pub fn prop<'a>(&'a self, class: &str, key: &str) -> Option<&'a str> {
        self.prop_hash(class, fnv1a_lowercase(key))
    }

    /// `prop` for a key whose hash is known but whose name is not (the BF2
    /// ordnance falloff keys, re-blaster-damage §7.3).
    pub fn prop_hash(&self, class: &str, h: u32) -> Option<&str> {
        let mut cur = class;
        for _ in 0..16 {
            let def = self.classes.get(cur)?;
            if let Some(p) = def.props.iter().find(|p| p.0 == h) {
                return Some(p.1.as_str());
            }
            cur = &def.base;
        }
        None
    }

    /// EVERY value for `key` on the child-most class of the chain that
    /// defines it, in FILE ORDER ([`ClassDb::prop`] can only see the first).
    /// Child-most wins whole. This is NOT how the PS2 builds a soldier's
    /// weapon list — a class is a copy of its parent with its own PROPs
    /// applied slot by slot — so weapons go through [`ClassDb::weapon_slots`].
    pub fn props_all(&self, class: &str, key: &str) -> Vec<&str> {
        self.props_all_hash(class, fnv1a_lowercase(key))
    }

    /// [`ClassDb::props_all`] for a key known only by its hash.
    pub fn props_all_hash(&self, class: &str, h: u32) -> Vec<&str> {
        let mut cur = class;
        for _ in 0..16 {
            let Some(def) = self.classes.get(cur) else {
                return Vec::new();
            };
            let vals: Vec<&str> = def
                .props
                .iter()
                .filter(|p| p.0 == h)
                .map(|p| p.1.as_str())
                .collect();
            if !vals.is_empty() {
                return vals;
            }
            cur = &def.base;
        }
        Vec::new()
    }

    /// A class's weapon slots the PS2's way (`analysis/re-grenades.md` §2.1,
    /// `analysis/unit-audit-bf2.md` §1.1; B1 `FUN_0020f2e0`, B2
    /// `FUN_00252bb0`): the class starts as a copy of its parent, then its
    /// own PROPs are applied in file order. `WEAPONSECTION n` moves the
    /// cursor to slot n-1 (it starts at 0 in every class) for the unnumbered
    /// `WeaponName` / `WeaponAmmo` / `WeaponChannel`; the numbered keys
    /// `…1..4` write slots 0..3 whatever the cursor. Slots with no weapon
    /// are left out; the rest come in slot order.
    pub fn weapon_slots(&self, class: &str) -> Vec<WeaponSlot> {
        let numbered = |stem: &str| -> [u32; 4] {
            std::array::from_fn(|i| fnv1a_lowercase(&format!("{stem}{}", i + 1)))
        };
        let (h_sec, h_name, h_ammo, h_chan) = (
            fnv1a_lowercase("WEAPONSECTION"),
            fnv1a_lowercase("WeaponName"),
            fnv1a_lowercase("WeaponAmmo"),
            fnv1a_lowercase("WeaponChannel"),
        );
        let (n_name, n_ammo, n_chan) = (
            numbered("WeaponName"),
            numbered("WeaponAmmo"),
            numbered("WeaponChannel"),
        );
        let num = |v: &str| v.trim().parse::<f32>().unwrap_or(0.0);
        let mut names: [Option<&str>; MAX_WEAPON_SLOTS] = [None; MAX_WEAPON_SLOTS];
        let mut ammo: [Option<u32>; MAX_WEAPON_SLOTS] = [None; MAX_WEAPON_SLOTS];
        let mut chan = [0u8; MAX_WEAPON_SLOTS];
        for def in self.chain(class).iter().rev() {
            let mut cur = 0usize;
            for (h, v) in &def.props {
                let (h, v) = (*h, v.as_str());
                // A value < 1 is "infinite" (B1 stores 0x7fffffff, B2 bit 29).
                let clips = |v: &str| Some(if num(v) < 1.0 { 0 } else { num(v) as u32 });
                if h == h_sec {
                    cur = (num(v) as usize)
                        .saturating_sub(1)
                        .min(MAX_WEAPON_SLOTS - 1);
                } else if h == h_name {
                    names[cur] = Some(v.trim());
                } else if h == h_ammo {
                    ammo[cur] = clips(v);
                } else if h == h_chan {
                    chan[cur] = num(v) as u8;
                } else if let Some(i) = n_name.iter().position(|k| *k == h) {
                    names[i] = Some(v.trim());
                } else if let Some(i) = n_ammo.iter().position(|k| *k == h) {
                    ammo[i] = clips(v);
                } else if let Some(i) = n_chan.iter().position(|k| *k == h) {
                    chan[i] = num(v) as u8;
                }
            }
        }
        (0..MAX_WEAPON_SLOTS)
            .filter_map(|i| {
                Some(WeaponSlot {
                    slot: i as u8,
                    weapon: names[i].filter(|n| !n.is_empty())?.to_owned(),
                    ammo: ammo[i],
                    channel: chan[i],
                })
            })
            .collect()
    }

    /// A soldier class's drop table the PS2's way (card PWUP,
    /// `analysis/re-powerups.md` §3.1; B1 `FUN_0020f2e0`, B2 `FUN_00252bb0`):
    /// the class starts as a copy of its parent's table with the write index
    /// at 0, then its own PROPs in file order — `DropItemClass` and
    /// `DropItemProbability` write the slot at the index, `NextDropItem`
    /// moves it on. So a child overrides its parent's entries by position and
    /// keeps the rest. Returns the slots up to the count, each (item class
    /// name — empty when none was written —, probability). Whether the class
    /// is a soldier is the caller's question: a droideka authors the keys
    /// and its class never reads them.
    pub fn drop_table(&self, class: &str) -> Vec<(String, f32)> {
        let (h_class, h_prob, h_next) = (
            fnv1a_lowercase("DropItemClass"),
            fnv1a_lowercase("DropItemProbability"),
            fnv1a_lowercase("NextDropItem"),
        );
        let mut slots: [(String, f32); MAX_DROP_SLOTS] = Default::default();
        let mut count = 0usize;
        for def in self.chain(class).iter().rev() {
            let mut cur = 0usize;
            for (h, v) in &def.props {
                if *h == h_next {
                    cur += 1;
                } else if cur < MAX_DROP_SLOTS && (*h == h_class || *h == h_prob) {
                    if *h == h_class {
                        slots[cur].0 = v.trim().to_ascii_lowercase();
                    } else {
                        slots[cur].1 = v.trim().parse().unwrap_or(0.0);
                    }
                    count = count.max(cur + 1);
                }
            }
        }
        slots.into_iter().take(count).collect()
    }

    pub fn prop_f32(&self, class: &str, key: &str) -> Option<f32> {
        self.prop(class, key)?.trim().parse().ok()
    }

    pub fn prop_hash_f32(&self, class: &str, h: u32) -> Option<f32> {
        self.prop_hash(class, h)?.trim().parse().ok()
    }

    /// The engine base at the end of the BASE chain ("soldier", "hover", ...).
    pub fn engine_base<'a>(&'a self, class: &'a str) -> Option<&'a str> {
        let mut cur = class;
        for _ in 0..16 {
            match self.classes.get(cur) {
                Some(def) => cur = &def.base,
                None => return (cur != class).then_some(cur),
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_matches_the_munge_scheme() {
        assert_eq!(fnv1a_lowercase("GeometryName"), 0x47C8_6B4A);
        assert_eq!(fnv1a_lowercase("CAMERASECTION"), 0xBACA_6BF1);
        // `| 0x20`, not to_ascii_lowercase: underscores hash differently
        // (to_ascii_lowercase would give 0x60FFCE72). Cross-checked against
        // the manifest key ValueBleed = 0x4EBF97DA, where both agree.
        assert_eq!(fnv1a_lowercase("ValueBleed_Alliance"), 0xFDE7_C9D2);
        assert_eq!(fnv1a_lowercase("ValueBleed"), 0x4EBF_97DA);
    }

    fn db() -> ClassDb {
        let mut db = ClassDb::default();
        for (name, base, key, val) in [
            ("rifleman", "inf_default", "MaxHealth", "350.0"),
            ("inf_default", "com_default", "MaxSpeed", "7.0"),
            ("com_default", "soldier", "MaxHealth", "400.0"),
        ] {
            db.classes.insert(
                name.to_string(),
                ClassDef {
                    name: name.to_string(),
                    kind: "entc".to_string(),
                    base: base.to_string(),
                    props: vec![(fnv1a_lowercase(key), val.to_string())],
                },
            );
        }
        db
    }

    #[test]
    fn base_chain_lookup_child_overrides_parent() {
        let db = db();
        // Inherited two levels up.
        assert_eq!(db.prop_f32("rifleman", "MaxSpeed"), Some(7.0));
        // Child override shadows the com_default value.
        assert_eq!(db.prop_f32("rifleman", "MaxHealth"), Some(350.0));
        // Chain end = engine base.
        assert_eq!(db.engine_base("rifleman"), Some("soldier"));
        // Unknown key/class.
        assert_eq!(db.prop("rifleman", "NoSuchKey"), None);
        assert_eq!(db.prop("nobody", "MaxSpeed"), None);
        assert_eq!(db.engine_base("nobody"), None);
    }

    /// `props_all` returns every value the CHILD-MOST definer authored, in
    /// file order, and stops there (the BF2 repeated-`WeaponName` shape).
    #[test]
    fn props_all_walks_child_first_and_keeps_file_order() {
        let mut db = db();
        let h = fnv1a_lowercase("WeaponName");
        db.classes.get_mut("com_default").unwrap().props.extend([
            (h, "base_rifle".to_string()),
            (h, "base_pistol".to_string()),
        ]);
        assert_eq!(
            db.props_all("rifleman", "WeaponName"),
            ["base_rifle", "base_pistol"],
            "inherited from two levels up, in order"
        );
        db.classes.get_mut("rifleman").unwrap().props.extend([
            (h, "child_rifle".to_string()),
            (h, "child_grenade".to_string()),
        ]);
        assert_eq!(
            db.props_all("rifleman", "WeaponName"),
            ["child_rifle", "child_grenade"],
            "the child's own list REPLACES the parent's, it does not append"
        );
        assert_eq!(db.props_all("rifleman", "NoSuchKey"), Vec::<&str>::new());
        assert_eq!(db.props_all("nobody", "WeaponName"), Vec::<&str>::new());
    }

    fn push(db: &mut ClassDb, class: &str, props: &[(&str, &str)]) {
        db.classes.get_mut(class).unwrap().props.extend(
            props
                .iter()
                .map(|(k, v)| (fnv1a_lowercase(k), (*v).to_string())),
        );
    }

    fn slot(slot: u8, weapon: &str, ammo: Option<u32>, channel: u8) -> WeaponSlot {
        WeaponSlot {
            slot,
            weapon: weapon.to_string(),
            ammo,
            channel,
        }
    }

    /// The section model: a child is its parent's copy, `WEAPONSECTION`
    /// moves the cursor, a numbered key writes its own slot.
    #[test]
    fn weapon_slots_copy_the_parent_then_apply_the_child() {
        let mut db = db();
        push(
            &mut db,
            "com_default",
            &[
                ("WEAPONSECTION", "1"),
                ("WeaponName", "rifle"),
                ("WeaponAmmo", "6"),
                ("WEAPONSECTION", "2"),
                ("WeaponName", "pistol"),
                ("WeaponAmmo", "0"),
                ("WEAPONSECTION", "3"),
                ("WeaponName", "grenade"),
                ("WeaponAmmo", "4"),
                ("WeaponChannel", "1"),
            ],
        );
        let base = [
            slot(0, "rifle", Some(6), 0),
            slot(1, "pistol", Some(0), 0),
            slot(2, "grenade", Some(4), 1),
        ];
        assert_eq!(db.weapon_slots("rifleman"), base, "inherited whole");
        // A numbered key replaces one slot and keeps the rest of the copy.
        push(
            &mut db,
            "inf_default",
            &[("WeaponName2", "shotgun"), ("WeaponAmmo2", "3")],
        );
        // An unnumbered key with no section writes slot 0; a new slot is
        // appended where its section says.
        push(
            &mut db,
            "rifleman",
            &[
                ("WeaponName", "carbine"),
                ("WEAPONSECTION", "5"),
                ("WeaponName", "mine"),
                ("WeaponChannel", "1"),
            ],
        );
        assert_eq!(
            db.weapon_slots("rifleman"),
            [
                slot(0, "carbine", Some(6), 0),
                slot(1, "shotgun", Some(3), 0),
                slot(2, "grenade", Some(4), 1),
                slot(4, "mine", None, 1),
            ]
        );
        assert_eq!(db.weapon_slots("nobody"), []);
    }

    /// The drop table: the parent's copy, the child's keys from index 0.
    #[test]
    fn drop_table_copies_the_parent_and_overrides_by_position() {
        let mut db = db();
        let entry = |c: &str, p: f32| (c.to_string(), p);
        push(
            &mut db,
            "com_default",
            &[
                ("DropItemClass", "Item_Ammo"),
                ("DropItemProbability", "0.25"),
                ("NextDropItem", "-"),
                ("DropItemClass", "item_health"),
                ("DropItemProbability", "0.15"),
                ("NextDropItem", "-"),
                ("DropItemClass", "item_dual"),
                ("DropItemProbability", "0.08"),
                ("NextDropItem", "-"),
            ],
        );
        let base = vec![
            entry("item_ammo", 0.25),
            entry("item_health", 0.15),
            entry("item_dual", 0.08),
        ];
        assert_eq!(db.drop_table("rifleman"), base, "inherited whole");
        // The child writes slot 0's probability and slot 1's class only.
        push(
            &mut db,
            "rifleman",
            &[
                ("DropItemProbability", "0.5"),
                ("NextDropItem", "-"),
                ("DropItemClass", "item_energy"),
            ],
        );
        assert_eq!(
            db.drop_table("rifleman"),
            [
                entry("item_ammo", 0.5),
                entry("item_energy", 0.15),
                entry("item_dual", 0.08),
            ]
        );
        assert_eq!(db.drop_table("inf_default"), base);
        assert!(db.drop_table("nobody").is_empty());
    }

    #[test]
    fn profiles_follow_inherited_soldier_geometry_and_keep_other_classes_unchanged() {
        let def = |name: &str, base: &str, kind: &str, props: &[(&str, &str)]| ClassDef {
            name: name.into(),
            base: base.into(),
            kind: kind.into(),
            props: props
                .iter()
                .map(|(k, v)| (fnv1a_lowercase(k), (*v).into()))
                .collect(),
        };
        let mut db = ClassDb::default();
        for d in [
            def(
                "parent",
                "soldier",
                "entc",
                &[
                    ("GeometryName", "cis_inf_sbdroid"),
                    ("ChunkGeometryName", "bdroid_chunk1"),
                ],
            ),
            def("child", "parent", "entc", &[]),
            def(
                "bdroid",
                "soldier",
                "entc",
                &[
                    ("GeometryName", "cis_inf_bdroid"),
                    ("ChunkGeometryName", "bdroid_chunk1"),
                ],
            ),
            def(
                "weapon",
                "blaster",
                "wpnc",
                &[("GeometryName", "cis_inf_sbdroid")],
            ),
        ] {
            db.classes.insert(d.name.clone(), d);
        }
        let control = db.get("bdroid").unwrap().props.clone();
        let dir =
            std::env::temp_dir().join(format!("openbf_chunk_profiles_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("profiles.ron");
        let profile = def(
            "cis_inf_sbdroid",
            "",
            "chunk_profile",
            &[
                ("ChunkFrequency", "0.6"),
                ("CHUNKSECTION", "CHUNK1"),
                ("ChunkGeometryName", "sbdroid_chunk1"),
            ],
        );
        std::fs::write(&path, ron::to_string(&vec![profile]).unwrap()).unwrap();
        for _ in 0..2 {
            db.load_droid_chunks(&path).unwrap();
        }
        for name in ["parent", "child"] {
            assert_eq!(db.prop(name, "ChunkGeometryName"), Some("sbdroid_chunk1"));
            assert_eq!(db.prop_f32(name, "ChunkFrequency"), Some(0.6));
            assert_eq!(
                db.get(name)
                    .unwrap()
                    .props
                    .iter()
                    .filter(|(k, _)| *k == fnv1a_lowercase("CHUNKSECTION"))
                    .count(),
                1
            );
        }
        assert_eq!(db.get("bdroid").unwrap().props, control);
        assert_eq!(db.prop("weapon", "ChunkGeometryName"), None);
        assert!(
            db.get("cis_inf_sbdroid").is_none(),
            "profiles are not fieldable classes"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
