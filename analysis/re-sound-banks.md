# RE: BF1 (PS2) sound banks and the loading-screen beep

Recon result for P1 acceptance item "extracts the BF1 loading-beep audio to OGG".
Date: 2026-08-30. All offsets are absolute file offsets, LE byte order.

## TL;DR — where the beep is

The loading-screen beep wave is **`ui_load_lp`**, a 720-byte mono SPU2-ADPCM sample in the
**`global` SampleBank inside `DATA/_LVL_PS2/CORE.LVL`** (1,069,444 bytes, always resident):

| thing | value |
|---|---|
| file (disc) | `DATA/_LVL_PS2/CORE.LVL` (extracted to `extracted/bf1/sound/DATA/_LVL_PS2/CORE.LVL`) |
| chunk path | `ucfb` → `0x5E262256` (=hash "SoundBankList") @0x8 → `0x9A644D15` ("SampleBank") @0x10 → `0x0FB40705` ("info") @0x18 + `0xD872E2A5` ("data") @0x7F8 |
| bank name | hash 0x1DFF06AE = `global`, format=1, numsamples=6 |
| beep sample | id hash **0xB62DC1E9 = `ui_load_lp`** (name cracked by brute force, verified vs hash fn) |
| ADPCM bytes | **absolute offset 0x800, size 720** (first sample in the data chunk) |
| stored rate | 3605 Hz, 1260 decoded frames = 0.350 s (tone ≈0.2 s + silent tail) |
| content | pure ~440 Hz tone at stored rate; loading screen plays it pitched up (see below) |

The loading screen plays it through two `soundproperties` defined in the `snd_` chunk of the same
CORE.LVL (@0x3000, size 16644), whose name hashes were cracked to names that literally appear as
loading-screen element names in `LOAD/LOAD.LVL`'s `load+` config chunk (@0x5A6C8):

| property (name hash) | wave | pitch | gain | flags | as-played fundamental |
|---|---|---|---|---|---|
| `load_xtracking` (0x58F43FC3) | ui_load_lp | 2.1 | 0.7 | looping, playinterval 0.05 | ≈925 Hz, period 0.166 s |
| `load_ytracking` (0x9A7F54D0) | ui_load_lp | 1.8 | 0.7 | looping, playinterval 0.05 | ≈793 Hz, period 0.194 s |
| `load_bar` (0xAE0C5993) | ui_load_lp | 0.4 | **0** | looping | muted in retail |
| `load_zoom` (0x0A98B669) | 0xB75D27CD (accept wave) | 1.5 | 1 | one-shot | — |
| `load_transition` (0xFA2B9691) | 0xE502F23B (not in this bank) | 1 | 1 | one-shot | — |

The iconic repeating "boop-beep" of the load screen = the x/y tracking-crosshair sweeps each
looping this one 440 Hz tone at pitches 2.1 and 1.8. Confidence: **high** — property names match
the `load_*` element names in the LOAD.LVL loading-screen config verbatim, the wave name decodes
to `ui_load_lp` ("UI load loop"), the bank is in the always-resident CORE.LVL, and the decoded
waveform is a clean beep (verification numbers below). The later [PS2 loading captures](re-hud-bf1-loading.md) confirm the two pitches and the pauses.

Negative results: `LOAD/*.LVL` contain only textures/models/`load+` config, **no** audio chunks;
`COMMON/ENG/COMMON.LVL` has no `snd_`/bank chunks either. `SOUND/SHELL.LVL` holds shell music/VO,
not the load beep.

## Hash function (critical — NOT plain lowercase FNV)

Chunk tags and all name/id hashes use FNV-1a-32 where each byte is OR'ed with 0x20
(decompiled at ELF `FUN_00444b30`, 0x00444b30):

```python
def swbf_hash(s):                      # NB: '_' (0x5F) hashes as 0x7F!
    h = 0x811C9DC5
    for c in s.encode():
        h ^= (c | 0x20)
        h = (h * 0x1000193) & 0xFFFFFFFF
    return h
```

`c | 0x20` lowercases letters but also maps `_` → 0x7F. Plain `.lower()` FNV-1a matches only for
names without underscores (that's why `soundbanklist`, `samplebank`, `info`, `data` resolved but
`shell_menu_accept` didn't until this was found). Digits/`.` are unchanged.

Resolved tag hashes (used below): `SoundBankList`=0x5E262256, `SampleBank`=0x9A644D15,
`StreamList`=0x5BB97F21, `Stream`=0x5F6F6D65 (LE bytes spell `emo_` — it is a hash, not ASCII),
`info`=0x0FB40705, `data`=0xD872E2A5, `name`=0x8D39BDE6, `format`=0xB99D8552,
`numsamples`=0x98B889CE, `numsegments`=0x40FBDEBD, `numchannels`=0x7816084B, `size`=0x23A0D95C,
`frequency`=0x2FB31C01, `id`=0x37386AE0, `padding`=0x809608B6, `sample`=0x96E382A7,
`segment`=0xF3C342E6, `segmentlist`=0x0A8325F2, `stream`=0x5F6F6D65, `layer`=0x07A640F6.

## SampleBank format (field-level)

Container (standard ucfb: `[u32 tag][u32 size][payload][pad to 4]`):

```
0x5E262256 "SoundBankList"
└─ 0x9A644D15 "SampleBank"
   ├─ 0x0FB40705 "info"   — metadata, a flat stream of (u32 field-hash, u32 value) pairs
   └─ 0xD872E2A5 "data"   — concatenated raw SPU2-ADPCM, samples laid out sequentially
```

`info` payload, in order (confidence: high for named fields, verified on CORE.LVL and
SOUND/SHELL.LVL banks):

```
name        = <bank name hash>            ("global" in CORE.LVL, "shell" in SOUND/SHELL.LVL)
format      = 1                           (only value seen; presumably = SPU2 ADPCM)
numsamples  = N
size        = <total ADPCM bytes of all samples>
then N sample records, each 6 pairs (the FIRST record omits the samplecount+padding pairs
and uses marker hash 0x8EBF7143 instead of 0x2E789FB4 — treat markers as record delimiters):
  0x8EBF7143|0x2E789FB4 = 0x96E382A7     (record marker; value = hash "sample")
  id          = <wave name hash>          (e.g. 0xB62DC1E9 = "ui_load_lp")
  frequency   = <sample rate in Hz>       (odd exact values like 3605, 22052, 44106 are normal)
  size        = <ADPCM bytes for this sample>
  0x1D48FEEF  = <decoded PCM frames>      (== size/16*28 exactly in banks; name unknown)
  padding     = <bytes of pad after this sample's data> (0 in banks; nonzero in streams)
then a trailer record (marker with value 0x809608B6 = hash "padding", then a small pad size),
then zero fill to the end of the info chunk.
```

Caveat: the leading `size,marker,id,frequency` of the FIRST record parses one pair short vs the
others — bake code should parse by scanning for marker pairs (value 0x96E382A7) rather than fixed
strides. **There is no offset field: a sample's data offset = sum of (size+padding) of all prior
samples**, starting at the data chunk's payload.

CORE.LVL `global` bank contents (data payload @0x800):

| # | id hash | name (cracked) | offset | bytes | rate Hz | dur s | used by |
|---|---|---|---|---|---|---|---|
| 1 | 0xB62DC1E9 | `ui_load_lp` | 0x800 | 720 | 3605 | 0.350 | **the loading beep** |
| 2 | 0xB75D27CD | ? (accept wave) | 0xAD0 | 2704 | 11060 | 0.428 | shell_menu_accept/enter, load_zoom |
| 3 | 0xB21A7DEA | ? (select wave) | 0x1560 | 592 | 16363 | 0.063 | shell_select_change |
| 4 | 0xC0D52579 | ? (key wave) | 0x17B0 | 896 | 16248 | 0.097 | shell_keyboard_key/backspace |
| 5 | 0xEBAA708C | ? (exit wave) | 0x1B30 | 2352 | 16074 | 0.256 | shell_menu_exit |
| 6 | 0x1C2ECDEE | `ui_cancel` | 0x2460 | 2720 | 16040 | 0.297 | shell_menu_cancel/error |

## Stream format (SOUND/*.LVL — for later, not needed for the beep)

`0x5BB97F21 "StreamList"` → `0x5F6F6D65 "Stream"` → `info` + `data`. Same pair-stream idea:
header = `name, format=1, numchannels, 0x182FD58D=16384 (block size?), numsegments,
size(total), 0x7AAF1A1C=1, 0x740FDB0C=16384`, then per-segment records like bank records but with
marker-first order (`marker, id, frequency, size, 0x1D48FEEF, padding`) and **nonzero padding**:
segment data is stored in 16 KiB-aligned blocks, `size`+`padding` = multiple of 0x4000, and here
0x1D48FEEF = (size+padding)/16*28 (frames of the padded block). SOUND/SHELL.LVL contains stream
`shell_vo` (0xB082ECB1, 181 segments, 6.9 MB), stream `shell_music` (0xF6885C99, 6 segments,
28 MB), and SampleBank `shell` (20 samples = L/R mono pairs layered into `shell_transition1..N`
stereo UI whooshes). Confidence: medium (structure parsed and self-consistent; only SHELL.LVL
examined; segment→block alignment rule inferred from sizes).

## snd_ property-script format

`snd_` chunk = `NAME` (u32 hash) then a compiled property script of `DATA`/`SCOP` chunks.
`DATA` payload = `[u32 field-hash][u8 argc][argc × u32 args]` (args are raw u32; floats are IEEE
bit patterns, name references are hashes). A `DATA` with argc=0 whose hash is a class name
(`soundproperties`=0x1F2FA20E…, `soundstreamproperties`, `bus`, etc. — all resolve via swbf_hash)
opens a definition; the following `SCOP` holds its fields (`name`, `gain`, `pitch`, `looping`,
`playinterval`, `bus`, …; sample list = `DATA 0x4143ACAF` + `SCOP` of
`(0x96E382A7 "sample", 2 args: wave-id, weight)`; stream sounds use `stream(<stream name hash>)` +
`segmentlist` of `segment(id, weight)`). Field 0xCA04EFE0 = "inherit/template" (value = another
property's name hash). Buses in CORE.LVL: root, ingame, Main, soundfx, music, shellmusic,
shellmovies, ambience + 0x82C7F467 (UI bus, gain 0.4, unresolved name).

## SPU2 ADPCM decode recipe (verified)

Standard PS2 VAG/SPU ADPCM, mono, 16-byte blocks → 28 PCM frames:

```
block: byte0 = (filter<<4)|shift, byte1 = flags, bytes2..15 = 28 nibbles (low nibble first)
filters (f0,f1)/64: [(0,0),(60,0),(115,-52),(98,-55),(122,-60)]
per nibble n (sign-extend 4-bit): s = (n << 12) >> shift
pcm = clamp16( s + h1*f0/64 + h2*f1/64 );  h2 = h1;  h1 = pcm (pre-clamp float ok)
```

flags on `ui_load_lp`: first block 0x06 (loop-start|repeat), middle 0x02 (repeat), last 0x03
(loop-end|repeat) — the whole sample is one SPU loop region, matching `looping(1)` in the
properties. Working decoder: `scratchpad/re-beep/decode_bank.py` (session scratchpad).

## Verification (Tier-3 numeric checks)

Decoded `ui_load_lp` (WAVs in scratchpad `re-beep/wav/`):
- 1260 frames @3605 Hz; envelope = clean attack→sustain→decay over ~0.2 s then digital silence
  (60-frame means: 805, 6453, 17885, …, 20280, 17611, 7302, 941, 2, 0, 0 …) — a beep, not noise.
- FFT (Hann): single dominant peak at **440.6 Hz** at stored rate; harmonics negligible.
- ffmpeg astats: Peak −0.26 dBFS, RMS −7.64 dBFS, DC −0.000057, flat factor 0 (no clip runs).
- Cross-check on SHELL.LVL bank sample 1 (47328 B, 22052 Hz → 82824 frames = 3.756 s exactly as
  the info chunk's 0x1D48FEEF said): Peak −0.03 dBFS, RMS −16.9 dBFS — sane audio.
- As-played renders: 7570 Hz (pitch 2.1, "xtracking") and 6489 Hz (pitch 1.8, "ytracking") WAVs +
  a libvorbis OGG test encode all fine.

## What the bake tool must do for P1

1. Extract `DATA/_LVL_PS2/CORE.LVL` from the ISO (7z or ISO9660 reader; iso/ stays read-only).
2. Walk ucfb chunks for tag 0x5E262256 → 0x9A644D15 → children 0x0FB40705 (info) and
   0xD872E2A5 (data).
3. Parse info as (hash,u32) pairs: read `numsamples`, then per record `id`, `frequency`, `size`,
   `padding`; compute offsets cumulatively. Select id == swbf_hash("ui_load_lp") == 0xB62DC1E9
   (assert offset 0, size 720, rate 3605 for this disc build as a regression check).
4. Decode SPU2 ADPCM per the recipe above → s16 mono PCM.
5. Encode OGG. Recommend emitting three files: `ui_load_lp.ogg` at native 3605 Hz (archival) and
   the as-played `load_xtracking` (resample-rate 7570 Hz) / `load_ytracking` (6489 Hz) renders,
   since the audible loading beep is the pitched version. (Or bake pitch into metadata.)
6. Optional stretch: same code path extracts all 6 `global` waves and the SOUND/SHELL.LVL bank.

## Open questions / honest unknowns

- Field-name hashes 0x1D48FEEF (frame count), 0x8EBF7143/0x2E789FB4 (record markers),
  0x182FD58D/0x7AAF1A1C/0x740FDB0C (stream block params) not cracked; roles are inferred and
  numerically confirmed, names unknown.
- Wave names for global-bank samples 2–5 and the `load_transition` wave 0xE502F23B (defined
  elsewhere — likely SHELL/ENG/SHELL.LVL or a movie stream) not resolved.
