mod data0 {
    include!("vehicle_offsets_data0.rs");
}
mod data1 {
    include!("vehicle_offsets_data1.rs");
}
mod data2 {
    include!("vehicle_offsets_data2.rs");
}
mod data3 {
    include!("vehicle_offsets_data3.rs");
}
// Fingerprints of the player's donor files and geometry instructions only.
// Hailfire rack sides follow the PS2 silhouette, with donor tube cap vertices
// outside it projected onto PS2 art. No collision, joint, UV or material changes.
pub(super) static RECIPES: &[Recipe] = &[
    // xbox-bf1: 951 donor-relative vertex moves.
    Recipe {
        name: "cis_tread_hailfire",
        input: "fd6be09b1b9cf300136ef553e4812f2a3e83a97829072c45ea958922e88b2956",
        shapes: &[
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (16, 8),
            (58, 44),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (30, 6),
            (16, 8),
            (58, 44),
            (650, 466),
            (268, 180),
            (234, 250),
            (268, 180),
            (234, 250),
            (756, 252),
            (756, 252),
        ],
        moves: data0::MOVES_0,
        removals: &[],
    },
    // pc-bf1: 948 donor-relative vertex moves.
    Recipe {
        name: "cis_tread_hailfire",
        input: "4bbf5501608e347bf8e9b11629f1e99d430e77df1fdbe7369071fcaa2a2f5fea",
        shapes: &[
            (234, 250),
            (234, 250),
            (1597, 952),
            (210, 42),
            (136, 32),
            (480, 96),
            (756, 252),
            (756, 252),
        ],
        moves: data1::MOVES_1,
        removals: &[],
    },
    // xbox-bf2: 945 donor-relative vertex moves.
    Recipe {
        name: "cis_tread_hailfire",
        input: "e2ac2642593e5f14fe20a63c674157e0cf41f09133b9bd9a30911cc8dbb8bcd3",
        shapes: &[
            (234, 250),
            (234, 250),
            (1484, 928),
            (946, 194),
            (756, 252),
            (756, 252),
        ],
        moves: data2::MOVES_2,
        removals: &[],
    },
    // pc-bf2: 948 donor-relative vertex moves.
    Recipe {
        name: "cis_tread_hailfire",
        input: "0cb063cdeece779228cbe2e3dc6b245f0fa7ce34c05db0c40a31a59a5ba18229",
        shapes: &[
            (234, 250),
            (234, 250),
            (1604, 952),
            (210, 42),
            (166, 38),
            (450, 90),
            (756, 252),
            (756, 252),
        ],
        moves: data3::MOVES_3,
        removals: &[],
    },
];
