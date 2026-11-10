# OPUS RE (2026-10-16) - [O] UNAUDITED

# The PS2's mix headroom: what it does when one-shots stack (card MIXH)

The port's battle mix clips beside a front-line team-mate: peaks of 1.8-3.5 x full scale before any unit cry, 0.03-0.43 %
of frames over (card UCRY, `lead_tmp/ucry/census/mix/summary.txt`). This asks what the PS2 does with the same sum. Tags:
[V] read (lines cited, function checked with `re_check.py fn`), [L] measured (save-state RAM, the SPU2 state, a PCSX2
output capture), [D] derived, [H] hypothesis. Corpora: B1 `analysis/SWBFRONT.ELF-decomp.c`, B2
`analysis/SWBFRNT2.ELF-decomp.c`; PCSX2 source `pcsx2-master/pcsx2/SPU2/`. Kit: `engine/artifacts/lead_tmp/mixh/`
(`tools/`, `ps2/` captures and states, `checks/`).

## 0. Result

**The PS2 does not limit, compress or duck by count. Its mix rarely clips because each 3-D voice is quieter per ear than
the port's, by its own level chain:**

1. **A 3-D voice is split linearly between the ears, L + R = gain x level** (output mode 3, both games, in every
   reference state): a centred sound has HALF its gain x level in each ear, a sound at the side all of it in one.
   The port (kira's constant-power pan x sqrt 2) gives a centred sound its FULL gain x level in each ear and a side
   sound 1.30 x. **The port is +6.0 dB per ear at centre, +2.3 dB at the side.** 2-D sounds (balance law, centre =
   full gain both ears) match.
2. **BF2's effects bus plays at 0.63** (the effects slider 7/10 x the `main` bus 0.9; BF1: 1.0). The port applies no
   bus: on BF2 every effect is a further **+4.0 dB**.
3. **At most 39 voices sound** (78 virtual one-shots; the 39 of highest value - about their distance level - hold a
   hardware voice, the rest run silent); a start that finds all 78 busy steals the lowest-value voice only if the new
   start's value is higher (BF1: its level; BF2: its property's bus gain and priority), else it is refused. Every state
   whose 39 voices are all busy has the effects bus at its normal gain.
4. **The one run-time duck is voice-over**: the `voiceover` bus attenuates `main` (BF1 to 0.5, BF2 to 0.65, 0.5 s
   fade in and out) while a VO line plays - the only `AttenuateBus` on either disc.
5. **The SPU2 hard-clamps** each core's voice sum to 16 bits (PCSX2's model), master volume 0x3fff = unity.

Measured: four PS2 front-line fights (player beside the fight every 20 s, 240 s each, the SPU2 output recorded):
**at most 4 clamp runs of 8+ samples in 4 minutes (0.2 ms each: Naboo 4, Tatooine 4, Utapau 2, Mygeeto 0)**. The PS2's
OWN logged starts rendered offline through the port's law clip like the port (peaks 2.6-6.0, 0.40-2.9 % of frames over
full scale); through the PS2's law they peak 1.2-2.3 with at most 0.02 % over. So the port's crackle is its level chain,
not the battle: give the port the PS2's per-ear law and bus gains (card MIXH build), and measure before adding any
limiter.

## 1. Claims

### 1.1 Level chain

1. **[V] Register value.** B1 `FUN_00456528` (516726-516731): `((int)(v * 32767.0) & 0xffff) >> 1`, constant 0x46fffe00
   (lui 0x46ff / ori 0xfe00 at 0x456528 / 0x45652c); called by the voice update `FUN_00457020` (517287, 517345,
   517354). B2 twin `FUN_00548c00` (679209-679214, same words at 0x548c00), called by `FUN_00549728` (679775, 679837,
   679846). B1 `FUN_00453170` sends it as IOP command 0x60 (voice, L, R) for voice < 0x30 only (514479, sltiu 0x30 at
   0x45319c): 48 SPU2 voices. L comes from `FUN_00456528`, R from its sibling `FUN_00456550` (the float at +4).
2. **[V] PCSX2's SPU2** (the emulator's model, [D] for the console): a fixed volume register `reg` plays as
   `SignExtend16(reg << 1)` (`ADSR.cpp` `V_VolumeSlide::RegSet`), `ApplyVolume = (vol * data) >> 15` (`Mixer.cpp`), so
   register 0x3fff = x 0.99994 and v of claim 1 is the amplitude multiplier; each core's dry / wet voice sum is clamped to
   [-0x8000, 0x7fff] (`MixCore`, `clamp_mix` in `defs.h`), then the master volume; core 0 feeds core 1 through ExtVol,
   core 1's sum is clamped and its master applied (`spu2Mix`); `spu2Output` divides by 32767 and DC-filters. No
   compressor or limiter is in the path - clamps are the only nonlinearity (verifier: see 4).
3. **[L] Master and routing**: `tools/spu2regs.py` (PCSX2 v2.8 `SPU2.bin`, layout checked: 0 mismatches over 96 volume
   decodes a state) on `bf1_nab1_cp1_cp2_postA_spawn`, `bf2_uta1_cp3_con_cp1_postA_spawn` and `ps2/shot1-3`: MVOL
   register 0x3fff on both cores, core 1 ExtVol 1.0, FxVol 0. The EE's `FUN_00454498` (16383.0 to IOP command 0x15 on
   both cores) belongs to a 0.3 s fade around a mode change (`FUN_004551a0`, `0x3e99999a`) and reads 0 in the states:
   not the master [H on what 0x15 drives].
4. **[V] 3-D pan.** B1 `FUN_0044cb78` (510376-510411): p = 1 - clamp(0.5 - 0.5 dot, 0, 1); output mode 2 narrows p to
   p x 0.5 + 0.25 (510404); modes 3/5/6/7 give **L = a (1 - p), R = a p** (510409-510411), a = level x gain; mode 1
   L = R = a. Mode word `DAT_005220cc`, written only in `FUN_00449078` (507561) from the profile's byte
   (`FUN_0034d1b8` -> `FUN_004552d0`). B2 twin `FUN_00540b98` (674083-674090), `DAT_0060b65c` written only in
   `FUN_0053ce90` (671053); both called by the voice-level function (B1 `FUN_0044c818` 510134, B2 `FUN_005404f0`
   673691). Both mode writers clamp the mode to 1..4 (cases 5-7 never run); mode 4 is a surround path not described here.
   **[L]** mode = 3 in all 162 BF1 and 181 BF2 reference states (`checks/output_mode.txt`).
5. **[V] 2-D pan** B1 `FUN_0046bdf0` (532348, 532352): pan > 0: R = g, L = g (1 - pan); else L = g, R = g (1 + pan) -
   a centred 2-D sound is full gain in both ears. B2 twin `FUN_0055c888` (693621), the same callers' 2-D branch.
6. **[V] Space term.** `FUN_00457020` doubles the four values of a voice whose byte +0x74 bit 0 is set (517259-517262),
   and `FUN_00456600` multiplies them by the mean of the voice's space gains (516802-516816); in the listener's default
   space (0x676020 holds 0.5 / 0.5 in every state read) the two cancel. [H]: the full space chain (`re-sound-distance.md`
   claim 7) - other spaces read lower (1.2 below).

### 1.2 What the voices really hold [L]

`tools/levelcensus.py bf1|bf2` over every reference state: for each playing one-shot bound to a handle voice, the handle's
volume block (+0x120 / +0x124, the block the register is made of: `tools/regmatch.py`, **515 of 527** BF1 and **355 of 384**
BF2 blocks found as the SPU2's own register pair in the same state) against drawn gain (+0xf4) x the distance level
(computed from the voice's own distance +0x20 and distance fields +0x60..+0x6c) x the effects bus's final gain:
**L + R equals it within 1 % for 739 of 888 BF1 and 549 of 659 BF2 voices**; 125 / 103 read lower (other sound spaces,
voices fading in or out), 9 / 2 higher (Kashyyyk interiors). The pan share R / (L + R) spans 0.000-1.000.
The player's own blaster shot (`ps2/shot1.p2s`, `tools/voices_b1.py --id BCD626DD`): gain 0.7012, distance 6.05 m from
the listener (not the camera eye's 4.2 m), level 0.8603, handle block 0.3016 / 0.3016, SPU2 register 0x134d / 0x134d
(= 0.3016) **= 0.7012 x 0.8603 x 0.5**. The PCSX2 output capture of 12 such shots (`ps2/cal_nab1.raw`, de-duplicated,
`tools/matchfine.py --win 0.015`): 0.24-0.31 per ear at the onset.

### 1.3 Voices: the cap and the steal

7. **[V] 78 virtual one-shots** (B1 `FUN_00459500` loop < 0x4e 518747, sltiu at 0x4595cc; B2 `FUN_0054c6c8` 681741) and
   **39 handle voices** (B1 `FUN_00448c70` loop < 0x27 507414, sltiu at 0x448d28, each appended to the group's list
   0x66f148 and counted in `DAT_0066f14c` 507413; B2 `FUN_0053ca78` 670904). One group for 2-D and 3-D: B1
   `FUN_00449638` returns `&DAT_0066f130` whatever its argument (507821); B2's getter `FUN_0053d4e0` returns
   `&DAT_0073dad0`.
8. **[V] Steal at a start** (B1 `FUN_004599e0`): a free voice from `FUN_0045a018` (518982); none free -> value =
   `FUN_0044e4e8`(0, start level x the bus's +0x24, prop+0x48 {priority, bias}) (518990) = (level' (1 - bias) + bias x
   priority) x age term (511369), and 0 unless the start's state flag is 1 (the function's *(decompiler detail omitted)* gate;
   `Bias` is 0.0001 on the properties read, so about the level); `FUN_0045a0f0` (518992) walks the group's list from
   its TAIL and stops and reuses the pool's tail-most voice - the lowest value once the list is sorted (claim 9) - only
   if its value is lower (519304); otherwise the start returns 0 (519002) - refused. **B2 differs**: its `FUN_0054cba8`
   (681977-681997) passes the property bus's +0x28 (its final gain) ALONE, no start level, priority pair at prop+0x50
   (verifiers, ELF 0x54cd98: `lwc1 f13, 0x28(v1)`, no `mul.s`); same less-than rule in `FUN_0054d268`; `FUN_00542758`
   (675134) is `FUN_0044e4e8`'s twin. So a BF2 start that finds 78 busy voices steals by property priority, not by
   loudness [D].
9. **[V]/[D] The 39** (B1 `FUN_0044c078`, run by the sound update `FUN_00448ed8` 507533 on 0x66f130): every playing
   voice's value is recomputed; above the group's capacity (+0x1c = the 39) the list is reordered (509845) and the voices
   inside the capacity get a handle voice (`FUN_0044c770`, `FUN_0044bbb0` 509937), the rest lose theirs (509942) and run
   on silently (`FUN_0044bc28` advances their position). B2 twin `FUN_0053fd78` (673402, 673491). [D] the order is by
   value, highest kept (the steal of claim 8 takes the list's other end).
   **[L]** `tools/voicecount.py`: virtual voices playing up to 70 (BF1) / 78 (BF2); handle voices never above 39 (at 39 in
   22 of 162 BF1 and 21 of 181 BF2 states).

### 1.4 Buses

10. **[L] The disc's bus tree** (`tools/buses.py` on each game's CORE.LVL): BF1 `ingame` 1.0 > `main` 1.0 > `soundfx` 1.0,
    `music` 1.0 (> `shellmusic` 0.6, `ingamemusic` 0.4); `ambience` 0.6 and `voiceover` 0.8 under `ingame`. BF2 the same
    with `main` 0.9 (under two unnamed 1.0 buses), `battlechatter` (0x74d771fc) 1.0 under `main`, `ingamemusic` 0.9,
    `ambience` 0.6 under `main`, `voiceover` 0.7. **`voiceover` is the one bus with `AttenuateBus`: `main` x 0.5 (BF1) /
    0.65 (BF2), AttenuationLeadIn / LeadOut 0.5 s.** The key hash 0x247db064 occurs 3 times in the 323 LVLs of both
    discs, once in each CORE.LVL (BF1 ships two byte-identical copies, `lvl/` and `sound/`; `checks/attenuatebus_count.txt`)
    [receipt:78f2aa0a] (the receipt's raw scan is ASCII; the count is the u32 scan). Also under `root`: a bus
    0x82c7f467 at 0.4 in both games, scaled by the effects slider; it hashes as fnv1a("shellfx"), the bus of the shell
    menu clicks. Open: openbf's BF2 table carries both a `shellfx` 0.4 row and a 0.4 x 0.7 row for it, and the first wins. Properties by bus (resolved through `Inherit`): BF1
    soundfx 6,648, voiceover 1,090, ingamemusic 192, ambience 173; BF2 battlechatter 20,363, soundfx 8,779, voiceover
    6,679, ingamemusic 840, ambience 328.
11. **[V] Option sliders scale the buses**: B1 `FUN_00356178` sets bus gain = slider x the bus's disc gain (saved by
    `FUN_00356290`): soundfx = `DAT_00646fc4` x `DAT_00646fac` (343516), called by `FUN_0034d1b8` (337167) with the profile
    bytes x 0.1. B2 `FUN_0043e1c8` (499724), from `FUN_00435ad8` (493549); BF2 has a fourth slider (battlechatter).
    **[L]** sliders in every state: BF1 music 9, effects 10, voice 8 (162 states); BF2 music 6, effects 7, voice 9,
    battlechatter 10 (181 states) - **so BF2's soundfx final gain is 0.7 x 0.9 = 0.63**. [H] these are the factory
    defaults (all captures use one memory card; the profile constructor was not read).
12. **[L] Effective gain = drawn gain x the bus's final gain**: voice +0x44 = +0xf4 x bus +0x28 in both censuses of 1.2;
    BF1 `shot2` 0.5 with `main` ducked. **No count-driven duck** (`tools/duckcount.py`): all 15 BF1 and 5 BF2 states with
    the 39 handle voices busy have the effects bus at its battle value; the five lower BF1 states hold 12-38 voices.
    BF2 has six more states with `soundfx` at 0.22-0.25 (`main` at 0.35 x its 0.9; 25-47 voices, all named at a spawn:
    `checks/bf2_low_bus_states.txt`) while 33 states at 0.63, on the same maps, hold as many or more bound voices (27-39):
    not the count [L]. What sets `main` there (a spawn / menu-screen fade in engine code; no mission or shell script
    fades `main`) is [H]; BF2's side-select and spawn-map states read `main` 0.2.

### 1.5 The PS2's own output in a fight [L]

`ps2/run_audio.sh` (PCSX2's SDL backend on SDL3's disk driver: the float stereo stream it hands a sound card, no
device; SyncMode Disabled) with USND's sampler (`--follow 20`: the player beside the team-mate nearest an enemy):
NAB1, TAT1, UTA1, MYG1, 240 game s each. The emulator ran at 37 % speed: PCSX2 then repeats frames or inserts zeros
(`AudioStream::ReadFrames`), never scales one; `tools/dedup.py` drops them (the calibration's shot spacing then matches
the game clock: 1.92 / 1.84 / 1.83 s against 1.94 / 1.84 / 1.80). `tools/plateaus.py`: a held SPU2 value is EXACT in the
capture (the DC filter runs in float: `y[n] == float32(0.995) * y[n-1]`); runs of 8+ held frames at |x| >= 0.9, whole
captures (`--skip 0`: the state resumes mid-battle, so second 0 is already the fight; `checks/plateaus_skip0.txt`):

| capture | s | clamp runs | frames | longest | ps2render, port law: peak / over | PS2 law: peak / over | port (UCRY, no cries): peak / over |
|---|---|---|---|---|---|---|---|
| NAB1 | 238 | 4 | 39 | 0.2 ms | 2.63 / 0.40 % | 1.32 / 0.002 % | 1.80 / 0.03 % |
| TAT1 | 240 | 4 | 34 | 0.2 ms | 4.50 / 0.54 % | 2.25 / 0.020 % | 2.23 / 0.15 % |
| UTA1 | 240 | 2 | 19 | 0.2 ms | 5.97 / 1.95 % | 1.88 / 0.020 % | 2.22-3.52 / 0.05-0.40 % |
| MYG1 | 239 | 0 | 0 | - | 3.79 / 2.89 % | 1.19 / 0.001 % | 2.44 / 0.43 % |

Control: the same detector on a capture clipped on purpose (x2, x3, re-filtered as PCSX2 does) reads 0.25 % / 1.50 %
of a true 0.51 % / 3.07 % - it sees half the clamped frames (runs under 8 are not counted), so the PS2 fights clamp at
most about 0.0004 % of frames. `tools/ps2render.py` renders the PS2's OWN logged one-shot starts (property gain x the
distance law at the logged distance, the take, mono; streams, loops and the voice cap left out; about 30 % of the
starts have no take in the port's bank, so it is a floor) through the port's law (gain x level per ear) and the PS2's
(x 0.5 at centre x the bus). The PS2's fights were as busy near the player (starts within 20 m a minute: PS2 485 / 156 /
607 / 746, port 170 / 343 / 171-243 / 299) and stacked as hard (up to 10 `imp_weap_ord_exp` starts in one tick on
UTA1, the port 10-14).

### 1.6 The port [V]

kira 0.10.8 `Frame::panned`: L x sqrt(1 - m) x sqrt 2, R x sqrt(m) x sqrt 2, m = (pan + 1) / 2, unchanged at pan 0;
the renderer clamps every sample to [-1, 1]. `game::sounds::pan_for` = dot(d, right) / dist x 0.7; one-shots play at
20 log10(gain x level); no bus anywhere. Per ear against the PS2 (mode 3, s = dot): centre 1.0 vs 0.5 (+6.0 dB), s = 1
1.304 vs 1.0 (+2.3 dB); BF2 x 1 / 0.63 more (+4.0 dB).

## 2. For the port (card MIXH, build)

- The PS2's per-ear law for 3-D sounds (one-shots and looping voices): L = g l (1 - p), R = g l p, p = 0.5 + 0.5 s.
  kira can carry it exactly: volume = g l sqrt(p^2 + (1-p)^2) / sqrt 2, panning = 2m - 1 with m = p^2 / (p^2 + (1-p)^2).
- Bus gains by the property's `Bus` (bake it): the disc gains and the slider chain of 1.4 (BF2 soundfx 0.63, BF1 1.0).
- A stereo SOUND_LOG (per-ear gains) and a stereo `mix.py`, then UCRY's five fight logs again: per-ear frames over full
  scale against the PS2 law's <= 0.02 %. A master limiter only if that still fails (the PS2 has none; it clamps).
- Not in the build: the `voiceover` duck (its own card), the 39-voice cap (no measured loudness effect: the ones it
  silences are the quietest).

## 3. Checks run

`checks/fn.txt` (43 lines placed, every cited line), `checks/callers_writers.txt` (twins' callers; writers of
`DAT_005220cc`, `DAT_0060b65c`, `DAT_00646fc4`, `DAT_0071efec`), `checks/words.txt` (32767.0, 16383.0, 0x4e, 0x27,
0x30, 0.5 / 0.25 in the ELF words of both games), `checks/attenuatebus_count.txt` + `re_check.py absent b1 0x247db064
--src ...` [receipt:78f2aa0a]. Twin opens: B1 `FUN_0044c818` / B2 `FUN_005404f0` call the 2-D and 3-D pans the same way;
B1 `FUN_00448ed8` / B2 `FUN_0053cce0` run the group pass on their groups (0x66f130 / 0x73dad0).

## 4. Verifier

Workflow `mixh-re-verify` (6 agents, medium effort, claims and citations only): four batches (A level chain, B voices,
C buses + E port, D reproductions) and two second verifiers, each blind to the other, for every twin and absence claim
(A1, A3, A4, A5, B1, B2, B4, B5, B6, C1, C2). 22 claims: 18 SUPPORTED, 4 PARTLY (B4, B5, C1, D5), 0 NOT SUPPORTED.
Folded in: B4 - the steal takes the list's TAIL (lowest value), and BF2's steal value is the property bus's +0x28 alone,
priority pair at prop+0x50 (both readers found it); B5 - the value is 0 unless the state flag is 1 (both); C1 - 3
occurrences, BF1's two CORE.LVL copies (the second reader; the first supported the substance); D5 - the "in battle"
filter was circular: six BF2 spawn-named states at 0.22-0.25 now listed and tested against same-map states (1.4 claim
12). Overturned by the second verifier: 1 (C1's count as written). Also noted: `levelcensus.py` has no roll-in branch
(properties with RollInDistance read "below"); the table first used `--skip 8`, which hid UTA1's 2 runs at 4.3 s -
battle audio, the state resumes mid-fight; the table now counts whole captures; the reverb path has clamps of its own
(FxVol is 0 in battle).

## 5. Not settled

- Whether the sliders read (BF2 effects 7) are the factory defaults [H].
- The space term's full chain (doubling x space mean) outside the default space; Kashyyyk's higher ratios.
- What IOP command 0x15 drives (the 0.3 s mode-change fade).
- The 39-voice reorder's exact order (claim 9 [D]).
