# Optional DLC heroes

Kit Fisto and Asajj Ventress are optional. A build without them plays exactly as
it does with no DLC at all. Nothing else in the DLC is used: no maps, missions,
scripts, menus or localization. Nothing from it belongs in git.

## Adding them

They come from the community PC conversion of the Xbox Battlefront II DLC, not
from this project or from either game disc. Its addon folder is usually called
`DLC_XBOX` and holds `addme.script` and `data/_LVL_PC/`. Put that whole folder
anywhere under `iso/`, under any name. The other folders in such a pack, such as
`UPDATE_XBOX`, are recognized and ignored.

```text
iso/
  (your BF1 and BF2 discs, as usual)
  DLC_XBOX/
    addme.script
    data/_LVL_PC/SIDE/dlc.lvl
    data/_LVL_PC/SOUND/hero.lvl
    ...
```

Then build as usual. `python3 scripts/input_scan.py` prints what it found and
says `Kit Fisto and Asajj Ventress used`. Adding or removing the folder makes the
next `scripts/pack.py` bake everything again.

A disc that was rebuilt to hold the DLC is refused, as any modified disc is.

## In the game

On every BF2 battle with a Republic or CIS hero, Kit Fisto is the Republic's and
Asajj Ventress the CIS's. A battle has one hero per team, so they stand in for the
map's usual Republic or CIS hero whenever the bake holds them. Alliance and Empire battles
keep their own heroes. BF1 maps have none. The usual hero options, score offer,
acceptance, health drain and death rules apply.

What the heroes do never depends on which copy of the DLC went in: the PS2 and
PC formats give the same combat, checked hash for hash. A build with them is not
the same game as a build without them, though: every other input changes only the
look, and these change who fights, so the two do not stay in step with each other.

## What they are missing

- **Voices.** The PC folder's heroes name no voice lines and it holds none, so
  they say nothing of their own. The Republic and CIS order responses and their
  hero music come from the game itself; the PC folder gives Ventress the Republic's
  response, so the bake gives her the CIS's.
- **Tentacle animation.** The DLC holds none. Kit Fisto's tentacles use a small
  wave written for this port, seen only on screen.

Their own effects (hit stars, Fisto's bubble blast, the starblade's trail) and
Fisto's bubble sounds are baked.

## How the bake works

`scripts/rebake.sh` runs `scripts/bake_dlc.py` as its `dlc` group, and after its
`side` group, which clears the heroes' files. It passes the folder the scan chose
in `OPENBF_DLC_DIR`, and `OPENBF_DLC_DONOR` when a PC copy can lend a PS2 copy its
larger textures. Without a folder, it removes an earlier bake's enabling files.

```sh
REBAKE_OUT="$PWD/my-bake" scripts/rebake.sh dlc  # a relative path would be under engine/
```

`detect_dlc` in `scripts/dlc_inputs.py` accepts a folder, side file or ISO and
returns `DlcSource` records with `path`, `platform`, `member`, `offset`, `size`
and bounded `read_bytes()`. It checks parsed class records and geometry format,
never a file name. A retail disc returns an empty list. Multiple records are
alternative copies; use one. `detect_hero_sounds` finds the smallest LVL holding
the bubble sounds' name hash.

The bake writes `classes/bf2_dlc.ron`, the heroes' assets under `side_bf2`, their
effects in `side_bf2/effects/dlc`, then `side_bf2/dlc_heroes.ron` as its completion
marker. Their optional sounds follow in `side_bf2/dlc_sounds`: a bank that cannot be
read leaves the heroes without them. A rebake removes the marker before reading the
source. The game reads none of it without the marker.

Hero dependency classes and textures have their own `dlch_` names, and all outputs
are staged before installation, so a shared source name cannot replace a retail
class or texture. The effect layer is read after every retail one, so a shared
effect name stays retail's, and likewise for sounds. Any console BF2 `INGAME.LVL`
serves as the animation reference: the PS2 and Xbox ones give identical output.

The supplied melee programs use the existing saber implementation. Two source
quirks are accepted explicitly: `BendTimeTo` becomes `BlendTimeTo`, and numeric
trailing slots after an offhand clip/channel are retained. Kit Fisto's tentacle
mesh is folded into his model with its own joints, appended after the body's, so
no body joint moves.

Community hero recreation: **Gogie and Ryan Hank**. DLC hero assembly:
**iamashaymin**. These credits come from the source pack's supplied credits;
its updated Ventress model also credits Gogie. These assets remain local to
the player's own bake.
