"""Generated PCM exercises the restoration used in the normal pack path."""

from array import array
import math
from pathlib import Path
import random
import shutil
import subprocess
import sys
import unittest

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import restore_audio as audio


def le_pcm(samples):
    values = array("h", samples)
    if sys.byteorder != "little":
        values.byteswap()
    return values.tobytes()


def rng_with_tone(rate, tone_end):
    rng = random.Random(98)
    return [
        rng.randrange(-120, 120) + (round(9200 * math.sin(math.tau * 190 * i / rate)) if i < tone_end else 0)
        for i in range(rate)
    ]


def relative_db(original, restored):
    return 10 * math.log10(sum(x * x for x in restored) / sum(x * x for x in original))


class AudioRestore(unittest.TestCase):
    def test_effects_keep_cue_and_loop_boundaries_and_level(self):
        rate = 16248  # An irregular sample rate exercised by the console bank path.
        samples = [round(9000 * math.sin(math.tau * 132 * i / rate)) + 390 for i in range(rate // 3)]
        original = le_pcm(samples)
        restored = audio.process_pcm(original, rate, "effects")
        self.assertNotEqual(original, restored)
        self.assertEqual(restored, audio.process_pcm(original, rate, "effects"))
        self.assertEqual(len(original), len(restored))
        self.assertEqual((original[:2], original[-2:]), (restored[:2], restored[-2:]))
        out = audio.from_le("h", restored)
        self.assertLess(abs(relative_db(samples, out)), 0.35)

    def test_honors_authored_rates_outside_typical_output_device_range(self):
        for rate in (322, 200012):
            with self.subTest(rate=rate):
                source = le_pcm(int(5300 * math.sin(i * 0.13)) for i in range(rate // 20))
                result = audio.process_pcm(source, rate, "effects")
                self.assertEqual(len(source), len(result))
                self.assertNotEqual(source, result)

    def test_music_keeps_stereo_shape_without_crosstalk(self):
        if not shutil.which("ffmpeg"):
            self.skipTest("ffmpeg is required for pack.py audio")
        rate = 48000
        track = [int(5600 * math.sin(math.tau * 220 * i / rate)) for i in range(rate // 2)]
        source = le_pcm(v for sample in track for v in (sample, 0))
        restored = audio.process_pcm(source, rate, "music")
        output = audio.from_le("h", restored)
        self.assertEqual(len(source), len(restored))
        self.assertEqual(source[:4], restored[:4])
        self.assertEqual(source[-4:], restored[-4:])
        self.assertNotEqual(source, restored)
        self.assertEqual(max(map(abs, output[1::2])), 0)
        self.assertLess(abs(relative_db(track, output[::2])), 0.25)

    def test_music_limiter_keeps_a_hot_cue_sample_accurate_and_repeatable(self):
        if not shutil.which("ffmpeg"):
            self.skipTest("ffmpeg is required for pack.py audio")
        rate = 22054
        frames = rate // 8
        source = [0] * (frames * 2)
        impulse = rate // 17
        source[0], source[1], source[-2], source[-1] = 5, -17, 51, -83
        source[2 * impulse] = -32768
        pcm = le_pcm(source)
        restored = audio.process_pcm(pcm, rate, "music")
        output = audio.from_le("h", restored)
        self.assertEqual(len(pcm), len(restored))
        self.assertEqual(output[:2], array("h", source[:2]))
        self.assertEqual(output[-2:], array("h", source[-2:]))
        self.assertEqual(max(range(frames), key=lambda i: abs(output[2 * i])), impulse)
        self.assertLessEqual(max(map(abs, output)), 32767)
        self.assertEqual(restored, audio.process_pcm(pcm, rate, "music"))

    def test_stationary_radio_noise_and_tonal_tail_keep_their_character(self):
        rate = 22054
        rng = random.Random(8)
        radio = [rng.randrange(-1200, 1200) for _ in range(rate)]
        tone_bed = [rng.randrange(-1200, 1200) for _ in range(rate // 2)]
        tone_bed += [int(230 * math.sin(math.tau * 27 * i / rate)) for i in range(rate // 2 + 1)]
        for source in (radio, tone_bed):
            window = rate // 20
            self.assertIsNone(audio.noise_floor(source, audio.window_powers(source, window), window))
            original = le_pcm(source)
            # An unavailable FFmpeg is a test probe for entering the spectral path.
            result = audio.process_pcm(original, rate, "dialogue", ffmpeg="unused-ffmpeg")
            self.assertEqual(len(result), len(original))

    def test_dialogue_reduces_measured_noise_without_delaying_the_signal(self):
        if not shutil.which("ffmpeg"):
            self.skipTest("ffmpeg is required for pack.py audio")
        for rate in (8000, 22054, 48000):
            with self.subTest(rate=rate):
                samples = rng_with_tone(rate, rate // 2)
                # A cue-like click after the sounding half catches FFT delay
                # even if the result has the correct number of samples.
                impulse = rate * 3 // 5
                samples[impulse] = 26000
                source = le_pcm(samples)
                restored = audio.process_pcm(source, rate, "dialogue")
                out = audio.from_le("h", restored)
                self.assertEqual(len(out), len(samples))
                self.assertEqual(max(range(impulse - rate // 40, impulse + rate // 25),
                                     key=lambda i: abs(out[i])), impulse)
                gap = slice(rate * 7 // 10, rate * 85 // 100)
                self.assertLess(relative_db(samples[gap], out[gap]), -4)
                sounding = slice(rate // 16, rate * 3 // 8)
                self.assertLess(abs(relative_db(samples[sounding], out[sounding])), 0.5)

    def test_silence_and_very_short_click_are_preserved(self):
        silence = bytes(16000)
        click = le_pcm((0, 5000, -5000, 0))
        self.assertEqual(audio.process_pcm(silence, 8000, "effects"), silence)
        self.assertEqual(audio.process_pcm(click, 48000, "effects"), click)

    def test_worker_multiplexes_requests_and_rejects_partial_payload(self):
        program = [sys.executable, "-I", str(SCRIPTS / "restore_audio.py"), "--worker", "effects"]
        pcm = le_pcm(round(1600 * math.sin(0.14 * i)) for i in range(3000))
        requests = [audio.REQUEST.pack(rate, len(pcm)) + pcm for rate in (11025, 22054)]
        run = subprocess.run(program, input=b"".join(requests), capture_output=True, check=True)
        expected = b"".join(audio.process_pcm(pcm, rate, "effects") for rate in (11025, 22054))
        self.assertEqual(run.stdout, expected)
        self.assertFalse(run.stderr)
        truncated = subprocess.run(program, input=requests[0][:-2], capture_output=True)
        self.assertNotEqual(truncated.returncode, 0)
        self.assertFalse(truncated.stdout)
        self.assertIn(b"Truncated audio request payload", truncated.stderr)

    def test_malformed_input_does_not_get_processed(self):
        with self.assertRaisesRegex(ValueError, "Invalid effects PCM"):
            audio.process_pcm(b"\x00", 16000, "effects")
        with self.assertRaisesRegex(ValueError, "Invalid music PCM"):
            audio.process_pcm(b"\x00\x00", 48000, "music")


if __name__ == "__main__":
    unittest.main()
