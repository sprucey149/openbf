---
id: VOICEP
title: Voice playback keeps the selected definition and positional rate consistent
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
A commander line can be played at a different pitch and bus gain from the
definition that supplied its take. The queue then waits for a different duration
from the playback. Positional squad answers also ignore the active slow-sound
multiplier. These are GSND findings 2 and 3, grouped because they share one file.

## Evidence
On main 1e781f3, temporary tests load generated sound RON and one-second mono
WAVs through SoundLib::load and configure the battle through for_battle.
For a Republic/Empire battle loading kas2gcw, rep_test_voice resolves kas2cw's
sample 2, pitch 1.6, gain 0.5 and voiceover bus. The other definition has sample
1, pitch 0.8, gain 0.9 and soundfx bus. play_voice selects sample 2 and returns
0.625 seconds, but Handed records pitch 0.8, duration 1.25 and ears (0.5,0.5)
instead of the selected definition's (0.32,0.32). Separate pitch and bus
assertions fail. play_take at rate_mul=0.3 records pitch 0.8 instead of 0.24;
duration is 1.25 rather than 4.166667 seconds. That assertion also fails.
All actual caller tests use a null ALSA device, without skips. A same-era
commander, slow-exempt commander, ordinary-rate positional answer, missing name
and missing take controls pass. Production source was restored byte-for-byte.
These are synthetic runtime observations, not measured shipped-bank prevalence.

Existing policy: sounds.rs find_named and foley::foreign_armies choose the
foreign army's own era; play_voice documents the commander exemption from slow.
windowed.rs's slow cheat sets SoundLib::rate_mul, and sounds.rs::mix applies the
product before the 0.05 floor for other positional starts. No new PS2 rule or
sound constant is introduced. Existing audio references remain
analysis/re-announcer.md, re-mix-headroom.md and re-weapon-sounds.md.

## Scope
Only GSND findings 2 and 3. Keep take selection, gain, bus, pitch and returned
duration consistent with the ONE property selected for a commander start.
Preserve voice_take's public API and weighting; do not draw a second take.
For play_take, apply the existing positional rate product and floor to both
playback and returned duration. Preserve its current hashed property/take
lookup policy; changing foreign positional-line selection is outside this slice.
Do not edit simulation, event schema, spawn sounds, hover filters, foley tables,
bus gains, cheat values, random gates, ducking or voice-queue policy. GSND
retains findings 1, 4, 5, merged SPWS and its entire original Acceptance.

## Acceptance
1. Permanent generated RON/WAV regressions cover a foreign commander with
   deliberately different samples, gains, pitches and bus identities. Assert
   the selected file and one draw, exact resolved gain/ears/pitch and agreement
   of returned seconds with decoded duration / actual rate. Exercise case-blind
   names, same-era and missing-foreign fallback, resident CORE fallback, unknown
   names and missing files. Normal and slow commander playback stay identical.
2. For a fixed positional take, cover rate_mul 1.0 and 0.3, off-centre
   attenuation/pan, muted/out-of-range and absent nth controls. Assert actual
   rate and duration; a low base pitch must floor the PRODUCT at 0.05, not
   clamp its factors first. Preserve existing take identity and selection.
3. Core regressions must execute without an audio device, using the same
   preparation path unconditionally used by playback; do not skip them when
   Audio::new returns None. Retain an actual public-caller run with an explicit
   null ALSA config where available, checking Handed and returned duration.
   Reproduce parent assertion failures; separately restore wrong pitch lookup,
   wrong bus lookup and missing rate multiplier and require corresponding
   assertions to fail. A second-take draw mutation must fail the selection
   check; use distinguishable weighted takes, not a single-take coincidence.
4. Run focused sounds tests, full game library, fmt and workspace all-target
   Clippy sequentially, plus full CI and unchanged applicable map/protect,
   audio, sim/sweep/arena/leak/bake/GPU/golden/parity/behavior gates. Keep module
   docs/CRATES rows accurate and existing size policy. Any test extraction
   necessary for size stays within sounds.rs and sounds/tests.rs; no broad split.
5. With actual BF1/BF2 sound banks and mission definitions, census conflicting
   named/hashed resolutions for all selectable mixed-faction battles: property,
   level, takes, pitch, gain and bus. Report every affected row or measured zero;
   compare actual playback rate/returned duration before/after. Verify positional
   squad answers with slow off/on and commander exemption through a real battle
   and sound log. Preserve GSND/SPWS's separate primary-evidence and all-gate
   requirements. Fixtures alone do not earn real-content acceptance.
