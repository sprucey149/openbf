//! Embedded mission-generation and Lua-extraction helper.
//!
//! Embeds the exact Python mission scripts and correction JSONs inside
//! `openbf-launcher` via `include_str!` and materializes them inside the
//! setup's temporary staging directory (which is deleted once packing finishes),
//! keeping the game folder completely free of `scripts/` and `engine/` clutter.

use super::rebake::BakeTrees;
use super::scan::{ci_path, BuildPlan, GameId, GameSource};
use super::{args, ExecCtx};
use crate::config::Platform;
use anyhow::{Context, Result};
use std::ffi::OsString;
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

    /// Extract compiled Lua bytecode from the active game LVLs into `trees.lua_root`.
    pub fn extract_lua(&self, ctx: &ExecCtx, trees: &BakeTrees) -> Result<()> {
        let script = self.scripts_dir.join("extract_lua.py");
        for (game, tree) in trees.games() {
            let lvls = match game {
                GameId::Bf1 => BF1_LUA_LVLS,
                GameId::Bf2 => BF2_LUA_LVLS,
            };
            for (sub, rel) in lvls {
                let out = trees.lua_root.join(game.as_str()).join(sub);
                std::fs::create_dir_all(&out)?;
                ctx.run_python(&script, &args![ci_path(tree, rel), "--out", out], &[])?;
            }
        }
        Ok(())
    }

    /// Generate `missions/*.ron` using `gen_missions.py`.
    pub fn generate_missions(
        &self,
        ctx: &ExecCtx,
        plan: &BuildPlan,
        trees: &BakeTrees,
        missions_out: &Path,
    ) -> Result<()> {
        ctx.check_cancelled()?;
        std::fs::create_dir_all(missions_out)?;
        let mut args = Vec::from(args![
            "--ps2-only",
            "--lua-root",
            trees.lua_root,
            "--out",
            missions_out,
            "--hunt",
            "--space",
        ]);
        if !plan.has_bf1() {
            args.push("--bf2-only".into());
        }

        // Gameplay reference tables + sources for PC/Xbox primaries.
        let mut table_only_games: Vec<&'static str> = Vec::new();
        if let (Some(src), Some(tree)) = (&plan.bf1, &trees.bf1) {
            self.push_correction_args(GameId::Bf1, src, tree, &mut args, &mut table_only_games);
        }
        self.push_correction_args(
            GameId::Bf2,
            &plan.bf2,
            &trees.bf2,
            &mut args,
            &mut table_only_games,
        );
        for game in table_only_games {
            args.extend(args!["--table-only-game", game]);
        }

        ctx.run_python(
            &self.scripts_dir.join("gen_missions.py"),
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
        args: &mut Vec<OsString>,
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
        let mut source_arg = OsString::from(format!("{}=", game.as_str()));
        source_arg.push(tree);
        args.extend(args![
            "--gameplay-reference",
            table,
            "--gameplay-source",
            source_arg
        ]);
    }

    /// Generate `common_bf2/shell/credits.json` from BF2 `SHELL.LVL` for console builds.
    pub fn generate_shell_credits(
        &self,
        ctx: &ExecCtx,
        shell_lvl: &Path,
        credits_out: &Path,
    ) -> Result<()> {
        ctx.run_python(
            &self.scripts_dir.join("gen_missions.py"),
            &args!["--shell-credits", shell_lvl, credits_out],
            &[],
        )
    }
}
