#!/usr/bin/env bash
# Rebake every baked asset the engine ships with, from the local extracted
# discs. Output goes to engine/assets_baked/ (gitignored), or to $REBAKE_OUT
# when set (a staging tree). Run from anywhere:
#
#   scripts/rebake.sh            # everything
#   scripts/rebake.sh tat1 side  # only the named groups
#
# Groups: REBAKE_GROUPS below; an unknown name is refused (exit 2) before
# anything is built, so a typo cannot bake nothing and report success.
# Sources are read-only: extracted/, and the inputs scripts/input_scan.py finds by
# their content in iso/ (run by hand; under scripts/pack.py the scan already chose).
set -euo pipefail

# land.py reads this line too (it refuses a card whose `rebake=` names another).
REBAKE_GROUPS="tat1 tat2 maps common side dlc classes sounds global-sound load beep"
CLI_PC_DIR=""
MENU_FONT_CORE="${OPENBF_MENU_FONT_CORE:-}"
WANT_GROUPS=()
while [ $# -gt 0 ]; do
  case "$1" in
    --menu-font-core) [ $# -ge 2 ] || { echo "rebake: --menu-font-core needs a local PS2 BF2 CORE.LVL" >&2; exit 2; }; MENU_FONT_CORE="${2//\\//}"; shift 2 ;;
    --menu-font-core=*) v="${1#*=}"; MENU_FONT_CORE="${v//\\//}"; shift ;;
    --pc-dir) [ $# -ge 2 ] || { echo "rebake: --pc-dir needs a directory" >&2; exit 2; }; CLI_PC_DIR="${2//\\//}"; shift 2 ;;
    --pc-dir=*) v="${1#*=}"; CLI_PC_DIR="${v//\\//}"; shift ;;
    --pc1) [ $# -ge 2 ] || { echo "rebake: --pc1 needs a directory" >&2; exit 2; }; PC1="${2//\\//}"; shift 2 ;;
    --pc1=*) v="${1#*=}"; PC1="${v//\\//}"; shift ;;
    --pc2) [ $# -ge 2 ] || { echo "rebake: --pc2 needs a directory" >&2; exit 2; }; PC2="${2//\\//}"; shift 2 ;;
    --pc2=*) v="${1#*=}"; PC2="${v//\\//}"; shift ;;
    *) WANT_GROUPS+=("$1"); shift ;;
  esac
done
for g in ${WANT_GROUPS[@]+"${WANT_GROUPS[@]}"}; do
  case " $REBAKE_GROUPS " in
    *" $g "*) ;;
    *) echo "rebake: unknown group '$g' (groups: $REBAKE_GROUPS)" >&2; exit 2 ;;
  esac
done

# Under Git for Windows' bash (card WIN, scripts/pack.py) `pwd -W` gives C:/... : the bake.exe it runs reads
# those as they are, comma-joined lists included (MSYS converts only whole /c/... arguments).
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && { pwd -W 2>/dev/null || pwd; })"
ci() {
  local cur="$1" rel="$2" part found entry base p_lc b_lc
  [ -e "$cur/$rel" ] && { printf '%s\n' "$cur/$rel"; return 0; }
  local IFS=/
  for part in $rel; do
    [ -z "$part" ] && continue
    found=""
    if [ -d "$cur" ]; then
      p_lc=$(printf '%s' "$part" | tr 'A-Z' 'a-z')
      for entry in "$cur"/*; do
        [ -e "$entry" ] || continue
        base="${entry##*/}"
        b_lc=$(printf '%s' "$base" | tr 'A-Z' 'a-z')
        if [ "$b_lc" = "$p_lc" ]; then
          found="$entry"
          break
        fi
      done
    fi
    cur="${found:-$cur/$part}"
  done
  printf '%s\n' "$cur"
}

# scripts/pack.py passes the interpreter running it. Run by hand, take the first that is Python 3: on Windows `python3` is
# often only the Store's stand-in, which runs nothing, and on an older Mac `python` is Python 2.
PYTHON="${OPENBF_PYTHON:-}"
if [ -z "$PYTHON" ]; then
  for candidate in python3 python; do
    if "$candidate" -c 'import sys; sys.exit(sys.version_info[0] != 3)' >/dev/null 2>&1; then PYTHON="$candidate"; break; fi
  done
  [ -n "$PYTHON" ] || { echo "rebake: no Python 3 found (tried python3 and python)" >&2; exit 2; }
fi

# Run by hand, the scan supplies what the command line left out. Its SCAN_* are
# empty for anything not chosen. Under pack.py the choice arrives in the environment.
SCAN_PC1=""; SCAN_PC2=""; SCAN_XB1=""; SCAN_XB2=""; SCAN_BF2_CONSOLE_ISO=""
SCAN_OPENBF_BF1_TREE=""; SCAN_OPENBF_BF2_TREE=""
SCAN_OPENBF_DLC_DIR=""; SCAN_OPENBF_DLC_DONOR=""
SCAN_OPENBF_BF1_WITHHELD=""; SCAN_OPENBF_BF2_WITHHELD=""
if [ -z "${OPENBF_INPUTS_CHOSEN:-}" ]; then
  SCAN="$("$PYTHON" "$ROOT/scripts/input_scan.py" --root "$ROOT" --shell | tr -d '\r')" || exit 2
  eval "$SCAN"
  if [ -z "$CLI_PC_DIR${PC1:-}${PC2:-}" ]; then
    PC1="$SCAN_PC1"; PC2="$SCAN_PC2"
  fi
fi
OPENBF_BF1_TREE="${OPENBF_BF1_TREE:-$SCAN_OPENBF_BF1_TREE}"
OPENBF_BF2_TREE="${OPENBF_BF2_TREE:-$SCAN_OPENBF_BF2_TREE}"
BF2_CONSOLE_ISO="${BF2_CONSOLE_ISO:-$SCAN_BF2_CONSOLE_ISO}"
# Xbox texture donors: the trees `bake extract-xiso` wrote. Empty until pack.py chooses an Xbox disc.
XB1="${XB1:-$SCAN_XB1}"; XB2="${XB2:-$SCAN_XB2}"
# The optional DLC heroes' folder, and a second copy's larger textures. Empty without them.
OPENBF_DLC_DIR="${OPENBF_DLC_DIR:-$SCAN_OPENBF_DLC_DIR}"; OPENBF_DLC_DONOR="${OPENBF_DLC_DONOR:-$SCAN_OPENBF_DLC_DONOR}"

is_pc_lvl_dir() {
  "$PYTHON" "$ROOT/scripts/console_input.py" --pc "$2" "$1"
}

find_pc_lvl() {
  local base="${1//\\//}" want="$2" d
  for d in "$base" "$(ci "$base" GameData/Data/_LVL_PC)"; do
    if is_pc_lvl_dir "$d" "$want"; then printf '%s\n' "$d"; return 0; fi
  done
  if [ -d "$base" ]; then
    while IFS= read -r d; do
      if [ -n "$d" ] && is_pc_lvl_dir "$d" "$want"; then printf '%s\n' "$d"; return 0; fi
    done < <(find "$base" -maxdepth 6 -type d -iname '_lvl_pc' 2>/dev/null)
  fi
  return 1
}

[ -n "${PC1:-}" ] && PC1="$(find_pc_lvl "$PC1" bf1 || printf '%s' "${PC1//\\//}")"
[ -n "${PC2:-}" ] && PC2="$(find_pc_lvl "$PC2" bf2 || printf '%s' "${PC2//\\//}")"
if [ -n "$CLI_PC_DIR" ]; then
  [ -n "${PC1:-}" ] || PC1="$(find_pc_lvl "$CLI_PC_DIR" bf1 || true)"
  [ -n "${PC2:-}" ] || PC2="$(find_pc_lvl "$CLI_PC_DIR" bf2 || true)"
fi

if [ -n "$CLI_PC_DIR" ] && [ -z "${PC1:-}${PC2:-}" ]; then
  echo "rebake: no recognized BF1/BF2 PC data under $CLI_PC_DIR" >&2; exit 2
fi
for game in 1 2; do
  var="PC$game"; input="${!var:-}"
  if [ -n "$input" ] && ! is_pc_lvl_dir "$input" "bf$game"; then
    echo "rebake: $input is not a recognized BF$game PC install" >&2; exit 2
  fi
done

E="$ROOT/extracted"
# The selected BF2 copy supplies the shared menus, shell and BF2 HUD. A selected
# PC BF2 primary reaches this script through OPENBF_BF2_TREE.
# OPENBF_BF*_TREE: a game the scan chose non-PS2 gameplay data for (no PS2 copy).
UI2="${OPENBF_BF2_TREE:-$E/bf2/lvl/DATA/_LVL_PS2}"
if [ -n "$CLI_PC_DIR${PC1:-}${PC2:-}" ] && [ -z "${OPENBF_BF2_TREE:-}" ]; then
  "$PYTHON" "$ROOT/scripts/console_input.py" "$UI2"
fi
B1="${OPENBF_BF1_TREE:-$E/bf1/lvl/DATA/_LVL_PS2}"
B2="$UI2"
S1="$E/bf1/sound/DATA/_LVL_PS2"
S2="$E/bf2/sound/DATA/_LVL_PS2"
# scripts/pack.py extracts each disc whole into lvl/ (bake extract-iso): the
# sound LVLs are then beside the rest.
{ [ -d "$S1" ] && [ -z "${OPENBF_BF1_TREE:-}" ]; } || S1="$B1"
{ [ -d "$S2" ] && [ -z "${OPENBF_BF2_TREE:-}" ]; } || S2="$B2"
OUT="${REBAKE_OUT:-$ROOT/engine/assets_baked}"

# Classic-plus tier 2: the PC install the scan chose (or one named on the command
# line); a folder the scan refused is never read. Any texture the PC ships
# LARGER than the PS2 one adds an optional tier; the PS2 base is preserved
# (bake::tier; the maintainer's chain is
# Harrisonfog HD > PC stock where larger > our upscale > PS2 original).
# Explicit --pc1 also adds BF1 map texture tiers. The install is gitignored
# and optional: without it every bake is PS2-only, which is a real difference
# in output, so the script SAYS which way it ran rather than deciding quietly.
PC=""
if [ -n "${PC1:-}${PC2:-}" ]; then
  if [ -n "${PC1:-}" ] && [ -z "${OPENBF_BF1_TREE:-}" ]; then
    "$PYTHON" - "$ROOT/scripts" "$B1" <<'PYREF'
import sys
sys.path.insert(0, sys.argv[1])
from console_input import require_ps2_bf1_reference
try:
    require_ps2_bf1_reference(True, sys.argv[2])
except ValueError as error:
    sys.exit("rebake: " + str(error))
PYREF
  fi
  PC="${PC2:-}"
fi
echo "Gameplay data: BF1=${OPENBF_BF1_TREE:-PS2} BF2=${OPENBF_BF2_TREE:-PS2}"
echo "Texture donors: PC BF1=${PC1:-none} PC BF2=${PC2:-none} Xbox BF1=${XB1:-none} Xbox BF2=${XB2:-none}"
# Every copy of an LVL the bake may take a larger texture from, as ONE comma-joined argument (bake::tier):
# empty arguments and absent files are skipped, so a partial install degrades to the PS2 alone.
join_donors() {
  local list="" f
  for f in "$@"; do
    if [ -n "$f" ] && [ -f "$f" ]; then list="${list:+$list,}$f"; fi
  done
  printf '%s' "$list"
}
xbox_lvl() {  # <bf1|bf2> <lvl path under the game's data>
  local tree="$XB1"
  [ "$1" = bf2 ] && tree="$XB2"
  if [ -n "$tree" ]; then ci "$tree" "$2"; fi
  return 0
}
# Set PC_ARG to one map's donors, or to nothing. An ARRAY, because the install
# path contains spaces ("Star Wars Battlefront II Classic").
PC_ARG=()
map_donors() {  # <bf1|bf2> <lvl path under the game's data>
  local root="${PC1:-}" pc="" list
  [ "$1" = bf2 ] && root="$PC"
  if [ -n "$root" ]; then pc="$(ci "$root" "$2")"; fi
  list="$(join_donors "$pc" "$(xbox_lvl "$1" "$2")")"
  PC_ARG=()
  if [ -n "$list" ]; then PC_ARG=("$list"); fi
  return 0
}
# One side's soldier, weapon and vehicle donors: the PC's SIDE LVL, then the Xbox's.
side_donors() {  # <bf1|bf2> <side, lowercase>
  local root="${PC1:-}" pc=""
  [ "$1" = bf2 ] && root="$PC"
  if [ -n "$root" ]; then pc="$(ci "$root" "side/$2.lvl")"; fi
  join_donors "$pc" "$(xbox_lvl "$1" "side/$2.lvl")"
}
# The data roots an FPM LVL's copy is looked for under, as `bake fpm` options. Fills FPM_DONORS, an array: the install
# path holds spaces.
fpm_donors() {  # <bf1|bf2>
  local root="${PC1:-}" tree="$XB1" d
  [ "$1" = bf2 ] && { root="$PC"; tree="$XB2"; }
  FPM_DONORS=()
  for d in "$root" "$tree"; do
    if [ -n "$d" ] && [ -d "$d" ]; then FPM_DONORS+=(--donor "$d"); fi
  done
}
DS1_ALL="$(side_donors bf1 all)"; DS1_IMP="$(side_donors bf1 imp)"
DS1_REP="$(side_donors bf1 rep)"; DS1_CIS="$(side_donors bf1 cis)"
DS2_ALL="$(side_donors bf2 all)"; DS2_IMP="$(side_donors bf2 imp)"
DS2_REP="$(side_donors bf2 rep)"; DS2_CIS="$(side_donors bf2 cis)"

cd "$ROOT/engine"
cargo build -p bake --release
BAKE="${CARGO_TARGET_DIR:-$ROOT/engine/target}/release/bake"
[ -f "$BAKE.exe" ] && BAKE="$BAKE.exe"

SEL=(${WANT_GROUPS[@]+"${WANT_GROUPS[@]}"})
want() {
  [ -z "${SEL[0]:-}" ] && return 0
  for g in ${SEL[@]+"${SEL[@]}"}; do [ "$g" = "$1" ] && return 0; done
  return 1
}

# Each bake owns a few subdirectories; clear those (and only those) so a
# renamed or dropped output cannot linger as a stale file.
clean() { local t; for t in "$@"; do rm -rf "$t"; done; }

# The heroes' bake refuses a link, so say so before any group runs, and before `clean` below deletes through it what
# another tree's bake holds. A link with no folder to bake is still fine for `side`, which must clear the heroes it deletes.
if [ -L "$OUT" ] && { want side || want dlc; } && { [ -n "$OPENBF_DLC_DIR" ] || ! want side; }; then
  echo "rebake: $OUT is a link to other assets; set REBAKE_OUT to a folder of your own" >&2; exit 2
fi

# Every other ground map, baked to assets_baked/<game>_<lvl> (the LVL name,
# not the world name inside it: BF1 and BF2 both ship "hoth", "yavin1", ...).
# Space (SPA*) follows below; OPENBF_BAKE_SPACE=0 leaves it out.
BF1_MAPS="BES/BES1 BES/BES2 END/END1 GEO/GEO1 HOT/HOT1 KAM/KAM1 KAS/KAS1 KAS/KAS2 NAB/NAB1 NAB/NAB2 RHN/RHN1 RHN/RHN2 TAT/TAT2 YAV/YAV1 YAV/YAV2"
BF2_MAPS="COR/COR1 DAG/DAG1 DEA/DEA1 END/END1 FEL/FEL1 GEO/GEO1 HOT/HOT1 KAM/KAM1 KAS/KAS2 MUS/MUS1 MYG/MYG1 NAB/NAB2 POL/POL1 TAN/TAN1 TAT/TAT3 UTA/UTA1 YAV/YAV1"
BF1_MAPS="${OPENBF_BF1_MAPS:-$BF1_MAPS}"
BF2_MAPS="${OPENBF_BF2_MAPS:-$BF2_MAPS}"
# Maps the chosen gameplay source cannot build alone (scripts/input_scan.py WITHHELD_MAPS). A map not baked is not offered.
without_maps() {  # <list> <withheld>
  local out="" m w keep
  for m in $1; do
    keep=1
    for w in $2; do [ "$m" = "$w" ] && keep=0; done
    [ "$keep" = 1 ] && out="$out $m"
  done
  printf '%s' "${out# }"
}
BF1_MAPS="$(without_maps "$BF1_MAPS" "${OPENBF_BF1_WITHHELD:-$SCAN_OPENBF_BF1_WITHHELD}")"
BF2_MAPS="$(without_maps "$BF2_MAPS" "${OPENBF_BF2_WITHHELD:-$SCAN_OPENBF_BF2_WITHHELD}")"
# An earlier bake's copy of a left-out map would still be offered, so it goes too. Never through a link to other assets.
if want maps && [ ! -L "$OUT" ]; then
  for w in ${OPENBF_BF1_WITHHELD:-$SCAN_OPENBF_BF1_WITHHELD}; do
    clean "$OUT/bf1_$(basename "$w" | tr 'A-Z' 'a-z')"
  done
  for w in ${OPENBF_BF2_WITHHELD:-$SCAN_OPENBF_BF2_WITHHELD}; do
    clean "$OUT/bf2_$(basename "$w" | tr 'A-Z' 'a-z')"
  done
fi


if want tat1; then
  echo "== TAT1 (BF1) =="
  clean "$OUT/tat1/models" "$OUT/tat1/textures" "$OUT/tat1/collision" "$OUT/tat1/nav" "$OUT/tat1/shadows"
  map_donors bf1 TAT/TAT1.LVL
  "$BAKE" map "$(ci "$B1" TAT/TAT1.LVL)" "$OUT/tat1" ${PC_ARG[@]+"${PC_ARG[@]}"}
fi
if want tat2; then
  echo "== TAT2 (BF2) =="
  clean "$OUT/tat2/models" "$OUT/tat2/textures" "$OUT/tat2/collision" "$OUT/tat2/nav" "$OUT/tat2/shadows"
  map_donors bf2 tat/tat2.lvl
  "$BAKE" map "$(ci "$B2" TAT/TAT2.LVL)" "$OUT/tat2" ${PC_ARG[@]+"${PC_ARG[@]}"}
fi
if want common; then
  echo "== common assets =="
  clean "$OUT/common_bf1/models" "$OUT/common_bf1/textures" "$OUT/common_bf1/collision" \
        "$OUT/common_bf2/models" "$OUT/common_bf2/textures" "$OUT/common_bf2/collision" \
        "$OUT/common_bf1/fonts" "$OUT/common_bf1/locl" \
        "$OUT/common_bf2/fonts" "$OUT/common_bf2/hud" "$OUT/common_bf2/locl"
  "$BAKE" common "$(ci "$B1" COMMON/ENG/COMMON.LVL)" "$OUT/common_bf1"
  "$BAKE" common "$(ci "$UI2" INGAME.LVL)" "$OUT/common_bf2"
  # Fonts keep the selected source's layout metrics. Compare matching glyph
  # art only against copies explicitly selected by the input planner.
  FONT_DONORS=()
  for donor in "$PC" "$XB2"; do
    if [ -n "$donor" ] && [ -f "$(ci "$donor" CORE.LVL)" ]; then
      FONT_DONORS+=("$(ci "$donor" CORE.LVL)")
    fi
  done
  MENU_FONT_ARGS=()
  if [ -n "$MENU_FONT_CORE" ]; then
    MENU_FONT_ARGS=(--menu-font-core "$MENU_FONT_CORE")
  fi
  # The selected PS2 copy is already preferred by the input planner. An
  # optional explicit PS2 font source changes only menu glyphs. Otherwise
  # native PC/Xbox glyphs get a similar treatment with different source art.
  if [ -n "${FONT_DONORS[0]:-}$MENU_FONT_CORE" ]; then
    "$BAKE" fonts "$(ci "$UI2" CORE.LVL)" "$OUT/common_bf2" "" ${FONT_DONORS[@]+"${FONT_DONORS[@]}"} ${MENU_FONT_ARGS[@]+"${MENU_FONT_ARGS[@]}"}
  fi
  if [ -n "${FONT_DONORS[0]:-}" ]; then
    "$BAKE" ui-art "$(ci "$UI2" CORE.LVL)" "$OUT/common_bf2" ${FONT_DONORS[@]+"${FONT_DONORS[@]}"}
  fi
  # The specular pass's lookup table, from each game's own CORE.LVL (the
  # texels are identical on both discs; analysis/re-specular-pass.md §5).
  "$BAKE" specular "$(ci "$B1" CORE.LVL)" "$OUT/common_bf1"
  "$BAKE" specular "$(ci "$UI2" CORE.LVL)" "$OUT/common_bf2"
  # BF2's terrain light pass texture (card FXBL, analysis/re-bolt.md §5.3):
  # BF1 draws no light on its terrain (§5.1).
  "$BAKE" omniproj "$(ci "$UI2" CORE.LVL)" "$OUT/common_bf2"
  # Each game's English text table (card EVMSG, analysis/re-event-messages.md):
  # the HUD's event lines, team, post and soldier names.
  "$BAKE" locl "$(ci "$B1" CORE.LVL)" "$OUT/common_bf1"
  "$BAKE" locl "$(ci "$UI2" CORE.LVL)" "$OUT/common_bf2"
  # The original Xbox pad's button icons, from the Xbox BF2 disc the scan chose
  # or, run by hand, a dev tree's own extraction; under pack.py only what it
  # chose is read. The game draws its own without one (game::input_icons).
  XBOX_COMMON="$(xbox_lvl bf2 common.lvl)"
  if [ -z "$XBOX_COMMON" ] && [ -z "${OPENBF_INPUTS_CHOSEN:-}" ]; then
    XBOX_COMMON="$(ci "$E/xbox_bf2" common.lvl)"
  fi
  if [ -f "$XBOX_COMMON" ]; then
    "$BAKE" xbox-icons "$XBOX_COMMON" "$OUT/common_bf2"
  fi
  # BF1's own bitmap fonts (card SQHUD: the squad cross's labels are BF1's
  # gamefont_tiny), prefixed so they sit beside BF2's in one name space.
  FONT_DONORS=()
  for donor in "${PC1:-}" "$XB1"; do
    if [ -n "$donor" ] && [ -f "$(ci "$donor" CORE.LVL)" ]; then
      FONT_DONORS+=("$(ci "$donor" CORE.LVL)")
    fi
  done
  "$BAKE" fonts "$(ci "$B1" CORE.LVL)" "$OUT/common_bf1" bf1_ ${FONT_DONORS[@]+"${FONT_DONORS[@]}"}
  # BF2's PS2 shell (card BSHLA, analysis/re-bf2-shell.md §1, §4): SHELL.LVL's
  # logo, the background movie's menu loops (ffmpeg transcodes them to frame
  # packs) and SOUND/SHELL.LVL's bank + music index. The movie is read from
  # the extracted tree when `bake extract-iso` kept it, else out of the disc
  # image (a dev tree's extracted discs leave MOVIES/ out).
  # The disc image is the one the scan chose (a worktree without iso/ scans the
  # main checkout's).
  echo "Shared UI: $UI2; shell sound: $S2"
  MVS="$(ci "$UI2" MOVIES/SHELL.MVS)"
  [ -f "$MVS" ] || MVS="$BF2_CONSOLE_ISO"
  if [ ! -f "$MVS" ]; then
    echo "rebake: common needs MOVIES/SHELL.MVS: no copy in $UI2 and no BF2 PS2 disc was chosen. Add the disc to iso/." >&2
    exit 2
  fi
  clean "$OUT/common_bf2/shell"
  "$BAKE" shell "$(ci "$UI2" SHELL.LVL)" "$MVS" "$(ci "$S2" SOUND/SHELL.LVL)" "$(ci "$UI2" CORE.LVL)" "$OUT/common_bf2"
fi
if want side; then
  echo "== soldiers + weapons + vehicles =="
  clean "$OUT/side_bf1/soldiers" "$OUT/side_bf1/weapons" "$OUT/side_bf1/textures" \
        "$OUT/side_bf2/soldiers" "$OUT/side_bf2/weapons" "$OUT/side_bf2/textures" \
        "$OUT/side_bf1/vehicles" "$OUT/side_bf2/vehicles" \
        "$OUT/side_bf1/fpm" "$OUT/side_bf2/fpm" \
        "$OUT/side_bf1/effects" "$OUT/side_bf2/effects" \
        "$OUT/side_bf1/droid_chunks" "$OUT/side_bf2/droid_chunks" \
        "$OUT/droid_effects" "$OUT/classes/droid_chunks.ron"
  # EVERY soldier geometry and every PRIMARY-weapon geometry named by the four
  # default class lists (game::roster::DEFAULT_CLASSES) or by any of the 57
  # mission RONs' per-team class mixes, deduped and grouped by the side LVL
  # that carries it (engine/artifacts/roster/README.md has the derivation and
  # the per-geometry bake table). `bake soldier|weapon` take a COMMA LIST.
  #
  # Note both discs' REP/CIS and ALL/IMP are baked: a BF1 map's clone-wars
  # mission fields BF1 SIDE/REP + SIDE/CIS classes and a BF2 map's GCW
  # mission fields BF2 SIDE/ALL + SIDE/IMP ones.
  # A BF1 soldier picked on a BF2 map throws BF2's throw: BF2_ANIMS lends BF2's
  # rifle throw clips to every BF1 soldier (bake::soldier::BORROWED_THROW, card GTHRW).
  A1="$(ci "$B1" COMMON/ENG/COMMON.LVL)"
  BF2_ANIMS="$(ci "$B2" INGAME.LVL)" "$BAKE" soldier "$(ci "$B1" SIDE/ALL.LVL)" all_inf_marksperson,all_inf_marksperson_snow,all_inf_pilot,all_inf_snowtrooper,all_inf_soldier,all_inf_trooperfleet,all_inf_wookie "$OUT/side_bf1" "$A1" ${DS1_ALL:+"$DS1_ALL"}
  BF2_ANIMS="$(ci "$B2" INGAME.LVL)" "$BAKE" soldier "$(ci "$B1" SIDE/IMP.LVL)" imp_inf_atatpilot,imp_inf_atstpilot,imp_inf_darktrooper,imp_inf_pilot,imp_inf_scout,imp_inf_shocktrooper,imp_inf_snowtrooper,imp_inf_stormtrooper "$OUT/side_bf1" "$A1" ${DS1_IMP:+"$DS1_IMP"}
  BF2_ANIMS="$(ci "$B2" INGAME.LVL)" "$BAKE" soldier "$(ci "$B1" SIDE/REP.LVL)" rep_inf_arctrooper,rep_inf_jettrooper,rep_inf_pilot,rep_inf_trooper "$OUT/side_bf1" "$A1" ${DS1_REP:+"$DS1_REP"}
  BF2_ANIMS="$(ci "$B2" INGAME.LVL)" "$BAKE" soldier "$(ci "$B1" SIDE/CIS.LVL)" cis_inf_bdroid,cis_inf_sbdroid "$OUT/side_bf1" "$A1" ${DS1_CIS:+"$DS1_CIS"}
  # BF2 SIDE gets the PC-stock tier too, from the matching PC side LVL (the donors, above).
  # This is where the proportional gain is biggest: a first-person weapon
  # sheet is 128x64 on the PS2 and 512x256 on the PC.
  # BF2 has no prone clips: BF1_ANIMS lends BF1's rifle prone set to every BF2
  # soldier (bake::soldier::BORROWED_PRONE, issue #40; the bones share names).
  BF1_ANIMS="$A1" "$BAKE" soldier "$(ci "$B2" SIDE/REP.LVL)" rep_inf_arctrooper,rep_inf_clonecommander,rep_inf_ep3armoredpilot,rep_inf_ep3heavytrooper,rep_inf_ep3jettrooper,rep_inf_ep3sniper,rep_inf_ep3spacepilot,rep_inf_ep3trooper,rep_inf_feluciatrooper,rep_inf_jettrooper,rep_inf_pilot,rep_inf_trooper "$OUT/side_bf2" "$(ci "$B2" INGAME.LVL)" ${DS2_REP:+"$DS2_REP"}
  BF1_ANIMS="$A1" "$BAKE" soldier "$(ci "$B2" SIDE/CIS.LVL)" cis_inf_bdroid,cis_inf_magnaguard,cis_inf_sbdroid "$OUT/side_bf2" "$(ci "$B2" INGAME.LVL)" ${DS2_CIS:+"$DS2_CIS"}
  BF1_ANIMS="$A1" "$BAKE" soldier "$(ci "$B2" SIDE/ALL.LVL)" all_inf_bothanspy,all_inf_engineer,all_inf_marksperson,all_inf_markspersonsnow,all_inf_pilot,all_inf_snowtrooper,all_inf_soldier,all_inf_tantive4trooper,all_inf_vanguard,all_inf_wookie "$OUT/side_bf2" "$(ci "$B2" INGAME.LVL)" ${DS2_ALL:+"$DS2_ALL"}
  BF1_ANIMS="$A1" "$BAKE" soldier "$(ci "$B2" SIDE/IMP.LVL)" imp_inf_atatcommander,imp_inf_atatpilot,imp_inf_darktrooper,imp_inf_gunner,imp_inf_pilot,imp_inf_scout,imp_inf_shocktrooper,imp_inf_snowtrooper,imp_inf_stormtrooper "$OUT/side_bf2" "$(ci "$B2" INGAME.LVL)" ${DS2_IMP:+"$DS2_IMP"}
  # Native bodies share texture names with playable sides, with different
  # sheets. Keep their models and textures in a separate side directory.
  bake_native_side() {
    local game="$1" root="$2" side="$3" bodies="$4" weapons="$5" anim="$6" donor
    local output="$OUT/side_${game}/locals"
    donor="$(side_donors "$game" "$side")"
    "$BAKE" soldier "$(ci "$root" "SIDE/$side.LVL")" "$bodies" "$output" "$anim" ${donor:+"$donor"}
    "$BAKE" weapon "$(ci "$root" "SIDE/$side.LVL")" "$weapons" "$output" ${donor:+"$donor"}
  }
  clean "$OUT/side_bf1/locals" "$OUT/side_bf2/locals"
  A2="$(ci "$B2" INGAME.LVL)"
  BF2_ANIMS="$A2" bake_native_side bf1 "$B1" ewk ewk_inf,ewk_inf2 ewk_rock,ewk_spear "$A1"
  BF2_ANIMS="$A2" bake_native_side bf1 "$B1" wok wok_inf_wookie wok_weap_inf_bowcaster,wok_weap_inf_fusioncutter,wok_weap_inf_launcher "$A1"
  BF2_ANIMS="$A2" bake_native_side bf1 "$B1" gun nab_inf_gungan gun_weap_inf_grenadethermal,gun_weap_inf_fusioncutter "$A1"
  BF2_ANIMS="$A2" bake_native_side bf1 "$B1" geo gen_inf_geonosian geo_weap_inf_blunderbuss "$A1"
  BF2_ANIMS="$A2" bake_native_side bf1 "$B1" des tat_inf_jawa,tat_inf_tusken tat_weap_tuskrifle "$A1"
  BF1_ANIMS="$A1" bake_native_side bf2 "$B2" ewk ewk_inf,ewk_inf2 ewk_rock,ewk_spear,ewk_weap_inf_fusioncutter "$A2"
  BF1_ANIMS="$A1" bake_native_side bf2 "$B2" wok wok_inf_wookiewarrior wok_weap_inf_bowcaster,wok_weap_inf_fusioncutter,wok_weap_inf_launcher,wok_weap_inf_thermaldetonator "$A2"
  BF1_ANIMS="$A1" bake_native_side bf2 "$B2" geo gen_inf_geonosian geo_weap_inf_blunderbuss "$A2"
  BF1_ANIMS="$A1" bake_native_side bf2 "$B2" des tat_inf_jawa,tat_inf_tusken tat_weap_tuskrifle,des_weap_inf_fusioncutter "$A2"
  # Every BF2 hero body, cloth, weapon, HUD icon and blade sheet comes from
  # its class references, including the campaign variants.
  BF1_ANIMS="$A1" "$BAKE" heroes "$B2" "$OUT/side_bf2"
  for s in ALL IMP REP CIS TUR DES WOK GAM EWK GEO; do
    "$BAKE" combos "$(ci "$B2" "SIDE/$s.LVL")" "$OUT/side_bf2/combos/$(echo "$s" | tr 'A-Z' 'a-z').ron"
  done
  # BF2's pistols are baked for the HUD's weapon row (card HUDW): the PS2
  # shows a weapon's own model there, and every BF2 side's second weapon is
  # its pistol (IMP's has been baked since the officer's primary names it).
  # BF1's pistols too since card WHELD: a soldier holds the model of the
  # weapon in his hand, and every class slot the sim can switch to must have
  # its model (game/tests/suite/roster.rs `every_held_slots_hand_model_is_baked`).
  "$BAKE" weapon  "$(ci "$B1" SIDE/ALL.LVL)" all_weap_inf_bowcaster,all_weap_inf_fusioncutter,all_weap_inf_launcher,all_weap_inf_pistol,all_weap_inf_rifle,all_weap_inf_seeker,all_weap_inf_sniperrifle "$OUT/side_bf1" ${DS1_ALL:+"$DS1_ALL"}
  "$BAKE" weapon  "$(ci "$B1" SIDE/IMP.LVL)" imp_weap_inf_fusioncutter,imp_weap_inf_launcher,imp_weap_inf_pistol,imp_weap_inf_rifle,imp_weap_inf_sniperrifle "$OUT/side_bf1" ${DS1_IMP:+"$DS1_IMP"}
  "$BAKE" weapon  "$(ci "$B1" SIDE/REP.LVL)" rep_weap_inf_fusioncutter,rep_weap_inf_launcher,rep_weap_inf_pistol,rep_weap_inf_rifle,rep_weap_inf_sniperrifle,rep_weap_inf_landmine "$OUT/side_bf1" ${DS1_REP:+"$DS1_REP"}
  "$BAKE" weapon  "$(ci "$B1" SIDE/CIS.LVL)" cis_weap_inf_fusioncutter,cis_weap_inf_launcher,cis_weap_inf_pistol,cis_weap_inf_sniperrifle "$OUT/side_bf1" ${DS1_CIS:+"$DS1_CIS"}
  "$BAKE" weapon  "$(ci "$B2" SIDE/REP.LVL)" rep_weap_inf_chaingun,rep_weap_inf_fusioncutter,rep_weap_inf_launcher,rep_weap_inf_pistol,rep_weap_inf_rifle,rep_weap_inf_sniperrifle "$OUT/side_bf2" ${DS2_REP:+"$DS2_REP"}
  "$BAKE" weapon  "$(ci "$B2" SIDE/CIS.LVL)" cis_weap_inf_fusioncutter,cis_weap_inf_launcher,cis_weap_inf_pistol,cis_weap_inf_rifle,cis_weap_inf_sniperrifle,hud_cis_wristblaster,hud_cis_trishot,hud_cis_wristrocket,hud_droideka_shield "$OUT/side_bf2" ${DS2_CIS:+"$DS2_CIS"}
  # [ours] BF1 SBD chunks on both games; reproducible Magnaguard mesh cuts.
  # Every droid chunk model goes through here, never `bake weapon`: its sheets
  # get an `openbf_droid_` name, so no living soldier's sheet is overwritten
  # (a chunk's weapon bake made the battle droid's sheet opaque). After the
  # soldier bake: the Magnaguard cuts draw its living sheets.
  "$BAKE" droid-chunks "$(ci "$B1" SIDE/CIS.LVL)" "$(ci "$B2" SIDE/CIS.LVL)" "$A1" "$OUT" ${DS2_CIS:+"$DS2_CIS"}
  "$BAKE" weapon  "$(ci "$B2" SIDE/ALL.LVL)" all_weap_inf_bowcaster,all_weap_inf_fusioncutter,all_weap_inf_launcher,all_weap_inf_pistol,all_weap_inf_rifle,all_weap_inf_seeker,all_weap_inf_sniperrifle "$OUT/side_bf2" ${DS2_ALL:+"$DS2_ALL"}
  "$BAKE" weapon  "$(ci "$B2" SIDE/IMP.LVL)" imp_weap_inf_fusioncutter,imp_weap_inf_launcher,imp_weap_inf_pistol,imp_weap_inf_rifle,imp_weap_inf_sniperrifle "$OUT/side_bf2" ${DS2_IMP:+"$DS2_IMP"}
  # The torpedo launcher's model is SHARED, not per-side: BF1 IMP/REP/CIS's
  # `*_weap_inf_torpedo_launcher` and BF2 REP's all name `com_weap_inf_torpedo`,
  # whose `modl` is in each game's common LVL and in no side LVL (measured:
  # `bake weapon SIDE/ALL.LVL com_weap_inf_torpedo` -> "modl not found",
  # COMMON.LVL/INGAME.LVL -> 474 verts, 494 tris).
  # So is the thermal detonator's (card WHELD: drawn in the hand while a
  # throw is wound up and before HideOnFire hides it), every side's
  # `*_weap_inf_thermaldetonator` naming `com_weap_inf_grenadethermal`.
  "$BAKE" weapon  "$A1"              com_weap_inf_torpedo,com_weap_inf_grenadethermal,com_weap_inf_grenadethermal_ord,com_weap_inf_landmine,com_weap_inf_landmine_placed,com_weap_inf_seismic_mine "$OUT/side_bf1"
  "$BAKE" weapon  "$(ci "$B2" INGAME.LVL)"   com_weap_inf_torpedo,com_weap_inf_grenadethermal,com_weap_inf_grenadethermal_ord,com_weap_inf_landmine,com_weap_inf_landmine_placed,com_weap_inf_seismic_mine,com_weap_inf_detpack,com_weap_missile "$OUT/side_bf2"
  "$BAKE" weapon  "$(ci "$B2" INGAME.LVL),$(ci "$B2" SIDE/REP.LVL)" com_weap_inf_dropturret "$OUT/side_bf2"
  # First person (card FPA): every FPM LVL's arm model (the soldier class's
  # `FirstPerson` key; cockpit models carry no hp_camera and are skipped) with
  # the 11-column clip table's clips, and every `HighResGeometry` weapon.
  shopt -s nocaseglob nullglob
  # nocaseglob does not reach a folder name with no wildcard in it: a tree with lowercase names has fpm/, not FPM/.
  FPM1=("$(ci "$B1" FPM)"/*/*.LVL)
  FPM2=("$(ci "$B2" FPM)"/*/*.LVL)
  shopt -u nocaseglob nullglob
  fpm_donors bf1
  "$BAKE" fpm "$A1" "$OUT/side_bf1" ${FPM_DONORS[@]+"${FPM_DONORS[@]}"} ${FPM1[@]+"${FPM1[@]}"}
  fpm_donors bf2
  "$BAKE" fpm "$(ci "$B2" INGAME.LVL)" "$OUT/side_bf2" ${FPM_DONORS[@]+"${FPM_DONORS[@]}"} ${FPM2[@]+"${FPM2[@]}"}
  W1ST=fusioncutter,launcher,pistol,rifle,sniperrifle
  for g in 1 2; do
    eval "G=\$B$g"
    for s in all imp rep cis; do
      x="$W1ST"
      [ "$s" = all ] && x="$x,bowcaster,seeker"
      [ "$s" = rep ] && [ "$g" = 1 ] && x="$x,landmine"
      [ "$s" = rep ] && [ "$g" = 2 ] && x="$x,chaingun"
      eval "pc=\$DS${g}_$(echo "$s" | tr a-z A-Z)"
      "$BAKE" weapon "$(ci "$G" "SIDE/$(echo "$s" | tr a-z A-Z).LVL")" "$(echo "$x" | sed "s/[^,]*/${s}_1st_weap_inf_&/g")" "$OUT/side_bf$g" ${pc:+"$pc"}
    done
  done
  "$BAKE" weapon  "$A1"            com_1st_weap_inf_grenadethermal,com_1st_weap_inf_torpedo,com_1st_weap_inf_landmine "$OUT/side_bf1"
  "$BAKE" weapon  "$(ci "$B2" INGAME.LVL)" com_1st_weap_inf_grenadethermal,com_1st_weap_inf_torpedo,com_1st_weap_inf_landmine "$OUT/side_bf2"
  # Every vehicle class of the four sides (card VBAKE): models (every LOD),
  # destruction chunks, collision, textures -> side_<game>/vehicles/<side>/.
  "$BAKE" vehicle "$(ci "$B1" SIDE/ALL.LVL)" "$OUT/side_bf1/vehicles/all" ${DS1_ALL:+"$DS1_ALL"}
  "$BAKE" vehicle "$(ci "$B1" SIDE/CIS.LVL)" "$OUT/side_bf1/vehicles/cis" ${DS1_CIS:+"$DS1_CIS"}
  "$BAKE" vehicle "$(ci "$B1" SIDE/IMP.LVL)" "$OUT/side_bf1/vehicles/imp" ${DS1_IMP:+"$DS1_IMP"}
  "$BAKE" vehicle "$(ci "$B1" SIDE/REP.LVL)" "$OUT/side_bf1/vehicles/rep" ${DS1_REP:+"$DS1_REP"}
  # BF1 SIDE/GUN: Naboo Plains' missions read it (nab1c/nab1i/nab1c_h.luac)
  # and three NAB1 pads key its gun_walk_kaadu for the defenders (card KAAD).
  "$BAKE" vehicle "$(ci "$B1" SIDE/GUN.LVL)" "$OUT/side_bf1/vehicles/gun"
  "$BAKE" vehicle "$(ci "$B2" SIDE/ALL.LVL)" "$OUT/side_bf2/vehicles/all" ${DS2_ALL:+"$DS2_ALL"}
  "$BAKE" vehicle "$(ci "$B2" SIDE/IMP.LVL)" "$OUT/side_bf2/vehicles/imp" ${DS2_IMP:+"$DS2_IMP"}
  "$BAKE" vehicle "$(ci "$B2" SIDE/REP.LVL)" "$OUT/side_bf2/vehicles/rep" ${DS2_REP:+"$DS2_REP"}
  "$BAKE" vehicle "$(ci "$B2" SIDE/CIS.LVL)" "$OUT/side_bf2/vehicles/cis" ${DS2_CIS:+"$DS2_CIS"}
  # MapTexture is class-authored, independent of the vehicle meshes' materials.
  # Keep the sheets in the side texture directories the HUD already searches.
  "$BAKE" map-icons "$OUT/side_bf1" "$(ci "$B1" SIDE/ALL.LVL)" "$(ci "$B1" SIDE/IMP.LVL)" \
    "$(ci "$B1" SIDE/REP.LVL)" "$(ci "$B1" SIDE/CIS.LVL)" "$(ci "$B1" SIDE/GUN.LVL)"
  "$BAKE" map-icons "$OUT/side_bf2" "$(ci "$B2" SIDE/ALL.LVL)" "$(ci "$B2" SIDE/IMP.LVL)" \
    "$(ci "$B2" SIDE/REP.LVL)" "$(ci "$B2" SIDE/CIS.LVL)" "$(ci "$B2" SIDE/TUR.LVL)"
  # Every side LVL's `fx__` particle effects + the textures/models they name
  # (card FXP1, analysis/re-fx.md §1): BF2's sides carry per-class effects
  # (IMP 8, SNW 6, CIS 5, ...); BF1's carry none, and a side with none writes
  # nothing. ALL side LVLs, not only the four baked above: the runtime looks
  # an effect up by hash through whatever side a mission loads.
  SD1="$(ci "$B1" SIDE)"
  for f in "$SD1"/*.[Ll][Vv][Ll]; do
    [ -f "$f" ] || continue
    base="${f##*/}"
    s=$(printf '%s' "${base%.*}" | tr 'A-Z' 'a-z')
    "$BAKE" effects "$f" "$OUT/side_bf1/effects/$s" bf1
  done
  SD2="$(ci "$B2" SIDE)"
  for f in "$SD2"/*.[Ll][Vv][Ll]; do
    [ -f "$f" ] || continue
    base="${f##*/}"
    s=$(printf '%s' "${base%.*}" | tr 'A-Z' 'a-z')
    "$BAKE" effects "$f" "$OUT/side_bf2/effects/$s" bf2
  done
fi
# Kit Fisto and Asajj Ventress, when the scan found them (scripts/DLC.md). With `side`: it just cleared their bodies and
# weapons, and a completion marker must never be left standing over deleted files. Without a folder, nothing enables them.
if want side || want dlc; then
  clean "$OUT/side_bf2/dlc_heroes.ron" "$OUT/classes/bf2_dlc.ron" "$OUT/side_bf2/dlc_sounds" "$OUT/side_bf2/effects/dlc"
  if [ -n "$OPENBF_DLC_DIR" ]; then
    echo "== DLC heroes =="
    mkdir -p "$OUT"
    # Its real path: the bake refuses to write through a link, and a link above the checkout (/home on some Linux
    # systems) is not one it needs to worry about.
    DLC_OUT="$(cd "$OUT" && { pwd -W 2>/dev/null || pwd -P; })"
    "$PYTHON" "$ROOT/scripts/bake_dlc.py" --source "$OPENBF_DLC_DIR" ${OPENBF_DLC_DONOR:+--source "$OPENBF_DLC_DONOR"} \
      --ingame "$(ci "$B2" INGAME.LVL)" --out "$DLC_OUT" --bake "$BAKE"
  fi
fi
if want classes; then
  echo "== entity/weapon classes =="
  "$BAKE" classes "$(ci "$B1" SIDE/ALL.LVL)"          "$OUT/classes/bf1_all.ron"
  "$BAKE" classes "$(ci "$B1" SIDE/IMP.LVL)"          "$OUT/classes/bf1_imp.ron"
  "$BAKE" classes "$(ci "$B1" COMMON/ENG/COMMON.LVL)" "$OUT/classes/bf1_common.ron"
  "$BAKE" classes "$(ci "$B2" SIDE/REP.LVL)"          "$OUT/classes/bf2_rep.ron"
  "$BAKE" classes "$(ci "$B2" SIDE/CIS.LVL)"          "$OUT/classes/bf2_cis.ron"
  "$BAKE" classes "$(ci "$B2" INGAME.LVL)"            "$OUT/classes/bf2_ingame.ron"
  # Bake all four factions per source game: game::roster::class_files loads
  # the sides in play, including clone-wars on BF1 maps and GCW on BF2 maps.
  "$BAKE" classes "$(ci "$B1" SIDE/REP.LVL)"          "$OUT/classes/bf1_rep.ron"
  "$BAKE" classes "$(ci "$B1" SIDE/CIS.LVL)"          "$OUT/classes/bf1_cis.ron"
  "$BAKE" classes "$(ci "$B2" SIDE/ALL.LVL)"          "$OUT/classes/bf2_all.ron"
  "$BAKE" classes "$(ci "$B2" SIDE/IMP.LVL)"          "$OUT/classes/bf2_imp.ron"
  for side in ewk wok geo des; do
    "$BAKE" classes "$(ci "$B1" "SIDE/$side.LVL")" "$OUT/classes/bf1_$side.ron"
    "$BAKE" classes "$(ci "$B2" "SIDE/$side.LVL")" "$OUT/classes/bf2_$side.ron"
  done
  # A mission's extra side (game::vehicle_pads::mission_side_files).
  "$BAKE" classes "$(ci "$B1" SIDE/GUN.LVL)"          "$OUT/classes/bf1_gun.ron"
  # BF2's building turrets (card VTUR2): their PROPs; each map bake pulls the
  # models, collision and skel nodes of the TUR classes its worlds place.
  "$BAKE" classes "$(ci "$B2" SIDE/TUR.LVL)"          "$OUT/classes/bf2_tur.ron"
fi
if want sounds; then
  # One SOUND LVL per PLANET on both discs (RE_LOG "BF1 does NOT ship sound
  # banks per era"): the first three letters of a map name are its planet, so
  # bf1_kas1 and bf1_kas2 share KAS.LVL. The game loads <mapdir>/sounds/
  # sounds.ron, so each map dir gets its own copy of its planet's bank.
  echo "== map sound banks =="
  # The game's CORE.LVL goes in too: a property's Inherit template (distances,
  # gain, rolloff) is there, not in the planet's LVL (card SNDT).
  # A PC install donates its own recordings for the samples a console disc
  # aliases to other takes (bake::sounds::bake_sounds_with_donor); the disc's
  # properties and mix are kept. PC-only trees read the PC banks directly.
  donor_bnk() {  # <PC _lvl_pc dir or empty> <the sound root in use>
    local bnk
    [ -n "$1" ] && [ "$1" != "$2" ] || return 0
    bnk="$(ci "$1" SOUND/COMMON.BNK)"
    [ -f "$bnk" ] && printf '%s' "$bnk"
    return 0
  }
  D1="$(donor_bnk "${PC1:-}" "$S1")"
  D2="$(donor_bnk "${PC2:-}" "$S2")"
  echo "Sound sample donors: BF1=${D1:-none} BF2=${D2:-none}"
  bake_map_sounds() {  # <sound root> <map dir name> <map short name> <CORE.LVL> [donor bnk]
    local root="$1" dir="$2" short="$3" core="$4" donor="${5:-}"
    local planet sf
    planet=$(echo "${short:0:3}" | tr 'a-z' 'A-Z')
    sf="$(ci "$root" "SOUND/$planet.LVL")"
    if [ -f "$sf" ]; then
      clean "$OUT/$dir/sounds"
      "$BAKE" sounds "$sf" "$OUT/$dir" "$core" ${donor:+--donor-bnk "$donor"}
    else
      echo "  (no SOUND/$planet.LVL for $dir — skipped)"
    fi
  }
  C1="$(ci "$B1" CORE.LVL)"
  C2="$(ci "$B2" CORE.LVL)"
  bake_map_sounds "$S1" tat1 tat1 "$C1" "$D1"
  bake_map_sounds "$S2" tat2 tat2 "$C2" "$D2"
  for p in $BF1_MAPS; do
    n=$(basename "$p" | tr 'A-Z' 'a-z'); bake_map_sounds "$S1" "bf1_$n" "$n" "$C1" "$D1"
  done
  for p in $BF2_MAPS; do
    n=$(basename "$p" | tr 'A-Z' 'a-z'); bake_map_sounds "$S2" "bf2_$n" "$n" "$C2" "$D2"
  done
  if [ "${OPENBF_BAKE_SPACE:-1}" != 0 ]; then
    for n in SPA1 SPA3 SPA6 SPA7 SPA8 SPA9; do
      [ -f "$(ci "$B2" "SPA/$n.LVL")" ] || continue
      n=$(printf '%s' "$n" | tr 'A-Z' 'a-z'); bake_map_sounds "$S2" "bf2_$n" "$n" "$C2"
    done
  fi
  # The RESIDENT bank both games load and never unload: CORE.LVL's `global`
  # SampleBank + `snd_` — every shell/load sound, incl. shell_menu_accept
  # (analysis/re-sound-global.md §0.6; the per-map banks do not carry them).
  # Baked here rather than with the streams so that `sound-names` below sees
  # these two directories on a FRESH tree too.
  echo "== resident global banks (CORE.LVL) =="
  clean "$OUT/core_bf1/sounds" "$OUT/core_bf2/sounds"
  "$BAKE" sounds "$C1" "$OUT/core_bf1"
  "$BAKE" sounds "$C2" "$OUT/core_bf2"
  # Recover readable names for the hashed properties/samples so
  # scripts/audition.sh can lay a bank out under names a human can read.
  # Needs the baked classes, so it runs after them; harmless if they are
  # missing (the dictionary just gets smaller).
  echo "== sound names =="
  LUA_ROOT="${OPENBF_CONSOLE_LUA_ROOT:-}"
  LUA_PATHS="$E/bf1/lua,$E/bf2/lua"
  [ -n "$LUA_ROOT" ] && LUA_PATHS="$LUA_ROOT/bf1,$LUA_ROOT/bf2"
  # An array: a checkout's path can hold spaces.
  SOUND_DIRS=()
  for d in "$OUT"/*/sounds; do
    if [ -d "$d" ]; then SOUND_DIRS+=("${d%/sounds}"); fi
  done
  "$BAKE" sound-names "$OUT/classes" "$LUA_PATHS" ${SOUND_DIRS[@]+"${SOUND_DIRS[@]}"}
fi
if want global-sound; then
  # The sound BOTH games keep outside the per-map banks (the maintainer's unified sound
  # plan, PLAN Log 2026-09-08 (b); analysis/re-sound-global.md §5):
  #  (i)  BF2 SOUND/GLOBAL.LVL — 29 mono VO streams. Its two stereo MUSIC
  #       streams (630 MB) are indexed, never decoded: music stays streamed.
  #  (ii) BF1 SOUND/CW.LVL + GCW.LVL — the four VO streams, including the
  #       nine-command tactical chatter the per-map banks never carried.
  # Only segments a shipped `snd_` script names are encoded, so every per-map
  # sound LVL goes in as the reference set (BF1 TEST.LVL is not shipped
  # content and is left out). Both write <id>.ogg keyed by SEGMENT ID — the
  # only name that data has.
  echo "== global VO streams =="
  clean "$OUT/audio/global_bf1" "$OUT/audio/global_bf2"
  SND1="$(ci "$S1" SOUND)"
  SND2="$(ci "$S2" SOUND)"
  REF1=$(ls "$SND1"/*.[Ll][Vv][Ll] | grep -viE '/(cw|gcw|test)\.lvl$' | paste -sd, -)
  REF2=$(ls "$SND2"/*.[Ll][Vv][Ll] | grep -viE '/global\.lvl$' | paste -sd, -)
  "$BAKE" global-sound "$(ci "$S1" SOUND/CW.LVL),$(ci "$S1" SOUND/GCW.LVL)" "$OUT/audio/global_bf1" "$REF1"
  "$BAKE" global-sound "$(ci "$S2" SOUND/GLOBAL.LVL)" "$OUT/audio/global_bf2" "$REF2"
fi
if want load; then
  echo "== load screens =="
  "$BAKE" loadscreen "$(ci "$B1" LOAD/TAT1.LVL)"   "$OUT/tat1"
  "$BAKE" loadscreen "$(ci "$B2" LOAD/TAT2.LVL)"   "$OUT/tat2"
  "$BAKE" loadscreen "$(ci "$UI2" LOAD/COMMON.LVL)" "$OUT/common_bf2"
fi

if want maps; then
  echo "== other ground maps =="
  for p in $BF1_MAPS; do
    n=$(basename "$p" | tr 'A-Z' 'a-z'); d="$OUT/bf1_$n"
    clean "$d/models" "$d/textures" "$d/collision" "$d/nav" "$d/shadows"
    map_donors bf1 "$p.LVL"
    "$BAKE" map "$(ci "$B1" "$p.LVL")" "$d" ${PC_ARG[@]+"${PC_ARG[@]}"}
    LF="$(ci "$B1" "LOAD/$(basename "$p").LVL")"
    [ -f "$LF" ] && "$BAKE" loadscreen "$LF" "$d"
  done
  for p in $BF2_MAPS; do
    n=$(basename "$p" | tr 'A-Z' 'a-z'); d="$OUT/bf2_$n"
    planet=$(dirname "$p" | tr 'A-Z' 'a-z')
    clean "$d/models" "$d/textures" "$d/collision" "$d/nav" "$d/shadows"
    map_donors bf2 "$planet/$n.lvl"
    "$BAKE" map "$(ci "$B2" "$p.LVL")" "$d" ${PC_ARG[@]+"${PC_ARG[@]}"}
    LF="$(ci "$B2" "LOAD/$(basename "$p").LVL")"
    [ -f "$LF" ] && "$BAKE" loadscreen "$LF" "$d"
  done
  # Card COVR: BF1 Yavin Temple takes BF2 Yavin Temple's Cover hints (same
  # graph; bake::donor::DONORS). After both are baked; run again it replaces
  # its own hints.
  if [ -f "$OUT/bf1_yav1/manifest.ron" ] && [ -f "$OUT/bf2_yav1/manifest.ron" ]; then
    "$BAKE" donor-hints "$OUT"
  fi
fi
if [ "${OPENBF_BAKE_SPACE:-1}" != 0 ] && want maps; then
  echo "== space maps =="
  SKY_LVL="$(ci "$B2" SPA/SPA_SKY.LVL)"
  for n in SPA1 SPA3 SPA6 SPA7 SPA8 SPA9; do
    [ -f "$(ci "$B2" "SPA/$n.LVL")" ] || continue
    case "$n" in
      SPA1) SKY=yav ;;
      SPA3) SKY=kas ;;
      SPA6) SKY=myg ;;
      SPA7) SKY=fel ;;
      SPA8) SKY=hot ;;
      SPA9) SKY=tat ;;
    esac
    d="$OUT/bf2_$(printf '%s' "$n" | tr 'A-Z' 'a-z')"
    clean "$d/models" "$d/textures" "$d/collision" "$d/nav" "$d/shadows"
    map_donors bf2 "spa/$(printf '%s' "$n" | tr 'A-Z' 'a-z').lvl"
    "$BAKE" map "$(ci "$B2" "SPA/$n.LVL")" "$d" ${PC_ARG[@]+"${PC_ARG[@]}"} --sky-sublevel "$SKY_LVL" "$SKY"
    LF="$(ci "$B2" LOAD/SPA.LVL)"
    [ -f "$LF" ] && "$BAKE" loadscreen "$LF" "$d"
  done
fi
if want beep; then
  echo "== BF1 loading beep =="
  "$BAKE" beep "$(ci "$S1" CORE.LVL)" "$OUT/audio"
fi
# Compared with the PS2's counts shipped in the source, not with another bake: two bakes
# of one bad extraction agree with each other. A tree that is not the PS2's owes no such counts.
if want tat1 || want tat2 || want maps; then
  "$BAKE" check-foliage "$OUT" ${OPENBF_BF1_TREE:+--warn bf1} ${OPENBF_BF2_TREE:+--warn bf2}
fi
echo "rebake done -> $OUT"
