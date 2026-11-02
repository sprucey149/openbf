---
id: FXMF
title: Muzzle flash: draw the PS2's flash MODEL (muzzleflash / com_sfx_muzzleflash, additive, tinted FlashColor, scaled FlashRadius, fading over FlashLength) at every soldier-held gun's fire point (hp_weapons × the weapon's hp_fire) along the aim — analysis/re-muzzle-flash.md §8; replaces the PROVISIONAL quad
milestone: M1
value: 5
phase: build
status: done
deps:
files: engine/crates/core/src/flash.rs (new), engine/crates/core/src/lib.rs, engine/crates/game/src/roster.rs, engine/crates/game/src/fx.rs, engine/crates/game/src/soldier_vis.rs, engine/crates/game/src/bolt_vis.rs, engine/crates/bake/src/soldier.rs, engine/crates/bake/src/alpha.rs, engine/crates/manifest/src/lib.rs, engine/crates/render/src/particles.rs, engine/crates/render/src/particles.wgsl, engine/CRATES.md, engine/artifacts/loop/FEEL_NOTES.md
declares: rebake=common,side
needs:
branch: fxmf
resume: LANDED bd05f80 (report reports/20260930_1032_card_FXMF.md). Nothing left; ADDM/FXP4 carry the reuse notes.
created: 2026-09-30
alpha: yes
---

## Why (original)
The muzzle flash is seen on every shot. It is NOT an `fx__` particle effect: `analysis/re-fx.md` §7 [V] — B1
`FUN_002864f0` (210374–210405) hashes the weapon's `MuzzleFlash` value and, for `small_muzzle_flash` /
`med_muzzle_flash` / `large_muzzle_flash` only, writes hardcoded fields (+0x1a4 = 0.6 / 1.0 / 1.8, +0x1a8 = 0.15 /
0.25 / 0.45, +0x194 = 0.1 / 0.15 / 0.3, +0x1ac = 0xff32ffff, +0x1b0 = 0xff0d3f3f / 0xff197f7f / 0xff25bfbf); any other
value keeps the weapon ctor's defaults (BF2 twin `FUN_00328fb0` 320842–320880). The port draws a PROVISIONAL quad.
