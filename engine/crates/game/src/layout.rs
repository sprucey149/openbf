//! Gameplay layout of a baked map: command posts (with their capture /
//! control regions and spawn paths) pulled out of the manifest's instances,
//! regions and paths. Runtime-only view over `assets::Manifest`.
//!
//! CP instance PROP keys (TAT1, hashes verified against the manifest):
//! Team 0xA2FD7D0C, Label 0xF69717FD, CaptureRegion 0xC6C161F9,
//! ControlRegion 0x805C1F8A, SpawnPath 0xDDE0B14F, ValueBleed 0x4EBF97DA.
//! Region/path names are matched case-insensitively (props say
//! "CP10Capture", regions are stored "cp10capture").
//!
//! Children: [`nav`] owns `nav_graph` and the per-map routing fixes;
//! `tests` owns the manifest/mission regressions. This facade keeps the
//! post/spawn-path types, the PROP keys, the construction from a manifest and
//! the mission-folding methods.

use crate::assets::Manifest;
use crate::mission::MissionDef;
use core::announcer::{self, PostEvent, PostLines};
use core::fmath::FloatExt;
use core::region::Region;
use glam::Vec3;

mod nav;
pub use self::nav::nav_graph;

pub const PROP_TEAM: u32 = 0xA2FD_7D0C;
pub const PROP_LABEL: u32 = 0xF697_17FD;
pub const PROP_CAPTURE_REGION: u32 = 0xC6C1_61F9;
pub const PROP_CONTROL_REGION: u32 = 0x805C_1F8A;
pub const PROP_SPAWN_PATH: u32 = 0xDDE0_B14F;
pub const PROP_ALLY_PATH: u32 = 0xDAD2_15B0;
pub const PROP_ALLY_COUNT: u32 = 0x7136_FD3E;
pub const PROP_VALUE_BLEED: u32 = 0x4EBF_97DA;
/// `AISpawnWeight` (BF2; re-command-walker-post §6 [O]).
pub const PROP_AI_SPAWN_WEIGHT: u32 = 0xE072_82C2;

/// Per-faction attack / defend post values, in FACTION ORDER `[Alliance,
/// Empire, Republic, CIS, Locals]`. These ten are RAW PROP HASHES read off the
/// disc (analysis/re-cp-capture.md §176-177), retained as provenance. They
/// equal `classdb::fnv1a_lowercase` of the key names: the game hashes each
/// byte ORed with 0x20, which differs from ordinary ASCII lowercasing.
pub const PROP_VALUE_ATK: [u32; 5] = [
    0xA340_D6B5,
    0x4DB4_4670,
    0x9E78_B410,
    0x00E7_3245,
    0x573C_98A4,
];
pub const PROP_VALUE_DEF: [u32; 5] = [
    0x44A2_C4A6,
    0xA867_9457,
    0x8F55_40CB,
    0x5BF8_5AE8,
    0xE689_EF93,
];
/// A post that authors no value for a faction takes the command-post class
/// constructor's own default.
pub const VALUE_DEFAULT: i32 = 100;

#[derive(Debug, Clone)]
pub struct SpawnNode {
    pub pos: Vec3,
    /// Yaw in the engine's convention (0 = −Z, positive left), derived from
    /// the stored (w, x, y, z) quaternion as 2·atan2(y, w). PROVISIONAL
    /// mapping of the editor's facing to ours.
    pub yaw: f32,
}

#[derive(Debug, Clone)]
pub struct SpawnPath {
    pub name: String,
    pub nodes: Vec<SpawnNode>,
}

#[derive(Debug, Clone)]
pub struct CommandPostDef {
    pub name: String,
    pub label: String,
    pub class: String,
    /// Starting owner: 0 = neutral, 1/2 = the two main teams (mission
    /// numbering; core::battle uses 0-based team indices = this − 1).
    pub team: u8,
    pub pos: Vec3,
    pub capture: Option<Region>,
    pub control: Option<Region>,
    pub spawn_path: Option<String>,
    /// Authored `AllyPath`, separate from this post's own spawn path.
    pub ally_path: Option<String>,
    /// Authored `AllyCount` budget, before any class default.
    pub ally_count: Option<u32>,
    pub value_bleed: i32,
    /// `AISpawnWeight`: the post's weight in a bot's draw of where to spawn
    /// (BF2 only; the constructor's 100 where the instance authors none —
    /// on the discs one instance does, BF2 KAS2's `baserush`, 1000 — and
    /// clamped ≥ 0 as the PS2's setter). OURS #171: [`CONQUEST_PARITY`]
    /// `SpawnWeight` rows set unequal weights on some BF1 maps too, so the
    /// draw is weighted there.
    pub ai_spawn_weight: f32,
    /// `Value_ATK_<faction>` per faction, in [`PROP_VALUE_ATK`]'s order
    /// (Alliance, Empire, Republic, CIS, Locals); [`VALUE_DEFAULT`] where the
    /// post authors none.
    pub value_atk: [i32; 5],
    /// `Value_DEF_<faction>`, same order and same default.
    pub value_def: [i32; 5],
    pub major: bool,
    /// A space hangar's post: it spawns its team and is never taken.
    pub space: bool,
    /// Whether this post can be captured or neutralized.
    pub capturable: bool,
    /// Whether this post is drawn on the minimap or spawn map.
    pub visible: bool,
    /// The post's `VO_<Listener>_<Subject><Event>` lines (card ANNC,
    /// analysis/re-announcer.md §2.1); empty on a post that authors none.
    pub vo: PostLines,
}

#[derive(Debug, Clone, Default)]
pub struct MapLayout {
    /// The DISC's own map name (`Manifest::map`: `tat1`, `hoth`, `kamino1`),
    /// not the baked directory's. Read by [`crate::walk::WalkSim::rescue_spawn`]
    /// for the one map whose start it must not move.
    pub map: String,
    pub cps: Vec<CommandPostDef>,
    pub paths: Vec<SpawnPath>,
    /// Every named world region the manifest could resolve, in manifest order.
    /// Looked up by name (case-insensitively) — see [`MapLayout::region`].
    pub regions: Vec<(String, Region)>,
}

/// One Conquest map-parity fix (issue #171).
#[derive(Debug, Clone, Copy)]
pub enum ParityFix {
    /// This post starts neutral.
    NeutralStart(usize),
    /// This post starts owned by `team` (1-based as in
    /// [`CommandPostDef::team`]; 0 is neutral, like [`ParityFix::NeutralStart`]).
    StartTeam { post: usize, team: u8 },
    /// This post's `AISpawnWeight` (the owner's bot spawn-post draw weight,
    /// authored 100): which of a team's posts its bots respawn at.
    SpawnWeight { post: usize, weight: f32 },
    /// This post's attack/defend values for one team index (0-based).
    Goals {
        post: usize,
        team: usize,
        atk: i32,
        def: i32,
    },
    /// Drop these nodes (bit `i` = node `i`, in authored order) from this
    /// post's `AllyPath`, where the natives its friendship admits spawn.
    AllyNodes { post: usize, drop: u32 },
}

/// Issue #171 Conquest map-parity fixes: (map, clone-wars era, fix), applied
/// over the mission's own post data by [`MapLayout::apply_mission`].
/// OURS #171, each from the bot-only Hard `sides` census (authored tickets):
/// measure: bot-only Hard seeds 1–8 with the map's `conquest_defaults` entry
/// at its authored pair, after any sim, AI or map change; replace: a fix
/// measured to even the map on its own, or drop the row.
const fn goals(post: usize, team: usize, atk: i32, def: i32) -> ParityFix {
    ParityFix::Goals {
        post,
        team,
        atk,
        def,
    }
}

pub const CONQUEST_PARITY: &[(&str, bool, ParityFix)] = &[
    // OURS #171: the Empire starts on one post of five (cp2) and the Rebels
    // hold the majority from the first second, so the Empire bleeds all round
    // (one neutral alone was retaken in 50 s: 0/8). The two Rebel posts
    // nearest the Empire's (cp1, cp3) start neutral. Bot-only Hard seeds 1-8
    // at the authored 232/232: Empire 0/8 -> 1/8, mean Rebel margin 70 -> 44.
    // OURS #171 (p3, natives on): with the 2026-11-20 bot changes the Empire
    // still lost 37 % at the 60 % cap with cp3 neutral; it now starts on cp3
    // (the Rebel post 155 m by nav path from their cp4). Bot-only Hard at
    // the cap 278/186: Empire 37 % (63, seeds 31001-32032) -> 22/31 with cp3
    // (seeds 31101-31132), tickets refitted to 270/194. measure: the sides
    // census; replace: drop when a ticket pair alone evens it.
    ("bf1_hot1", false, ParityFix::NeutralStart(1)),
    ("bf1_hot1", false, ParityFix::StartTeam { post: 3, team: 1 }),
    // OURS #171: cp2 is the swing post; the Empire (team 1) spends its waves
    // on the Alliance's three home posts (cp0, cp1, cp5), which it never
    // takes, and loses cp2. Those three are worth less to it to attack, and
    // cp2 twice as much to attack and hold. Bot-only Hard seeds 1-8 at the
    // authored 150/150: still 8/8 for the Alliance, mean margin 82 -> 71.
    ("bf2_kam1", false, goals(0, 1, 3, 10)),
    ("bf2_kam1", false, goals(1, 1, 3, 10)),
    ("bf2_kam1", false, goals(5, 1, 3, 10)),
    ("bf2_kam1", false, goals(2, 1, 20, 20)),
    // OURS #171 (p3): Death Star. The Rebel/Republic far post CP7 (idx 5)
    // is 318 m and 397 m by soldier nav path from its side's other two posts,
    // and took a third of that side's respawns, so its south post CP4 (idx 4)
    // got 10 respawns a round and fell at 65-190 s in 16 of 16 rounds while
    // the Empire's three posts sit within 120-266 m of each other. The
    // south post draws three times the respawns and CP7 under a third.
    // Bot-only Hard at the authored 251/251 and 259/259, seeds 1-96: team 0
    // 3/32 -> 31/96 (GCW), 5/32 -> 57/96 (CW); Empire majority 308 s -> about
    // 100 s. measure: the sides census (sidespost2 spawned) after any spawn or
    // AI change; replace: a spawn rule that weighs the front, or drop.
    ("bf2_dea1", false, spawn_weight(5, 30.0)),
    ("bf2_dea1", false, spawn_weight(4, 300.0)),
    ("bf2_dea1", true, spawn_weight(5, 30.0)),
    ("bf2_dea1", true, spawn_weight(4, 300.0)),
    // OURS #171 (p3): BF1 Kashyyyk Docks. The centre dock (idx 6) is 114 m
    // by nav path from the Empire/CIS posts and 229-279 m from the
    // Alliance/Republic decks, and team 1 spread a quarter of its waves on
    // team 0's far home idx 0 (481-512 m). Team 1 starts on the centre,
    // respawns three times as often at its nearest post idx 3, sends three
    // times the waves to the centre and almost none to idx 0. Bot-only Hard
    // at the authored 250/250, seeds 1-32: team 0 32/32 -> 17/32 (GCW),
    // 32/32 -> 26/32 (CW). With the native Wookiees and the 2026-11-20 bot
    // changes (p3 refit) the 24-to-32 populations overshot (team 0 6 % at
    // 247/253); even 32/32 populations with these rows play 84 %/81 % at
    // 250/250 and the tickets even the rest (`conquest_defaults`).
    // measure: the sides census; replace: a nav or AI fix for the deck
    // descent, or drop.
    ("bf1_kas2", false, ParityFix::StartTeam { post: 6, team: 2 }),
    ("bf1_kas2", false, spawn_weight(3, 300.0)),
    ("bf1_kas2", false, goals(6, 1, 10, 30)),
    ("bf1_kas2", false, goals(0, 1, 10, 1)),
    ("bf1_kas2", true, ParityFix::StartTeam { post: 6, team: 2 }),
    ("bf1_kas2", true, spawn_weight(3, 300.0)),
    ("bf1_kas2", true, goals(6, 1, 10, 30)),
    ("bf1_kas2", true, goals(0, 1, 10, 1)),
    // OURS #171 (p3): Rhen Var Harbor. The neutral centre idx 3 is 351-518 m
    // by nav path from team 0's posts and 236-240 m from team 1's, which
    // took it at 34-53 s every round. Team 0 starts on it and respawns there
    // more often while it holds it: three times in GCW, 1.7 times in CW
    // (three times overshot there, 49/64 for team 0 at the 60 % cap).
    // Bot-only Hard at the authored 213/213 and 184/184: team 0 5/32 ->
    // 19/32 (GCW, seeds 1-32), 5/32 -> 33/64 (CW, seeds 33-96). measure: the
    // sides census; replace: drop when a ticket pair alone evens it.
    ("bf1_rhn1", false, ParityFix::StartTeam { post: 3, team: 1 }),
    ("bf1_rhn1", false, spawn_weight(3, 300.0)),
    ("bf1_rhn1", true, ParityFix::StartTeam { post: 3, team: 1 }),
    ("bf1_rhn1", true, spawn_weight(3, 170.0)),
    // OURS #171 (p3): Bespin Platforms. The neutral centre idx 6 is 144 m
    // by nav path from the Alliance/Republic's nearest post and 187 m from
    // the Empire/CIS's (idx 1), and team 0 took it at 37-67 s. Team 1 starts
    // on it, respawns three times as often at idx 1 and sends three times
    // the waves to the centre (BF1: team 1 reads the defend column).
    // Bot-only Hard at the authored 107/107 and 95/95, seeds 1-32: team 0
    // 27/32 -> 19/32 (GCW), 31/32 -> 22/32 (CW). measure: the sides census;
    // replace: drop when a ticket pair alone evens it.
    ("bf1_bes1", false, ParityFix::StartTeam { post: 6, team: 2 }),
    ("bf1_bes1", false, spawn_weight(1, 300.0)),
    ("bf1_bes1", false, goals(6, 1, 10, 30)),
    ("bf1_bes1", true, ParityFix::StartTeam { post: 6, team: 2 }),
    ("bf1_bes1", true, spawn_weight(1, 300.0)),
    ("bf1_bes1", true, goals(6, 1, 10, 30)),
    // OURS #171 (p3): Mygeeto. Team 1 took the neutral centre idx 0 at
    // 25-45 s in every round; team 0's far post idx 2 (293 m from the
    // centre by nav path) took as many of its respawns as idx 3 (167 m).
    // Team 0 starts on the centre, respawns three times as often at idx 3
    // and under a third at idx 2. Bot-only Hard at the authored 231/231 and
    // 246/246, seeds 1-32: team 0 4/32 -> 12/32 (GCW), 4/32 -> 19/32 (CW).
    // measure: the sides census; replace: drop when a ticket pair alone
    // evens it.
    ("bf2_myg1", false, ParityFix::StartTeam { post: 0, team: 1 }),
    ("bf2_myg1", false, spawn_weight(3, 300.0)),
    ("bf2_myg1", false, spawn_weight(2, 30.0)),
    ("bf2_myg1", true, ParityFix::StartTeam { post: 0, team: 1 }),
    ("bf2_myg1", true, spawn_weight(3, 300.0)),
    ("bf2_myg1", true, spawn_weight(2, 30.0)),
    // OURS #171 (p3): Utapau. Team 1 authors attack 0 on every post, so all
    // of its first wave takes the four neutrals (first capture 24-29 s);
    // idx 4, 124-126 m from both sides' nearest posts by nav path, went to
    // team 1 in 32 of 32 rounds. Team 0 starts on it. Bot-only Hard at the
    // authored 150/150 and 166/166, seeds 1-32: team 0 2/32 -> 25/32 (GCW),
    // 1/32 -> 13/32 (CW). measure: the sides census; replace: drop when a
    // ticket pair alone evens it.
    ("bf2_uta1", false, ParityFix::StartTeam { post: 4, team: 1 }),
    ("bf2_uta1", true, ParityFix::StartTeam { post: 4, team: 1 }),
    // OURS #171 (p3): BF1 Geonosis. The Republic starts on one post of four
    // and bled from the first second (114 tickets a round to 0); it also
    // starts on idx 1. Bot-only Hard at the authored 237/237, seeds 1-32:
    // team 0 0/32 -> 6/32. measure: the sides census; replace: drop when a
    // ticket pair alone evens it.
    ("bf1_geo1", true, ParityFix::StartTeam { post: 1, team: 1 }),
    // OURS #171 (p3): Tatooine Mos Eisley GCW. The Alliance post idx 4 is 143
    // m by nav path from the Empire's idx 3 and 219-225 m from the
    // Alliance's other posts; it drew 8 Alliance respawns a round and the
    // Empire took it in 94 of 96 rounds (first capture 70 s). It draws three
    // times the respawns. Bot-only Hard, seeds 33-96: team 0 62 % at the 60 %
    // cap 120/180 before, 41/64 at the authored 150/150 and 17/64 at
    // 120/180 with it. measure: the sides census; replace: drop when a
    // ticket pair alone evens it.
    ("tat2", false, spawn_weight(4, 300.0)),
    // OURS (p3, the Jawa change): the same post in the Clone Wars era. With
    // the Jawas fighting both armies team 1's idx 4 fell early and team 0
    // held the majority most of the round (owned 3.8 to 2.2 posts);
    // team 0 won 62-71 % with team 1 at the 60 % cap 147/217. It draws
    // three times the respawns, as in GCW, and the pair returns toward the
    // authored 182/182. Bot-only Hard, seeds 401-432: 16/32 at 182/182.
    // measure: the sides census; replace: drop when a ticket pair alone
    // evens it.
    ("tat2", true, spawn_weight(4, 300.0)),
    // OURS (p3, the Jawa change): BF1 Mos Eisley. The Jawas spawn on cp7's
    // (idx 0) ally path, 14 nodes spread over the town; seven sit within 60 m
    // of team 0's posts (idx 1, 3, 5) against five of team 1's, so with the
    // Jawas hostile to both armies they killed team 0 about twice as often
    // as team 1 and team 0 won 0-8 % (CW) and 8-31 % (GCW), outside the
    // ticket cap's reach. Nodes 3, 4, 5, 11 and 13 (20-62 m from team 0's
    // posts) are dropped, leaving two near team 0 and four near team 1.
    // Bot-only Hard at the earlier pairs 112/144 (GCW) and 140/130 (CW):
    // team 0 2/24 and 0/24 with every node (seeds 101-124), 11/32 and 14/32
    // with these dropped (seeds 401-432; dropping only 4, 11 and 13 gave 8/32
    // and 1/32); `conquest_defaults` refits the pairs on top. measure: the
    // natives census (kills per army); replace: a native spawn rule that
    // weighs both armies, or drop.
    (
        "bf1_tat2",
        false,
        ParityFix::AllyNodes {
            post: 0,
            drop: BF1_TAT2_JAWA_DROP,
        },
    ),
    (
        "bf1_tat2",
        true,
        ParityFix::AllyNodes {
            post: 0,
            drop: BF1_TAT2_JAWA_DROP,
        },
    ),
    // OURS #171 (p3): BF1 Kamino. Five of seven posts start neutral, and
    // three of them (idx 0, 1, 5) sit within 80 m of each other by nav path
    // beside the CIS start; the CIS took idx 0 and 1 in 92-96 of 96 rounds
    // at 30 s and held the majority 188 s to 67 s. The Republic starts on
    // idx 5. Bot-only Hard, seeds 33-96: team 0 62 % at the 60 % cap 139/206
    // before, 12/64 at the authored 164/181 with it. measure: the sides
    // census; replace: drop when a ticket pair alone evens it.
    ("bf1_kam1", true, ParityFix::StartTeam { post: 5, team: 2 }),
    // OURS #171 (p3 refit, natives on): BF1 Endor. The Ewoks fight for the
    // Alliance and team 0 held three posts all round (90 % at the 60 % cap,
    // 207/309). The Empire starts on the centre post idx 4 (144-147 m by nav
    // path from both sides' nearest posts). Bot-only Hard at the cap:
    // 88 % (64) -> 19 % (32, seeds 31101-31132); tickets refitted to 221/295.
    // measure: the sides census; replace: drop when a ticket pair alone evens it.
    ("bf1_end1", false, ParityFix::StartTeam { post: 4, team: 2 }),
    // OURS #171 (p3 refit, natives on): BF1 Kashyyyk Islands. The Wookiees
    // fight for the Alliance/Republic and the Empire/CIS won 3 of 64 at the
    // 60 % cap (202/136). Team 0 starts on the Alliance post idx 1 (116 m by
    // nav path from the contested idx 2). Spawn weights and goal rows on
    // idx 2/3 left it at 0-3 %. Bot-only Hard: GCW 59 % at 202/136, CW 72 %
    // at 274/186 (seeds 31201-31232); tickets refitted to 196/142 and 253/207.
    // measure: the sides census; replace: drop when a ticket pair alone evens it.
    ("bf1_kas1", false, ParityFix::StartTeam { post: 1, team: 1 }),
    ("bf1_kas1", true, ParityFix::StartTeam { post: 1, team: 1 }),
    // OURS #171 (p3 refit, natives on): BF1 Yavin Arena GCW. The Empire lost
    // 36 % (64) at the 60 % cap 240/160; it now starts on the neutral centre
    // idx 2 (82-85 m from the flanking posts): 17/32 at the cap (seeds
    // 31101-31132), tickets 239/161. measure: the sides census;
    // replace: drop when a ticket pair alone evens it.
    ("bf1_yav2", false, ParityFix::StartTeam { post: 2, team: 1 }),
    // OURS #171 (p3 refit, natives on): BF2 Polis Massa. With `route_kept`
    // (PR #183) team 0 won 66 %/58 % at the 60 % cap (150/224, 120/180),
    // a kill race with the posts static. Team 1 respawns three times as often
    // at idx 5 (150 m by nav path from the contested idx 0) and a tenth as
    // often at its far idx 2 (284 m). Bot-only Hard at the cap: GCW 18/32,
    // CW 17/32 (seeds 31101-31132). measure: the sides census;
    // replace: drop when a ticket pair alone evens it.
    ("bf2_pol1", false, spawn_weight(5, 300.0)),
    ("bf2_pol1", false, spawn_weight(2, 30.0)),
    ("bf2_pol1", true, spawn_weight(5, 300.0)),
    ("bf2_pol1", true, spawn_weight(2, 30.0)),
    // OURS #171 (p3 refit, natives on): BF2 Yavin 4 GCW. Team 0 took the
    // neutral idx 6 (40 m from its idx 4) and won 77 % at the 60 % cap
    // 146/218. Team 1 starts on it: 15/32 at the cap (seeds 31101-31132),
    // tickets 147/217. measure: the sides census; replace: drop
    // when a ticket pair alone evens it.
    ("bf2_yav1", false, ParityFix::StartTeam { post: 6, team: 2 }),
];

/// BF1 Mos Eisley's dropped Jawa ally-path nodes (3, 4, 5, 11, 13).
const BF1_TAT2_JAWA_DROP: u32 = 1 << 3 | 1 << 4 | 1 << 5 | 1 << 11 | 1 << 13;

const fn spawn_weight(post: usize, weight: f32) -> ParityFix {
    ParityFix::SpawnWeight { post, weight }
}

/// What [`MapLayout::apply_mission`] did, for the caller's log.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MissionApplied {
    /// Command posts left in the layout afterwards.
    pub kept: usize,
    /// Dropped because the mission's `CommandPost:New` roster does not name
    /// them (BF2 only; a BF1 mission names none and drops nothing).
    pub dropped_by_roster: usize,
    /// Dropped by a `KillObject` call.
    pub killed: usize,
    /// Re-owned by `SetObjectTeam` / `SetProperty(cp, "Team", …)`.
    pub reassigned: usize,
    /// `AddDeathRegion` volumes, resolved, in the mission's call order.
    pub death_regions: Vec<Region>,
    /// `AddDeathRegion` names the manifest has no region for.
    pub missing_death_regions: Vec<String>,
    /// The mission's landing regions, the ones the map holds.
    pub landing_regions: Vec<Region>,
    /// The soldiers' fly-height ceiling for `Sim::fly_ceiling` (card JETP):
    /// the mission's `SetMaxPlayerFlyHeight`, else the engine's 200.
    pub fly_ceiling: f32,
    /// The four `Set*FlyHeight` calls, for `Sim::flight_limits`.
    pub flight_limits: core::sim::FlightLimits,
}

fn prop(props: &[(u32, String)], key: u32) -> Option<&str> {
    props.iter().find(|p| p.0 == key).map(|p| p.1.as_str())
}

/// The five per-faction values under `keys`, in the same order, defaulting to
/// [`VALUE_DEFAULT`] where the prop is absent or unparseable.
fn faction_values(props: &[(u32, String)], keys: &[u32; 5]) -> [i32; 5] {
    let mut out = [VALUE_DEFAULT; 5];
    for (i, &k) in keys.iter().enumerate() {
        if let Some(v) = prop(props, k).and_then(|s| s.trim().parse().ok()) {
            out[i] = v;
        }
    }
    out
}

/// A post's VO table out of its instance props: every
/// `VO_<Listener>_<Subject><Event>` over the four commander factions.
fn post_lines(props: &[(u32, String)]) -> PostLines {
    let mut out = PostLines::default();
    for l in 0..announcer::FACTIONS.len() {
        for s in 0..announcer::FACTIONS.len() {
            for e in PostEvent::ALL {
                let key = crate::classdb::fnv1a_lowercase(&announcer::post_property(l, s, e));
                if let Some(line) = prop(props, key).filter(|v| !v.is_empty()) {
                    out.set(l, s, e, line);
                }
            }
        }
    }
    out
}

impl MapLayout {
    pub fn from_manifest(m: &Manifest) -> MapLayout {
        Self::from_battle_manifest(m, false)
    }

    pub fn from_battle_manifest(m: &Manifest, hunt: bool) -> MapLayout {
        Self::from_mission_manifest(m, hunt, false)
    }

    pub fn from_mission_manifest(m: &Manifest, hunt: bool, campaign: bool) -> MapLayout {
        Self::from_mission_manifest_mode(m, hunt, campaign, false)
    }

    pub fn from_mission_manifest_mode(
        m: &Manifest,
        hunt: bool,
        campaign: bool,
        ctf: bool,
    ) -> MapLayout {
        let region = |name: &str| -> Option<Region> {
            let r = m
                .regions
                .iter()
                .find(|r| r.name.eq_ignore_ascii_case(name))?;
            Region::from_rows(&r.shape, r.rotation, r.translation, r.size)
        };
        let mut cps = Vec::new();
        for inst in &m.instances {
            // A space hangar's post has no capture region: its control region is where
            // it stands, and only its spawns matter.
            let has_spawn_path = prop(&inst.props, PROP_SPAWN_PATH).is_some();
            let class_lower = inst.class.to_ascii_lowercase();
            let space_post = class_lower == "com_bldg_controlzone_ctf" && has_spawn_path;
            let hunt_post = hunt && has_spawn_path && class_lower == "com_bldg_controlzone";
            let campaign_post = campaign
                && class_lower.contains("controlzone")
                && class_lower != "com_holo_controlzone"
                && !inst.name.eq_ignore_ascii_case(&inst.class);
            let cap_name = match prop(&inst.props, PROP_CAPTURE_REGION) {
                Some(name) => Some(name),
                None if space_post => match prop(&inst.props, PROP_CONTROL_REGION) {
                    Some(name) => Some(name),
                    None if m.spawn_only_posts || campaign || ctf => None,
                    None => continue,
                },
                None if hunt_post || campaign_post => None,
                None => continue, // not a command post
            };
            let capturable = !space_post
                && (!campaign || (cap_name.is_some() && !class_lower.contains("untakeable")));
            let visible = !campaign || !class_lower.contains("inv");
            let pos = Vec3::from(inst.translation);
            let mut capture = cap_name.and_then(region);
            // OURS, PROVISIONAL (Utapau campaign, `cam_cp4`): the authored
            // capture box's bottom sits 0.19 m above the post instance, so a
            // soldier standing on the platform at the post itself is outside
            // it and the first objective cannot be captured. Original-game
            // footage shows that post being captured from the platform. Bring
            // a small authored gap inside the box, without turning a distant
            // region (a layer mix-up) into a capture zone.
            // replace: a PS2 capture of the post with the soldier's feet
            // height and the live capture region, to fix the data instead.
            if campaign && capturable {
                if let Some(r) = &mut capture {
                    let local = r.to_local(pos);
                    let below = -r.size.y - local.y;
                    let horizontal = match r.shape {
                        core::region::Shape::Cylinder => {
                            local.x * local.x + local.z * local.z < r.size.x * r.size.x
                        }
                        _ => local.x.abs() < r.size.x && local.z.abs() < r.size.z,
                    };
                    if below > 0.0 && below <= 0.5 && horizontal {
                        r.center -= r.basis.y_axis * (below + 0.01);
                    }
                }
            }
            cps.push(CommandPostDef {
                name: inst.name.clone(),
                label: prop(&inst.props, PROP_LABEL).unwrap_or("").to_owned(),
                class: inst.class.clone(),
                team: prop(&inst.props, PROP_TEAM)
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0),
                pos,
                capture,
                control: prop(&inst.props, PROP_CONTROL_REGION).and_then(region),
                spawn_path: prop(&inst.props, PROP_SPAWN_PATH).map(str::to_owned),
                ally_path: prop(&inst.props, PROP_ALLY_PATH).map(str::to_owned),
                ally_count: prop(&inst.props, PROP_ALLY_COUNT).and_then(|s| s.trim().parse().ok()),
                // BF1: scalar ValueBleed (class default 100). BF2: per-faction
                // ValueBleed_<faction> (identical across main factions in
                // retail; class default 10) — first present wins.
                value_bleed: prop(&inst.props, PROP_VALUE_BLEED)
                    .or_else(|| {
                        ["Alliance", "Empire", "Republic", "CIS"]
                            .iter()
                            .find_map(|f| {
                                prop(
                                    &inst.props,
                                    crate::classdb::fnv1a_lowercase(&format!("ValueBleed_{f}")),
                                )
                            })
                    })
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(100),
                ai_spawn_weight: prop(&inst.props, PROP_AI_SPAWN_WEIGHT)
                    .and_then(|s| s.trim().parse::<f32>().ok())
                    .map_or(core::cp::AI_SPAWN_WEIGHT_DEFAULT, |w| w.fmax(0.0)),
                value_atk: faction_values(&inst.props, &PROP_VALUE_ATK),
                value_def: faction_values(&inst.props, &PROP_VALUE_DEF),
                major: inst.class.contains("major"),
                space: space_post && prop(&inst.props, PROP_CAPTURE_REGION).is_none(),
                capturable,
                visible,
                vo: post_lines(&inst.props),
            });
        }
        let regions = m
            .regions
            .iter()
            .filter_map(|r| {
                Region::from_rows(&r.shape, r.rotation, r.translation, r.size)
                    .map(|v| (r.name.clone(), v))
            })
            .collect();
        let paths = m
            .paths
            .iter()
            .map(|p| SpawnPath {
                name: p.name.clone(),
                nodes: p
                    .nodes
                    .iter()
                    .map(|(pos, q)| SpawnNode {
                        pos: Vec3::from(*pos),
                        // `fmath`, not std: the yaw reaches `Sim::hash` via
                        // the respawn, so the host libm must not pick it.
                        yaw: 2.0 * core::fmath::atan2(q[2], q[0]),
                    })
                    .collect(),
            })
            .collect();
        MapLayout {
            map: m.map.clone(),
            cps,
            paths,
            regions,
        }
    }

    pub fn path(&self, name: &str) -> Option<&SpawnPath> {
        self.paths
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// A named world region (`wrld regn`), matched case-insensitively — props
    /// say "CP10Capture" where the region is stored "cp10capture".
    pub fn region(&self, name: &str) -> Option<Region> {
        self.regions
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, r)| *r)
    }

    /// Issue #171 map-parity fixes for one Conquest mission ([`CONQUEST_PARITY`]).
    /// Post indexes are the layout's after the mission roster, the order the
    /// sim and the sweep's `sidespost` census number them.
    /// `extra` rows (the sweep's `SWEEP_PARITY`) apply after the table's.
    fn apply_conquest_parity(&mut self, m: &MissionDef, extra: &[(&str, bool, ParityFix)]) {
        let cw = m.era.as_deref() == Some("cw");
        let rows = m.factions().map(|f| crate::walk::faction_row(&f));
        for (map, era_cw, fix) in CONQUEST_PARITY.iter().chain(extra) {
            if *map != m.map || *era_cw != cw {
                continue;
            }
            match *fix {
                ParityFix::NeutralStart(i) => {
                    if let Some(c) = self.cps.get_mut(i) {
                        c.team = 0;
                    }
                }
                ParityFix::StartTeam { post, team } => {
                    if let Some(c) = self.cps.get_mut(post) {
                        c.team = team;
                    }
                }
                ParityFix::SpawnWeight { post, weight } => {
                    if let Some(c) = self.cps.get_mut(post) {
                        c.ai_spawn_weight = weight;
                    }
                }
                ParityFix::Goals {
                    post,
                    team,
                    atk,
                    def,
                } => {
                    if let Some(c) = self.cps.get_mut(post) {
                        c.value_atk[rows[team]] = atk;
                        c.value_def[rows[team]] = def;
                    }
                }
                ParityFix::AllyNodes { post, drop } => {
                    let Some(name) = self.cps.get(post).and_then(|c| c.ally_path.clone()) else {
                        continue;
                    };
                    if let Some(path) = self
                        .paths
                        .iter_mut()
                        .find(|p| p.name.eq_ignore_ascii_case(&name))
                    {
                        let mut i = 0;
                        path.nodes.retain(|_| {
                            let keep = i >= 32 || drop & (1 << i) == 0;
                            i += 1;
                            keep
                        });
                    }
                }
            }
        }
    }

    /// Fold a mission's command-post edits into this layout, and resolve its
    /// death regions.
    ///
    /// - `posts` (the BF2 `ObjectiveConquest` roster, in `CommandPost:New`
    ///   order) RESTRICTS the layout to the posts it names; the LAYOUT's own
    ///   order is kept, not the roster's, because the sim indexes posts by
    ///   position and the AI's `target_cp` is that index. Empty (every BF1
    ///   mission) keeps the world layer's posts as they are.
    /// - `killed` (`KillObject`) drops posts outright — BF2 Hoth's `CP7`.
    /// - `cps` re-owns posts (1-based team as stored in
    ///   [`CommandPostDef::team`]; 0 = neutral). BF2 Hoth's world layer
    ///   authors all five `hoth_cps` to one side and only the mission splits
    ///   them.
    /// - `death_regions` (`AddDeathRegion`) are resolved by name.
    ///
    /// Names are matched case-insensitively; an unknown one is reported once
    /// and skipped.
    pub fn apply_mission(&mut self, m: &MissionDef) -> MissionApplied {
        self.apply_mission_with(m, &[])
    }

    /// [`Self::apply_mission`] with `extra` Conquest parity rows applied after
    /// [`CONQUEST_PARITY`]'s. Only the sweep passes any (its `SWEEP_PARITY`
    /// ladder arms); the game passes none.
    pub fn apply_mission_with(
        &mut self,
        m: &MissionDef,
        extra: &[(&str, bool, ParityFix)],
    ) -> MissionApplied {
        let named = |list: &[String], name: &str| list.iter().any(|k| k.eq_ignore_ascii_case(name));

        let before = self.cps.len();
        if !m.cp_whitelist.is_empty() {
            self.cps.retain(|c| named(&m.cp_whitelist, &c.name));
        }
        if !m.cp_blacklist.is_empty() {
            self.cps.retain(|c| !named(&m.cp_blacklist, &c.name));
        }
        if !m.is_campaign() && !m.posts.is_empty() {
            self.cps.retain(|c| named(&m.posts, &c.name));
            for want in &m.posts {
                if !self.cps.iter().any(|c| c.name.eq_ignore_ascii_case(want)) {
                    eprintln!(
                        "mission {}/{}: roster names {want:?}, which the world layer has no post for",
                        m.map, m.mode
                    );
                }
            }
        }
        let dropped_by_roster = before - self.cps.len();

        let killed = if m.is_campaign() {
            // Campaign scripts kill CPs at startup so `RespawnObject` can
            // reactivate them in later stages.
            self.cps
                .iter()
                .filter(|c| named(&m.killed, &c.name))
                .count()
        } else {
            let before = self.cps.len();
            self.cps.retain(|c| !named(&m.killed, &c.name));
            before - self.cps.len()
        };

        let mut reassigned = 0;
        for (name, team) in &m.cps {
            match self
                .cps
                .iter_mut()
                .find(|c| c.name.eq_ignore_ascii_case(name))
            {
                Some(c) => {
                    c.team = *team;
                    reassigned += 1;
                }
                None => eprintln!(
                    "mission {}/{}: cp {name:?} re-owned by the script is not in the layout",
                    m.map, m.mode
                ),
            }
        }

        if m.mode == "con" {
            self.apply_conquest_parity(m, extra);
        }

        let mut death_regions = Vec::new();
        let mut missing_death_regions = Vec::new();
        for name in &m.death_regions {
            match self.region(name) {
                Some(r) => death_regions.push(r),
                None => {
                    eprintln!(
                        "mission {}/{}: death region {name:?} is not in the manifest",
                        m.map, m.mode
                    );
                    missing_death_regions.push(name.clone());
                }
            }
        }

        MissionApplied {
            kept: self.cps.len(),
            dropped_by_roster,
            killed,
            reassigned,
            death_regions,
            missing_death_regions,
            landing_regions: m
                .landing_regions
                .iter()
                .filter_map(|name| self.region(name))
                .collect(),
            fly_ceiling: m
                .max_player_fly_height
                .unwrap_or(core::soldier::DEFAULT_FLY_CEILING),
            flight_limits: core::sim::FlightLimits {
                player: [
                    m.min_player_fly_height.unwrap_or(-200.0),
                    m.max_player_fly_height.unwrap_or(200.0),
                ],
                ai: [
                    m.min_fly_height.unwrap_or(-200.0),
                    m.max_fly_height.unwrap_or(200.0),
                ],
            },
        }
    }
}

#[cfg(test)]
mod space_tests {
    use super::*;

    /// A hangar's post authors a control region and a spawn path and no capture region.
    /// Any other class with no capture region is not a post.
    #[test]
    fn a_space_hangars_post_is_a_post_that_cannot_be_taken() {
        let ctl = crate::classdb::fnv1a_lowercase("ControlRegion");
        let spawn = crate::classdb::fnv1a_lowercase("SpawnPath");
        let m: Manifest = ron::from_str(&format!(
            r#"(map: "s", source_game: "bf2", terrain_name: None, sky_name: None,
                terrain_glb: None, models: [], textures: [], classes: [], geometries: [], regions: [
                (shape: "sphere", name: "hangar", rotation: ((1,0,0),(0,1,0),(0,0,1)),
                 translation: (1,2,3), size: (5,5,5), props: []),
            ], instances: [
                (class: "com_bldg_controlzone_ctf", name: "cp", rotation: ((1,0,0),(0,1,0),(0,0,1)),
                 translation: (1,2,3), props: [({ctl}, "hangar"), ({spawn}, "hs")]),
                (class: "spa_prop_console", name: "prop", rotation: ((1,0,0),(0,1,0),(0,0,1)),
                 translation: (1,2,3), props: [({ctl}, "hangar"), ({spawn}, "hs")]),
            ])"#
        ))
        .unwrap();
        let layout = MapLayout::from_manifest(&m);
        assert_eq!(layout.cps.len(), 1);
        assert!(layout.cps[0].space);
        assert!(layout.cps[0].capture.is_some());
    }

    /// A mission's landing regions reach the sim's list by name; a name the map lacks
    /// falls out, and no other mission grows a list.
    #[test]
    fn landing_regions_come_through_the_mission() {
        let m: Manifest = ron::from_str(
            r#"(map: "s", source_game: "bf2", terrain_name: None, sky_name: None,
                terrain_glb: None, models: [], textures: [], classes: [], geometries: [], regions: [
                (shape: "sphere", name: "hangar", rotation: ((1,0,0),(0,1,0),(0,0,1)),
                 translation: (1,2,3), size: (5,5,5), props: []),
            ], instances: [])"#,
        )
        .unwrap();
        let mut layout = MapLayout::from_manifest(&m);
        let mission: MissionDef = ron::from_str(
            r#"(map: "s", mode: "ass", source_game: "bf2", teams: [
                (faction: "rep", units: 1, tickets: 1, classes: []),
                (faction: "cis", units: 1, tickets: 1, classes: [])],
                ai_alive_cap: 1, spawn_delay: 1.0, spawn_interval: 1.0, cps: [],
                death_regions: [], landing_regions: ["hangar", "not_here"])"#,
        )
        .unwrap();
        assert_eq!(layout.apply_mission(&mission).landing_regions.len(), 1);
    }
}

#[cfg(test)]
mod tests;
