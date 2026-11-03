mod data0 {
    include!("render_offsets_data0.rs");
}
mod data1 {
    include!("render_offsets_data1.rs");
}
mod data2 {
    include!("render_offsets_data2.rs");
}
mod data3 {
    include!("render_offsets_data3.rs");
}
// Donor-relative drawing corrections: no source vertices, UVs or triangle indices.
// Each included list contains only (primitive, vertex, displacement) instructions.
pub(super) static RECIPES: &[Recipe] = &[
    // rhn_bldg_broken_front1 (pc1): {'art_surface': 4, 'art_vertex': 5}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_broken_front1",
        input: "57c20df0cfe5f68f3e9d94022066ed185c42b1b45c876be2b864863f293c41c6",
        shapes: &[(14, 12), (13, 11), (22, 15)],
        moves: data0::MOVES_0,
        removals: &[],
    },
    // rhn_bldg_broken_front1 (xbox): {'art_vertex': 3}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_broken_front1",
        input: "95e2c9dc77bdfd376ee4fc298ef7c341a00b3e8c2923f1b252275abfc5059b66",
        shapes: &[(14, 12), (13, 11), (22, 15)],
        moves: data0::MOVES_1,
        removals: &[],
    },
    // rhn_bldg_look_out (pc1): {'art_edge': 8, 'art_gap': 8, 'art_surface': 12, 'art_vertex': 6, 'collision_edge': 6, 'collision_vertex': 26}; 2 removed faces.
    Recipe {
        name: "rhn_bldg_look_out",
        input: "af34285691be31150cb78bd981ddf5c40d5d7fe46cedde7bda4a941b2c420959",
        shapes: &[(168, 99), (17, 14), (86, 54), (597, 394), (80, 44)],
        moves: data0::MOVES_2,
        removals: &[(0, 61), (3, 228)],
    },
    // rhn_bldg_look_out (xbox): {'art_edge': 6, 'art_gap': 8, 'art_vertex': 6, 'collision_edge': 10, 'collision_surface': 2, 'collision_vertex': 46}; 2 removed faces.
    Recipe {
        name: "rhn_bldg_look_out",
        input: "5e2b71ab1c201f9989aaa1fa900833f4e2ad418c8acfed104b4f748ee3c309e0",
        shapes: &[(168, 99), (17, 14), (86, 54), (597, 394), (80, 44)],
        moves: data0::MOVES_3,
        removals: &[(0, 61), (3, 228)],
    },
    // rhn_bldg_look_out2 (pc1): {'art_vertex': 8, 'collision_edge': 3}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_look_out2",
        input: "42429c6f381823a19f8a607f4007716d4a853b4faec94c07c86a7b06b520d0cc",
        shapes: &[
            (1461, 759),
            (115, 85),
            (135, 71),
            (197, 125),
            (57, 27),
            (102, 64),
            (214, 121),
            (170, 93),
        ],
        moves: data0::MOVES_4,
        removals: &[],
    },
    // rhn_bldg_look_out2 (xbox): {'art_vertex': 8, 'collision_edge': 3, 'collision_surface': 2}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_look_out2",
        input: "c27e8f84d706f3d071820e0525674e2f48390679c28a2d9e9bf3460c27010330",
        shapes: &[
            (1461, 759),
            (115, 85),
            (135, 71),
            (197, 125),
            (57, 27),
            (102, 64),
            (214, 121),
            (170, 93),
        ],
        moves: data0::MOVES_5,
        removals: &[],
    },
    // rhn_bldg_ruin_13 (pc1): {'collision_vertex': 8}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_ruin_13",
        input: "c740a97e8bb17f4ac0964cdc1ae95d40ec5a97875ed6df87bc7d11da694697ed",
        shapes: &[(24, 18), (20, 18)],
        moves: data0::MOVES_6,
        removals: &[],
    },
    // rhn_bldg_ruin_13 (xbox): {'collision_vertex': 20}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_ruin_13",
        input: "311da159a6448a5cb1e86978a56138ed0e205d635fe876503ac5ee1076d23f5b",
        shapes: &[(81, 27), (27, 9)],
        moves: data0::MOVES_7,
        removals: &[],
    },
    // rhn_bldg_ruin_14 (pc1): {'art_gap': 3, 'art_vertex': 5, 'collision_edge': 2, 'collision_vertex': 2}; 11 removed faces.
    Recipe {
        name: "rhn_bldg_ruin_14",
        input: "6ea7ae7749502f0eb6d2c144f537189de7b526886ffc197f819d3bdf8de33820",
        shapes: &[(118, 95), (97, 63)],
        moves: data0::MOVES_8,
        removals: &[
            (0, 20),
            (0, 21),
            (0, 44),
            (0, 45),
            (0, 48),
            (0, 76),
            (0, 79),
            (0, 88),
            (0, 90),
            (0, 93),
            (1, 59),
        ],
    },
    // rhn_bldg_ruin_14 (xbox): {'art_gap': 3, 'collision_edge': 2, 'collision_surface': 3, 'collision_vertex': 2}; 11 removed faces.
    Recipe {
        name: "rhn_bldg_ruin_14",
        input: "1d0e200887f9e8b3a7fd6f622776cde7976bf1527d51d40963daaa219fda6bd3",
        shapes: &[(118, 95), (97, 63)],
        moves: data0::MOVES_9,
        removals: &[
            (0, 20),
            (0, 21),
            (0, 44),
            (0, 45),
            (0, 48),
            (0, 76),
            (0, 79),
            (0, 88),
            (0, 90),
            (0, 93),
            (1, 59),
        ],
    },
    // rhn_bldg_ruin_cathedral_1 (pc1): {}; 1 removed faces.
    Recipe {
        name: "rhn_bldg_ruin_cathedral_1",
        input: "d1d45e627207caad1adda978e4f3441054e2e4a2fd9d0d10ee3f681577ec9371",
        shapes: &[(12, 6), (4, 2), (60, 34), (99, 58)],
        moves: &[],
        removals: &[(3, 42)],
    },
    // rhn_bldg_ruin_cathedral_1 (xbox): {}; 1 removed faces.
    Recipe {
        name: "rhn_bldg_ruin_cathedral_1",
        input: "5d2152f6d634f4ba8facb7f3314440edf7a811a60c5630abfe64d93648602f22",
        shapes: &[(12, 6), (4, 2), (60, 34), (99, 58)],
        moves: &[],
        removals: &[(3, 42)],
    },
    // rhn_bldg_ruin_cathedral_entrence (pc1): {'art_surface': 3, 'art_vertex': 33, 'collision_edge': 3, 'collision_surface': 4, 'collision_vertex': 135}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_ruin_cathedral_entrence",
        input: "ebc96a9175324a30367758dea18ef6e0b45f89e31e6e8cd277017b47ce7b992e",
        shapes: &[(18, 10), (66, 34), (166, 102)],
        moves: data0::MOVES_12,
        removals: &[],
    },
    // rhn_bldg_ruin_cathedral_entrence (xbox): {'art_surface': 3, 'art_vertex': 33, 'collision_edge': 3, 'collision_surface': 4, 'collision_vertex': 138}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_ruin_cathedral_entrence",
        input: "282ec2318b0196bd69c93e36cb85c522e2eafe069a845d044b350b34bb93646c",
        shapes: &[(18, 10), (66, 34), (166, 102)],
        moves: data0::MOVES_13,
        removals: &[],
    },
    // rhn_bldg_ruin_cathedral_l (pc1): {'art_edge': 4, 'art_vertex': 1}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_ruin_cathedral_l",
        input: "dffd42267b7b9c44b16cdfa270e1a37b8fd19130f883d9a486ba30735dbea801",
        shapes: &[(102, 78), (90, 58)],
        moves: data0::MOVES_14,
        removals: &[],
    },
    // rhn_bldg_ruin_pillar (pc1): {'collision_vertex': 37}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_ruin_pillar",
        input: "b19993bf5e898dc4c10a7571d6ed44e83d6ef5392e26ce21ed217cfde8a5856a",
        shapes: &[(45, 28)],
        moves: data0::MOVES_15,
        removals: &[],
    },
    // rhn_bldg_ruin_pillar (xbox): {'collision_vertex': 37}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_ruin_pillar",
        input: "54844e3034650a5bc30d5a987e05ef85526e180b8dcdfd3e63ad9ee5e47bf26d",
        shapes: &[(45, 28)],
        moves: data0::MOVES_16,
        removals: &[],
    },
    // rhn_bldg_ruin_rotunda (pc1): {'art_gap': 2, 'art_surface': 3, 'collision_surface': 15, 'collision_vertex': 4}; 10 removed faces.
    Recipe {
        name: "rhn_bldg_ruin_rotunda",
        input: "0d16e1a48b85a1aadb27361e6f4f4b4bacf0c06d8cb5ed211345e521368ac71e",
        shapes: &[(155, 120), (476, 280)],
        moves: data0::MOVES_17,
        removals: &[
            (1, 30),
            (1, 32),
            (1, 33),
            (1, 52),
            (1, 94),
            (1, 95),
            (1, 118),
            (1, 119),
            (1, 123),
            (1, 164),
        ],
    },
    // rhn_bldg_ruin_rotunda (xbox): {'art_gap': 2, 'art_vertex': 3, 'collision_surface': 15, 'collision_vertex': 4}; 9 removed faces.
    Recipe {
        name: "rhn_bldg_ruin_rotunda",
        input: "8c7fc23b40dfdedfad247d334125edf032a5e3f3acae4ccfa42205b6f698d5cd",
        shapes: &[(155, 120), (476, 280)],
        moves: data0::MOVES_18,
        removals: &[
            (1, 30),
            (1, 32),
            (1, 33),
            (1, 52),
            (1, 94),
            (1, 95),
            (1, 118),
            (1, 123),
            (1, 164),
        ],
    },
    // rhn_bldg_ruin_rotunda_upper (pc1): {'collision_surface': 8}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_ruin_rotunda_upper",
        input: "88173ebf8d8a9efc49b598b80aab9380cf730d564499c3ab8de7238a0bde41af",
        shapes: &[(130, 72), (143, 74), (53, 25), (58, 45), (82, 42)],
        moves: data0::MOVES_19,
        removals: &[],
    },
    // rhn_bldg_ruin_rotunda_upper (xbox): {'collision_surface': 8}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_ruin_rotunda_upper",
        input: "e60b275fbb3a31f59ba9b4c537be04f5cf42d34be0cafffe544644579c225c63",
        shapes: &[(130, 72), (143, 74), (53, 25), (58, 45), (82, 42)],
        moves: data0::MOVES_20,
        removals: &[],
    },
    // rhn_bldg_ruin_stairs2 (pc1): {'art_vertex': 19}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_ruin_stairs2",
        input: "7552fd9ce5cf2ec86dadac7efe1b47c8ce646b3fe4a3adb2d55700405cb86d53",
        shapes: &[(12, 8), (508, 294)],
        moves: data0::MOVES_21,
        removals: &[],
    },
    // rhn_bldg_ruin_wallshort (pc1): {'art_surface': 6, 'art_vertex': 8}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_ruin_wallshort",
        input: "65423f996dec3ddfbd5ee74dd7d05a31de32d3e1c92a9b23affd0a515132ff34",
        shapes: &[(16, 10), (70, 34), (58, 30), (92, 60)],
        moves: data0::MOVES_22,
        removals: &[],
    },
    // rhn_bldg_ruin_wallshort (xbox): {'art_surface': 6, 'art_vertex': 4, 'collision_surface': 14}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_ruin_wallshort",
        input: "5875466481c4b134d8538c10c857dc20c315cc6f4b47937ee0c870204cc417f5",
        shapes: &[(16, 10), (70, 34), (58, 30), (92, 60)],
        moves: data0::MOVES_23,
        removals: &[],
    },
    // rhn_bldg_statue_alcove (pc1): {'collision_edge': 3, 'collision_surface': 3}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_statue_alcove",
        input: "075fe006a9178b4ef249663368fff35b6da235a4eebaaa472c30d91ca59f0308",
        shapes: &[(303, 171), (18, 12), (99, 60), (14, 6)],
        moves: data0::MOVES_24,
        removals: &[],
    },
    // rhn_bldg_statue_alcove (xbox): {'collision_edge': 6, 'collision_surface': 3, 'collision_vertex': 4}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_statue_alcove",
        input: "a7a0e85e86fa6aaea38c1d7fea66110917ccd2ded1289bda705b5b5e2e34f387",
        shapes: &[(303, 171), (18, 12), (99, 60), (14, 6)],
        moves: data0::MOVES_25,
        removals: &[],
    },
    // rhn_bldg_wall_facade_pillar_h2 (pc1): {}; 2 removed faces.
    Recipe {
        name: "rhn_bldg_wall_facade_pillar_h2",
        input: "0b2398bee4737e78e49b41a017a9f1e2fd0efd39b9ab5485f2ca1762bab25ca2",
        shapes: &[(216, 150)],
        moves: &[],
        removals: &[(0, 118), (0, 119)],
    },
    // rhn_bldg_wall_facade_pillar_h2 (xbox): {}; 2 removed faces.
    Recipe {
        name: "rhn_bldg_wall_facade_pillar_h2",
        input: "93405554db92daf72fd764524bd4b06cebb6d33c629ae8076934957953a0a93d",
        shapes: &[(216, 150)],
        moves: &[],
        removals: &[(0, 118), (0, 119)],
    },
    // rhn_bldg_wall_facade_pillar_h3a (pc1): {'collision_vertex': 3}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_wall_facade_pillar_h3a",
        input: "20b90e4af86805fa20db01f0f96f97f87f14f26185581d09c3ae3ad11db15336",
        shapes: &[(108, 68)],
        moves: data0::MOVES_28,
        removals: &[],
    },
    // rhn_bldg_wall_facade_pillar_h3a (xbox): {'collision_vertex': 3}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_wall_facade_pillar_h3a",
        input: "9e38e8e4fdc00d23a69525866a991ebc92519c27aa7a54a40a381b4aba3e6f81",
        shapes: &[(108, 68)],
        moves: data0::MOVES_29,
        removals: &[],
    },
    // rhn_bldg_wall_facade_pillar_l (pc1): {'art_vertex': 12}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_wall_facade_pillar_l",
        input: "97064c61f76a56cd4bbf979ce4cda2b7915b290b245d7295ddfad26ff5a87196",
        shapes: &[(54, 32)],
        moves: data0::MOVES_30,
        removals: &[],
    },
    // rhn_bldg_wall_facade_pillar_l (xbox): {'art_vertex': 12}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_wall_facade_pillar_l",
        input: "1df93eff1e485437778db7663e7a27b1aee45df76317371525857b3d5a2e7f5f",
        shapes: &[(54, 32)],
        moves: data0::MOVES_31,
        removals: &[],
    },
    // rhn_bldg_wall_facade_pillar_l1 (pc1): {'collision_vertex': 3}; 6 removed faces.
    Recipe {
        name: "rhn_bldg_wall_facade_pillar_l1",
        input: "26cf37df3c80635a472b0db776c2f750c9dfe3650d53b2ffacf1b4cdd49599b9",
        shapes: &[(104, 64)],
        moves: data0::MOVES_32,
        removals: &[(0, 22), (0, 23), (0, 40), (0, 41), (0, 42), (0, 43)],
    },
    // rhn_bldg_wall_facade_pillar_l1 (xbox): {'collision_vertex': 3}; 6 removed faces.
    Recipe {
        name: "rhn_bldg_wall_facade_pillar_l1",
        input: "322c51d65d54cc6e77352f6882bb88b2f13a46b9cb1eb20f4dfdfb2d9d9c4e65",
        shapes: &[(104, 64)],
        moves: data0::MOVES_33,
        removals: &[(0, 22), (0, 23), (0, 40), (0, 41), (0, 42), (0, 43)],
    },
    // rhn_bldg_wall_ruin_1a (pc1): {}; 2 removed faces.
    Recipe {
        name: "rhn_bldg_wall_ruin_1a",
        input: "64cbe2222043f74e501e8fb7acd6b182c15169dabaa40fad65fe87c42e94cac8",
        shapes: &[(30, 20), (14, 12)],
        moves: &[],
        removals: &[(0, 0), (0, 1)],
    },
    // rhn_bldg_wall_ruin_1b (pc1): {'art_vertex': 3}; 4 removed faces.
    Recipe {
        name: "rhn_bldg_wall_ruin_1b",
        input: "88949db67f1d203471bd7d2a395d19a558fff8b8f4bf147a0d6b5bbcdf8e61e2",
        shapes: &[(32, 18), (22, 14)],
        moves: data0::MOVES_35,
        removals: &[(0, 0), (0, 1), (0, 13), (0, 14)],
    },
    // rhn_prop_statue (pc1): {'art_edge': 26, 'art_surface': 34, 'art_vertex': 340, 'collision_edge': 19, 'collision_surface': 49, 'collision_vertex': 9}; 0 removed faces.
    Recipe {
        name: "rhn_prop_statue",
        input: "3054d7e741fa1b229c946f97777fe7a7bb774bf100af34ee123d8dbf0cdad39e",
        shapes: &[(1065, 687)],
        moves: data1::MOVES_36,
        removals: &[],
    },
    // rhn_bldg_ruin_cathedral_l (xbox): {}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_ruin_cathedral_l",
        input: "95bb8ca2ca22564e075e08723324561280c21ccdda558b1cfe5186f77dfa0431",
        shapes: &[(90, 58), (56, 32), (72, 46)],
        moves: &[],
        removals: &[],
    },
    // rhn_bldg_ruin_stairs2 (xbox): {}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_ruin_stairs2",
        input: "82e163179f0903c4ece85db97dd42a067fa913d86294e3a558630408e42eeb34",
        shapes: &[(12, 8), (510, 296)],
        moves: &[],
        removals: &[],
    },
    // rhn_bldg_wall_ruin_1a (xbox): {}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_wall_ruin_1a",
        input: "eea706a8405aa4227588619c8211dd3257ec39972b3cd2b14af10d2c25a2a66c",
        shapes: &[(26, 18), (14, 12)],
        moves: &[],
        removals: &[],
    },
    // rhn_bldg_wall_ruin_1b (xbox): {}; 0 removed faces.
    Recipe {
        name: "rhn_bldg_wall_ruin_1b",
        input: "f6b2bef8ea777918233e5612c261736090c98c9559b1d4d7387d1458fe57b6c6",
        shapes: &[(24, 14), (22, 14)],
        moves: &[],
        removals: &[],
    },
    // rhn_prop_statue (xbox): {}; 0 removed faces.
    Recipe {
        name: "rhn_prop_statue",
        input: "263aaae4447e27dde742e4e46db48c2d2ff205c5dc6c79827279ec83852b3dc6",
        shapes: &[(24, 22), (25, 22), (987, 687)],
        moves: &[],
        removals: &[],
    },
    // fel1_bldg_bladder (xbox): {'art_registration': 811}; 0 removed faces.
    Recipe {
        name: "fel1_bldg_bladder",
        input: "1d4d9d3cb85effd625504669660bbc322e58c31c342418069e0707479fae0a5b",
        shapes: &[(92, 72), (234, 308), (485, 608)],
        moves: data2::MOVES_42,
        removals: &[],
    },
    // fel1_bldg_bladder (pc2): {'art_registration': 811}; 0 removed faces.
    Recipe {
        name: "fel1_bldg_bladder",
        input: "b3f9722b8924125fcf1d822e234bce56fa10a0d947866cfe560c9ffd0c656226",
        shapes: &[(92, 72), (234, 308), (485, 608)],
        moves: data3::MOVES_43,
        removals: &[],
    },
];
