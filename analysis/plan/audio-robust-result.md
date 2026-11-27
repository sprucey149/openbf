# Audio robustness and audit (AUDIO-ROBUST), 2026-11-27

Evidence: the 4-player couch session of 2026-11-25 (openbf log, kernel log).
The silence that night came from the couch box's own output-follow tool moving
the stream to a wired DS4 headset jack; that is fixed on the box. This packet
makes openbf itself unable to go silent for good, and audits its audio.

## What changed

1. **Output watchdog** (`engine/crates/audio/src/output.rs`). The mixer no
   longer runs on kira's default cpal backend. That backend restarts its stream
   only on `DeviceNotAvailable` and only if the default device opens at that
   instant; ALSA's `POLLERR` loop (a sound-server node dropped under the
   stream) arrives as `BackendSpecific` and is ignored, and a failed restart
   `unwrap()`s and panics its thread with the renderer inside. Any of these was
   silence for the rest of the session.
   The new backend (`Watched<Cpal>`) keeps kira's renderer outside the stream
   (a mutex only the live stream's callback takes, with `try_lock`), so a
   stream is just a pump. A watchdog thread looks every 500 ms and replaces the
   pump when it has pulled no frame for 1.5 s, reports its device gone, or the
   default device or rate changes (kira's own check, kept). When no output
   opens it retries every second. A replaced stream that wakes again writes
   silence (an epoch check), so two streams never share the renderer. Because
   the renderer survives, every track, sound, music voice and handle the game
   holds plays on: there is no AudioManager rebuild and the game is not told.
   One line is logged per event: `audio: output reopened on '<dev>' (<rate>
   Hz): <reason>`, or `audio: output lost (...); no output opens yet (...),
   retrying every 1.0 s`. The first four stream errors of each stream are
   logged as `audio: output error: ...`; the rest are counted in the
   stall line.
   - **Renderer panics.** cpal 0.15.3's ALSA worker has no `catch_unwind`, and
     its `Stream::drop` unwraps the worker's join, so a panic inside kira's
     renderer would kill the worker and then the watchdog. The render call is
     now inside `catch_unwind`: that buffer is silence, the stream lives, and
     the watchdog logs the first panic and then a count at most every 30 s.
     Dropping an old stream is also inside `catch_unwind`.
   - **Streams that open but never pull** (a sound server that accepts the
     stream but never runs it): each one in a row doubles the wait before the
     next open (1 s, 2 s, 4 s ... up to 30 s). Only the first is logged; after
     that a count is logged at most every 30 s, and one line when a stream
     pulls again.
   - **Outages keep time.** While no stream is open, the watchdog mixes the
     elapsed time into nowhere (at most 1 s per look). Sounds end on time and
     the buses drain, so a long outage does not leave the game bus full and
     every queued one-shot does not fire at once when an output returns. The
     1.5 s before a stall is detected is not mixed; up to that much can still
     start together.
   - **Rate changes.** The renderer is switched to a new stream's rate inside
     that stream's callback, before its first buffer. Before, `play()` ran
     first, so the first buffers played at the wrong pitch.
2. **Non-finite mix values** (`audio::sane`, `audio::tween_secs`, limiter).
   A NaN level, rate or pan now reaches kira as silence, unity rate, centre;
   a fade time of infinity (which made `Duration::from_secs_f32` panic) is
   instant. The master limiter drops a non-finite sample to 0 so a NaN voice
   cannot write NaN to the device or into the gain. Finite values pass
   unchanged.
3. **Refused loops come back** (`sounds/playback.rs`). An engine, hum,
   repair, shield or walker loop the full game bus refused was stored as
   `None` and never asked again while its source stayed live, so a vehicle
   that spawned during a full bus stayed silent for its life. Refused loops are
   now remembered and started on the first tick the bus has room; a loop with
   no take still is not retried.
4. **Play-failure log is rate-limited.** The Kashyyyk XL round logged 855
   `play failed ... [512 of 512 sounds]` lines in about 100 s (one stderr
   write each, on the main thread). Now one line per bus at most every 5 s.
   The count refused in between is logged with the time from the first to the
   last of them, by the next line or when the game bus is cleared at the end of
   a round, so the last burst is not lost.

## Tests

- `cargo test -p audio` (21 passed): new
  `output::tests::a_stalled_output_is_reopened_and_the_same_sounds_play_on`,
  `a_lost_output_is_retried_until_one_opens`,
  `a_healthy_output_is_never_reopened`, `a_replaced_stream_writes_silence`,
  `a_renderer_panic_is_silence_and_the_mix_plays_on`,
  `sounds_keep_time_while_no_output_opens`, `streams_that_never_pull_back_off`,
  `tests::non_finite_mix_values_are_made_safe`,
  `limiter::tests::a_non_finite_frame_is_dropped`; the real-device tests
  (`backend_opens_or_degrades` opened: true, pause, clear, headroom) pass on
  the new backend. The seven watchdog tests passed 10 of 10 repeated runs.
- `cargo test -p game --lib -- sounds::` 72 passed, 7 ignored, including new
  `sounds::tests::a_loop_refused_by_a_full_bus_starts_when_there_is_room`
  (real mixer).
- `cargo check -p audio --no-default-features` clean; clippy on `audio`
  adds no warning (two `let_unit_value` in `clear_game_bus` are old).
- One-player gate: every row matches (bf1_hot1, bf2_uta1, bf2_spa1, LAN soak).

## Measurements

Mixer cost per second of output, N looping voices behind the limiter, pulled
1024 frames at a time (the `output::voice_cost` bench of commit 5a281e6b6,
since removed; build box i9-13980HX, one core):

| voices | 0 | 128 | 256 | 512 | 1024 |
|---|---|---|---|---|---|
| ms per s | 1.1 | 66 | 133 | 268 | 534 |

About 0.52 ms per voice per second: the 512-sound game bus at full is about a
quarter of a core here, more on the couch box's Zen 4 cores. So the cap was
**not** raised: 1024 would be half a core on the audio thread, beside four
views of rendering, and risks underruns.

## Audited, no change

- **Voices between rounds:** leaving a battle renews the game and voice buses
  (`clear_game_bus`) and clears the loop tables; in the session log the only
  saturated round is Kashyyyk XL, and the next round started clean. No leak
  found.
- **Splitscreen listeners (B11/B12):** a shared listener pans centre
  (`side` returns 0), `side` guards a zero-length direction, attenuation
  guards its denominators, loop rates are floored at 0.05 and the doppler
  factor at 0.05 through that floor. No NaN path found; item 2 is the guard
  behind them.
- **Music after an output loss:** the renderer survives and is mixed into
  nowhere while no output is open, so music voices, their `playing()` state
  and fades continue in time; nothing to restore.
- **4910 takes decoded at load (1.2 GB):** not changed. A BF2 round decodes
  the map bank and the whole BF2 core bank (every era's sides); BF1 rounds
  decode about 1100 takes (200 MB). kira holds every take as stereo f32
  (8 bytes a frame) even when mono, so about half the bytes are the duplicated
  channel. Cutting it cleanly needs either a per-side filter of the core bank
  (which props each side and sub-level can name, or a first shot decodes
  inside a frame, the hitch card HITCH removed) or a mono sound type of our
  own in place of `StaticSoundData`. Neither is a clean small win; left for a
  memory packet.

## Deviations

- The task asked for a rebuild of the AudioManager on a stall. Keeping the
  renderer alive under a replaceable stream does the same job without
  orphaning the handles the game holds (engine voices, music, cries would all
  have gone silent or stuck "playing" after a rebuild). The stall signal is
  the stream's own frame counter, bumped where the renderer is pulled, rather
  than the limiter's `process()`: the same thread and moment, without an
  extra hook.
- `cpal = "0.15"` is now a direct (optional, `backend`) dependency of
  `audio`; it resolves to the 0.15.3 kira already linked.

## Left

- Not exercised on real hardware: a sound-server restart or a pulled USB
  output during play. The fake output covers the watchdog logic; the cpal
  path is the same calls kira made. Worth one deliberate test on the couch
  box (`systemctl --user restart pipewire` mid-round, expect one
  `audio: output reopened` line and sound back within about 2 s).
- Ambient loops refused by a full bus still stay silent until their emitter
  leaves range (they start with the round, when the bus is empty).
- The full game bus itself in 4-player XL rounds: a nearest-N voice cull
  would keep the loudest sounds instead of refusing the newest.
