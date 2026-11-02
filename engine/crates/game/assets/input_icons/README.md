Button icons for the control prompts (`game::input_icons`, looked up through `ui::input_icons`). All of them are
SVG text compiled into the game and rasterized the first time a prompt shows one. Nothing here comes from a game disc.

## `kenney/`

From Kenney's Input Prompts, version 1.5A, the `Vector` folders of `PlayStation Series`, `Xbox Series` and
`Keyboard & Mouse`. The SVG files are unmodified; when `game::input_icons` rasterizes them it backs their cut-out
letters with a dark fill, so they read over any backdrop.

- Author: Kenney (www.kenney.nl)
- Source: https://kenney.nl/assets/input-prompts
- Licence: Creative Commons Zero (CC0), http://creativecommons.org/publicdomain/zero/1.0/
- Licence as shipped in the archive: `kenney/License.txt`, read on 2026-11-01
- Archive read: SHA-256 `ac2fcf599080b0f3ba2d174c9474db6df1a0e96ff0662580e2da79a122ab78a1`, 5070075 bytes

CC0 covers Kenney's copyright. It grants no trademark rights: the PlayStation face-button symbols and the Xbox button
names remain their owners' marks, shown here only to tell a player which button to press.

DS2 reuses the PlayStation symbols (`playstation3_button_select` and `_start` for its Select and Start).

## `drawn/`

Original art made for this project, under the project's own licence:

- `duke_*`: the original Xbox pad's A, B, X, Y, White and Black buttons, shown when no Xbox disc was supplied. With an
  Xbox disc, `bake xbox-icons` writes that pad's icons from the player's own disc into the local bake; they never
  enter the repo.

A pad the input library cannot name shows the Xbox One set, the layout most third-party pads copy.
