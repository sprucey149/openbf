# Carrying public pull request #195 into private main

Public pull request #195 (LightVelox): native launcher, engine extensions with OpenBF VR, two-flag CTF and legacy
BF2 mod support. 36 commits, 307 files. Its merge base is public main 5015e39c, the export of private 4c48bc11f.
Its diff against that base was applied here with `git apply --3way` on private main 34e29f56b (splitscreen B1-B22 and
LAN with local players). The code was not reviewed; this note covers the merge only.

## Left out of the private commit

- `README.md` is owned by the public repo. Its new "Mods & OpenBF VR" section arrives when the pull request is merged
  on public.
- `docs/extensions.md`, `docs/modding-architecture.md`, `docs/mods.md`, `docs/mods-advanced-maps.md`,
  `docs/mods-resource-replacements.md` and `docs/vr.md` are outside the export allowlist (`export_public.py`
  INCLUDE), so they are public-only files. The export keeps them as they are on public. `AGENTS.md` (exported) now
  links two of them. Those links work on public but have no target in the private tree.

## Conflicts and how they were resolved

17 files conflicted. Everything else applied cleanly, including `Cargo.lock`.

Rendering (`render/src/frame/submit.rs`, `world_scale.rs`, `lib.rs`, `device.rs`)
- His version of `submit.rs` is the base. His internal per-view function was `render_view(camera, projection, ..,
  external, clear_shadows)` and is now `render_scene(.., slot: Option<ViewSlot>)`. Splitscreen's public
  `Renderer::render_view(slot, ..)` is unchanged for the game crate and calls `render_scene`. A `slot` means: advance
  the sky on the first view only, update falling leaves every view at the window size (as before), use the slot's
  size, reuse the window image held from the first view, time only the first view, always draw into a world target
  and upscale it into the slot rectangle, and keep the image until the last view. `Frame::render_to` and
  `render_frame` in `output.rs` pass `None`.
- Splitscreen did not move onto his `begin_frame` / `FrameTarget` mechanism, because that would mean a compositor
  and a different HUD path. Both mechanisms now share one function, `render_scene`.
- `WorldScale`: `upscale_into(rect, clear)` (split) and `blit(viewport)` (his desktop mirror) both call one private
  `draw`. `upscale` is now test-only.
- `device.rs`: his `finish_windowed` split with the private comment wording. `lib.rs`: the exports of both sides.

HUD (`ui/src/hud.rs`, `hud_draw.rs`, `spawn_draw.rs`, `stats_draw.rs`, `game/src/hud_feed.rs`)
- Round timer: he moved the one-player timer to the top right under the minimap (BODY text, no panel) and hid it
  under the spawn page and the outcome banner. `draw_hud` (one view) now draws exactly his timer. Split cells keep
  their own timer under the counts (`draw_hud_in`), and now take his hide rule as well. The banner moved to a shared
  `draw_banner_in`. The reference test `draw_hud_with_salt_0_is_unchanged` uses his timer and also checks mid-round.
  The cell test checks the timer mid-round and the banner with the round decided.
- His `flag_timers(size)` now takes a `Frame` (`f.project`, `f.scale()`), like the other projected HUD pieces, so
  the two-flag CTF timers sit right in a split cell. `Frame::window` gives the same numbers as before.
- `help_line_at` (his, shared by the spawn and stats help hit tests) now takes a `Frame` and uses the panel's help
  screen. For the window this is identical. The stats callers pass `Frame::window(size)`.
- `hud_feed.rs`: `PlayerHud` and his `WorldMarkerStyle` are both kept, along with `spawn_help_at_in` and his
  `stats_help_at`. `update_in` feeds his reticle position.

Game (`main.rs`, `windowed.rs`, `windowed/events.rs`, `tick_hook.rs`, `xr.rs`, `sounds.rs`)
- Startup flags: his `extract_extension_flags` runs first, then `split_flags`. `--vr` with `--local-players` is
  refused.
- OpenBF VR is one player only. The stereo view, `ext_infantry`, the VR stats pointer and flat-frame capture are all
  off while a split battle runs (`split_on`). VR ears are used only with one local player. Otherwise splitscreen's
  `listeners` / `ears_and_views` apply.
- `play_event_sounds`: both sides had added a `listener` parameter. Splitscreen's `(listener, view)` signature is
  kept, with his by-reference use inside.
- Rendering: split views go through `split::render_views`. Every other frame goes through his `xr::render_battle`
  (stereo or desktop). The split results are mapped to `anyhow` for his `self.failure`. The render error handling is
  his, and the split render timing is splitscreen's.
- Menu input: his VR menu buttons, then splitscreen's keyboard-driver gating.
- HUD models: player 1 uses `hud_model_for_view` (his stereo hint projection). The other split players use
  `hud_model`. The camera keeps the split `fit_fov_y`, then his stereo override.
- `sounds.rs`: his PC region stream fade with splitscreen's `ears.dist`. `xr::listener` and his test use
  `Listener::new`.
- `event_feed/tests.rs`: both sides' new tests are kept.

Net (`client.rs`, `host.rs`, `tests/codec.rs`)
- His `strip_tracked_inputs` runs on every local's pad (`submit_inputs`) and in `submit_input_for` for every host
  local. Both sides' codec tests are kept.

## Integration fixes (second commit)

- Bake groups: his `rebake.sh` adds a `catalog` group, but `versions.rs` lacked it, so `bake_versions.py check`
  failed. `catalog` is now version 1 and a visual group (all of bake/src) in `bake_versions.py`. Its closures were
  recorded neutral on the merged tree. His version bumps (tat1 5, tat2 6, maps 8, common 7, side 5, dlc 5, classes 3)
  are kept. `sounds` stays at 2 even though he changed `bake/src/sounds.rs`, as in his pull request.
- One-player gate: the three scenarios' sim hashes, deaths and spawns are unchanged. Only the LAN soak's static
  digest moved, from dac14dbdede3d9be to 9d61d689cd3e3719. `--static-digest --dump` shows that the only difference is
  the new `hand_edges` on lightsaber weapons (his saber hit rework) in the `weapons` group. It was re-pinned, and the
  reason is in `scripts/one_player_gate.py`.

## Checks run

- `cargo check --workspace --all-targets` is clean apart from warnings: his launcher `f32: From<f64>` fallbacks and
  the existing `detailed_bench` `read`.
- `cargo build --release -p game -p launcher`.
- One-player gate: passes after the soak re-pin.
- `cargo test -p render --lib`: 154 passed. `cargo test -p net`: 77 passed. `cargo test -p ui --lib`: all passed
  except `typography::menu_modules_take_text_styles_from_typography` (known) and
  `shell_ia::tests::every_shell_help_line_fits_the_help_box`, which also fails on main as it stands.
- `cargo test -p game --lib -- split event_feed hud_feed sounds netplay player_names`: 213 passed.
  `cargo test -p game --bin game -- split xr lan extension menu`: 45 passed.
- `split_hud_edges` (HUD_ASSETS): 5 passed. `split_capture -- --ignored`: 2 passed. The four-way, two-way swapped
  and side-by-side name-tag captures look right: cells, HUD, timers, spawn pages and name tags.
