//! BF2's soldier animation sets, `FUN_00282f78` (see `super`): what a level's
//! class ODFs register and which clip each slot gets, a port of
//! `lead_tmp/anim/sets_bf2.py`.

use super::Inventory;
use crate::classes::ClassProps;
pub use manifest::anim_sets::bf2::{base_name, ACTIONS, BLOCKS, MOVES, PAIRS, SIMPLE, STANCES};
use manifest::anim_sets::bf2::{ACT0, MOVE0};
use manifest::ClassDef;
use std::collections::HashMap;

/// Words of a set: 151 pairs.
const WORDS: usize = 302;

/// A skeleton family (table 0x5fd3a8). A root's parent is `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Family {
    pub name: String,
    pub parent: Option<usize>,
}

/// A weapon animation class (table 0x5fd848).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponClass {
    pub name: String,
    pub parent: Option<usize>,
    /// A melee combo's row: its sets are not resolved here.
    pub combo: bool,
    /// The class has its own alert blocks.
    pub alert: bool,
}

/// What a level's classes registered: the rows and the (family, class)
/// sets.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Registry {
    pub families: Vec<Family>,
    pub classes: Vec<WeaponClass>,
    pub sets: Vec<(usize, usize)>,
}

impl Registry {
    /// The rows the ELF ships: the family `human` and the classes rifle
    /// (root, alert), bazooka (rifle, alert), tool (rifle), pistol (tool,
    /// alert), melee (tool) — §6.1.
    pub fn shipped() -> Self {
        let c = |name: &str, parent: Option<usize>, alert: bool| WeaponClass {
            name: name.into(),
            parent,
            combo: false,
            alert,
        };
        Self {
            families: vec![Family {
                name: "human".into(),
                parent: None,
            }],
            classes: vec![
                c("rifle", None, true),
                c("bazooka", Some(0), true),
                c("tool", Some(0), false),
                c("pistol", Some(2), true),
                c("melee", Some(2), false),
            ],
            sets: Vec::new(),
        }
    }

    pub fn family(&self, name: &str) -> Option<usize> {
        self.families
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(name))
    }

    /// The first non-combo class of that name.
    pub fn class(&self, name: &str) -> Option<usize> {
        self.classes
            .iter()
            .position(|c| !c.combo && c.name.eq_ignore_ascii_case(name))
    }

    /// Register a set once.
    pub fn add_set(&mut self, family: usize, class: usize) {
        if !self.sets.contains(&(family, class)) {
            self.sets.push((family, class));
        }
    }
}

/// What a weapon class's ODF says of its animation class, nearest class
/// of its `BASE` chain first (§6.2).
fn weapon_row(
    reg: &mut Registry,
    wpnc: &HashMap<String, &ClassDef>,
    weapon: &str,
) -> Option<usize> {
    let mut cur = weapon.to_ascii_lowercase();
    for _ in 0..16 {
        let Some(def) = wpnc.get(&cur) else { break };
        // `ComboAnimationBank` / `CustomAnimationBank` = "<name> <parent>
        // <combo | alert | noalert>": a row of its own.
        for (key, combo) in [("ComboAnimationBank", true), ("CustomAnimationBank", false)] {
            if let Some(v) = def.prop(key) {
                let mut w = v.split_whitespace();
                let name = w.next().unwrap_or_default();
                let name = name.split_once('_').map_or(name, |(_, n)| n);
                let parent = w.next().and_then(|p| reg.class(p));
                let third = w.next().unwrap_or_default();
                if !combo {
                    if let Some(i) = reg.class(name) {
                        return Some(i);
                    }
                }
                reg.classes.push(WeaponClass {
                    name: name.to_ascii_lowercase(),
                    parent,
                    combo,
                    alert: !combo && third.eq_ignore_ascii_case("alert"),
                });
                return Some(reg.classes.len() - 1);
            }
        }
        // `AnimationBank`: the text after the value's underscore, or the
        // whole value, against the class rows.
        if let Some(v) = def.prop("AnimationBank") {
            let v = v.trim();
            let name = v.split_once('_').map_or(v, |(_, n)| n);
            if let Some(i) = reg.class(name) {
                return Some(i);
            }
        }
        cur = def.base.to_ascii_lowercase();
    }
    // No key on the chain (grenades, detpacks, the officers' buffs): the
    // weapon registers no set. [D] from PS2 memory, not read in the code:
    // a state's sets for a family whose other weapons carry no key
    // (`tests/suite/anim_sets.rs`, `hansolo`).
    None
}

/// A soldier class's weapons, slot by slot, the PS2's way
/// (`game::classdb::ClassDb::weapon_slots`, `analysis/re-grenades.md`
/// §2.1): a copy of the parent's, then the class's own PROPs in file
/// order; `WEAPONSECTION n` moves the cursor for the unnumbered
/// `WeaponName`, `WeaponName1..4` write slots 0..3.
pub(crate) fn weapon_names<'a>(chain: &[&'a ClassDef]) -> Vec<&'a str> {
    let h = mungefs::fnv1a_lowercase;
    let (h_sec, h_name) = (h("WEAPONSECTION"), h("WeaponName"));
    let numbered: Vec<u32> = (1..=4).map(|i| h(&format!("WeaponName{i}"))).collect();
    let mut names: [Option<&str>; 8] = [None; 8];
    for def in chain.iter().rev() {
        let mut cur = 0usize;
        for (k, v) in &def.props {
            if *k == h_sec {
                cur = (v.trim().parse::<f32>().unwrap_or(0.0) as usize)
                    .saturating_sub(1)
                    .min(7);
            } else if *k == h_name {
                names[cur] = Some(v.trim());
            } else if let Some(i) = numbered.iter().position(|n| n == k) {
                names[i] = Some(v.trim());
            }
        }
    }
    names
        .into_iter()
        .flatten()
        .filter(|n| !n.is_empty())
        .collect()
}

/// A soldier class [`Registry::register`] read: its model and its family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoldierClass {
    pub name: String,
    /// `GeometryName`, lowercase.
    pub geometry: String,
    pub family: usize,
}

impl Registry {
    /// Register what `classes` (the `entc` and `wpnc` records of the
    /// level's side LVLs) register on the PS2 (§6.2): each soldier class
    /// its family — `AnimationName` / `SkeletonName` = "<name>
    /// [<parent>]", a child of `human` without the parent word, `human`
    /// itself without the key — and a set per weapon slot (the family,
    /// the weapon's animation class). `(human, melee)` is registered in
    /// every battle state read, whatever the classes: taken as built in
    /// (PROVISIONAL, [D] from PS2 memory).
    pub fn register(&mut self, classes: &[ClassDef]) -> Vec<SoldierClass> {
        let mut soldiers = Vec::new();
        let by_kind = |kind: &str| -> HashMap<String, &ClassDef> {
            let mut m = HashMap::new();
            for c in classes.iter().filter(|c| c.kind == kind) {
                m.entry(c.name.to_ascii_lowercase()).or_insert(c);
            }
            m
        };
        let (entc, wpnc) = (by_kind("entc"), by_kind("wpnc"));
        if let Some(m) = self.class("melee") {
            self.add_set(0, m);
        }
        for c in classes.iter().filter(|c| c.kind == "entc") {
            // The chain, the class first; its last BASE is the type.
            let mut chain = vec![c];
            while let Some(p) = entc.get(&chain[chain.len() - 1].base.to_ascii_lowercase()) {
                if chain.len() > 16 {
                    break;
                }
                chain.push(p);
            }
            if !chain[chain.len() - 1].base.eq_ignore_ascii_case("soldier") {
                continue;
            }
            let weapons = weapon_names(&chain);
            if weapons.is_empty() {
                continue;
            }
            let key = chain.iter().find_map(|d| {
                d.props
                    .iter()
                    .rev()
                    .find(|p| {
                        p.0 == mungefs::fnv1a_lowercase("AnimationName")
                            || p.0 == mungefs::fnv1a_lowercase("SkeletonName")
                    })
                    .map(|p| p.1.as_str())
            });
            let mut words = key.unwrap_or("human").split_whitespace();
            let name = words.next().unwrap_or("human").to_ascii_lowercase();
            let family = self.family(&name).unwrap_or_else(|| {
                let parent = match words.next() {
                    None => Some(0),
                    Some(p) => self.family(p),
                };
                self.families.push(Family { name, parent });
                self.families.len() - 1
            });
            for w in weapons {
                if let Some(row) = weapon_row(self, &wpnc, w) {
                    self.add_set(family, row);
                }
            }
            soldiers.push(SoldierClass {
                name: c.name.clone(),
                geometry: chain
                    .iter()
                    .find_map(|d| d.prop("GeometryName"))
                    .unwrap_or_default()
                    .trim()
                    .to_ascii_lowercase(),
                family,
            });
        }
        soldiers
    }
}

/// The `entc` and `wpnc` records of a side LVL, first definition of a
/// name first (`crate::classes::bake_classes`' rule).
pub fn side_classes(lvl: &[u8]) -> anyhow::Result<Vec<ClassDef>> {
    crate::classes::read_classes(
        lvl,
        &[b"entc", b"wpnc"],
        crate::classes::MalformedClass::Skip,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Slot {
    /// Not looked up yet.
    Unresolved,
    Empty,
    Clip(String),
}

impl Slot {
    fn clip(self) -> Option<String> {
        match self {
            Slot::Clip(n) => Some(n),
            _ => None,
        }
    }
}

/// One resolved set: word `2 * pair + layer` (0 upper, 1 lower) -> the
/// clip's name, for the first [`PAIRS`] pairs.
pub type SetSlots = Vec<Option<String>>;

/// The loader, `FUN_00282f78` (as `sets_bf2.py`'s `Model`).
struct Model<'a> {
    inv: &'a Inventory,
    reg: &'a Registry,
    rec: HashMap<(usize, usize), Vec<Slot>>,
    /// The set being built is a root set (a local flag of the PS2 function): the lookups it
    /// starts on its ancestors' records use ITS answer.
    ctx_root: bool,
}

impl Model<'_> {
    fn rec(&mut self, a: usize, b: usize) -> &mut Vec<Slot> {
        self.rec
            .entry((a, b))
            .or_insert_with(|| vec![Slot::Unresolved; WORDS])
    }

    fn get(&mut self, a: usize, b: usize, p: usize, layer: usize) -> Option<String> {
        self.rec(a, b)[p * 2 + layer].clone().clip()
    }

    fn put(&mut self, a: usize, b: usize, p: usize, layer: usize, v: Option<String>) {
        self.rec(a, b)[p * 2 + layer] = v.map_or(Slot::Empty, Slot::Clip);
    }

    fn parent_a(&self, a: usize) -> Option<usize> {
        self.reg.families[a].parent.filter(|&p| p != a)
    }

    fn parent_b(&self, b: usize) -> Option<usize> {
        self.reg.classes[b].parent.filter(|&p| p != b)
    }

    fn melee_rooted(&self, b: usize) -> bool {
        let mut b = Some(b);
        while let Some(i) = b {
            if self.reg.classes[i].name == "melee" {
                return true;
            }
            b = self.parent_b(i);
        }
        false
    }

    /// A root family, and no proper ancestor of `b` has a set with it.
    fn is_root_set(&self, a: usize, b: usize) -> bool {
        if self.parent_a(a).is_some() {
            return false;
        }
        let mut p = self.parent_b(b);
        while let Some(i) = p {
            if self.reg.sets.contains(&(a, i)) {
                return false;
            }
            p = self.parent_b(i);
        }
        true
    }

    /// 1 upper, 2 lower, 3 full.
    fn want(&self, p: usize) -> usize {
        if p >= ACT0 {
            1
        } else if p >= MOVE0 && matches!((p - MOVE0) % 13, 3 | 4) {
            2
        } else if self.ctx_root {
            3
        } else {
            1
        }
    }

    /// The first of the four candidate names the banks hold (B2
    /// 205364-205674).
    fn lookup(&self, a: usize, b: usize, p: usize, layer: usize) -> Option<String> {
        let (fam, wn) = (&self.reg.families[a].name, &self.reg.classes[b].name);
        let base = base_name(p);
        let want = self.want(p);
        let l = layer + 1;
        let mut cands = Vec::with_capacity(4);
        if want == l {
            cands.push(base.clone());
        }
        cands.push(format!(
            "{base}{}",
            if l == 1 { "_upper" } else { "_lower" }
        ));
        if want == 3 {
            cands.push(base.clone());
        }
        cands.push(format!("{base}_full"));
        cands
            .into_iter()
            .map(|c| format!("{fam}_{wn}_{c}"))
            .find(|n| self.inv.has(n))
    }

    fn try_slot(&mut self, a: usize, b: usize, slot: usize, f: bool) -> Option<String> {
        let v = self.rec(a, b)[slot].clone();
        if v == Slot::Unresolved || (v == Slot::Empty && f) {
            let found = self.lookup(a, b, slot / 2, slot % 2);
            self.rec(a, b)[slot] = found.clone().map_or(Slot::Empty, Slot::Clip);
            return found;
        }
        v.clip()
    }

    /// The parent class on the same family.
    fn wp(&mut self, a: usize, b: usize, slot: usize, f: bool, depth0: bool) -> Option<String> {
        let p = self.parent_b(b)?;
        if self.reg.classes[b].name == "melee" && f && !depth0 {
            return None;
        }
        let mut v = self.try_slot(a, p, slot, f);
        if v.is_none() {
            v = self.wp(a, p, slot, f, depth0);
        }
        if v.is_none() && f {
            v = self.sp(a, p, slot, f);
        }
        v
    }

    /// The nearest ancestor class with the alert flag, same family.
    fn wpa(&mut self, a: usize, b: usize, slot: usize, f: bool) -> Option<String> {
        let mut p = self.parent_b(b);
        while let Some(i) = p {
            if self.reg.classes[i].alert {
                break;
            }
            p = self.parent_b(i);
        }
        let p = p?;
        let mut v = self.try_slot(a, p, slot, f);
        if v.is_none() {
            v = self.wpa(a, p, slot, f);
        }
        if v.is_none() && f {
            v = self.sp(a, p, slot, f);
        }
        v
    }

    /// The parent family with the same class.
    fn sp(&mut self, a: usize, b: usize, slot: usize, f: bool) -> Option<String> {
        let pa = self.parent_a(a)?;
        let mut v = self.try_slot(pa, b, slot, f);
        if v.is_none() {
            v = self.sp(pa, b, slot, f);
        }
        if v.is_none() && f {
            v = self.wp(pa, b, slot, f, false);
        }
        v
    }

    fn pass1(&mut self, a: usize, b: usize) {
        self.ctx_root = self.is_root_set(a, b);
        let alert = self.reg.classes[b].alert;
        self.rec(a, b);
        // Weapon actions (a melee-rooted class without a combo gets none:
        // 205339-205340).
        if !self.melee_rooted(b) {
            for p in ACT0..PAIRS {
                for layer in 0..2 {
                    let s = p * 2 + layer;
                    let mut v = self.try_slot(a, b, s, false);
                    if v.is_none() {
                        v = self.sp(a, b, s, false);
                    }
                    self.put(a, b, p, layer, v);
                }
            }
        }
        // Movement.
        for blk in 0..6 {
            for m in 0..13 {
                let p = MOVE0 + blk * 13 + m;
                if blk >= 3 && !alert {
                    for layer in 0..2 {
                        let from = self.rec(a, b)[(p - 39) * 2 + layer].clone();
                        self.rec(a, b)[p * 2 + layer] = from;
                    }
                    continue;
                }
                let takeknee = m == 2 && (blk == 1 || blk == 4);
                let mut v = self.try_slot(a, b, p * 2, false);
                if v.is_none() && !(m == 3 || m == 4 || takeknee) {
                    v = self.sp(a, b, p * 2, false);
                }
                self.put(a, b, p, 0, v);
                let s = p * 2 + 1;
                let mut v = self.try_slot(a, b, s, false);
                if v.is_none() {
                    v = if blk < 3 {
                        self.wp(a, b, s, false, true)
                    } else {
                        self.wpa(a, b, s, false)
                    };
                }
                self.put(a, b, p, 1, v);
            }
        }
        // Simple ids.
        for p in 0..MOVE0 {
            let mut v = self.try_slot(a, b, p * 2, false);
            if v.is_none() {
                v = self.sp(a, b, p * 2, false);
            }
            self.put(a, b, p, 0, v);
            let mut v = self.try_slot(a, b, p * 2 + 1, false);
            if v.is_none() {
                v = self.wp(a, b, p * 2 + 1, false, true);
            }
            self.put(a, b, p, 1, v);
        }
    }

    fn pass2(&mut self, a: usize, b: usize) {
        self.ctx_root = self.is_root_set(a, b);
        let alert = self.reg.classes[b].alert;
        let root_family = self.parent_a(a).is_none();
        let melee = self.melee_rooted(b);
        if melee {
            for w in ACT0 * 2..PAIRS * 2 {
                self.rec(a, b)[w] = Slot::Empty;
            }
        }
        // Weapon actions. A crouch group with nothing of its own takes
        // stand's when stand has any.
        let mut crouch_empty = [false; 4];
        for (grp, ks) in [(0usize, &[0usize, 1][..]), (2, &[2][..]), (3, &[3][..])] {
            let mut none_crouch = true;
            let mut any_stand = false;
            for &k in ks {
                for l in 0..2 {
                    none_crouch &= self.get(a, b, ACT0 + 6 + k, l).is_none();
                    any_stand |= self.get(a, b, ACT0 + k, l).is_some();
                }
            }
            crouch_empty[grp] = none_crouch && any_stand;
        }
        if !melee {
            for st in 0..3 {
                for k in 0..6 {
                    let p = ACT0 + st * 6 + k;
                    for layer in 0..2 {
                        if self.get(a, b, p, layer).is_some() {
                            continue;
                        }
                        let mut v = None;
                        if k < 2 {
                            v = self.get(a, b, ACT0 + st * 6 + (1 - k), layer);
                        }
                        if v.is_none() && st == 1 && k < 4 {
                            let grp = if k < 2 { 0 } else { k };
                            if crouch_empty[grp] {
                                v = self.get(a, b, ACT0 + k, layer);
                            }
                        }
                        if v.is_none() {
                            v = self.wp(a, b, p * 2 + layer, true, true);
                        }
                        self.put(a, b, p, layer, v);
                    }
                }
                for k in 2..4 {
                    let p = ACT0 + st * 6 + k;
                    for layer in 0..2 {
                        if self.get(a, b, p, layer).is_none() {
                            let v = self.get(a, b, ACT0 + st * 6 + (5 - k), layer);
                            self.put(a, b, p, layer, v);
                        }
                    }
                }
            }
        }
        // Movement, the lower slots first (the loader's order:
        // 211260-212650 lower, 212654-213527 upper).
        for blk in 0..6 {
            if blk >= 3 && !alert {
                continue;
            }
            for m in 0..13 {
                let p = MOVE0 + blk * 13 + m;
                if self.get(a, b, p, 1).is_none() {
                    let v = self.sp(a, b, p * 2 + 1, true);
                    self.put(a, b, p, 1, v);
                }
            }
        }
        for blk in 0..6 {
            if blk >= 3 && !alert {
                continue;
            }
            for m in 0..13 {
                let p = MOVE0 + blk * 13 + m;
                if self.get(a, b, p, 0).is_some() {
                    continue;
                }
                let takeknee = m == 2 && (blk == 1 || blk == 4);
                let mut v = None;
                if m == 3 || m == 4 || takeknee {
                    v = self.get(a, b, MOVE0 + blk * 13, 0);
                    if v.is_none() && takeknee && blk == 4 {
                        v = self.get(a, b, MOVE0 + 13, 0);
                    }
                }
                if v.is_none() {
                    v = if blk < 3 {
                        self.wp(a, b, p * 2, true, true)
                    } else {
                        self.wpa(a, b, p * 2, true)
                    };
                }
                self.put(a, b, p, 0, v);
            }
        }
        for blk in 0..6 {
            for m in 0..13 {
                let p = MOVE0 + blk * 13 + m;
                for layer in 0..2 {
                    if self.get(a, b, p, layer).is_none() && blk >= 3 {
                        let v = self.get(a, b, p - 39, layer);
                        self.put(a, b, p, layer, v);
                    }
                    if self.get(a, b, p, layer).is_none() && root_family && m <= 4 {
                        let mut v = self.get(a, b, MOVE0 + blk * 13, layer);
                        if v.is_none() {
                            v = self.get(a, b, MOVE0, layer);
                        }
                        self.put(a, b, p, layer, v);
                    }
                }
            }
        }
        // Simple ids: the lower takes the parent family, then its alias;
        // the upper its alias, then the parent class. The four directed
        // jumps and jetpack_hover alias jump, sprint stand_runforward.
        for layer in [1usize, 0] {
            for p in 0..MOVE0 {
                if self.get(a, b, p, layer).is_some() {
                    continue;
                }
                let alias = |m: &mut Self| match p {
                    2..=5 | 25 => m.get(a, b, 1, layer),
                    0 => m.get(a, b, MOVE0 + 6, layer),
                    _ => None,
                };
                let v = if layer == 1 {
                    let v = self.sp(a, b, p * 2 + 1, true);
                    if v.is_none() {
                        alias(self)
                    } else {
                        v
                    }
                } else {
                    let v = alias(self);
                    if v.is_none() {
                        self.wp(a, b, p * 2, true, true)
                    } else {
                        v
                    }
                };
                self.put(a, b, p, layer, v);
            }
        }
    }
}

/// Depth-first order of a parent table, each root's subtree in index
/// order.
fn tree_order(parents: &[Option<usize>]) -> Vec<usize> {
    fn rec(i: usize, parents: &[Option<usize>], out: &mut Vec<usize>) {
        out.push(i);
        for j in 0..parents.len() {
            if j != i && parents[j] == Some(i) {
                rec(j, parents, out);
            }
        }
    }
    let mut out = Vec::new();
    for i in 0..parents.len() {
        if parents[i].is_none() || parents[i] == Some(i) {
            rec(i, parents, &mut out);
        }
    }
    out
}

/// Resolve every non-combo set of `reg` against `inv`: (family index,
/// class index) -> its slots.
pub fn build(inv: &Inventory, reg: &Registry) -> HashMap<(usize, usize), SetSlots> {
    build_selected(inv, reg, false)
}

/// Include combo weapons' ordinary movement slots. Their named attacks are
/// separate `comb` animations, not the fixed weapon-action table.
pub fn build_with_combos(inv: &Inventory, reg: &Registry) -> HashMap<(usize, usize), SetSlots> {
    build_selected(inv, reg, true)
}

fn build_selected(
    inv: &Inventory,
    reg: &Registry,
    combos: bool,
) -> HashMap<(usize, usize), SetSlots> {
    let mut m = Model {
        inv,
        reg,
        rec: HashMap::new(),
        ctx_root: false,
    };
    let fam_parents: Vec<_> = reg.families.iter().map(|f| f.parent).collect();
    let class_parents: Vec<_> = reg.classes.iter().map(|c| c.parent).collect();
    let order_b = tree_order(&class_parents);
    for a in tree_order(&fam_parents) {
        let bs: Vec<usize> = order_b
            .iter()
            .copied()
            .filter(|&b| reg.sets.contains(&(a, b)) && (combos || !reg.classes[b].combo))
            .collect();
        for &b in &bs {
            m.pass1(a, b);
        }
        for &b in &bs {
            m.pass2(a, b);
        }
    }
    reg.sets
        .iter()
        .filter(|&&(_, b)| combos || !reg.classes[b].combo)
        .map(|&k| {
            let slots = m.rec.remove(&k).map_or_else(
                || vec![None; PAIRS * 2],
                |r| r.into_iter().take(PAIRS * 2).map(Slot::clip).collect(),
            );
            (k, slots)
        })
        .collect()
}
