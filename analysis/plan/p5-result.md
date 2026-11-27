# P5 result: netplay and seat cleanup (2026-11-27)

## What changed
- Deleted (no callers): `LocalView::look_pads_of_slot` (netplay/view.rs), `Peer::pacer_of` (peer.rs), `desync::REPORT_BUNDLES` (the module doc now says "the last 300 bundles" in plain text).
- Behind `#[cfg(test)]`: `Party::is_unsure`, `ShotLedger::outstanding`, `NodeLink::client_mut`, `VirtualNet::host_skip`. Intra-doc links to them (local_seats.rs, virtual_net.rs) are plain text now.
- peer.rs: `Msg::DesyncHashes` rows decode once (`as_chunks::<6>`, no `expect`); the group check and the report rows use the same decode. One `own_rows(tick)` (hash text + rows text) replaces the two copies in the guest's report and the `SendRows` reply.
- player_names.rs: `clear()` is a plain `by_slot.clear()`.
- Net: 7 files, +41 / -56.

## Tests
- `cargo test -p game --lib --bin game -- netplay local_seats player_names lan_an_abandoned split`: lib 144 passed, 3 ignored; bin 32 passed.
- `cargo test -p game --test split_hud_edges -- --ignored`: 5 passed.
- `cargo test -p game --lib --bin game -- desync net_soak net_real`: 5 + 1 passed (net_soak desync report path unchanged).
- `one_player_gate.py` (release): every row matches; lan_soak ok, 4312 hash checks.
- clippy: no warnings in the touched code.

## Deviations
- `Peer::abandon_round` stays public and is not test-only: its one caller is a unit test in the `game` binary (main/windowed/lan.rs), which builds against the library without `cfg(test)`.
- `cargo clippy -p game --all-targets -- -D warnings` fails on main under the installed rustc 1.100 (new `chunks_exact_to_as_chunks` and others, plus `disallowed_methods` in audio, render, bake, ui and game). None of it is from this packet; the run was checked without `-D` and filtered to the touched files. The one old warning in the edited decode is gone.

## Left
- Bringing the tree clippy-clean under rustc 1.100 is a separate packet.
