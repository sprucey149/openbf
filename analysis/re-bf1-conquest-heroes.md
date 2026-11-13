# BF1 Conquest hero assignments

BF1 PS2 mission evidence for issue #166, checked November 13, 2026.
[O] UNAUDITED: independently repeated source and gameplay acceptance remain open.
This establishes the authored identities, not original-game play acceptance.

## Original assignments

[V] The 28 supported BF1 Conquest scripts contain 56 literal
`SetHeroClass` assignments across 16 maps: 14 Clone Wars missions and
14 Galactic Civil War missions. Each assigns both playable teams.
The mission interpreter reads the Lua 4 instruction operands and reconstructs
the arguments; it does not infer a hero from a loaded SIDE class name.
The BF1 binding at `0x184f00` (B1:27153–27169) reads team and class;
`FUN_002a41a8` (B1:231641–231651) hashes the supplied class into the
team-indexed hero slot. This supports retaining the mission's team numbers.

- Every CW mission assigns Mace Windu to Republic and Count Dooku to CIS.
  Checked scripts: `tat1r`, `tat2r`, `bes1r`, `bes2r`, `geo1r`, `kam1c`,
  `kas1c`, `kas2c`, `nab1c`, `nab2c`, `rhn1r`, `rhn2c`, `yav1c`, `yav2r`.
- Every GCW mission assigns Luke Skywalker to Alliance and Darth Vader
  to Empire. Checked scripts: `tat1i`, `tat2i`, `bes1a`, `bes2a`, `end1a`,
  `hot1i`, `kas1i`, `kas2i`, `nab1i`, `nab2a`, `rhn1i`, `rhn2a`, `yav1i`,
  `yav2i`. Only `hot1i` and `rhn1i` select snow Luke.
- Team 1 is not always the same faction. For example, Bespin Platforms
  GCW uses Alliance first; Dune Sea GCW uses Empire first. Kamino CW
  uses CIS first. Generation preserves these assignments, including when
  the user reverses the teams; a foreign faction receives no opposing hero.

[V] BF2's `SetHeroClass` binding at `0x18cb18` (B2:32701–32717)
likewise delegates the named class to its team (`0x36e800`). The original
game's PC binding at `0x58bfe0` supplies the same team/special-class lead;
the PS2 evidence is authoritative here. The existing
[hero flow analysis](re-bf2-hero-flow.md) owns offers, eligibility, health,
AI opt-in, audio and remaining original-game comparisons.

## Deliberate port choices

[D] Following the [canonical hero decision](plan/FEATURE_SET.md#heroes),
BF1 Conquest uses BF2's playable kits and the existing hero manager:
killable heroes, health drain and kill healing, player offers and the
optional AI eligibility switch. This does not import BF1 invincibility or
automatic NPC spawning. BF2 HUD applies on these maps too.

- Ordinary Luke maps use `all_hero_luke_jedi`; the two snow assignments
  use `all_hero_luke_pilot`. These retain the original identity and snow
  distinction with BF2's outfits, rather than BF1's body/animation assets.
- Vader, Mace and Dooku use `imp_hero_darthvader`, `rep_hero_macewindu`
  and `cis_hero_countdooku`. Infantry rosters and their min/max counts stay
  separate from the appended one-per-team hero entries.
- BF1 infantry does not register BF2 custom animation descriptors. The
  selected BF2 hero sublevels register in mission team order, with the
  existing first-definition-wins rule. BF2 missions retain their original
  complete SIDE load order. This BF1 registration order is a port choice.
- The class's source selects its body, rig, low companion, skins, hand
  weapons and first-person bank. BF2 saber bindings and textures load on
  BF1 maps. Force effects add BF2 layers after existing native layers.
- BF1 battles import missing sound dependencies for the selected hero
  kits from the BF2 maps that use them. Existing BF1 definitions retain priority.
  Imported sample paths and stream/music records retain their source.
  Thus coincident BF1 sound names can keep their native presentation.

The stock identity conversion and the faction-specific choices below apply
after the canonical mission overlay. Supported source combinations share
the assignments. Unknown BF1 hero names fail generation. Hunt and Hero
Assault have their own roster paths.

### BF1 map-specific matchups

The maintainer approved five Conquest matchups for variety using base-game
BF2 kits. The mission generator keys the changes by map, era and faction.
The other 23 BF1 Conquest missions keep their original hero identities.

- Bespin: Cloud City, Galactic Civil War — Han Solo and Boba Fett.
- Kamino: Tipoca City, Clone Wars — Obi-Wan Kenobi and Jango Fett.
- Kashyyyk: Islands, Clone Wars — Yoda and Count Dooku.
- Kashyyyk: Docks, Galactic Civil War — Chewbacca and Darth Vader.
- Naboo: Theed, Clone Wars — Obi-Wan Kenobi and Darth Maul.

## Verification boundary

The focused checks enumerate all 56 assignments, retain infantry, load the
12 BF2 bodies and animation banks, resolve sound takes/themes and stage
the shared manager's player offer/accept and optional AI path. Headless
GPU checks cover bodies, held weapons and blade submission. These checks require
locally owned data and missions generated into an owned output folder.
They do not certify a packaged Instant Action round, live controller
acceptance, animation feel, audio mix or an original-game comparison.

[V, bounded port gap] The first owned-data census found no selectable
native BF1 foley row for Mace or Dooku on the 14 CW missions. With the
varied roster the same bake reports 27 of 28 CW assignments without a
native row. Jango on Kamino and the GCW assignments resolve. Original
fallback behavior and the port's CW hero movement foley remain open.
Both Luke arms loaded on a fresh mixed bake in the earlier integration.
Live first-person presentation and weapon models across bake profiles
remain open.
