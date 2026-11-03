---
id: VTAT
title: A bot gunner triggers its seat gun as the PS2 does: by the weapon, not by the seat - a charge cannon (Hoth's dish turret, never fired by a bot today) is charged and released, a TriggerSingle seat weapon is tapped, a cannon keeps its bursts (re-turret-ai 7)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why (build; rewritten 2026-10-07 by the RE session)
A bot in a seat holds FIRE whenever its shot is on (`core/src/ai/gunner.rs`, VTAI) and never fires a seat weapon that
has `MaxPressedTime`. The PS2 has no seat rule at all: ONE fire controller per game serves every controllable kind and
dispatches on the WEAPON (`analysis/re-turret-ai.md` 7, [O]). Two things a player would see: BF2 Hoth's dish turret
(a showcase map; the one charged seat gun of each game) is never fired by a bot, and a seat weapon with the
`TriggerSingle` bit fires once per hold.

## Evidence (`analysis/re-turret-ai.md`, ab64c9c; verifier 41 of 46 SUPPORTED, 5 PARTLY folded in)
- The dispatch (B2 `FUN_002bb1b0`, B1 `FUN_0023cd40`; 7 callers each, no test of the controllable's kind): a cannon
  with `MaxPressedTime` > 0 -> charge; else the `TriggerSingle` bit -> tap; else a cannon-class weapon -> bursts; else
  held. `TriggerSingle` defaults to 1 in the base weapon class and to 0 in every class built on the cannon constructor
  (labels cannon, weapon, launcher, towcableweapon, grapplinghookweapon) and in repair. The port's class defaults
  already agree (`WeaponClass::default` false, the grenade class true).
- Disc census (`lead_tmp/vtat/seatguns.py`, distinct seat weapons): bursts 103 (BF2) / 59 (BF1); tap 19 / 12 (the
  guided rockets, the remote droids' weapons; authored 1 in BF1: `imp_weap_walk_atat_cannon_chin`,
  `cis_weap_tread_hailfire_missile`, `rep_weap_fly_jedifighter_missile`); charge 1 / 1 (`tur_weap_hoth_dishturret`,
  MaxPressedTime 2.0; BF1 `hoth_weap_bldg_dishturret_cannon`); held 1 / 3.
- The sub-controllers are already transcribed: the charge release and the cannon bursts in `re-ai-difficulty.md` 4.4,
  the tap in `re-round-stalls.md` 2 C4 (built on foot by STTAP: `gunner_foot::tap`).
- The band, live on BF2 Hoth (section 6): a fight is opened only on a target a weapon has inside MinRange..MaxRange
  and is kept to MaxRange + 10 m; the trigger was pressed only inside MaxRange (10-77 m for an 80 m gun).
- NOT in this card: bots BOARDING a stand-alone turret (card TURR; the PS2's rule is `re-soldier-ai.md` 7); the
  self-piloted automatic turret (card VSELF, post-alpha: no conquest world places one).

## Acceptance (run and paste)
- Three headless scene tests, a bot seated by hand, an enemy soldier standing 60 m off inside the gun's arc, 10 s:
  (a) BF2 Hoth's `tur_bldg_hoth_dishturret`: at least 1 shot (today 0); (b) a seat weapon with `TriggerSingle 1`
  (BF1's AT-AT chin cannon): at least 2 shots (today 1 per hold); (c) a cannon seat (`tur_bldg_laser`): shots within
  10 % of today's. Each mutation-checked (the branch removed fails its test).
- The sweep's `shots` census split by seat-weapon path (burst / tap / charge), 8 seeds x 32 maps, before and after;
  256 of 256 rounds finish. Declared: `sim; sweep; arena`.

## Original card (the RE phase, answered by `analysis/re-turret-ai.md`)
### Why
VTAI (2026-09-29) arms bots in hover seats only: ride::claim_seat requires a drivable hover, so no bot ever mans a building turret (VehicleKind::Turret), and the self-piloted turrets VTURB spawns (PilotType = self, not boardable) have no gunner at all — they sit silent.

### Evidence
VTAI card note (VTURB): BF2 UTA1's two uta1_bldg_turret author PilotType = self on the body and TURRET1. re-vehicles §9 locates the MountedTurret tick B1 FUN_00223850 (its acquisition) but does not transcribe it [H]. The port's bot gunner is core/src/ai/gunner.rs (look stick toward the seen target, on-foot fire gates) — reusable once the acquisition rule is known.

### Acceptance
analysis/re-<topic>.md: the PS2 automatic turret's acquisition (range, cone, LOS, team, refire) and whether/when AI soldiers board building turrets, [O] with the §5 verifier; then a build: a self-piloted turret fires at what it acquires; bots man turrets if the PS2's do; sweep counters show turret shots.

## Planner note (2026-09-30, M-ALPHA)
`alpha: yes` — bots manning turrets (HOT1). Value 4 (kept). Showcase maps: BF1 NAB1, BF1 TAT1 (the port's `tat1`), BF1 YAV2, BF2 UTA1, BF2 MYG1, BF2 HOT1 (PLAN §Milestones M-ALPHA).

- Planner 2026-10-02 09:30, from STTAP's report (`reports/20261002_0810_card_STTAP.md`): bots on foot now tap a
  `TriggerSingle` weapon by the PS2's rule; seat guns (`core/src/ai/gunner.rs`) still HOLD the trigger, so a
  single-shot vehicle or turret gun fires once per hold. Include the seat's trigger rule in this RE's questions.

- 2026-10-07 RE DONE (lane 2, `reports/20261007_0706_card_VTAT.md`, ab64c9c): the card's three questions are answered
  in `analysis/re-turret-ai.md`. (1) The automatic turret is `PilotType = self`: BF2 gives the seat a stationary AI
  controller at spawn and the order type 10 (wait agent); BF1 has no such AI. No conquest world places one -> card
  VSELF, post-alpha. (2) Bots boarding turrets was already `re-soldier-ai.md` 7 -> card TURR. (3) The seat's trigger:
  no seat rule, a dispatch on the weapon -> this card's build. "The MountedTurret acquisition" of the old title is the
  `Controllable = 0` ray, used only by BF1's fambaa (note on card FAMB).
- 2026-10-21 AUDIT BRSTU (lane 1; `lead_tmp/audit-brstu/EVIDENCE.md`): BRSTU (cbf6f68) landed this card's TAP half - a `TriggerSingle`
  seat gun goes to `gunner_foot::tap` with the shot test alone (`ai/gunner.rs` aim). Still open here: (a) the CHARGE - both Hoth dish turrets
  (BF1 `hoth_weap_bldg_dishturret_cannon` MaxPressedTime 1.5, BF2 `tur_weap_hoth_dishturret` 2.0, both TriggerSingle) are never fired by a
  bot: `shot` requires `max_pressed_time <= 0`, so tap gets false; in the audit's 8 x 32 sweep all 31 dish shots (47 on the parent) come on
  the boarding tick from a FIRE carried in from foot, against 6,183 seated-with-target samples; (b) the `TriggerSingle` default by class root
  (the base weapon ctor sets bit 50 of +0x1e0, the cannon and repair ctors clear it: re-turret-ai 7 C26, re-checked by two verifiers) - the port
  defaults it off except grenades; (c) a weapon that is neither cannon-kind, TriggerSingle nor charge skips the burst on the PS2 (repair).

## Current-source reconciliation (2026-10-24 10:33 UTC)
BRSTU already routes TriggerSingle seat fire through tap (ai/gunner.rs:125-135); the shot predicate still rejects MaxPressedTime > 0 at line 123. Remaining charge/class-default/held-path work follows the 10-21 audit note. The three original scene tests remain required, including tap as a regression control; no fresh PS2 claim or runtime result is inferred here.
