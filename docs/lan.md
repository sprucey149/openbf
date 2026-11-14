# Playing over a LAN or VPN

openbf can host Instant Action for up to 16 players on one network, or over a
VPN such as Tailscale, ZeroTier or WireGuard. Bots fill the remaining places.
Every player builds their own copy from their own discs, as usual.

## 1. Everyone builds the same version

All players, **the host included**, must build from **the same commit of the
public openbf repository**. The game refuses to connect two different builds
and names both. A build from any other checkout (a fork, a private copy, a
download of the source) has a different build id, so everyone clones the
public repository, pulls the same commit, then runs the usual build:

```bash
git pull
git rev-parse --short=12 HEAD   # everyone should see the same 12 characters
python3 scripts/pack.py         # Windows: python scripts\pack.py
```

Local changes count too: if you edited a file under `engine/crates`, your
build id ends in `+` and a hash, and nobody else matches it. `git status`
shows what changed, and `git stash` sets it aside. Files your
system or editor leaves behind (`.DS_Store`, `Thumbs.db`, swap files) do not
count, and neither do Windows line endings.

`pack.py` bakes again when the bake code changed. A copy baked by an older
version is checked when each map loads.

Any combination of PS2, PC and Xbox copies can play together. The host offers
every map that all players have. Each map is checked when it loads. If one player's copy plays that map differently, that
player goes back to the lobby with a message naming the map, and the other
players continue.

## 2. Check the connection (optional, one minute)

Before your first game, run this headless check between two machines. Start
the host first:

```bash
# on the host
dist/openbf --net-host --seconds 30                  # Windows: dist\openbf.exe --net-host --seconds 30
# on the guest (the host's LAN or VPN address)
dist/openbf --net-join 100.101.102.103 --seconds 30  # Windows: dist\openbf.exe --net-join 100.101.102.103 --seconds 30
```

Both sides print the ping and the hash checks while the round runs, and count
the 30 seconds from the moment the round starts. The last line is the
verdict:

- **`PASS: N hash checks, 0 desyncs`**: the two builds play the same game.
- **`FAIL: ...`** (exit status 1): the line says why. Nothing was compared
  when the other side never connected or the round never started (check the
  address and the firewall), the host refused the build, the copies of the
  map play differently, or a desync was found.

## 3. Host a game

1. Choose a profile, then **Multiplayer → LAN: Host Game**.
2. Set the maximum players, cheats and awards:
   - **Cheats** are Off, or a list of the cheats players may switch on for
     themselves.
   - **Awards** turns everyone's earned awards on or off.
3. The Host page lists your addresses. Tell your friends the right one:
   - the **100.x** address on Tailscale;
   - the LAN address on the same network;
   - on **ZeroTier** or a WireGuard tunnel that does not carry all your
     traffic, your VPN address. The Host page may not show it: read it in the
     VPN app (`zerotier-cli listnetworks`, `wg show`, or `ip addr` /
     `ipconfig`).
4. On Windows, when the firewall asks, allow openbf on **Private and Public**
   networks.
5. Build the playlist on the Instant Action screen. Guests see it update as
   you edit it. Then start.

## 4. Join a game

1. Choose a profile, then **Multiplayer → LAN: Join Game**.
2. Games on your network appear in the list.
3. Over Tailscale or WireGuard, games are not listed. Type the host's address
   (an IP, a Tailscale name, or `address:port`) under **Join by address**.
4. **Test connection** shows the ping and whether your build matches before you
   join.
5. You can join while a round is running: the game catches up and you spawn
   in.

## In a battle

- **There is no pause in multiplayer.** The pause menu opens over the running
  battle, and your soldier stands still while it is open.
- **Hold Select (View) or Tab** for the scoreboard, which includes everyone's
  ping.
- **F7** shows the network meter: ping, input delay, connection health and
  quality.
- **Names** show over other players' heads only when you can see them.
- **Cheats:** the host switches cheats for anyone. Players can switch only the
  cheats the host allows. `slow` is not available in multiplayer.
- **Leaving:** a player who leaves is replaced by a bot where they stood. They
  can reconnect into their place within 90 seconds. If the host leaves, the
  round ends for everyone.
- **Next round:** after each round, everyone sees their own stats. The playlist
  continues when every player is ready.

## Latency

- Your own movement, aiming and shots respond immediately.
- Hits are judged against what you saw when you fired.
- Other players' actions and hit markers arrive one round trip later. On a LAN
  that is a tick or two; on a VPN in the same region it is under about 100 ms.

## Troubleshooting

| Problem | What to do |
|---|---|
| **"The host runs a different openbf build"** | Everyone, the host included, builds from the same commit of the public repository and runs `pack.py` again. A build id ending in `+` and a hash has local changes: see `git status`. |
| **No answer from the host** | Check the address and the host's firewall. Make sure the host has started hosting. |
| **"Your copy of a map plays differently"** | That map stays playable for the others. Pick another map for mixed groups. |
| **A desync message** | The game rejoins on its own. A report is saved under `openbf_data/` next to the game. Please attach it to an issue. |
