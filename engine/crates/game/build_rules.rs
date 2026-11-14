//! The pure rules behind the build identity (issue #168), shared by
//! `build.rs` and the game's unit tests (`netplay::content`). The same commit
//! must give the same id on Linux, Windows and macOS, so:
//!
//! - untracked OS and editor litter never counts as a change: Finder's
//!   `.DS_Store` and `._name` files, Explorer's `Thumbs.db` and
//!   `desktop.ini`, editor swap and backup files, and anything inside a
//!   hidden folder (`.vscode/`, `.idea/`);
//! - line endings never count: CRLF is read as LF before hashing, so a
//!   Windows checkout (Git's `core.autocrlf`) or a Windows editor hashes the
//!   same bytes as Linux and macOS.

/// An untracked path (as `git ls-files --others` prints it, `/`-separated,
/// possibly starting with `../`) that is OS or editor litter, never source.
pub fn is_junk(path: &str) -> bool {
    let parts: Vec<&str> = path
        .split(['/', '\\'])
        .filter(|c| !c.is_empty() && *c != "." && *c != "..")
        .collect();
    let Some(name) = parts.last() else {
        return true;
    };
    let lower = name.to_ascii_lowercase();
    parts.iter().any(|c| c.starts_with('.'))
        || lower.ends_with('~')
        || [".orig", ".rej", ".bak", ".swp", ".swo"]
            .iter()
            .any(|e| lower.ends_with(e))
        || ["thumbs.db", "ehthumbs.db", "desktop.ini", "icon\r"].contains(&lower.as_str())
}

/// `bytes` with every CRLF read as LF.
pub fn lf(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
            continue;
        }
        out.push(*b);
    }
    out
}

/// FNV-1a 64: stable across Rust versions and hosts (unlike `DefaultHasher`).
pub fn fnv(h: &mut u64, bytes: &[u8]) {
    for b in bytes {
        *h ^= u64::from(*b);
        *h = h.wrapping_mul(0x0100_0000_01b3);
    }
}

/// The dirty-tree hash: `diff` is `git diff --binary HEAD` over the build's
/// paths, `untracked` each kept untracked file's name and bytes. Empty when
/// there is nothing to hash, so a clean tree has no suffix.
pub fn diff_hash<'a>(
    diff: &[u8],
    untracked: impl IntoIterator<Item = (&'a str, Vec<u8>)>,
) -> String {
    let untracked: Vec<_> = untracked
        .into_iter()
        .filter(|(name, _)| !is_junk(name))
        .collect();
    if diff.is_empty() && untracked.is_empty() {
        return String::new();
    }
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    fnv(&mut h, &lf(diff));
    for (name, bytes) in untracked {
        fnv(&mut h, name.as_bytes());
        fnv(&mut h, &lf(&bytes));
    }
    format!("{h:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Litter from Finder, Explorer and editors never makes a build dirty;
    /// a real new source file does.
    #[test]
    fn build_rules_ignore_os_and_editor_litter() {
        for junk in [
            "../core/src/.DS_Store",
            "../.DS_Store",
            "../game/src/._main.rs",
            "../game/src/.main.rs.swp",
            "../game/src/main.rs~",
            "../game/src/main.rs.orig",
            "../game/assets/Thumbs.db",
            "../game/assets/desktop.ini",
            "../game/Desktop.ini",
            "../.vscode/settings.json",
            "../.idea/workspace.xml",
        ] {
            assert!(is_junk(junk), "{junk}");
        }
        for source in [
            "../game/src/netplay/new.rs",
            "../core/src/sim/state/x.rs",
            "../../Cargo.toml",
            "../game/assets/menu_cursor/new.svg",
        ] {
            assert!(!is_junk(source), "{source}");
        }
        assert_eq!(diff_hash(b"", [("../.DS_Store", b"x".to_vec())]), "");
        assert_eq!(diff_hash(b"", []), "");
        assert_ne!(diff_hash(b"", [("../game/src/new.rs", b"x".to_vec())]), "");
    }

    /// The same change hashes the same with Windows line endings.
    #[test]
    fn build_rules_read_crlf_as_lf() {
        assert_eq!(lf(b"a\r\nb\r\n\rc\n"), b"a\nb\n\rc\n");
        let unix = diff_hash(
            b"+fn x() {}\n",
            [("../game/src/new.rs", b"fn y() {}\n".to_vec())],
        );
        let windows = diff_hash(
            b"+fn x() {}\r\n",
            [("../game/src/new.rs", b"fn y() {}\r\n".to_vec())],
        );
        assert_eq!(unix, windows);
        assert_eq!(unix.len(), 16);
    }
}
