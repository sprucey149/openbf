//! Presentation choices for the fingerprinted retail PC BF2 shell.
//!
//! The console-shaped screens keep their existing geometry and widgets. PC
//! has no Instant Action sheet, so we draw an original quiet blue backdrop.
//! Its four Bink segments are galaxy views (all inspected at 5 seconds), not
//! the console ship loops. Our presentation sequence uses 0/1 for the main
//! menu and 2/3 for submenus; this is an explicit appearance difference.
use sha2::{Digest, Sha256};

pub(super) fn known_shell(raw: &[u8]) -> bool {
    format!("{:x}", Sha256::digest(raw))
        == "82335ab55e17689cd620ba174568e6a5481d4523b821549aa153a6b88a831009"
}

pub(super) fn movie_order(raw: &[u8], property: &str) -> Option<Vec<u32>> {
    if format!("{:x}", Sha256::digest(raw))
        != "91f8c0629a6e247b302b2380fa77b3f24c7bea11da516dc9a8da10bd7fff9a94"
    {
        return None;
    }
    match property {
        "shell_main" => Some(vec![3030523609, 1027054943]),
        "shell_sub_left" => Some(vec![1002650538, 1927595057]),
        _ => None,
    }
}

/// Original project artwork. The screen reads the top 448 rows of a 512²
/// sheet, as it does for the console backdrop. No source pixels are stored.
pub(super) fn backdrop() -> Vec<u8> {
    (0..512)
        .flat_map(|y| {
            let lift = (448_u32.saturating_sub(y) / 32) as u8;
            (0..512).flat_map(move |_| [8 + lift, 14 + lift, 24 + lift, 255])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_shell_or_movie_never_gets_a_guessed_mapping() {
        assert!(!known_shell(b"unknown"));
        assert!(movie_order(b"unknown", "shell_main").is_none());
        let art = backdrop();
        assert_eq!(art.len(), 512 * 512 * 4);
        assert!(art.chunks_exact(4).all(|p| p[3] == 255));
    }

    #[test]
    #[ignore = "owned PC shell: OPENBF_PC_SHELL and OPENBF_PC_SHELL_MOVIE"]
    fn retail_fingerprints_reject_mutation() {
        let mut shell = std::fs::read(std::env::var("OPENBF_PC_SHELL").unwrap()).unwrap();
        let mut movie = std::fs::read(std::env::var("OPENBF_PC_SHELL_MOVIE").unwrap()).unwrap();
        assert!(known_shell(&shell));
        assert_eq!(movie_order(&movie, "shell_main").unwrap().len(), 2);
        assert_eq!(movie_order(&movie, "shell_sub_left").unwrap().len(), 2);
        assert!(movie_order(&movie, "unknown").is_none());
        shell[100] ^= 1;
        movie[100] ^= 1;
        assert!(!known_shell(&shell));
        assert!(movie_order(&movie, "shell_main").is_none());
    }
}
