//! Embedded mission-generation and Lua-extraction helper.
//!
//! Embeds the exact Python mission scripts and correction JSONs inside
//! `openbf-launcher` via `include_str!` and materializes them inside the
//! setup's temporary staging directory (which is deleted once packing finishes),
//! keeping the game folder completely free of `scripts/` and `engine/` clutter.

use super::scan::{ci_path, BuildPlan, GameId, GameSource};
use super::ExecCtx;
use crate::config::Platform;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

const EMBEDDED_PY: &[(&str, &str)] = &[
    (
        "extract_lua.py",
        include_str!("../../../../../scripts/extract_lua.py"),
    ),
    (
        "gen_missions.py",
        include_str!("../../../../../scripts/gen_missions.py"),
    ),
    (
        "gameplay_missions.py",
        include_str!("../../../../../scripts/gameplay_missions.py"),
    ),
    (
        "campaign_ir.py",
        include_str!("../../../../../scripts/campaign_ir.py"),
    ),
    (
        "campaign_programs.py",
        include_str!("../../../../../scripts/campaign_programs.py"),
    ),
    (
        "version_census.py",
        include_str!("../../../../../scripts/version_census.py"),
    ),
    (
        "version_geometry.py",
        include_str!("../../../../../scripts/version_geometry.py"),
    ),
    (
        "version_sky.py",
        include_str!("../../../../../scripts/version_sky.py"),
    ),
];

const EMBEDDED_CORRECTIONS: &[(&str, &str)] = &[
    (
        "bf1-pc.json",
        include_str!("../../../manifest/data/gameplay/corrections-v1/bf1-pc.json"),
    ),
    (
        "bf1-xbox.json",
        include_str!("../../../manifest/data/gameplay/corrections-v1/bf1-xbox.json"),
    ),
    (
        "bf2-pc.json",
        include_str!("../../../manifest/data/gameplay/corrections-v1/bf2-pc.json"),
    ),
    (
        "bf2-pc-campaign.json",
        include_str!("../../../manifest/data/gameplay/corrections-v1/bf2-pc-campaign.json"),
    ),
    (
        "bf2-xbox.json",
        include_str!("../../../manifest/data/gameplay/corrections-v1/bf2-xbox.json"),
    ),
    (
        "bf2-xbox-campaign.json",
        include_str!("../../../manifest/data/gameplay/corrections-v1/bf2-xbox-campaign.json"),
    ),
];

const BF1_LUA_LVLS: &[(&str, &str)] = &[
    ("mission", "MISSION.LVL"),
    ("common", "COMMON/ENG/COMMON.LVL"),
    ("shell", "SHELL/ENG/SHELL.LVL"),
];

const BF2_LUA_LVLS: &[(&str, &str)] = &[
    ("mission", "MISSION.LVL"),
    ("common", "COMMON.LVL"),
    ("shell", "SHELL.LVL"),
    ("ingame", "INGAME.LVL"),
    ("inshell", "INSHELL.LVL"),
];

/// Materialized copies of the embedded mission scripts inside the setup temp dir.
pub struct MaterializedMissions {
    pub scripts_dir: PathBuf,
    pub corrections_dir: PathBuf,
}

impl MaterializedMissions {
    pub fn materialize(temp_root: &Path) -> Result<Self> {
        let scripts_dir = temp_root.join("mission_scripts");
        let corrections_dir = temp_root.join("corrections-v1");
        std::fs::create_dir_all(&scripts_dir)
            .with_context(|| format!("creating {}", scripts_dir.display()))?;
        std::fs::create_dir_all(&corrections_dir)
            .with_context(|| format!("creating {}", corrections_dir.display()))?;
        for (name, body) in EMBEDDED_PY {
            std::fs::write(scripts_dir.join(name), body)
                .with_context(|| format!("writing embedded {name}"))?;
        }
        for (name, body) in EMBEDDED_CORRECTIONS {
            std::fs::write(corrections_dir.join(name), body)
                .with_context(|| format!("writing embedded {name}"))?;
        }
        Ok(Self {
            scripts_dir,
            corrections_dir,
        })
    }

    /// Extract compiled Lua bytecode from the active game LVLs into `lua_root`.
    pub fn extract_lua(
        &self,
        ctx: &ExecCtx,
        bf1_tree: Option<&Path>,
        bf2_tree: &Path,
        lua_root: &Path,
    ) -> Result<()> {
        let script = self.scripts_dir.join("extract_lua.py");
        type LuaJob<'a> = (GameId, &'a Path, &'static [(&'static str, &'static str)]);
        let mut jobs: Vec<LuaJob<'_>> = Vec::new();
        if let Some(t1) = bf1_tree {
            jobs.push((GameId::Bf1, t1, BF1_LUA_LVLS));
        }
        jobs.push((GameId::Bf2, bf2_tree, BF2_LUA_LVLS));

        for (game, tree, lvls) in jobs {
            for (sub, rel) in lvls {
                ctx.check_cancelled()?;
                let lvl = ci_path(tree, rel);
                let out = lua_root.join(game.as_str()).join(sub);
                std::fs::create_dir_all(&out)?;
                let args: Vec<String> = vec![
                    "-I".to_string(),
                    script.to_string_lossy().into_owned(),
                    lvl.to_string_lossy().into_owned(),
                    "--out".to_string(),
                    out.to_string_lossy().into_owned(),
                ];
                ctx.run_python(&args, &[])?;
            }
        }
        Ok(())
    }

    /// Generate `missions/*.ron` using `gen_missions.py`.
    pub fn generate_missions(
        &self,
        ctx: &ExecCtx,
        plan: &BuildPlan,
        bf1_tree: Option<&Path>,
        bf2_tree: &Path,
        lua_root: &Path,
        missions_out: &Path,
    ) -> Result<()> {
        ctx.check_cancelled()?;
        std::fs::create_dir_all(missions_out)?;
        let script = self.scripts_dir.join("gen_missions.py");
        let mut args = vec![
            "-I".to_string(),
            script.to_string_lossy().into_owned(),
            "--ps2-only".to_string(),
            "--lua-root".to_string(),
            lua_root.to_string_lossy().into_owned(),
            "--out".to_string(),
            missions_out.to_string_lossy().into_owned(),
            "--hunt".to_string(),
            "--space".to_string(),
        ];
        if !plan.has_bf1() {
            args.push("--bf2-only".to_string());
        }

        // Gameplay reference tables + sources for PC/Xbox primaries.
        let mut table_only_games: Vec<&'static str> = Vec::new();
        if let (Some(src), Some(tree)) = (&plan.bf1, bf1_tree) {
            self.push_correction_args(GameId::Bf1, src, tree, &mut args, &mut table_only_games);
        }
        self.push_correction_args(GameId::Bf2, &plan.bf2, bf2_tree, &mut args, &mut table_only_games);
        for game in table_only_games {
            args.push("--table-only-game".to_string());
            args.push(game.to_string());
        }

        ctx.run_python(
            &args,
            &[
                ("OPENBF_GAMEPLAY_CORRECTIONS", None),
                ("OPENBF_GAMEPLAY_SOURCE_BF1", None),
                ("OPENBF_GAMEPLAY_SOURCE_BF2", None),
            ],
        )
    }

    fn push_correction_args(
        &self,
        game: GameId,
        source: &GameSource,
        tree: &Path,
        args: &mut Vec<String>,
        table_only: &mut Vec<&'static str>,
    ) {
        let plat_str = match source.platform() {
            Platform::Pc => {
                table_only.push(game.as_str());
                "pc"
            }
            Platform::Xbox => "xbox",
            Platform::Ps2 => return,
        };
        let table = self
            .corrections_dir
            .join(format!("{}-{plat_str}.json", game.as_str()));
        args.push("--gameplay-reference".to_string());
        args.push(table.to_string_lossy().into_owned());
        args.push("--gameplay-source".to_string());
        args.push(format!("{}={}", game.as_str(), tree.to_string_lossy()));
    }

    /// Generate `common_bf2/shell/credits.json` from BF2 `SHELL.LVL` for console builds.
    pub fn generate_shell_credits(
        &self,
        ctx: &ExecCtx,
        shell_lvl: &Path,
        credits_out: &Path,
    ) -> Result<()> {
        ctx.check_cancelled()?;
        let script = self.scripts_dir.join("gen_missions.py");
        let args = vec![
            "-I".to_string(),
            script.to_string_lossy().into_owned(),
            "--shell-credits".to_string(),
            shell_lvl.to_string_lossy().into_owned(),
            credits_out.to_string_lossy().into_owned(),
        ];
        ctx.run_python(&args, &[])
    }
}
