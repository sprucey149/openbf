---
id: RKBARC
title: Rockets still pass through the BF2 BARC speeder (0/6 after RKHUL): its coll mesh sits 0.96 m below the drawn body (coll y -0.59..0.66, drawn 0.34..1.66; node dummyroot, identity bind) - the only one of 84 side coll meshes off its model; and the STAP misses 2/6 at one GEO1 pad
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Card RKHUL's repro on its scratch bake (lead_tmp/rkhul/repro_branch.txt, scorer lead_tmp/rkhul/score.py): walkers now hit 6/6, but rep_hover_barcspeeder stays 0/6 on KAS2 (the rocket lands on terrain 9-111 m behind). The BARC's ordnance bodies are its coll mesh plus six tiny '-so' boxes (pedals, handles, gun fronts); its hull prims are MASK v|t|b ('-vtb', no ordnance, re-collision-layers §1/§5). The coll mesh is 0.96 m below the drawn model in y with x/z matching (lead_tmp/rkhul/coll_census.txt: every other side coll mesh matches its drawn model within 0.06 m, except where a turret top stands above it). The blast-sphere aim point (y +1.04) is above the mesh, so a level shot passes over it. The '-vtb' cylinder (r 1, through boneroot bound at y 1.0) touches the ground at the origin, so the drawn model (and not the mesh) is likely where the PS2 has the body. [H] the PS2 places this coll mesh by boneroot's bind (or its NODE resolves differently) - read the coll loader's NODE use (re-collision-layers §1 hop) or a live BARC's CollisionModel in RAM.
STAP: cis_hover_stap V5 on GEO1 misses at 5 and 20 m (14.2 / 68.8 m terrain), V11 hits 6/6: the STAP's ordnance body is a thin column (x +-0.35); check its heading/tilt at the miss and the trimesh ray against an open column.
## Evidence
lead_tmp/rkhul/coll_census.txt, repro_branch.txt, repro_main.txt; the scratch test lead_tmp/rkhul/rkex_repro.rs (drop into game/tests/suite, mod rkex_repro;, RKEX_MAPS="bf2_geo1 bf2_kas2 bf1_geo1", --ignored --nocapture).
## Acceptance
The same repro: BARC 6/6 and STAP 12/12 at 5/20/60 m on KAS2 + GEO1; the RE answer for where the PS2 puts a vehicle coll mesh (a RAM read or the loader's lines) in analysis/; every other class's hit count unchanged.- 2026-10-21 (card TCOL re, lane 1), a lead for "where the PS2 puts a vehicle coll mesh" [H, not read for vehicles]:
  every collision body's matrix is its XFRM through the owner's pose table by its parent hash (B2/B1 `FUN_00160160`,
  `analysis/re-turret-collision.md` 1.1); the BUILDING parser builds its `CollisionMesh` body with the identity XFRM and
  parent hash 0 (B2 `FUN_001e4970` B2:94422), i.e. at the model root whatever the `coll` chunk's NODE says. B2
  `FUN_001d1f58` builds a 'CollisionMesh' body the same way (B2:80739) - check whether that is the vehicle class's
  parser before reading the BARC's mesh as root-placed.

## 2026-10-24 cloud RE checkpoint
Blocked on input restoration at main `3d487d9`: 20/20 named prerequisite paths are missing
(`engine/artifacts/lead_tmp/rkbarc/prerequisites.json`). Neither the original RKHUL/RKEX repro/scorer/rows,
the BF2 decomp/ELF and SIDE LVLs, nor the KAS2/GEO1 bakes, vehicle sides and mission directory are present.
Scoped `/workspace` filename search found no matching alternate copies; the broader `/tmp` search was incomplete
because a daemon directory is unreadable. `re_check.py fn b2 80739 94422` exits 1 on the missing BF2 corpus.
Source inspection only: `bake/src/vehicle.rs:439-440` stores NODE and writes the coll mesh;
`game/src/assets.rs:912-917` adds that mesh with an identity matrix. These are port observations, not a PS2 placement rule.
The TCOL building-parser lead remains [H] for vehicles. No height offset, MASK change, hit-count measurement,
new RE claim, implementation, commit or landing. Acceptance above is unchanged.
Restore the original instrument and inputs, reproduce on current main, then establish the vehicle loader's
identity/parent/NODE rule from primary evidence with an independent verifier before changing collision.
