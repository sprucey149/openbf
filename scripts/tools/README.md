# scripts/tools

Small standalone helpers that the analysis notes and cards cite. The commands below run from the
repository root. The reverse-engineering tools locate the PS2 executables relative to the repository, from your own disc extract
(`extracted/bf1/SWBFRONT.ELF`, `extracted/bf2/SWBFRNT2.ELF`); nothing here ships game data.

| tool | what it does | usage |
|---|---|---|
| `mix.py` | renders the event sounds of a playtest black-box snapshot (or a SOUND_LOG csv) to a WAV and reports overloads; needs numpy, and ffmpeg for Ogg takes | `python3 scripts/tools/mix.py <snapshot.json> engine/assets_baked --wav out.wav` |
| `charge_survey.py` | every weapon / ordnance class with charge keys or `NextCharge` stages, from the baked classes (`analysis/re-weapon-charge.md`) | `python3 -I scripts/tools/charge_survey.py table` |
| `charge_keys_decomp.py` | the charge keys' reads in the Ghidra listings, by enclosing function | `python3 -I scripts/tools/charge_keys_decomp.py analysis/SWBFRONT.ELF-decomp.c analysis/SWBFRNT2.ELF-decomp.c` |
| `charge_keys_elf.py` | the charge keys as ELF data words and lui/ori builds | `python3 -I scripts/tools/charge_keys_elf.py extracted/bf1/SWBFRONT.ELF extracted/bf2/SWBFRNT2.ELF` |
| `jet_census.py` | every soldier class on a disc that authors a jet key, with its jet and movement values | `python3 scripts/tools/jet_census.py bf1` |
| `elfrd.py` | prints 32-bit words (hex and float) of an ELF at a virtual address | `python3 scripts/tools/elfrd.py extracted/bf1/SWBFRONT.ELF 534b10 8` |
| `refs.py` | every `jal` / `j` instruction and data word that references an address | `python3 scripts/tools/refs.py b1 22c7f0` |
| `eedis.py` | small R5900 disassembler: unaligned loads/stores, FPU/COP2 and likely branches, VU0 macro ops with their field masks | `python3 scripts/tools/eedis.py bf1 22ad3c 22ad80` |
| `vu1dis.py` | VU1 microprogram disassembler (`tags`, `find <regex>`, or a range) | `python3 scripts/tools/vu1dis.py bf2` |
| `pure_move_check.py` | checks that every meaningful old line survives a split, reporting visibility changes and new-only lines | `python3 scripts/tools/pure_move_check.py <parent-sha>:<old.rs> <new files...>` |
| `win_run.sh` / `xkey.py` | the windowed game on a private Xvfb display with a scratch profile and no sound device, driven by a key script | header of `win_run.sh` |

`scripts/README.md` lists everything else. `scripts/re_check.py` (`fn`, `callers`, `word`) and `scripts/re_calc.py` are the main RE checkers; see the
"RE and absence claims" rules in the loop's RULES.md.

For a split, run `pure_move_check.py` from the candidate checkout's root, naming the exact parent revision and
every resulting file, including the facade. File arguments resolve from the current directory, and revision
arguments use that directory's Git repository. Run it after formatting: whitespace is trimmed per line, but
rewrapping a statement across lines changes the comparison.

Exit 0 proves old-line containment under the helper's documented exclusions. Added executable lines can still
produce exit 0: inspect every new-only line, visibility change and the complete diff, and run the unchanged
behavior gates. Follow `engine/artifacts/briefs/SPLIT_RECIPE.md` for the full split proof and acceptance.
