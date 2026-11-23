//! Presentation choices for the fingerprinted retail PC BF2 shell.
//!
//! The console-shaped screens keep their existing geometry and widgets. PC
//! has no Instant Action sheet (no `iface_instant_action` in any PC BF1 or
//! BF2 file), so we draw an original flat panel in the PS2 sheet's grey.
//! Its four Bink segments are galaxy views (all inspected at 5 seconds), not
//! the console ship loops. Our presentation sequence uses 0/1 for the main
//! menu and 2/3 for submenus; this is an explicit appearance difference.
use sha2::{Digest, Sha256};

pub(super) fn known_shell(raw: &[u8]) -> bool {
    matches!(
        format!("{:x}", Sha256::digest(raw)).as_str(),
        // Steam retail PC BF2 SHELL.LVL
        "82335ab55e17689cd620ba174568e6a5481d4523b821549aa153a6b88a831009"
        // GOG.com v1.1 retail PC BF2 SHELL.LVL
        | "7cde6a5fcb121653d4f647ce7a2b158305e88bad01bf55d14cfabcd4d0b3d00b"
    )
}

pub(super) fn movie_order(raw: &[u8], property: &str) -> Option<Vec<u32>> {
    if !matches!(
        format!("{:x}", Sha256::digest(raw)).as_str(),
        // Steam retail PC BF2 MOVIES/SHELL.MVS
        "91f8c0629a6e247b302b2380fa77b3f24c7bea11da516dc9a8da10bd7fff9a94"
        // GOG.com v1.1 retail PC BF2 MOVIES/SHELL.MVS (identical 4 Bink segment IDs)
        | "74bc379cc5b4f8efb2e7c2cdedd8ece9d005716a5116268dbddad95225f2da76"
    ) {
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
/// `[ours]` A flat panel in the PS2 sheet's tone: black outside its body rows
/// [60,386) (the landmarks `super::register_xbox_backdrop` cites) and the
/// body's mean colour inside. measure: `bake tex-dump` of the PS2
/// `SHELL.LVL` `iface_instant_action`, rows 60..386 all columns, mean RGB
/// (63.7, 64.9, 64.1); the bands above and below average (0, 1, 1).
pub(super) fn backdrop() -> Vec<u8> {
    (0..512)
        .flat_map(|y| {
            let px = if (BODY_ROWS[0]..BODY_ROWS[1]).contains(&y) {
                BODY_RGB
            } else {
                [0, 0, 0]
            };
            (0..512).flat_map(move |_| [px[0], px[1], px[2], 255])
        })
        .collect()
}

const BODY_ROWS: [u32; 2] = [60, 386];
const BODY_RGB: [u8; 3] = [64, 65, 64];

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
        let row = |y: usize| &art[y * 512 * 4..y * 512 * 4 + 3];
        assert_eq!(row(59), [0, 0, 0]);
        assert_eq!(row(60), BODY_RGB);
        assert_eq!(row(385), BODY_RGB);
        assert_eq!(row(386), [0, 0, 0]);
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
