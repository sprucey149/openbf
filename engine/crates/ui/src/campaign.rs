//! Star Wars Battlefront II "Rise of the Empire" singleplayer campaign catalog.

/// Static metadata for one of the 18 Rise of the Empire campaign missions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CampaignMissionEntry {
    pub index: usize,
    /// Generated RON script stem under `missions/campaign/bf2_<script>.ron`.
    pub script: &'static str,
    /// Baked map id (`bf2_<map>`).
    pub map: &'static str,
    /// Era code (`"cw"` or `"gcw"`).
    pub era: &'static str,
    /// Localization key for the planet / campaign turn title.
    pub planet_key: &'static str,
    /// Localization key for the mission briefing summary.
    pub desc_key: &'static str,
    /// Fallback English mission row label.
    pub title: &'static str,
    /// Fallback English briefing description.
    pub desc_fallback: &'static str,
    /// Planet / camera stem inside `GAL1.LVL` (`<planet>` + `<planet>_camera`).
    pub planet: &'static str,
    /// Planet fly-in movie from `SHELL.MVS` (`*tran1`) played on `Attack`.
    pub transition_movie: &'static str,
    /// Optional opening crawl movie (`crawl1` / `crawl2`) before mission briefing.
    pub crawl_movie: Option<&'static str>,
    /// Pre-battle narrator briefing movie from `INGAME.MVS` (`*mon01`).
    pub intro_movie: &'static str,
    /// Post-victory narrator outro movie from `INGAME.MVS` (`*mon02`).
    pub outro_movie: &'static str,
}

/// All 18 Rise of the Empire missions in canonical story order (slot 0 = Training, slots 1..=17 = Story turns).
pub const RISE_OF_THE_EMPIRE: [CampaignMissionEntry; 18] = [
    CampaignMissionEntry {
        index: 0,
        script: "geo1c_c",
        map: "bf2_geo1",
        era: "cw",
        planet_key: "ifs.sp.training",
        desc_key: "ifs.sp.training",
        title: "Geonosis: Attack of the Clones",
        desc_fallback: "Complete basic training on the sands of Geonosis.",
        planet: "geo",
        transition_movie: "geo1tran1",
        crawl_movie: None,
        intro_movie: "geomon01",
        outro_movie: "geomon02",
    },
    CampaignMissionEntry {
        index: 1,
        script: "myg1c_c",
        map: "bf2_myg1",
        era: "cw",
        planet_key: "ifs.campaignname.1",
        desc_key: "ifs.campaigndesc.1",
        title: "Mygeeto: Amongst the Ruins",
        desc_fallback: "A powerful energy source must be acquired from the CIS manufacturing plant.",
        planet: "myg",
        transition_movie: "myg1tran1",
        crawl_movie: Some("crawl1"),
        intro_movie: "mygmon01",
        outro_movie: "mygmon02",
    },
    CampaignMissionEntry {
        index: 2,
        script: "spa2c_c",
        map: "bf2_spa2",
        era: "cw",
        planet_key: "ifs.campaignname.2",
        desc_key: "ifs.campaigndesc.2",
        title: "Coruscant: A Desperate Rescue",
        desc_fallback: "Palpatine's kidnapping gives the 501st a hands-on crash course in space combat.",
        planet: "cor",
        transition_movie: "sb2tran1",
        crawl_movie: None,
        intro_movie: "sb2mon01",
        outro_movie: "sb2mon02",
    },
    CampaignMissionEntry {
        index: 3,
        script: "fel1c_c",
        map: "bf2_fel1",
        era: "cw",
        planet_key: "ifs.campaignname.3",
        desc_key: "ifs.campaigndesc.3",
        title: "Felucia: Heart of Darkness",
        desc_fallback: "The droid presence here threatens the Republic cause. They must be destroyed.",
        planet: "fel",
        transition_movie: "fel1tran1",
        crawl_movie: None,
        intro_movie: "felmon01",
        outro_movie: "felmon02",
    },
    CampaignMissionEntry {
        index: 4,
        script: "spa3c_c",
        map: "bf2_spa3",
        era: "cw",
        planet_key: "ifs.campaignname.4",
        desc_key: "ifs.campaigndesc.4",
        title: "Kashyyyk: First Line of Defense",
        desc_fallback: "The CIS blockade on the Wookiee homeworld must be broken by any means necessary.",
        planet: "kas",
        transition_movie: "sb3tran1",
        crawl_movie: None,
        intro_movie: "sb3mon01",
        outro_movie: "sb3mon02",
    },
    CampaignMissionEntry {
        index: 5,
        script: "kas2c_c",
        map: "bf2_kas2",
        era: "cw",
        planet_key: "ifs.campaignname.5",
        desc_key: "ifs.campaigndesc.5",
        title: "Kashyyyk: A Line in the Sand",
        desc_fallback: "A fortified Wookiee refinery is the last defense against the planetary droid invasion.",
        planet: "kas",
        transition_movie: "kas2tran1",
        crawl_movie: None,
        intro_movie: "kasmon01",
        outro_movie: "kasmon02",
    },
    CampaignMissionEntry {
        index: 6,
        script: "uta1c_c",
        map: "bf2_uta1",
        era: "cw",
        planet_key: "ifs.campaignname.6",
        desc_key: "ifs.campaigndesc.6",
        title: "Utapau: Underground Ambush",
        desc_fallback: "Ambushing General Grievous and his army will be the final crushing blow to the CIS.",
        planet: "uta",
        transition_movie: "uta1tran1",
        crawl_movie: None,
        intro_movie: "utamon01",
        outro_movie: "utamon02",
    },
    CampaignMissionEntry {
        index: 7,
        script: "cor1c_c",
        map: "bf2_cor1",
        era: "cw",
        planet_key: "ifs.campaignname.7",
        desc_key: "ifs.campaigndesc.7",
        title: "Coruscant: Knightfall",
        desc_fallback: "The corrupt Jedi order must be wiped out, by Order 66 of the new Galactic Emperor.",
        planet: "cor",
        transition_movie: "cor1tran1",
        crawl_movie: None,
        intro_movie: "cormon01",
        outro_movie: "cormon02",
    },
    CampaignMissionEntry {
        index: 8,
        script: "nab2g_c",
        map: "bf2_nab2",
        era: "gcw",
        planet_key: "ifs.campaignname.8",
        desc_key: "ifs.campaigndesc.8",
        title: "Naboo: Imperial Diplomacy",
        desc_fallback: "The Queen of Naboo threatens the peace of the New Order. She must be silenced.",
        planet: "nab",
        transition_movie: "nab1tran1",
        crawl_movie: Some("crawl2"),
        intro_movie: "nabmon01",
        outro_movie: "nabmon02",
    },
    CampaignMissionEntry {
        index: 9,
        script: "spa4g_c",
        map: "bf2_spa4",
        era: "gcw",
        planet_key: "ifs.campaignname.9",
        desc_key: "ifs.campaigndesc.9",
        title: "Mustafar: Preventative Measures",
        desc_fallback: "The Seperatist threat is stirring again and lingers in orbit over Mustafar.",
        planet: "mus",
        transition_movie: "sb4tran1",
        crawl_movie: None,
        intro_movie: "sb4mon01",
        outro_movie: "sb4mon02",
    },
    CampaignMissionEntry {
        index: 10,
        script: "mus1c_c",
        map: "bf2_mus1",
        era: "cw",
        planet_key: "ifs.campaignname.10",
        desc_key: "ifs.campaigndesc.10",
        title: "Mustafar: Tying up Loose Ends",
        desc_fallback: "The Geonosian insurrectionist must be destroyed, along with his droids and facilities.",
        planet: "mus",
        transition_movie: "mus1tran1",
        crawl_movie: None,
        intro_movie: "musmon01",
        outro_movie: "musmon02",
    },
    CampaignMissionEntry {
        index: 11,
        script: "kam1c_c",
        map: "bf2_kam1",
        era: "cw",
        planet_key: "ifs.campaignname.11",
        desc_key: "ifs.campaigndesc.11",
        title: "Kamino: Changing of the Guard",
        desc_fallback: "The Kaminoan facility must be seized. They resist us with our own brethren.",
        planet: "kam",
        transition_movie: "kam1tran1",
        crawl_movie: None,
        intro_movie: "kammon01",
        outro_movie: "kammon02",
    },
    CampaignMissionEntry {
        index: 12,
        script: "dea1g_c",
        map: "bf2_dea1",
        era: "gcw",
        planet_key: "ifs.campaignname.12",
        desc_key: "ifs.campaigndesc.12",
        title: "Death Star: Prison Break",
        desc_fallback: "Military detainees aboard our secret battle station have broken free. They must not escape.",
        planet: "dea",
        transition_movie: "dea1tran1",
        crawl_movie: None,
        intro_movie: "deamon01",
        outro_movie: "deamon02",
    },
    CampaignMissionEntry {
        index: 13,
        script: "pol1g_c",
        map: "bf2_pol1",
        era: "gcw",
        planet_key: "ifs.campaignname.13",
        desc_key: "ifs.campaigndesc.13",
        title: "Polis Massa: Birth of the Rebellion",
        desc_fallback: "The Death Star plans have been traced to a secret Rebel facility hidden in an asteroid field.",
        planet: "pol",
        transition_movie: "pol1tran1",
        crawl_movie: None,
        intro_movie: "polmon01",
        outro_movie: "polmon02",
    },
    CampaignMissionEntry {
        index: 14,
        script: "tan1g_c",
        map: "bf2_tan1",
        era: "gcw",
        planet_key: "ifs.campaignname.14",
        desc_key: "ifs.campaigndesc.14",
        title: "Tantive IV: Recovering the Plans",
        desc_fallback: "Lord Vader has sensed the plans are aboard a consular ship making its way to Tatooine.",
        planet: "tat",
        transition_movie: "tan1tran1",
        crawl_movie: None,
        intro_movie: "tanmon01",
        outro_movie: "tanmon02",
    },
    CampaignMissionEntry {
        index: 15,
        script: "spa1g_c",
        map: "bf2_spa1",
        era: "gcw",
        planet_key: "ifs.campaignname.15",
        desc_key: "ifs.campaigndesc.15",
        title: "Yavin 4: Vader's Fist Strikes Back",
        desc_fallback: "The Rebels must not escape our retribution. Strike them out of the skies.",
        planet: "yav",
        transition_movie: "sb1tran1",
        crawl_movie: None,
        intro_movie: "sb1mon01",
        outro_movie: "sb1mon02",
    },
    CampaignMissionEntry {
        index: 16,
        script: "yav1g_c",
        map: "bf2_yav1",
        era: "gcw",
        planet_key: "ifs.campaignname.16",
        desc_key: "ifs.campaigndesc.16",
        title: "Yavin 4: Revenge of the Empire",
        desc_fallback: "Their base is ripe for the taking. The Death Star's destruction will be repaid in blood.",
        planet: "yav",
        transition_movie: "yav1tran1",
        crawl_movie: None,
        intro_movie: "yavmon01",
        outro_movie: "yavmon02",
    },
    CampaignMissionEntry {
        index: 17,
        script: "hot1g_c",
        map: "bf2_hot1",
        era: "gcw",
        planet_key: "ifs.campaignname.17",
        desc_key: "ifs.campaigndesc.17",
        title: "Hoth: Our Finest Hour",
        desc_fallback: "The Rebel outpost has been discovered and will be destroyed. Galactic peace is at hand.",
        planet: "hot",
        transition_movie: "hot1tran1",
        crawl_movie: None,
        intro_movie: "hotmon01",
        outro_movie: "hotmon02",
    },
];

/// Look up a Rise of the Empire campaign entry by its mission script stem.
pub fn find_by_script(script: &str) -> Option<&'static CampaignMissionEntry> {
    RISE_OF_THE_EMPIRE.iter().find(|e| e.script == script)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn rise_of_the_empire_catalog_has_18_unique_missions() {
        let mut scripts = HashSet::new();
        let mut transitions = HashSet::new();
        let mut intros = HashSet::new();
        let mut outros = HashSet::new();
        for (i, entry) in RISE_OF_THE_EMPIRE.iter().enumerate() {
            assert_eq!(entry.index, i);
            assert!(
                scripts.insert(entry.script),
                "duplicate script {}",
                entry.script
            );
            assert!(
                transitions.insert(entry.transition_movie),
                "duplicate transition {}",
                entry.transition_movie
            );
            assert!(
                intros.insert(entry.intro_movie),
                "duplicate intro {}",
                entry.intro_movie
            );
            assert!(
                outros.insert(entry.outro_movie),
                "duplicate outro {}",
                entry.outro_movie
            );
            assert!(entry.map.starts_with("bf2_"));
            assert!(entry.era == "cw" || entry.era == "gcw");
            assert_eq!(find_by_script(entry.script), Some(entry));
        }
        assert_eq!(RISE_OF_THE_EMPIRE[0].crawl_movie, None);
        assert_eq!(RISE_OF_THE_EMPIRE[1].crawl_movie, Some("crawl1"));
        assert_eq!(RISE_OF_THE_EMPIRE[8].crawl_movie, Some("crawl2"));
        assert_eq!(RISE_OF_THE_EMPIRE[9].crawl_movie, None);
    }
}
