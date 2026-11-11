# RE: a soldier standing on a flyer — BF2's platform carry, the throw and the tip (card DECK)

**The question** (the maintainer, 2026-11-11). On a space map, a soldier standing on a ship in the hangar is "thrown away"
when it takes off, and usually takes fall damage. In the port he rises a little and drops, with no fall animation. Is that
the soldier controller or something on the ship?

**Answer.** It is the soldier's update. BF2 records the object a soldier stands on as his *platform* and moves him with it
every update. A flyer is always a platform. While he has one, no gravity acts on him. He leaves it in four ways:

- a sudden loss of the ship's speed throws him;
- a tilt past the limit tumbles him;
- half a second off its floor lets him go;
- a jump, a jet or a throw adds the ship's velocity to his own.

So the ship lifts him out of the hangar and carries him off. He comes away high and moving, then falls. The port had no
carry. A hull was only a floor under his feet, so the ship rose out from under him and left.

Corpus: `analysis/SWBFRNT2.ELF-decomp.c` (B2 lines below are this file's). Disassembly by `scripts/tools/eedis.py bf2`
(the ELF is `extracted/bf2/lvl/SWBFRNT2.ELF` in this checkout). [V] read in the decomp; [V-instr] read in the
disassembly. Soldier offsets are BF2's (`re-soldier-ground.md` §1.1).

## 1. The record [V][V-instr]

The soldier's collision response `FUN_0023cfd0` (B2:154854):

- The contact's model byte +2 is the platform flag (B2:154919-154921). The flyer class's `GeometryName` setter sets it to 1
  (B2:124524). The building class's clears it (B2:80135). A third class sets it too (B2:148163, `FUN_00233ca8`'s caller;
  `trace:` which entity class that is; the port carries on flyers only).
- The floor test uses the velocity **relative** to the other object: `v - other.velocity` (vtable +0x4c), with the soldier's
  x and z dropped for a layer-8 body (B2:155038-155060).
- A lower-body contact with normal y over 0.6 and that relative approach under 0.1 is a floor (B2:155474-155480). A floor on
  a layer-8 body, or on a flagged model, records the object at s+0x480 with its id at s+0x484. It records the inverse of the
  body's pose at s+0x4f0..0x520 and the object's velocity at s+0x490 (B2:155481-155637). A floor on anything else clears
  the record (B2:155491-155495).
- After the floor test the into-normal part comes off the soldier's **own** velocity s+0x550, not the relative one
  (0x23df74 `lqc2 vf4, 0x550(s1)`, 0x23dfb4-0x23dfbc).

## 2. The carry [V][V-instr]

At the start of the soldier update `FUN_00240bf8` (B2:157894-158328), when s+0x4ac is clear (`trace:` that class word) and
he has a platform:

1. **Let go.** With the airborne latch (s+0xa74 bit 0x80) set and the stance timer at 0.5 s or more, the record is cleared
   (B2:157911-157918). The stand, crouch, prone and sprint cases set the latch on their first update off the floor and zero
   the timer (B2:159599-159614, 159678-159694, 159761-159776, 159821-159836).
2. **Carry.** His pose = his pose x the stored inverse x the body's pose now. If his up axis is then off vertical, it is
   rebuilt upright (B2:157946-158178).
3. **Throw.** If the object's speed now is under the stored one by more than **8.2** (0x24120c, `0x41033333`), slot +0x94 is
   called. That is the push apply `FUN_002388b8` (`re-knockback.md` §4), with direction normalize(stored velocity) + (0,
   **0.3**, 0) (0x24124c) and scale the loss (0x241208 `sub.s $f12, $f20, $f12`) (B2:158181-158234).
4. **Tip.** Otherwise, if the body's up axis has a height under **0.55** (0x2412dc, `0x3f0ccccd`), the stance becomes 10,
   the tumble (B2:158235-158256).
5. **Re-store.** If he still has it, the inverse pose and the velocity are stored again (B2:158257-158327).

## 3. The rest [V]

- **No gravity on a platform.** The stand integrator `FUN_002480a0` applies −18 only when not grounded and s+0x480 is
  clear (`re-soldier-ground.md` §1.1, the s+0x4a0 there is a slip for s+0x480; B2:163098-163100 in that doc's numbering).
- **The state setter** `FUN_00247bf8` (B2:162017-162160): to 4 (jump), 6 (jump-jet) or 9 (thrown) the platform's velocity
  is added to his and the record cleared. To 7 (hover), 8 (fall), 10 (tumble), 11 (get-up), 15 or 16 (dead) it is cleared
  alone. The jump `FUN_00246ed0` sets 4 before its kick and its factors, so the ship's velocity is scaled with his
  (B2:161393-161440).
- **BF1** has the platform word (s+0x950) and the gravity gate, but only one writer, a setter (B1:118162), and no floor
  contact records it. BF1 has no carry (`trace:` the setter's caller).

## 3a. The carry meets walls [V]

The carry writes his pose. It does not write the soldier collision object's previous position (+0xa0). That word is
written only by the object's constructor `FUN_00168e18`, the list inserts `FUN_00161510`… and the sweep itself. The
collision update `FUN_00163d08` sweeps from that previous position to the pose now, in sub-steps, and snaps only past
100 m. So the carry is swept with the rest of his move, and a soldier-layer body stops him. The space hangar's shield
`imp_cap_stardestroyer_shield` authors `VehicleCollision = clear` with flyers on the vehicle subset, so a ship passes it.
The soldier subset keeps every body, so its rider does not pass. He is held at the shield while the ship goes on. Off its
floor, he is let go after 0.5 s and falls in the hangar.

## 4. The port

`core::sim::deck`: `Unit::deck` (the vehicle, its pose and velocity at the last record) and `Soldier::platform_vel`.
BF2 rules only.

- `Sim::touch_deck` runs after the floor contact. A grounded soldier whose feet are on a flyer's hull records it. The check is
  the foot's centre ray, else its ring, at his feet within 2 cm. Any other floor clears the record.
- `Sim::carry_on_deck` runs before his update and does the let-go, the carry, the throw and the tip, in that order. The carry
  moves the position and turns the yaw by the hull's motion. The throw is `Soldier::knock` with the BF2 apply. The tip is
  `Soldier::tip_off`.
- The wall pass's start `from` is taken before the carry, so `ceiling_sweep` and `push_out_soldier_from` sweep the carry
  with his own step (§3a). Carried first and swept from after it, a rider left the hangar through its shield (the
  maintainer, 2026-11-11).
- The sweep's sub-steps have no cap below 100 m, as on the PS2 (`SWEEP_SNAP`, `re-soldier-walls.md`). With the cap of 8,
  a ship leaving at about 120 m/s or more took its rider through the shield.
- `Sim::hold_at_walls` (PROVISIONAL, ours): after a carried rider's wall pass, his whole move is swept at the waist
  against the map's soldier bodies and stopped a radius short of a crossing. The pass also pushes his position out of
  the ship's own hull. Held at the shield, he was shoved through it by a part of the ship passing him. That happened on
  spa6 with a rider on the LAAT's low side, at 1 of 223 spots on the four Republic ships. On the PS2 a moving ship meets
  him as a velocity response, and every change of his position is swept (`trace:` a PCSX2 rider held at a shield while
  the tail passes him).
- `Soldier::set_stance` adds or drops `platform_vel` by the setter's lists.
- The stand integrator applies no gravity while it is set.

Named differences:

- The floor test stays the port's absolute one. The PS2's is relative to the platform, so a soldier on a descending ship
  has no floor contact there (`trace:` with a PCSX2 landing).
- Only flyers are decks. The layer-8 rule and the third flagged class are not ported (`trace:`).
- The upright rebuild is the yaw alone, since the port's soldier has no tilt.

Tests: `core::sim::deck::tests` (8). The shield test fails with the old order (the rider 20 m out through it). The fin
test fails without `hold_at_walls` (the rider 3.5 m through it). Scratch runs on the baked maps held the rider every time.
On spa6's Republic hangar they covered the four Republic ships with rider spots on a 1 m grid (223), exits at 40 and
170 m/s, turns, pitches and a throw at the shield. Single runs covered both hangars of spa3 and spa7. Unchecked: a matched PCSX2 run of a soldier on a ship that takes off from a BF2 space
hangar. The running emulator was held by another controller on 2026-11-11.
