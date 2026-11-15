#!/usr/bin/env python3
"""Offline PCM restoration for the pack bake.

Each worker request is a little-endian u32 sample rate, a little-endian u32
payload byte count, and interleaved signed 16-bit little-endian PCM. The reply
is the same number of PCM bytes at the same sample rate and channel count.
There is no output header. EOF between requests closes the worker.
"""

import argparse
from array import array
from dataclasses import dataclass
import math
import struct
import subprocess
import sys


@dataclass(frozen=True)
class Profile:
    channels: int
    rumble_hz: float
    body_hz: float
    body_db: float
    presence_db: float


# Deliberate presentation choices. measure: level-matched retail playback.
PROFILES = {
    "dialogue": Profile(1, 65, 210, 1.6, 1.4),
    "effects": Profile(1, 28, 160, 0.9, 0.9),
    "music": Profile(2, 16, 145, 0.35, 0.25),
}
REQUEST = struct.Struct("<II")
FULL_SCALE = 32768


def from_le(typecode, data):
    values = array(typecode)
    values.frombytes(data)
    if sys.byteorder != "little":
        values.byteswap()
    return values


def window_powers(samples, size):
    powers = []
    for start in range(0, len(samples), size):
        window = samples[start:start + size]
        powers.append(sum(sample * sample for sample in window) / len(window))
    return powers


def noise_floor(samples, powers, window):
    """Estimate steady noise only where a speech pause provides evidence."""
    peak = max(powers)
    if peak < FULL_SCALE**2 * 10**-3:
        return None
    floor_min = FULL_SCALE**2 * 10**-6.8
    floor_max = min(peak * 10**-2.2, FULL_SCALE**2 * 10**-3.7)
    # Skip leading padding and the final partial window. Require three
    # consecutive windows within ~2.2 dB before accepting a pause.
    for start in range(2, len(powers) - 3):
        candidate = powers[start:start + 3]
        lowest, highest = min(candidate), max(candidate)
        if not floor_min <= lowest <= highest <= floor_max or highest > 1.65 * lowest:
            continue
        chunk = samples[start * window:(start + 3) * window]
        difference = sum((right - left) ** 2 for left, right in zip(chunk, chunk[1:]))
        # Steady low pitches may be part of a voice, effect or musical bed.
        if difference >= 0.16 * len(chunk) * highest:
            return 10 * math.log10(sum(candidate) / (len(candidate) * FULL_SCALE**2))
    return None


def ffmpeg_filter(pcm, rate, channels, filters, ffmpeg, *, output_format="f32le"):
    command = [
        ffmpeg, "-nostdin", "-hide_banner", "-loglevel", "error", "-filter_threads", "1",
        "-f", "s16le", "-ar", str(rate), "-ac", str(channels), "-i", "pipe:0",
        "-af", filters, "-f", output_format, "pipe:1",
    ]
    call = subprocess.run(command, input=pcm, capture_output=True)
    if call.returncode:
        detail = call.stderr.decode(errors="replace").strip()
        raise RuntimeError(f"FFmpeg audio filter failed: {detail or call.returncode}")
    expected = len(pcm) * (2 if output_format == "f32le" else 1)
    if len(call.stdout) != expected:
        raise RuntimeError("FFmpeg audio filter changed the sample count")
    return call.stdout


def denoise(pcm, rate, floor, ffmpeg):
    # FFmpeg's afftdn overlap-add has two groups of rate // 80 samples of
    # delay (libavfilter/af_afftdn.c). Supply the trailing padding and trim
    # precisely that delay, retaining the take's original first and last time.
    delay = 2 * (rate // 80)
    filters = (
        f"apad=pad_len={delay},afftdn=nr=6:nf={floor:.2f}:gs=5,"
        f"atrim=start_sample={delay}"
    )
    return from_le("f", ffmpeg_filter(pcm, rate, 1, filters, ffmpeg))


def shape_tone(samples, rate, profile):
    """Apply gentle one-pole rumble, body and presence filters per channel."""
    def coefficient(hz):
        return -math.expm1(-math.tau * hz / rate)

    ac = coefficient(profile.rumble_hz)
    ab = coefficient(profile.body_hz)
    al = coefficient(min(850, rate * 0.16))
    ah = coefficient(min(2850, rate * 0.38))
    body_gain = 10 ** (profile.body_db / 20) - 1
    presence_gain = 10 ** (profile.presence_db / 20) - 1
    output = array("f", [0]) * len(samples)
    for channel in range(profile.channels):
        rumble = body = low = high = samples[channel]
        for i in range(channel, len(samples), profile.channels):
            current = samples[i]
            rumble += ac * (current - rumble)
            body += ab * (current - body)
            low += al * (current - low)
            high += ah * (current - high)
            output[i] = current - rumble + body_gain * (body - rumble) + presence_gain * (high - low)
    return output


def restore_music(pcm, rate, frames, profile, ffmpeg):
    """Filter whole stereo cues natively and preserve the original cue joins."""
    presence = min(2600, rate // 3)
    filters = (
        f"highpass=f={profile.rumble_hz:g},bass=f={profile.body_hz:g}:g={profile.body_db:g},"
        f"equalizer=f={presence}:t=q:w=1.1:g={profile.presence_db:g},"
        "volume=-0.25dB,alimiter=limit=0.99996:level=0:latency=1"
    )
    result = ffmpeg_filter(pcm, rate, profile.channels, filters, ffmpeg, output_format="s16le")
    edge = max(1, min(rate // 120, frames // 4))
    # Convert only the taper spans to Python arrays. Every interior sample
    # remains in native FFmpeg and the first and last stereo frames stay exact.
    size = edge * 2 * profile.channels
    prefix = music_edge(pcm[:size], result[:size], profile.channels, edge, reverse=False)
    suffix = music_edge(pcm[-size:], result[-size:], profile.channels, edge, reverse=True)
    return b"".join((prefix, memoryview(result)[size:-size], suffix))


def music_edge(original_pcm, restored_pcm, channels, frames, *, reverse):
    original = from_le("h", original_pcm)
    restored = from_le("h", restored_pcm)
    output = array("h")
    for i, (old, new) in enumerate(zip(original, restored)):
        frame = i // channels
        blend = (frames - 1 - frame if reverse else frame) / frames
        output.append(round(old + blend * (new - old)))
    if sys.byteorder != "little":
        output.byteswap()
    return output.tobytes()


def tone(pcm, original, rate, kind, profile, powers, window, ffmpeg):
    floor = noise_floor(original, powers, window) if kind == "dialogue" and rate >= 8000 else None
    if floor is None:
        return shape_tone(original, rate, profile)

    filtered = denoise(pcm, rate, floor, ffmpeg)
    # Blend spectral cleanup out of louder blocks, where transmitter hiss and
    # consonants belong to the line's character. Interpolate power between
    # windows so the wet signal cannot step at window boundaries.
    floor_power = FULL_SCALE**2 * 10 ** (floor / 10)
    wet = [min(1, 32 * floor_power / max(power, floor_power)) for power in powers]
    at = 0
    for i in range(len(filtered)):
        if (at + 1) * window <= i:
            at += 1
        left = wet[max(0, at - 1)]
        amount = left + (wet[at] - left) * ((i - at * window) / window)
        original_sample = original[i]
        filtered[i] = original_sample + amount * (filtered[i] * FULL_SCALE - original_sample)
    return shape_tone(filtered, rate, profile)


def process_pcm(pcm, rate, kind, *, ffmpeg="ffmpeg"):
    """Return level-matched LE PCM with its original sample rate and shape."""
    profile = PROFILES[kind]
    frame_bytes = 2 * profile.channels
    if rate <= 0 or len(pcm) % frame_bytes:
        raise ValueError(f"Invalid {kind} PCM: {rate} Hz, {len(pcm)} bytes")
    frames = len(pcm) // frame_bytes
    if frames < rate // 35:
        return pcm
    if kind == "music":
        return pcm if frames < 4 else restore_music(pcm, rate, frames, profile, ffmpeg)

    original = from_le("h", pcm)
    window = max(1, rate // 20) * profile.channels
    powers = window_powers(original, window)
    peak_power = max(powers)
    if peak_power <= 1:
        return pcm
    restored = tone(pcm, original, rate, kind, profile, powers, window, ffmpeg)

    # Preserve the authored mix level, with enough headroom for the new peak.
    # Quiet pauses do not influence the level match.
    restored_power = window_powers(restored, window)
    active = [index for index, power in enumerate(powers) if power >= peak_power * 0.01]
    input_power = sum(powers[index] for index in active)
    output_power = sum(restored_power[index] for index in active)
    peak = max(max(restored), -min(restored))
    if output_power <= 0 or not math.isfinite(output_power) or not math.isfinite(peak):
        return pcm
    gain = min(math.sqrt(input_power / output_power), (FULL_SCALE - 1) / peak)

    # Fade the correction back to the source at each end of the take. This
    # retains the actual samples at boundaries used by the original loop/cue.
    edge = max(1, min(rate // 120, frames // 4))
    output = array("h")
    for index, (old, new) in enumerate(zip(original, restored)):
        frame = index // profile.channels
        blend = min(frame, frames - 1 - frame, edge) / edge
        output.append(round(old + blend * (gain * new - old)))
    if sys.byteorder != "little":
        output.byteswap()
    return output.tobytes()


def worker(source, sink, kind):
    while header := source.read(REQUEST.size):
        if len(header) != REQUEST.size:
            raise ValueError("Truncated audio request header")
        rate, size = REQUEST.unpack(header)
        pcm = source.read(size)
        if len(pcm) != size:
            raise ValueError("Truncated audio request payload")
        sink.write(process_pcm(pcm, rate, kind))
        sink.flush()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--worker", choices=PROFILES, required=True)
    args = parser.parse_args()
    try:
        worker(sys.stdin.buffer, sys.stdout.buffer, args.worker)
    except (OSError, RuntimeError, ValueError) as exc:
        parser.exit(1, f"restore_audio: {exc}\n")


if __name__ == "__main__":
    main()
