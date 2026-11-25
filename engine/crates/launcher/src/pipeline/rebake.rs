//! Native Rust replacement for `scripts/rebake.sh`, `scripts/bake_dlc.py`,
//! and `scripts/global_sound_cache.py`.
//!
//! Invokes `bake` subcommands directly — zero Git Bash or shell dependency.

use super::missions::MaterializedMissions;
use super::scan::{ci_path, detect_dlc, detect_hero_sounds, BuildPlan, GameId, GameSource};
use super::ExecCtx;
use crate::config::Platform;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

const BF1_MAPS: &[&str] = &[
    "BES/BES1", "BES/BES2", "END/END1", "GEO/GEO1", "HOT/HOT1", "KAM/KAM1", "KAS/KAS1", "KAS/KAS2",
    "NAB/NAB1", "NAB/NAB2", "RHN/RHN1", "RHN/RHN2", "TAT/TAT2", "YAV/YAV1", "YAV/YAV2",
];

const BF2_MAPS: &[&str] = &[
    "COR/COR1", "DAG/DAG1", "DEA/DEA1", "END/END1", "FEL/FEL1", "GEO/GEO1", "HOT/HOT1", "KAM/KAM1",
    "KAS/KAS2", "MUS/MUS1", "MYG/MYG1", "NAB/NAB2", "POL/POL1", "TAN/TAN1", "TAT/TAT3", "UTA/UTA1",
    "YAV/YAV1",
];

const BF2_SPACE_MAPS: &[(&str, &str)] = &[
    ("SPA1", "yav"),
    ("SPA2", "cor"),
    ("SPA3", "kas"),
    ("SPA4", "mus"),
    ("SPA6", "myg"),
    ("SPA7", "fel"),
    ("SPA8", "hot"),
    ("SPA9", "tat"),
];

const BF2_COMBO_SIDES: &[&str] = &[
    "ALL", "IMP", "REP", "CIS", "TUR", "DES", "WOK", "GAM", "EWK", "GEO", "JED", "GAR", "GUN",
    "SNW",
];

/// Extracted/resolved LVL directories for a build.
pub struct BakeTrees {
    pub bf1: Option<PathBuf>,
    pub bf2: PathBuf,
    pub donors: DonorTrees,
    pub bf2_iso: Option<PathBuf>,
    pub lua_root: PathBuf,
    pub out_dir: PathBuf,
    pub temp_root: PathBuf,
}

impl BakeTrees {
    /// The games this build bakes with their LVL trees, Battlefront I first.
    pub fn games(&self) -> impl Iterator<Item = (GameId, &Path)> {
        let bf1 = self.bf1.as_deref().map(|tree| (GameId::Bf1, tree));
        bf1.into_iter().chain([(GameId::Bf2, self.bf2.as_path())])
    }
}

/// The LVL roots of the donor copies beside each game's primary: a PC
/// install's `_LVL_PC` and an extracted Xbox disc's `_LVL_XBOX`
/// (`rebake.sh`'s `PC1`, `PC2`, `XB1`, `XB2`).
#[derive(Debug, Default)]
pub struct DonorTrees {
    pub pc1: Option<PathBuf>,
    pub pc2: Option<PathBuf>,
    pub xb1: Option<PathBuf>,
    pub xb2: Option<PathBuf>,
}

impl DonorTrees {
    /// Record `tree` as `game`'s `platform` donor. A PS2 copy is always the
    /// primary, never a donor.
    pub fn set(&mut self, game: GameId, platform: Platform, tree: PathBuf) {
        let slot = match (game, platform) {
            (GameId::Bf1, Platform::Pc) => &mut self.pc1,
            (GameId::Bf2, Platform::Pc) => &mut self.pc2,
            (GameId::Bf1, Platform::Xbox) => &mut self.xb1,
            (GameId::Bf2, Platform::Xbox) => &mut self.xb2,
            (_, Platform::Ps2) => return,
        };
        *slot = Some(tree);
    }

    fn pc(&self, game: GameId) -> Option<&Path> {
        match game {
            GameId::Bf1 => self.pc1.as_deref(),
            GameId::Bf2 => self.pc2.as_deref(),
        }
    }

    fn xbox(&self, game: GameId) -> Option<&Path> {
        match game {
            GameId::Bf1 => self.xb1.as_deref(),
            GameId::Bf2 => self.xb2.as_deref(),
        }
    }

    /// `game`'s donor roots, the PC's first.
    fn roots(&self, game: GameId) -> impl Iterator<Item = &Path> {
        self.pc(game).into_iter().chain(self.xbox(game))
    }

    /// Every donor copy of the LVL at `rel` that exists, comma-joined as one
    /// argument, the PC's first (`rebake.sh` `map_donors` / `side_donors`);
    /// `None` when there is none.
    fn lvl(&self, game: GameId, rel: &str) -> Option<String> {
        let found: Vec<String> = self
            .roots(game)
            .map(|root| ci_path(root, rel))
            .filter(|p| p.is_file())
            .map(|p| s(&p))
            .collect();
        (!found.is_empty()).then(|| found.join(","))
    }

    /// The donor copies of `game`'s CORE.LVL (`rebake.sh` `FONT_DONORS`).
    fn cores(&self, game: GameId) -> Vec<String> {
        self.roots(game)
            .map(|root| ci_path(root, "CORE.LVL"))
            .filter(|p| p.is_file())
            .map(|p| s(&p))
            .collect()
    }

    /// `--donor <root>` for each donor root (`rebake.sh` `fpm_donors`).
    fn fpm_args(&self, game: GameId) -> Vec<String> {
        self.roots(game)
            .filter(|root| root.is_dir())
            .flat_map(|root| ["--donor".to_owned(), s(root)])
            .collect()
    }

    /// The PC donor's `SOUND/COMMON.BNK`, whose own recordings replace the
    /// samples a console disc aliases (`rebake.sh` `donor_bnk`).
    fn sound_bank(&self, game: GameId) -> Option<PathBuf> {
        self.pc(game)
            .map(|root| ci_path(root, "SOUND/COMMON.BNK"))
            .filter(|p| p.is_file())
    }
}

/// `args` with `donor` appended when there is one.
fn with_donor(mut args: Vec<String>, donor: Option<String>) -> Vec<String> {
    args.extend(donor);
    args
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// The entries of `dir` in sorted order (none when it cannot be read).
fn sorted_entries(dir: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    entries.sort();
    entries
}

/// The `*.lvl` files directly in `dir`, sorted.
fn lvl_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = sorted_entries(dir);
    files.retain(|p| {
        p.is_file()
            && p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("lvl"))
    });
    files
}

/// Run the full asset bake (equivalent to `rebake.sh` across every active group).
pub fn run_all_bake_groups(
    ctx: &ExecCtx,
    plan: &BuildPlan,
    trees: &BakeTrees,
    missions: &MaterializedMissions,
) -> Result<()> {
    std::fs::create_dir_all(&trees.out_dir)
        .with_context(|| format!("creating {}", trees.out_dir.display()))?;

    bake_tat(ctx, trees)?;
    bake_common(ctx, plan, trees, missions)?;
    bake_sides(ctx, trees)?;
    bake_dlc(ctx, plan, trees)?;
    bake_classes(ctx, trees)?;
    bake_sounds(ctx, trees)?;
    bake_global_sound(ctx, trees)?;
    bake_load_screens(ctx, trees)?;
    bake_maps(ctx, trees)?;
    bake_beep(ctx, trees)?;
    check_foliage(ctx, plan, trees)?;
    bake_stock_catalog(ctx, plan, trees)?;
    Ok(())
}

fn bake_tat(ctx: &ExecCtx, trees: &BakeTrees) -> Result<()> {
    if let Some(b1) = &trees.bf1 {
        ctx.log("== TAT1 (BF1) ==");
        ctx.run_bake(
            &with_donor(
                vec![
                    "map".into(),
                    s(&ci_path(b1, "TAT/TAT1.LVL")),
                    s(&trees.out_dir.join("tat1")),
                ],
                trees.donors.lvl(GameId::Bf1, "TAT/TAT1.LVL"),
            ),
            &[],
        )?;
    }
    ctx.log("== TAT2 (BF2) ==");
    ctx.run_bake(
        &with_donor(
            vec![
                "map".into(),
                s(&ci_path(&trees.bf2, "TAT/TAT2.LVL")),
                s(&trees.out_dir.join("tat2")),
            ],
            trees.donors.lvl(GameId::Bf2, "TAT/TAT2.LVL"),
        ),
        &[],
    )
}

fn bake_common(
    ctx: &ExecCtx,
    plan: &BuildPlan,
    trees: &BakeTrees,
    missions: &MaterializedMissions,
) -> Result<()> {
    ctx.log("== common assets ==");
    let out1 = trees.out_dir.join("common_bf1");
    let out2 = trees.out_dir.join("common_bf2");
    if let Some(b1) = &trees.bf1 {
        ctx.run_bake(
            &[
                "common".into(),
                s(&ci_path(b1, "COMMON/ENG/COMMON.LVL")),
                s(&out1),
            ],
            &[],
        )?;
    }
    ctx.run_bake(
        &[
            "common".into(),
            s(&ci_path(&trees.bf2, "INGAME.LVL")),
            s(&out2),
        ],
        &[],
    )?;
    // Fonts keep the primary's layout metrics; donors lend matching glyph art
    // and larger same-art UI sheets (`rebake.sh` FONT_DONORS).
    let core2 = s(&ci_path(&trees.bf2, "CORE.LVL"));
    let font_donors2 = trees.donors.cores(GameId::Bf2);
    if !font_donors2.is_empty() {
        let mut fonts = vec!["fonts".into(), core2.clone(), s(&out2), String::new()];
        fonts.extend(font_donors2.iter().cloned());
        ctx.run_bake(&fonts, &[])?;
        let mut art = vec!["ui-art".into(), core2, s(&out2)];
        art.extend(font_donors2);
        ctx.run_bake(&art, &[])?;
    }

    if let Some(b1) = &trees.bf1 {
        ctx.run_bake(
            &["specular".into(), s(&ci_path(b1, "CORE.LVL")), s(&out1)],
            &[],
        )?;
    }
    ctx.run_bake(
        &[
            "specular".into(),
            s(&ci_path(&trees.bf2, "CORE.LVL")),
            s(&out2),
        ],
        &[],
    )?;
    ctx.run_bake(
        &[
            "omniproj".into(),
            s(&ci_path(&trees.bf2, "CORE.LVL")),
            s(&out2),
        ],
        &[],
    )?;
    // The PC bump materials' specular cube, from a PC BF2 install's own core
    // (`bake::bump`, analysis/re-pc-bump.md); without one the pass draws nothing.
    let pc_core2 = trees
        .donors
        .pc(GameId::Bf2)
        .map(|d| ci_path(d, "CORE.LVL"))
        .filter(|p| p.is_file())
        .map(|p| s(&p))
        .unwrap_or_default();
    ctx.run_bake(&["pc-spec-cube".into(), pc_core2, s(&out2)], &[])?;
    if let Some(b1) = &trees.bf1 {
        ctx.run_bake(&["locl".into(), s(&ci_path(b1, "CORE.LVL")), s(&out1)], &[])?;
    }
    ctx.run_bake(
        &["locl".into(), s(&ci_path(&trees.bf2, "CORE.LVL")), s(&out2)],
        &[],
    )?;

    let xbox_bf2 = if matches!(plan.bf2, GameSource::XboxIso(_)) {
        Some(trees.bf2.as_path())
    } else {
        trees.donors.xbox(GameId::Bf2)
    };
    if let Some(xbox) = xbox_bf2 {
        let xbox_common = ci_path(xbox, "common.lvl");
        if xbox_common.is_file() {
            ctx.run_bake(&["xbox-icons".into(), s(&xbox_common), s(&out2)], &[])?;
        }
    }

    if let Some(b1) = &trees.bf1 {
        let mut fonts = vec![
            "fonts".into(),
            s(&ci_path(b1, "CORE.LVL")),
            s(&out1),
            "bf1_".into(),
        ];
        fonts.extend(trees.donors.cores(GameId::Bf1));
        ctx.run_bake(&fonts, &[])?;
    }

    // Shell movies + campaign cutscenes/crawls.
    let mvs_file = ci_path(&trees.bf2, "MOVIES/SHELL.MVS");
    let mvs = if mvs_file.is_file() {
        mvs_file
    } else if let Some(iso) = &trees.bf2_iso {
        iso.clone()
    } else {
        bail!(
            "common needs MOVIES/SHELL.MVS: not found under {}",
            trees.bf2.display()
        );
    };

    let mut camp_movies = Vec::new();
    for rel in ["MOVIES/INGAME.MVS", "MOVIES/CRAWL.MVS"] {
        let f = ci_path(&trees.bf2, rel);
        if f.is_file() {
            camp_movies.push(f);
        }
    }
    if camp_movies.is_empty() {
        if let Some(iso) = &trees.bf2_iso {
            if iso.is_file() {
                camp_movies.push(iso.clone());
            }
        }
    }

    let shell_lvl = ci_path(&trees.bf2, "SHELL.LVL");
    let inshell = ci_path(&trees.bf2, "INSHELL.LVL");
    let mut shell_args = vec![
        "shell".into(),
        s(&shell_lvl),
        s(&mvs),
        s(&ci_path(&trees.bf2, "SOUND/SHELL.LVL")),
        s(&ci_path(&trees.bf2, "CORE.LVL")),
        s(&out2),
    ];
    if inshell.is_file() && !camp_movies.is_empty() {
        shell_args.push("--campaign".into());
        shell_args.push(s(&inshell));
        for cm in &camp_movies {
            shell_args.push(s(cm));
        }
    }
    ctx.run_bake(&shell_args, &[])?;

    if !matches!(plan.bf2, GameSource::PcLvlDir(_)) {
        missions.generate_shell_credits(
            ctx,
            &shell_lvl,
            &out2.join("shell").join("credits.json"),
        )?;
    }

    // Rise of the Empire 3D planet gallery.
    let gal_lvl = ci_path(&trees.bf2, "GAL/GAL1.LVL");
    if gal_lvl.is_file() {
        let gal_out = trees.out_dir.join("bf2_gal1");
        ctx.run_bake(
            &with_donor(
                vec!["map".into(), s(&gal_lvl), s(&gal_out)],
                trees.donors.lvl(GameId::Bf2, "GAL/GAL1.LVL"),
            ),
            &[],
        )?;
        let gal_snd = ci_path(&trees.bf2, "SOUND/GAL.LVL");
        if gal_snd.is_file() {
            ctx.run_bake(
                &[
                    "sounds".into(),
                    s(&gal_snd),
                    s(&gal_out),
                    s(&ci_path(&trees.bf2, "CORE.LVL")),
                ],
                &[],
            )?;
        }
    }
    Ok(())
}

fn bake_sides(ctx: &ExecCtx, trees: &BakeTrees) -> Result<()> {
    ctx.log("== soldiers + weapons + vehicles ==");
    let a2 = ci_path(&trees.bf2, "INGAME.LVL");
    let a2_str = s(&a2);
    let a1 = trees
        .bf1
        .as_ref()
        .map(|b1| ci_path(b1, "COMMON/ENG/COMMON.LVL"));
    let a1_str = a1.as_ref().map(|p| s(p));

    let side1_out = trees.out_dir.join("side_bf1");
    let side2_out = trees.out_dir.join("side_bf2");

    // BF1 playable soldiers (borrow BF2 throw clips via BF2_ANIMS).
    if let (Some(b1), Some(a1s)) = (&trees.bf1, &a1_str) {
        let env = [("BF2_ANIMS", Some(a2_str.as_str())), ("BF1_ANIMS", None)];
        for (lvl, bodies) in [
            ("SIDE/ALL.LVL", "all_inf_marksperson,all_inf_marksperson_snow,all_inf_pilot,all_inf_snowtrooper,all_inf_soldier,all_inf_trooperfleet,all_inf_wookie"),
            ("SIDE/IMP.LVL", "imp_inf_atatpilot,imp_inf_atstpilot,imp_inf_darktrooper,imp_inf_pilot,imp_inf_scout,imp_inf_shocktrooper,imp_inf_snowtrooper,imp_inf_stormtrooper"),
            ("SIDE/REP.LVL", "rep_inf_arctrooper,rep_inf_jettrooper,rep_inf_pilot,rep_inf_trooper"),
            ("SIDE/CIS.LVL", "cis_inf_bdroid,cis_inf_sbdroid"),
        ] {
            ctx.run_bake(
                &with_donor(
                    vec![
                        "soldier".into(),
                        s(&ci_path(b1, lvl)),
                        bodies.into(),
                        s(&side1_out),
                        a1s.clone(),
                    ],
                    trees.donors.lvl(GameId::Bf1, lvl),
                ),
                &env,
            )?;
        }
    }

    // BF2 playable soldiers (borrow BF1 prone clips when BF1 is present).
    let bf2_prone_env: [(&str, Option<&str>); 2] =
        [("BF1_ANIMS", a1_str.as_deref()), ("BF2_ANIMS", None)];
    for (lvl, bodies) in [
        ("SIDE/REP.LVL", "rep_inf_arctrooper,rep_inf_clonecommander,rep_inf_ep3armoredpilot,rep_inf_ep3heavytrooper,rep_inf_ep3jettrooper,rep_inf_ep3sniper,rep_inf_ep3spacepilot,rep_inf_ep3trooper,rep_inf_feluciatrooper,rep_inf_jettrooper,rep_inf_pilot,rep_inf_trooper"),
        ("SIDE/CIS.LVL", "cis_inf_bdroid,cis_inf_magnaguard,cis_inf_sbdroid"),
        ("SIDE/ALL.LVL", "all_inf_bothanspy,all_inf_engineer,all_inf_marksperson,all_inf_markspersonsnow,all_inf_pilot,all_inf_snowtrooper,all_inf_soldier,all_inf_tantive4trooper,all_inf_vanguard,all_inf_wookie"),
        ("SIDE/IMP.LVL", "imp_inf_atatcommander,imp_inf_atatpilot,imp_inf_darktrooper,imp_inf_gunner,imp_inf_pilot,imp_inf_scout,imp_inf_shocktrooper,imp_inf_snowtrooper,imp_inf_stormtrooper"),
    ] {
        ctx.run_bake(
            &with_donor(
                vec![
                    "soldier".into(),
                    s(&ci_path(&trees.bf2, lvl)),
                    bodies.into(),
                    s(&side2_out),
                    a2_str.clone(),
                ],
                trees.donors.lvl(GameId::Bf2, lvl),
            ),
            &bf2_prone_env,
        )?;
    }

    // Native sides (locals).
    if let (Some(b1), Some(a1s)) = (&trees.bf1, &a1_str) {
        let loc1 = side1_out.join("locals");
        let env = [("BF2_ANIMS", Some(a2_str.as_str())), ("BF1_ANIMS", None)];
        for (side, bodies, weapons) in [
            ("ewk", "ewk_inf,ewk_inf2", "ewk_rock,ewk_spear"),
            ("wok", "wok_inf_wookie", "wok_weap_inf_bowcaster,wok_weap_inf_fusioncutter,wok_weap_inf_launcher"),
            ("gun", "nab_inf_gungan", "gun_weap_inf_grenadethermal,gun_weap_inf_fusioncutter"),
            ("gar", "gar_inf_nabooguard", "gar_weap_inf_rifle,gar_weap_inf_pistol,gar_weap_inf_fusioncutter,gar_weap_inf_launcher,gar_weap_inf_thermaldetonator"),
            ("geo", "gen_inf_geonosian", "geo_weap_inf_blunderbuss"),
            ("des", "tat_inf_jawa,tat_inf_tusken", "tat_weap_tuskrifle"),
        ] {
            let lvl = s(&ci_path(b1, &format!("SIDE/{side}.LVL")));
            ctx.run_bake(&["soldier".into(), lvl.clone(), bodies.into(), s(&loc1), a1s.clone()], &env)?;
            ctx.run_bake(&["weapon".into(), lvl, weapons.into(), s(&loc1)], &[])?;
        }
    }

    let loc2 = side2_out.join("locals");
    for (side, bodies, weapons) in [
        ("ewk", "ewk_inf,ewk_inf2", "ewk_rock,ewk_spear,ewk_weap_inf_fusioncutter"),
        ("wok", "wok_inf_wookiewarrior", "wok_weap_inf_bowcaster,wok_weap_inf_fusioncutter,wok_weap_inf_launcher,wok_weap_inf_thermaldetonator"),
        ("geo", "gen_inf_geonosian,geo_inf_acklay", "geo_weap_inf_blunderbuss,ack_weap_r_smash,ack_weap_l_smash"),
        ("jed", "jed_inf_jedi,jed_inf_sith", "all_weap_inf_lightsabre,rep_weap_aalya_lightsabre,rep_weap_inf_lightsabre,cis_weap_inf_doublesabre"),
        ("gar", "gar_inf_nabooqueen,gar_inf_nabooguard,gar_inf_jediguard", "gar_weap_inf_rifle,gar_weap_inf_pistol,gar_weap_inf_launcher,gar_weap_inf_thermaldetonator"),
        ("gun", "nab_inf_gungan", "gun_weap_inf_grenadethermal,gun_weap_inf_fusioncutter"),
        ("gam", "gam_inf_gamorreanguard", "gam_inf_weap_axe"),
        ("snw", "hot_inf_wampa", "hot_inf_weap_l_smash,hot_inf_weap_r_smash,hot_inf_weap_head_smash"),
        ("des", "tat_inf_jawa,tat_inf_tusken", "tat_weap_tuskrifle,des_weap_inf_fusioncutter"),
    ] {
        let lvl = s(&ci_path(&trees.bf2, &format!("SIDE/{side}.LVL")));
        ctx.run_bake(&["soldier".into(), lvl.clone(), bodies.into(), s(&loc2), a2_str.clone()], &bf2_prone_env)?;
        ctx.run_bake(&["weapon".into(), lvl, weapons.into(), s(&loc2)], &[])?;
    }

    // BF2 heroes & combos.
    ctx.run_bake(
        &["heroes".into(), s(&trees.bf2), s(&side2_out)],
        &bf2_prone_env,
    )?;
    let combos_dir = side2_out.join("combos");
    std::fs::create_dir_all(&combos_dir)?;
    for side in BF2_COMBO_SIDES {
        let lower = side.to_ascii_lowercase();
        ctx.run_bake(
            &[
                "combos".into(),
                s(&ci_path(&trees.bf2, &format!("SIDE/{side}.LVL"))),
                s(&combos_dir.join(format!("{lower}.ron"))),
            ],
            &[],
        )?;
    }

    // Third-person weapons.
    if let Some(b1) = &trees.bf1 {
        for (lvl, weaps) in [
            ("SIDE/ALL.LVL", "all_weap_inf_bowcaster,all_weap_inf_fusioncutter,all_weap_inf_launcher,all_weap_inf_pistol,all_weap_inf_rifle,all_weap_inf_seeker,all_weap_inf_sniperrifle"),
            ("SIDE/IMP.LVL", "imp_weap_inf_fusioncutter,imp_weap_inf_launcher,imp_weap_inf_pistol,imp_weap_inf_rifle,imp_weap_inf_sniperrifle"),
            ("SIDE/REP.LVL", "rep_weap_inf_fusioncutter,rep_weap_inf_launcher,rep_weap_inf_pistol,rep_weap_inf_rifle,rep_weap_inf_sniperrifle,rep_weap_inf_landmine"),
            ("SIDE/CIS.LVL", "cis_weap_inf_fusioncutter,cis_weap_inf_launcher,cis_weap_inf_pistol,cis_weap_inf_sniperrifle"),
        ] {
            ctx.run_bake(
                &with_donor(
                    vec!["weapon".into(), s(&ci_path(b1, lvl)), weaps.into(), s(&side1_out)],
                    trees.donors.lvl(GameId::Bf1, lvl),
                ),
                &[],
            )?;
        }
    }

    for (lvl, weaps) in [
        ("SIDE/REP.LVL", "rep_weap_inf_chaingun,rep_weap_inf_fusioncutter,rep_weap_inf_launcher,rep_weap_inf_pistol,rep_weap_inf_rifle,rep_weap_inf_sniperrifle"),
        ("SIDE/CIS.LVL", "cis_weap_inf_fusioncutter,cis_weap_inf_launcher,cis_weap_inf_pistol,cis_weap_inf_rifle,cis_weap_inf_sniperrifle,hud_cis_wristblaster,hud_cis_trishot,hud_cis_wristrocket,hud_droideka_shield"),
    ] {
        ctx.run_bake(
            &with_donor(
                vec!["weapon".into(), s(&ci_path(&trees.bf2, lvl)), weaps.into(), s(&side2_out)],
                trees.donors.lvl(GameId::Bf2, lvl),
            ),
            &[],
        )?;
    }

    let cis2_donor = trees.donors.lvl(GameId::Bf2, "SIDE/CIS.LVL");
    if let (Some(b1), Some(a1s)) = (&trees.bf1, &a1_str) {
        ctx.run_bake(
            &with_donor(
                vec![
                    "droid-chunks".into(),
                    s(&ci_path(b1, "SIDE/CIS.LVL")),
                    s(&ci_path(&trees.bf2, "SIDE/CIS.LVL")),
                    a1s.clone(),
                    s(&trees.out_dir),
                ],
                cis2_donor,
            ),
            &[],
        )?;
    } else {
        ctx.run_bake(
            &with_donor(
                vec![
                    "droid-chunks".into(),
                    "--bf2-only".into(),
                    s(&ci_path(&trees.bf2, "SIDE/CIS.LVL")),
                    s(&trees.out_dir),
                ],
                cis2_donor,
            ),
            &[],
        )?;
    }

    for (lvl, weaps) in [
        ("SIDE/ALL.LVL", "all_weap_inf_bowcaster,all_weap_inf_fusioncutter,all_weap_inf_launcher,all_weap_inf_pistol,all_weap_inf_rifle,all_weap_inf_seeker,all_weap_inf_sniperrifle"),
        ("SIDE/IMP.LVL", "imp_weap_inf_fusioncutter,imp_weap_inf_launcher,imp_weap_inf_pistol,imp_weap_inf_rifle,imp_weap_inf_sniperrifle"),
    ] {
        ctx.run_bake(
            &with_donor(
                vec!["weapon".into(), s(&ci_path(&trees.bf2, lvl)), weaps.into(), s(&side2_out)],
                trees.donors.lvl(GameId::Bf2, lvl),
            ),
            &[],
        )?;
    }

    if let Some(a1s) = &a1_str {
        ctx.run_bake(&[
                "weapon".into(),
                a1s.clone(),
                "com_weap_inf_torpedo,com_weap_inf_grenadethermal,com_weap_inf_grenadethermal_ord,com_weap_inf_landmine,com_weap_inf_landmine_placed,com_weap_inf_seismic_mine".into(),
                s(&side1_out),
            ],
            &[],
        )?;
    }
    ctx.run_bake(&[
            "weapon".into(),
            a2_str.clone(),
            "com_weap_inf_torpedo,com_weap_inf_grenadethermal,com_weap_inf_grenadethermal_ord,com_weap_inf_landmine,com_weap_inf_landmine_placed,com_weap_inf_seismic_mine,com_weap_inf_detpack,com_weap_missile".into(),
            s(&side2_out),
        ],
        &[],
    )?;
    ctx.run_bake(
        &[
            "weapon".into(),
            format!("{a2_str},{}", s(&ci_path(&trees.bf2, "SIDE/REP.LVL"))),
            "com_weap_inf_dropturret".into(),
            s(&side2_out),
        ],
        &[],
    )?;

    // First-person cockpits (FPM).
    if let (Some(b1), Some(a1s)) = (&trees.bf1, &a1_str) {
        let mut args = vec!["fpm".into(), a1s.clone(), s(&side1_out)];
        args.extend(trees.donors.fpm_args(GameId::Bf1));
        args.extend(collect_fpm_lvls(b1));
        ctx.run_bake(&args, &[])?;
    }
    {
        let mut args = vec!["fpm".into(), a2_str.clone(), s(&side2_out)];
        args.extend(trees.donors.fpm_args(GameId::Bf2));
        args.extend(collect_fpm_lvls(&trees.bf2));
        ctx.run_bake(&args, &[])?;
    }

    // First-person weapons.
    for (game, root) in trees.games() {
        let out = match game {
            GameId::Bf1 => &side1_out,
            GameId::Bf2 => &side2_out,
        };
        for side in ["all", "imp", "rep", "cis"] {
            let mut items = vec!["fusioncutter", "launcher", "pistol", "rifle", "sniperrifle"];
            if side == "all" {
                items.extend(["bowcaster", "seeker"]);
            }
            if side == "rep" {
                items.push(match game {
                    GameId::Bf1 => "landmine",
                    GameId::Bf2 => "chaingun",
                });
            }
            let joined = items
                .iter()
                .map(|w| format!("{side}_1st_weap_inf_{w}"))
                .collect::<Vec<_>>()
                .join(",");
            let lvl = ci_path(root, &format!("SIDE/{}.LVL", side.to_ascii_uppercase()));
            ctx.run_bake(&["weapon".into(), s(&lvl), joined, s(out)], &[])?;
        }
    }
    if let Some(a1s) = &a1_str {
        ctx.run_bake(&[
                "weapon".into(),
                a1s.clone(),
                "com_1st_weap_inf_grenadethermal,com_1st_weap_inf_torpedo,com_1st_weap_inf_landmine".into(),
                s(&side1_out),
            ],
            &[],
        )?;
    }
    ctx.run_bake(
        &[
            "weapon".into(),
            a2_str.clone(),
            "com_1st_weap_inf_grenadethermal,com_1st_weap_inf_torpedo,com_1st_weap_inf_landmine"
                .into(),
            s(&side2_out),
        ],
        &[],
    )?;

    if let Some(pc) = trees.donors.pc(GameId::Bf2) {
        ctx.run_bake(
            &[
                "pc-first-person".into(),
                s(pc),
                s(&side2_out.join("pc_first_person")),
            ],
            &[],
        )?;
    }

    // Vehicles.
    if let Some(b1) = &trees.bf1 {
        for (lvl, name) in [
            ("SIDE/ALL.LVL", "all"),
            ("SIDE/CIS.LVL", "cis"),
            ("SIDE/IMP.LVL", "imp"),
            ("SIDE/REP.LVL", "rep"),
            ("SIDE/GUN.LVL", "gun"),
            ("TAT/TAT1.LVL", "tat1"),
        ] {
            // `rebake.sh` passes the side donors to the four sides only, not GUN or TAT1.
            let donor = matches!(name, "all" | "cis" | "imp" | "rep")
                .then(|| trees.donors.lvl(GameId::Bf1, lvl))
                .flatten();
            ctx.run_bake(
                &with_donor(
                    vec![
                        "vehicle".into(),
                        s(&ci_path(b1, lvl)),
                        s(&side1_out.join("vehicles").join(name)),
                    ],
                    donor,
                ),
                &[],
            )?;
        }
    }
    for (lvl, name) in [
        ("SIDE/ALL.LVL", "all"),
        ("SIDE/IMP.LVL", "imp"),
        ("SIDE/REP.LVL", "rep"),
        ("SIDE/CIS.LVL", "cis"),
    ] {
        ctx.run_bake(
            &with_donor(
                vec![
                    "vehicle".into(),
                    s(&ci_path(&trees.bf2, lvl)),
                    s(&side2_out.join("vehicles").join(name)),
                ],
                trees.donors.lvl(GameId::Bf2, lvl),
            ),
            &[],
        )?;
    }

    // Map icons.
    if let Some(b1) = &trees.bf1 {
        ctx.run_bake(
            &[
                "map-icons".into(),
                s(&side1_out),
                s(&ci_path(b1, "SIDE/ALL.LVL")),
                s(&ci_path(b1, "SIDE/IMP.LVL")),
                s(&ci_path(b1, "SIDE/REP.LVL")),
                s(&ci_path(b1, "SIDE/CIS.LVL")),
                s(&ci_path(b1, "SIDE/GUN.LVL")),
            ],
            &[],
        )?;
    }
    ctx.run_bake(
        &[
            "map-icons".into(),
            s(&side2_out),
            s(&ci_path(&trees.bf2, "SIDE/ALL.LVL")),
            s(&ci_path(&trees.bf2, "SIDE/IMP.LVL")),
            s(&ci_path(&trees.bf2, "SIDE/REP.LVL")),
            s(&ci_path(&trees.bf2, "SIDE/CIS.LVL")),
            s(&ci_path(&trees.bf2, "SIDE/TUR.LVL")),
        ],
        &[],
    )?;

    // Side particle effects.
    if let Some(b1) = &trees.bf1 {
        bake_side_effects(ctx, b1, &side1_out, "bf1")?;
    }
    bake_side_effects(ctx, &trees.bf2, &side2_out, "bf2")?;

    Ok(())
}

fn collect_fpm_lvls(root: &Path) -> Vec<String> {
    sorted_entries(&ci_path(root, "FPM"))
        .into_iter()
        .filter(|dir| dir.is_dir())
        .flat_map(|dir| lvl_files(&dir))
        .map(|p| s(&p))
        .collect()
}

fn bake_side_effects(ctx: &ExecCtx, root: &Path, side_out: &Path, game: &str) -> Result<()> {
    for f in lvl_files(&ci_path(root, "SIDE")) {
        let stem = f
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        ctx.run_bake(
            &[
                "effects".into(),
                s(&f),
                s(&side_out.join("effects").join(stem)),
                game.into(),
            ],
            &[],
        )?;
    }
    Ok(())
}

fn bake_dlc(ctx: &ExecCtx, plan: &BuildPlan, trees: &BakeTrees) -> Result<()> {
    let Some(dlc_dir) = &plan.dlc_dir else {
        return Ok(());
    };
    let mut candidates = detect_dlc(dlc_dir);
    if candidates.is_empty() {
        ctx.log("DLC heroes absent in selected folder; skipping.");
        return Ok(());
    }
    ctx.log("== DLC heroes ==");
    candidates.sort_by(|a, b| {
        (a.platform != "ps2", &a.path, &a.member, a.offset).cmp(&(
            b.platform != "ps2",
            &b.path,
            &b.member,
            b.offset,
        ))
    });
    let chosen = &candidates[0];
    let pc_donor = if chosen.platform == "ps2" {
        candidates.iter().find(|c| c.platform == "pc")
    } else {
        None
    };

    let stage = trees.temp_root.join("dlc_stage");
    std::fs::create_dir_all(&stage)?;
    let lvl_path = stage.join("heroes.lvl");
    std::fs::write(&lvl_path, chosen.read_bytes()?)?;

    let ingame = ci_path(&trees.bf2, "INGAME.LVL");
    let mut args = vec![
        "dlc-heroes".into(),
        s(&lvl_path),
        s(&ingame),
        s(&trees.out_dir),
    ];
    if let Some(pc) = pc_donor {
        let donor_path = stage.join("pc-heroes.lvl");
        std::fs::write(&donor_path, pc.read_bytes()?)?;
        args.push(s(&donor_path));
    }
    ctx.run_bake(&args, &[])?;

    if let Some(bank) = detect_hero_sounds(dlc_dir) {
        let core = ci_path(&trees.bf2, "CORE.LVL");
        if core.is_file() {
            if let Err(e) = ctx.run_bake(
                &["dlc-sounds".into(), s(&bank), s(&core), s(&trees.out_dir)],
                &[],
            ) {
                ctx.log(&format!(
                    "DLC heroes: sound bank skipped ({e}); they use the game's sounds"
                ));
            }
        }
    } else {
        ctx.log("DLC heroes: no sound bank found; they use the game's sounds");
    }
    Ok(())
}

fn bake_classes(ctx: &ExecCtx, trees: &BakeTrees) -> Result<()> {
    ctx.log("== entity/weapon classes ==");
    let classes_dir = trees.out_dir.join("classes");
    std::fs::create_dir_all(&classes_dir)?;
    if let Some(b1) = &trees.bf1 {
        for (rel, out) in [
            ("SIDE/ALL.LVL", "bf1_all.ron"),
            ("SIDE/IMP.LVL", "bf1_imp.ron"),
            ("COMMON/ENG/COMMON.LVL", "bf1_common.ron"),
            ("SIDE/REP.LVL", "bf1_rep.ron"),
            ("SIDE/CIS.LVL", "bf1_cis.ron"),
            ("SIDE/ewk.LVL", "bf1_ewk.ron"),
            ("SIDE/wok.LVL", "bf1_wok.ron"),
            ("SIDE/geo.LVL", "bf1_geo.ron"),
            ("SIDE/des.LVL", "bf1_des.ron"),
            ("SIDE/gun.LVL", "bf1_gun.ron"),
            ("SIDE/gar.LVL", "bf1_gar.ron"),
        ] {
            ctx.run_bake(
                &[
                    "classes".into(),
                    s(&ci_path(b1, rel)),
                    s(&classes_dir.join(out)),
                ],
                &[],
            )?;
        }
    }
    for (rel, out) in [
        ("SIDE/REP.LVL", "bf2_rep.ron"),
        ("SIDE/CIS.LVL", "bf2_cis.ron"),
        ("INGAME.LVL", "bf2_ingame.ron"),
        ("SIDE/ALL.LVL", "bf2_all.ron"),
        ("SIDE/IMP.LVL", "bf2_imp.ron"),
        ("SIDE/ewk.LVL", "bf2_ewk.ron"),
        ("SIDE/wok.LVL", "bf2_wok.ron"),
        ("SIDE/geo.LVL", "bf2_geo.ron"),
        ("SIDE/des.LVL", "bf2_des.ron"),
        ("SIDE/gun.LVL", "bf2_gun.ron"),
        ("SIDE/snw.LVL", "bf2_snw.ron"),
        ("SIDE/gam.LVL", "bf2_gam.ron"),
        ("SIDE/jed.LVL", "bf2_jed.ron"),
        ("SIDE/gar.LVL", "bf2_gar.ron"),
        ("SIDE/TUR.LVL", "bf2_tur.ron"),
    ] {
        ctx.run_bake(
            &[
                "classes".into(),
                s(&ci_path(&trees.bf2, rel)),
                s(&classes_dir.join(out)),
            ],
            &[],
        )?;
    }
    Ok(())
}

fn bake_map_sound(
    ctx: &ExecCtx,
    sound_root: &Path,
    out_dir: &Path,
    dir_name: &str,
    short: &str,
    core: &Path,
    donor_bnk: Option<&Path>,
) -> Result<()> {
    let planet = short[..3.min(short.len())].to_ascii_uppercase();
    let sf = ci_path(sound_root, &format!("SOUND/{planet}.LVL"));
    if sf.is_file() {
        let mut args = vec!["sounds".into(), s(&sf), s(&out_dir.join(dir_name)), s(core)];
        if let Some(bnk) = donor_bnk {
            args.extend(["--donor-bnk".to_owned(), s(bnk)]);
        }
        ctx.run_bake(&args, &[])?;
    }
    Ok(())
}

fn bake_sounds(ctx: &ExecCtx, trees: &BakeTrees) -> Result<()> {
    ctx.log("== map sound banks ==");
    let d1 = trees.donors.sound_bank(GameId::Bf1);
    let d2 = trees.donors.sound_bank(GameId::Bf2);
    if let Some(b1) = &trees.bf1 {
        let c1 = ci_path(b1, "CORE.LVL");
        bake_map_sound(ctx, b1, &trees.out_dir, "tat1", "tat1", &c1, d1.as_deref())?;
        for map in BF1_MAPS {
            let short = map.rsplit('/').next().unwrap().to_ascii_lowercase();
            bake_map_sound(
                ctx,
                b1,
                &trees.out_dir,
                &format!("bf1_{short}"),
                &short,
                &c1,
                d1.as_deref(),
            )?;
        }
    }
    let c2 = ci_path(&trees.bf2, "CORE.LVL");
    bake_map_sound(ctx, &trees.bf2, &trees.out_dir, "tat2", "tat2", &c2, d2.as_deref())?;
    for map in BF2_MAPS {
        let short = map.rsplit('/').next().unwrap().to_ascii_lowercase();
        bake_map_sound(
            ctx,
            &trees.bf2,
            &trees.out_dir,
            &format!("bf2_{short}"),
            &short,
            &c2,
            d2.as_deref(),
        )?;
    }
    for (spa, _) in BF2_SPACE_MAPS {
        if ci_path(&trees.bf2, &format!("SPA/{spa}.LVL")).is_file() {
            let short = spa.to_ascii_lowercase();
            bake_map_sound(
                ctx,
                &trees.bf2,
                &trees.out_dir,
                &format!("bf2_{short}"),
                &short,
                &c2,
                None,
            )?;
        }
    }

    ctx.log("== resident global banks (CORE.LVL) ==");
    if let Some(b1) = &trees.bf1 {
        ctx.run_bake(
            &[
                "sounds".into(),
                s(&ci_path(b1, "CORE.LVL")),
                s(&trees.out_dir.join("core_bf1")),
            ],
            &[],
        )?;
    }
    ctx.run_bake(
        &["sounds".into(), s(&c2), s(&trees.out_dir.join("core_bf2"))],
        &[],
    )?;

    ctx.log("== sound names ==");
    let lua_paths = if trees.bf1.is_some() {
        format!(
            "{},{}",
            s(&trees.lua_root.join("bf1")),
            s(&trees.lua_root.join("bf2"))
        )
    } else {
        s(&trees.lua_root.join("bf2"))
    };
    let mut args = vec![
        "sound-names".into(),
        s(&trees.out_dir.join("classes")),
        lua_paths,
    ];
    args.extend(
        sorted_entries(&trees.out_dir)
            .into_iter()
            .filter(|d| d.join("sounds").is_dir())
            .map(|d| s(&d)),
    );
    ctx.run_bake(&args, &[])
}

fn bake_global_sound(ctx: &ExecCtx, trees: &BakeTrees) -> Result<()> {
    ctx.log("== global VO streams ==");
    let audio_dir = trees.out_dir.join("audio");
    std::fs::create_dir_all(&audio_dir)?;

    for (game, root) in trees.games() {
        let sound_dir = ci_path(root, "SOUND");
        let (names, excluded): (&[&str], &[&str]) = match game {
            GameId::Bf1 => (&["CW.LVL", "GCW.LVL"], &["cw.lvl", "gcw.lvl", "test.lvl"]),
            GameId::Bf2 => (&["GLOBAL.LVL"], &["global.lvl"]),
        };
        let sources: Vec<String> = names.iter().map(|n| s(&ci_path(&sound_dir, n))).collect();
        let ref_list = lvl_files(&sound_dir)
            .iter()
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| !excluded.contains(&n.to_ascii_lowercase().as_str()))
            })
            .map(|p| s(p))
            .collect::<Vec<_>>()
            .join(",");
        let out = audio_dir.join(format!("global_{}", game.as_str()));
        std::fs::create_dir_all(&out)?;
        ctx.run_bake(
            &["global-sound".into(), sources.join(","), s(&out), ref_list],
            &[],
        )?;
    }
    Ok(())
}

fn bake_load_screens(ctx: &ExecCtx, trees: &BakeTrees) -> Result<()> {
    ctx.log("== load screens ==");
    if let Some(b1) = &trees.bf1 {
        ctx.run_bake(
            &[
                "load-layout".into(),
                s(&ci_path(b1, "LOAD/LOAD.LVL")),
                s(&trees.out_dir.join("common_bf1")),
            ],
            &[],
        )?;
        ctx.run_bake(
            &[
                "loadscreen".into(),
                s(&ci_path(b1, "LOAD/TAT1.LVL")),
                s(&trees.out_dir.join("tat1")),
            ],
            &[],
        )?;
    }
    ctx.run_bake(
        &[
            "loadscreen".into(),
            s(&ci_path(&trees.bf2, "LOAD/TAT2.LVL")),
            s(&trees.out_dir.join("tat2")),
        ],
        &[],
    )?;
    ctx.run_bake(
        &[
            "loadscreen".into(),
            s(&ci_path(&trees.bf2, "LOAD/COMMON.LVL")),
            s(&trees.out_dir.join("common_bf2")),
        ],
        &[],
    )?;
    ctx.run_bake(
        &[
            "load-orbits".into(),
            s(&ci_path(&trees.bf2, "GAL/GAL1.LVL")),
            s(&trees.out_dir.join("common_bf2")),
        ],
        &[],
    )
}

fn bake_maps(ctx: &ExecCtx, trees: &BakeTrees) -> Result<()> {
    ctx.log("== other ground maps ==");
    if let Some(b1) = &trees.bf1 {
        for map in BF1_MAPS {
            let short = map.rsplit('/').next().unwrap();
            let dir = trees
                .out_dir
                .join(format!("bf1_{}", short.to_ascii_lowercase()));
            ctx.run_bake(
                &with_donor(
                    vec![
                        "map".into(),
                        s(&ci_path(b1, &format!("{map}.LVL"))),
                        s(&dir),
                    ],
                    trees.donors.lvl(GameId::Bf1, &format!("{map}.LVL")),
                ),
                &[],
            )?;
            let lf = ci_path(b1, &format!("LOAD/{short}.LVL"));
            if lf.is_file() {
                ctx.run_bake(&["loadscreen".into(), s(&lf), s(&dir)], &[])?;
            }
        }
    }
    for map in BF2_MAPS {
        let short = map.rsplit('/').next().unwrap();
        let dir = trees
            .out_dir
            .join(format!("bf2_{}", short.to_ascii_lowercase()));
        ctx.run_bake(
            &with_donor(
                vec![
                    "map".into(),
                    s(&ci_path(&trees.bf2, &format!("{map}.LVL"))),
                    s(&dir),
                ],
                trees.donors.lvl(GameId::Bf2, &format!("{map}.LVL")),
            ),
            &[],
        )?;
        let lf = ci_path(&trees.bf2, &format!("LOAD/{short}.LVL"));
        if lf.is_file() {
            ctx.run_bake(&["loadscreen".into(), s(&lf), s(&dir)], &[])?;
        }
    }
    if trees.bf1.is_some()
        && trees
            .out_dir
            .join("bf1_yav1")
            .join("manifest.ron")
            .is_file()
        && trees
            .out_dir
            .join("bf2_yav1")
            .join("manifest.ron")
            .is_file()
    {
        ctx.run_bake(&["donor-hints".into(), s(&trees.out_dir)], &[])?;
    }

    ctx.log("== space maps ==");
    let sky_lvl = ci_path(&trees.bf2, "SPA/SPA_SKY.LVL");
    let spa_load = ci_path(&trees.bf2, "LOAD/SPA.LVL");
    for (spa, sky) in BF2_SPACE_MAPS {
        let lvl = ci_path(&trees.bf2, &format!("SPA/{spa}.LVL"));
        if !lvl.is_file() {
            continue;
        }
        let dir = trees
            .out_dir
            .join(format!("bf2_{}", spa.to_ascii_lowercase()));
        let mut args = with_donor(
            vec!["map".into(), s(&lvl), s(&dir)],
            trees.donors.lvl(GameId::Bf2, &format!("SPA/{spa}.LVL")),
        );
        args.extend(["--sky-sublevel".into(), s(&sky_lvl), (*sky).into()]);
        ctx.run_bake(&args, &[])?;
        if spa_load.is_file() {
            ctx.run_bake(&["loadscreen".into(), s(&spa_load), s(&dir)], &[])?;
        }
    }
    Ok(())
}

fn bake_beep(ctx: &ExecCtx, trees: &BakeTrees) -> Result<()> {
    if let Some(b1) = &trees.bf1 {
        ctx.log("== BF1 loading beep ==");
        ctx.run_bake(
            &[
                "beep".into(),
                s(&ci_path(b1, "CORE.LVL")),
                s(&trees.out_dir.join("audio")),
            ],
            &[],
        )?;
    }
    Ok(())
}

fn check_foliage(ctx: &ExecCtx, plan: &BuildPlan, trees: &BakeTrees) -> Result<()> {
    let mut args = vec!["check-foliage".into(), s(&trees.out_dir)];
    if let Some(bf1) = &plan.bf1 {
        if !matches!(bf1, GameSource::Ps2Iso(_)) {
            args.push("--warn".into());
            args.push("bf1".into());
        }
    }
    if !matches!(plan.bf2, GameSource::Ps2Iso(_)) {
        args.push("--warn".into());
        args.push("bf2".into());
    }
    ctx.run_bake(&args, &[])
}

fn platform_key(p: Platform) -> &'static str {
    match p {
        Platform::Pc => "pc",
        Platform::Ps2 => "ps2",
        Platform::Xbox => "xbox",
    }
}

fn bake_stock_catalog(ctx: &ExecCtx, plan: &BuildPlan, trees: &BakeTrees) -> Result<()> {
    ctx.log("== stock resource catalog ==");
    let out = s(&trees.out_dir);
    let bf2_plat = platform_key(plan.bf2.platform());
    ctx.run_bake(
        &[
            "stock-record".into(),
            out.clone(),
            "gameplay".into(),
            "bf2".into(),
            bf2_plat.into(),
        ],
        &[],
    )?;
    ctx.run_bake(
        &[
            "stock-record".into(),
            out.clone(),
            "dep".into(),
            "bf2_anims".into(),
            "bf2".into(),
            bf2_plat.into(),
            s(&trees.bf2),
            "ingame.lvl".into(),
        ],
        &[],
    )?;
    ctx.run_bake(
        &[
            "stock-catalog".into(),
            "bf2".into(),
            bf2_plat.into(),
            s(&trees.bf2),
            out.clone(),
        ],
        &[],
    )?;
    if let Some(pc2) = trees.donors.pc2.as_deref() {
        ctx.run_bake(
            &[
                "stock-catalog".into(),
                "bf2".into(),
                "pc".into(),
                s(pc2),
                out.clone(),
            ],
            &[],
        )?;
    }
    if let Some(xb2) = trees.donors.xb2.as_deref() {
        ctx.run_bake(
            &[
                "stock-catalog".into(),
                "bf2".into(),
                "xbox".into(),
                s(xb2),
                out.clone(),
            ],
            &[],
        )?;
    }
    if let (Some(bf1_src), Some(b1)) = (&plan.bf1, trees.bf1.as_deref()) {
        let bf1_plat = platform_key(bf1_src.platform());
        ctx.run_bake(
            &[
                "stock-record".into(),
                out.clone(),
                "gameplay".into(),
                "bf1".into(),
                bf1_plat.into(),
            ],
            &[],
        )?;
        ctx.run_bake(
            &[
                "stock-record".into(),
                out.clone(),
                "dep".into(),
                "bf1_anims".into(),
                "bf1".into(),
                bf1_plat.into(),
                s(b1),
                "common/eng/common.lvl".into(),
            ],
            &[],
        )?;
        ctx.run_bake(
            &[
                "stock-catalog".into(),
                "bf1".into(),
                bf1_plat.into(),
                s(b1),
                out.clone(),
            ],
            &[],
        )?;
        if let Some(pc1) = trees.donors.pc1.as_deref() {
            ctx.run_bake(
                &[
                    "stock-catalog".into(),
                    "bf1".into(),
                    "pc".into(),
                    s(pc1),
                    out.clone(),
                ],
                &[],
            )?;
        }
        if let Some(xb1) = trees.donors.xb1.as_deref() {
            ctx.run_bake(
                &[
                    "stock-catalog".into(),
                    "bf1".into(),
                    "xbox".into(),
                    s(xb1),
                    out,
                ],
                &[],
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn donor_copies_list_the_pc_before_the_xbox_and_skip_missing_files() {
        let dir = std::env::temp_dir()
            .join(format!("openbf_launcher_donor_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let pc = dir.join("_LVL_PC");
        let xbox = dir.join("_LVL_XBOX");
        std::fs::create_dir_all(pc.join("SIDE")).unwrap();
        std::fs::create_dir_all(xbox.join("SIDE")).unwrap();
        std::fs::write(pc.join("SIDE").join("REP.LVL"), b"ucfb").unwrap();
        std::fs::write(xbox.join("SIDE").join("REP.LVL"), b"ucfb").unwrap();
        std::fs::write(xbox.join("SIDE").join("CIS.LVL"), b"ucfb").unwrap();

        let mut donors = DonorTrees::default();
        donors.set(GameId::Bf2, Platform::Xbox, xbox.clone());
        donors.set(GameId::Bf2, Platform::Pc, pc.clone());
        donors.set(GameId::Bf2, Platform::Ps2, dir.clone());

        let rep = donors.lvl(GameId::Bf2, "SIDE/REP.LVL").unwrap();
        assert_eq!(
            rep,
            format!(
                "{},{}",
                s(&ci_path(&pc, "SIDE/REP.LVL")),
                s(&ci_path(&xbox, "SIDE/REP.LVL"))
            )
        );
        assert_eq!(
            donors.lvl(GameId::Bf2, "SIDE/CIS.LVL").unwrap(),
            s(&ci_path(&xbox, "SIDE/CIS.LVL"))
        );
        assert_eq!(donors.lvl(GameId::Bf2, "SIDE/ALL.LVL"), None);
        assert_eq!(donors.lvl(GameId::Bf1, "SIDE/REP.LVL"), None);
        assert_eq!(
            donors.fpm_args(GameId::Bf2),
            vec!["--donor".to_owned(), s(&pc), "--donor".to_owned(), s(&xbox)]
        );
        assert!(donors.fpm_args(GameId::Bf1).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
