//! Synthetic PCM only: exercise the CLI/environment boundary without retail data.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "openbf-audio-routes-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("restore.py"), WORKER).unwrap();
        Self(path)
    }

    fn bake(&self, command: &str, source: &Path, out: &Path, restore: bool, donor: bool) -> Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bake"));
        cmd.args([command])
            .arg(source)
            .arg(out)
            .env_remove("OPENBF_AUDIO_RESTORE_SCRIPT")
            // A direct bake must ignore even a broken Python selection.
            .env("OPENBF_PYTHON", self.0.join("no-python"));
        if restore {
            cmd.env("OPENBF_AUDIO_RESTORE_SCRIPT", self.0.join("restore.py"))
                .env("OPENBF_PYTHON", "python3");
        }
        if donor {
            cmd.arg("--donor-bnk").arg(self.0.join("donor.bnk"));
        }
        cmd.output().unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const WORKER: &str = r#"
import sys, struct, os
from pathlib import Path
assert sys.flags.isolated
assert sys.argv[1] == '--worker'
kind = sys.argv[2]
assert kind in ('effects', 'dialogue')
log = Path(__file__).with_suffix('.log')
def record(line):
    with log.open('a') as f:
        f.write(f'{os.getpid()} {kind} {line}\n')
record('start')
while True:
    header = sys.stdin.buffer.read(8)
    if not header:
        record('eof')
        break
    rate, size = struct.unpack('<II', header)
    payload = sys.stdin.buffer.read(size)
    assert len(payload) == size
    record(f'{rate} {size}')
    samples = struct.unpack('<' + 'h' * (size // 2), payload)
    out = [-s if s != -32768 else 32767 for s in samples]
    sys.stdout.buffer.write(struct.pack('<' + 'h' * len(out), *out))
    sys.stdout.buffer.flush()
"#;

fn raw_chunk(tag: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = tag.to_vec();
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    out.resize((out.len() + 3) & !3, 0);
    out
}

fn chunk(name: &str, data: &[u8]) -> Vec<u8> {
    raw_chunk(&mungefs::fnv1a_lowercase(name).to_le_bytes(), data)
}

fn pairs(fields: &[(&str, u32)]) -> Vec<u8> {
    fields
        .iter()
        .flat_map(|(key, value)| {
            mungefs::fnv1a_lowercase(key)
                .to_le_bytes()
                .into_iter()
                .chain(value.to_le_bytes())
        })
        .collect()
}

fn sample(id: u32, rate: u32, size: usize, alias: Option<u32>) -> Vec<u8> {
    let mut out = pairs(&[
        ("record", mungefs::fnv1a_lowercase("sample")),
        ("id", id),
        ("frequency", rate),
        ("size", size as u32),
    ]);
    if let Some(target) = alias {
        out.extend(pairs(&[("alias", target)]));
    }
    out
}

fn source(kind: &str, format: u32, records: &[u8], data: &[u8]) -> Vec<u8> {
    let mut info = pairs(&[("name", 0x12345678), ("format", format), ("numchannels", 1)]);
    info.extend(records);
    let mut body = chunk("info", &info);
    body.extend(chunk("data", data));
    raw_chunk(b"ucfb", &chunk(kind, &body))
}

fn success(output: Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn negate(pcm: &[i16]) -> Vec<i16> {
    pcm.iter().map(|s| s.saturating_neg()).collect()
}

fn decode(format: u32, raw: &[u8]) -> Vec<i16> {
    bake::adpcm::decode_bank_sample(if format == 4 { 5 } else { format }, raw)
}

#[test]
fn effects_local_common_and_donor_routes_keep_legacy_bytes_and_aliases() {
    for format in [1, 2, 4, 5] {
        let fixture = Fixture::new();
        // PS2 ADPCM, mono PCM, and the two IMA format numbers all reach
        // restoration after their own decoder. The bytes are synthetic.
        let raw = if format == 1 {
            vec![0x11; 64]
        } else if format == 2 {
            vec![0x11; 128]
        } else {
            vec![0x11; 144]
        };
        let mut records = sample(1, 14014, raw.len(), None);
        records.extend(sample(2, 22050, raw.len(), None));
        records.extend(sample(3, 14014, 0, Some(1)));
        records.extend(sample(4, 14014, 0, Some(1)));
        let data = [raw.clone(), raw.clone()].concat();
        let input = fixture.0.join("test.lvl");
        std::fs::write(&input, source("SampleBank", format, &records, &data)).unwrap();
        std::fs::write(
            fixture.0.join("donor.bnk"),
            source(
                "SampleBank",
                2,
                &sample(3, 8000, 8, None),
                &[1, 0, 2, 0, 3, 0, 4, 0],
            ),
        )
        .unwrap();
        let legacy = fixture.0.join("legacy");
        let restored = fixture.0.join("restored");
        success(fixture.bake("sounds", &input, &legacy, false, true));
        success(fixture.bake("sounds", &input, &restored, true, true));
        let manifest = std::fs::read(legacy.join("sounds/sounds.ron")).unwrap();
        assert_eq!(
            manifest,
            std::fs::read(restored.join("sounds/sounds.ron")).unwrap()
        );
        let ron: manifest::SoundsRon = ron::de::from_bytes(&manifest).unwrap();
        assert_eq!(ron.samples.len(), 4);
        assert_eq!(
            ron.samples[0].1, ron.samples[3].1,
            "ordinary aliases share one restored WAV"
        );
        for (id, rate, pcm) in [
            (1, 14014, decode(format, &raw)),
            (2, 22050, decode(format, &raw)),
            (3, 8000, vec![1, 2, 3, 4]),
        ] {
            let rel = &ron.samples.iter().find(|(key, _)| *key == id).unwrap().1;
            assert_eq!(
                std::fs::read(legacy.join("sounds").join(rel)).unwrap(),
                bake::adpcm::wav_mono_s16(rate, &pcm)
            );
            assert_eq!(
                std::fs::read(restored.join("sounds").join(rel)).unwrap(),
                bake::adpcm::wav_mono_s16(rate, &negate(&pcm))
            );
        }
        let log = std::fs::read_to_string(fixture.0.join("restore.log")).unwrap();
        assert_eq!(log.matches("effects start").count(), 1);
        assert_eq!(log.matches("effects eof").count(), 1);
        assert!(log.contains("effects 8000 8"));

        // A header-only bank takes its real PCM from adjacent common.bnk.
        std::fs::write(
            fixture.0.join("common.bnk"),
            source("SampleBank", format, &records, &data),
        )
        .unwrap();
        std::fs::write(&input, source("SampleBank", format, &records, &[])).unwrap();
        let common = fixture.0.join("common-out");
        success(fixture.bake("sounds", &input, &common, true, true));
        assert_eq!(
            std::fs::read(common.join("sounds/12345678/00000001.wav")).unwrap(),
            std::fs::read(restored.join("sounds/12345678/00000001.wav")).unwrap()
        );
    }
}

#[test]
fn voice_restores_before_single_encode_and_retains_paths_serials_and_index() {
    assert!(
        bake::ogg::have_ffmpeg(),
        "ffmpeg is required for voice route verification"
    );
    for format in [1, 4, 5] {
        let fixture = Fixture::new();
        let raw = if format == 1 {
            vec![0x11; 2048]
        } else {
            vec![0x11; 2304]
        };
        let mut records = sample(1, 22050, raw.len(), None);
        records.extend(sample(2, 14014, raw.len(), None));
        let input = fixture.0.join("voice.lvl");
        std::fs::write(
            &input,
            source(
                "Stream",
                format,
                &records,
                &[raw.clone(), raw.clone()].concat(),
            ),
        )
        .unwrap();
        let legacy = fixture.0.join("legacy");
        let restored = fixture.0.join("restored");
        success(fixture.bake("global-sound", &input, &legacy, false, false));
        success(fixture.bake("global-sound", &input, &restored, true, false));
        assert_eq!(
            std::fs::read(legacy.join("global.ron")).unwrap(),
            std::fs::read(restored.join("global.ron")).unwrap()
        );
        let pcm = decode(format, &raw);
        for (id, rate) in [(1, 22050), (2, 14014)] {
            let rel = format!("12345678/{id:08x}.ogg");
            assert_eq!(
                std::fs::read(legacy.join(&rel)).unwrap(),
                bake::ogg::encode_mono(&pcm, rate, 3, id).unwrap()
            );
            let audio = std::fs::read(restored.join(&rel)).unwrap();
            assert_eq!(
                audio,
                bake::ogg::encode_mono(&negate(&pcm), rate, 3, id).unwrap()
            );
            assert_eq!(&audio[14..18], &id.to_le_bytes());
        }
        let log = std::fs::read_to_string(fixture.0.join("restore.log")).unwrap();
        assert_eq!(
            log.matches("dialogue start").count(),
            log.matches("dialogue eof").count()
        );
        let starts = log.matches("dialogue start").count();
        assert!((1..=2).contains(&starts));
        assert_eq!(log.lines().count(), 2 + 2 * starts);
    }
}

#[test]
fn worker_failures_make_effect_and_voice_bakes_fail() {
    let fixture = Fixture::new();
    let records = sample(1, 22050, 64, None);
    for kind in ["SampleBank", "Stream"] {
        let input = fixture.0.join(format!("{kind}.lvl"));
        std::fs::write(&input, source(kind, 1, &records, &[0x11; 64])).unwrap();
        let command = if kind == "SampleBank" {
            "sounds"
        } else {
            "global-sound"
        };
        for (name, script) in [
            ("short", "raise SystemExit(17)\n"),
            ("late", &format!("{WORKER}\nraise SystemExit(18)\n")),
        ] {
            std::fs::write(fixture.0.join("restore.py"), script).unwrap();
            let out = fixture.0.join(format!("{kind}-{name}"));
            let result = fixture.bake(command, &input, &out, true, false);
            assert!(!result.status.success());
            assert!(String::from_utf8_lossy(&result.stderr).contains("audio restoration"));
        }
    }
}
