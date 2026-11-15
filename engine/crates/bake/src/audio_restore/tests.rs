use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new(script: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "openbf-audio-worker-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("worker.py"), script).unwrap();
        Self(root)
    }

    fn restore(&self) -> AudioRestore {
        AudioRestore {
            config: Some(Config {
                python: "python3".into(),
                script: self.0.join("worker.py"),
                kind: "effects",
            }),
            worker: None,
            failed: false,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const ECHO: &str = r#"
import sys, struct
from pathlib import Path
assert sys.flags.isolated
assert sys.argv[1:] == ['--worker', 'effects']
log = Path(__file__).with_suffix('.log')
with log.open('a') as f:
    f.write('start\n')
while True:
    header = sys.stdin.buffer.read(8)
    if not header:
        with log.open('a') as f:
            f.write('eof\n')
        break
    rate, size = struct.unpack('<II', header)
    payload = sys.stdin.buffer.read(size)
    assert len(payload) == size
    with log.open('a') as f:
        f.write(f'{rate} {size}\n')
    values = struct.unpack('<' + 'h' * (size // 2), payload)
    result = [min(32767, v + 7) for v in values]
    sys.stdout.buffer.write(struct.pack('<' + 'h' * len(result), *result))
    sys.stdout.buffer.flush()
"#;

#[test]
fn persistent_worker_preserves_frames_and_little_endian_rates() {
    let fixture = Fixture::new(ECHO);
    let mut restore = fixture.restore();
    let mut first = [i16::MIN, -1, 0, 1234, i16::MAX];
    restore.apply(14014, &mut first).unwrap();
    assert_eq!(first, [-32761, 6, 7, 1241, 32767]);
    let pid = restore.worker.as_ref().unwrap().child.id();
    let mut large = vec![1000; 100_000];
    restore.apply(48000, &mut large).unwrap();
    assert_eq!(restore.worker.as_ref().unwrap().child.id(), pid);
    assert_eq!(large, vec![1007; 100_000]);
    restore.apply(8000, &mut []).unwrap();
    restore.finish().unwrap();
    assert!(restore.worker.is_none());
    assert_eq!(
        std::fs::read_to_string(fixture.0.join("worker.log")).unwrap(),
        "start\n14014 10\n48000 200000\n8000 0\neof\n"
    );
}

#[test]
fn disabled_restoration_is_byte_exact_and_starts_no_worker() {
    let mut restore = AudioRestore {
        config: None,
        worker: None,
        failed: false,
    };
    let mut pcm = [i16::MIN, -1, 0, 5, i16::MAX];
    let original = pcm;
    let wav = crate::adpcm::wav_mono_s16(14014, &pcm);
    restore.apply(14014, &mut pcm).unwrap();
    restore.finish().unwrap();
    assert_eq!(pcm, original);
    assert_eq!(crate::adpcm::wav_mono_s16(14014, &pcm), wav);
    assert!(restore.worker.is_none());
}

#[test]
fn exit_after_valid_response_fails_finish() {
    let fixture = Fixture::new(&format!("{ECHO}\nsys.exit(23)\n"));
    let mut restore = fixture.restore();
    restore.apply(22050, &mut [10, 20]).unwrap();
    let error = restore.finish().unwrap_err();
    assert!(error
        .to_string()
        .contains("audio restoration worker failed"));
    assert!(restore.worker.is_none());
}

#[test]
fn failed_write_discards_worker_and_never_restarts() {
    let fixture = Fixture::new("raise SystemExit(21)\n");
    let mut restore = fixture.restore();
    let mut pcm = vec![1234; 100_000];
    assert!(restore.apply(22050, &mut pcm).is_err());
    assert_eq!(pcm, vec![1234; 100_000]);
    assert!(restore.worker.is_none());
    assert!(restore
        .apply(22050, &mut pcm)
        .unwrap_err()
        .to_string()
        .contains("previously failed"));
}

#[test]
fn invalid_configuration_fails_without_starting_worker() {
    let fixture = Fixture::new(ECHO);
    let mut restore = fixture.restore();
    restore.config.as_mut().unwrap().script = "relative.py".into();
    assert!(restore
        .apply(22050, &mut [1])
        .unwrap_err()
        .to_string()
        .contains("must be absolute"));
    assert!(restore.worker.is_none());
    let mut restore = fixture.restore();
    restore.config.as_mut().unwrap().python = fixture.0.join("missing-python").into();
    assert!(restore.apply(22050, &mut [1]).is_err());
    assert!(restore.worker.is_none());
}

#[cfg(target_os = "linux")]
#[test]
fn truncated_response_and_early_drop_kill_and_reap_children() {
    let fixture = Fixture::new(ECHO);
    let mut restore = fixture.restore();
    restore.apply(22050, &mut [1, 2]).unwrap();
    let pid = restore.worker.as_ref().unwrap().child.id();
    assert!(std::path::Path::new(&format!("/proc/{pid}")).exists());
    drop(restore);
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());

    // Close stdout after one byte, but stay alive. A read failure must kill
    // and reap this child instead of waiting for its 30-second sleep.
    let script = r#"
import os, sys, time
from pathlib import Path
Path(__file__).with_suffix('.pid').write_text(str(os.getpid()))
sys.stdin.buffer.read(12)
sys.stdout.buffer.write(b'x')
sys.stdout.buffer.flush()
os.close(1)
time.sleep(30)
"#;
    let fixture = Fixture::new(script);
    let mut restore = fixture.restore();
    let mut pcm = [10, 20];
    assert!(restore.apply(22050, &mut pcm).is_err());
    assert_eq!(pcm, [10, 20], "partial output must not modify PCM");
    let pid = std::fs::read_to_string(fixture.0.join("worker.pid")).unwrap();
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
}
