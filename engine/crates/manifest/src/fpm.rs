//! First-person arm animation clip tables and asset paths.
//!
//! Static tables from PS2 RAM for BF1 and BF2 (`analysis/re-first-person-arms.md` §2).

pub const COLUMNS: usize = 11;

pub const BF1_BANK: &str = "human_animation_fp";
pub const BF2_BANK: &str = "humanfp";

pub const BF1_TABLE: [[&str; COLUMNS]; 5] = [
    [
        "_1st_rifle_idle",
        "_1st_rifle_run",
        "_1st_rifle_shoot",
        "_1st_tool_idle",
        "_1st_tool_idle",
        "_1st_rifle_reload",
        "_1st_tool_idle",
        "_1st_rifle_jump",
        "_1st_rifle_flail",
        "_1st_handsdown",
        "_1st_rifle_jump_land",
    ],
    [
        "_1st_lascannon_idle",
        "_1st_lascannon_run",
        "_1st_lascannon_shoot",
        "_1st_tool_idle",
        "_1st_tool_idle",
        "_1st_tool_idle",
        "_1st_tool_idle",
        "_1st_lascannon_jump",
        "_1st_lascannon_flail",
        "_1st_handsdown",
        "_1st_rifle_jump_land",
    ],
    [
        "_1st_bazooka_idle",
        "_1st_bazooka_run",
        "_1st_bazooka_shoot",
        "_1st_tool_idle",
        "_1st_tool_idle",
        "_1st_bazooka_reload",
        "_1st_tool_idle",
        "_1st_bazooka_jump",
        "_1st_bazooka_flail",
        "_1st_handsdown",
        "_1st_bazooka_jump_land",
    ],
    [
        "_1st_tool_idle",
        "_1st_tool_run",
        "_1st_tool_shoot",
        "_1st_tool_idle",
        "_1st_tool_idle",
        "_1st_tool_reload",
        "_1st_tool_repair",
        "_1st_tool_jump",
        "_1st_tool_flail",
        "_1st_handsdown",
        "_1st_tool_jump_land",
    ],
    [
        "_1st_tool_idle",
        "_1st_tool_run",
        "_1st_grenade_shoot",
        "_1st_grenade_shoot2",
        "_1st_grenade_charge",
        "_1st_tool_reload",
        "_1st_tool_repair",
        "_1st_tool_jump",
        "_1st_tool_flail",
        "_1st_handsdown",
        "_1st_rifle_jump_land",
    ],
];

pub const BF2_TABLE: [[&str; COLUMNS]; 4] = [
    [
        "humanfp_rifle_idle",
        "humanfp_rifle_run",
        "humanfp_rifle_shoot",
        "humanfp_tool_idle",
        "humanfp_tool_idle",
        "humanfp_rifle_reload",
        "humanfp_tool_idle",
        "humanfp_rifle_jump",
        "humanfp_rifle_flail",
        "humanfp_handsdown",
        "humanfp_rifle_jump_land",
    ],
    [
        "humanfp_bazooka_idle",
        "humanfp_bazooka_run",
        "humanfp_bazooka_shoot",
        "humanfp_tool_idle",
        "humanfp_tool_idle",
        "humanfp_bazooka_reload",
        "humanfp_tool_idle",
        "humanfp_bazooka_jump",
        "humanfp_bazooka_flail",
        "humanfp_handsdown",
        "humanfp_bazooka_jump_land",
    ],
    [
        "humanfp_tool_idle",
        "humanfp_tool_run",
        "humanfp_tool_shoot",
        "humanfp_tool_idle",
        "humanfp_tool_idle",
        "humanfp_tool_reload",
        "humanfp_tool_repair",
        "humanfp_tool_jump",
        "humanfp_tool_flail",
        "humanfp_handsdown",
        "humanfp_tool_jump_land",
    ],
    [
        "humanfp_tool_idle",
        "humanfp_tool_run",
        "humanfp_grenade_shoot",
        "humanfp_grenade_shoot2",
        "humanfp_grenade_charge",
        "humanfp_tool_reload",
        "humanfp_tool_repair",
        "humanfp_tool_jump",
        "humanfp_tool_flail",
        "humanfp_handsdown",
        "humanfp_rifle_jump_land",
    ],
];

pub fn clip_names(bank: &str) -> Vec<&'static str> {
    let rows: Vec<&[&'static str; COLUMNS]> = if bank.eq_ignore_ascii_case(BF1_BANK) {
        BF1_TABLE.iter().collect()
    } else {
        BF2_TABLE.iter().collect()
    };
    let mut names: Vec<&'static str> = Vec::new();
    for name in rows.into_iter().flatten() {
        if !names.contains(name) {
            names.push(name);
        }
    }
    names
}

pub fn glb_path(side_dir: &std::path::Path, model: &str) -> std::path::PathBuf {
    side_dir
        .join("fpm")
        .join(format!("{}.glb", model.to_ascii_lowercase()))
}
