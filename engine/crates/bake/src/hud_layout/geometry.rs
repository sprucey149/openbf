//! Numeric geometry corrections only, measured against stock PS2 1playerhud.
//! Entries distinguish parent-local MeshInfo, Segment and Format records.
use super::{property, HudArg, HudNode};

#[derive(Clone, Copy)]
pub(super) enum Geometry {
    Position([f32; 3], &'static str),
    Rotation([f32; 3]),
    Scale([f32; 3]),
    BitmapRect([f32; 2], [&'static str; 3]),
    Rect([f32; 2], [&'static str; 3]),
    TextAlignment([&'static str; 2]),
    AngleStart(f32),
    AngleEnd(f32),
}

impl Geometry {
    pub(super) fn apply(self, node: &mut HudNode) {
        let nums = |v: &[f32]| v.iter().copied().map(HudArg::Num).collect::<Vec<_>>();
        let strings = |v: &[&str]| {
            v.iter()
                .map(|v| HudArg::Str((*v).into()))
                .collect::<Vec<_>>()
        };
        let (key, args) = match self {
            Self::Position(v, units) => {
                let mut a = nums(&v);
                a.push(HudArg::Str(units.into()));
                ("Position", a)
            }
            Self::Rotation(v) => ("rotation", nums(&v)),
            Self::Scale(v) => ("Scale", nums(&v)),
            Self::BitmapRect(v, align) | Self::Rect(v, align) => {
                let mut a = nums(&v);
                a.extend(strings(&align));
                (
                    if matches!(self, Self::Rect(..)) {
                        "Rect"
                    } else {
                        "bitmaprect"
                    },
                    a,
                )
            }
            Self::TextAlignment(v) => ("textalignment", strings(&v)),
            Self::AngleStart(v) => ("anglestart", nums(&[v])),
            Self::AngleEnd(v) => ("angleend", nums(&[v])),
        };
        property(node, key, args);
    }
}

use Geometry::*;
pub(super) const CORRECTIONS: &[(&str, &str, Geometry)] = &[
    (
        "",
        "DefeatTimerGroup",
        Position([-0.006754, 0.269734, 0.0], "Viewport"),
    ),
    (
        "",
        "defeattext",
        Position([0.664217, 0.088427, 0.0], "Viewport"),
    ),
    (
        "",
        "defeattimer",
        Position([0.81051, 0.069316, 0.0], "Viewport"),
    ),
    (
        "",
        "defeattimer_DROPSHADOW",
        Position([0.81307, 0.071945, 0.0], "Viewport"),
    ),
    (
        "",
        "VictoryTimerGroup",
        Position([-0.006754, 0.269734, 0.0], "Viewport"),
    ),
    (
        "",
        "victorytext",
        Position([0.845387, 0.040171, 0.0], "Viewport"),
    ),
    (
        "",
        "victorytimer",
        Position([0.81051, 0.069316, 0.0], "Viewport"),
    ),
    (
        "",
        "victorytimer_DROPSHADOW",
        Position([0.81307, 0.071945, 0.0], "Viewport"),
    ),
    (
        "",
        "objectivetimerGROUP",
        Position([0.792697, 0.30657, 0.0], "Viewport"),
    ),
    ("", "objectivetimer", Position([0.0, 0.0, 0.0], "Pixels")),
    (
        "",
        "player1.heroSelect",
        Position([0.500542, 0.784305, 0.0], "Viewport"),
    ),
    (
        "",
        "player1.heroSelectTimer",
        Position([-0.025154, -0.067248, 0.0], "Viewport"),
    ),
    (
        "",
        "player1.heroSelectTimer",
        Scale([1.663211, 1.663211, 1.663211]),
    ),
    (
        "",
        "player1.heroSelectTimer",
        TextAlignment(["Left", "Top"]),
    ),
    (
        "",
        "ObjectivePopupText.Backdrop",
        BitmapRect([2.0, 1.0], ["Center", "Top", "Viewport"]),
    ),
    (
        "",
        "ObjectivePopupText.Backdrop",
        Position([0.5, 0.0, 0.0], "Viewport"),
    ),
    (
        "",
        "player1ObjectivePopupText.myText",
        Position([0.134071, 0.2617, 0.0], "Viewport"),
    ),
    (
        "",
        "player1ObjectivePopupText.press_to_continue",
        Position([0.524381, 0.841001, 0.0], "Viewport"),
    ),
    (
        "",
        "player1ObjectivesUpdatedReminder",
        Position([0.207399, 0.812908, 0.0], "Viewport"),
    ),
    (
        "",
        "player1PressSelectToReturn",
        Position([0.21548, 0.813873, 0.0], "Viewport"),
    ),
    (
        "",
        "player1PressSelectToReturn",
        TextAlignment(["Right", "Top"]),
    ),
    (
        "",
        "player1ShowHints",
        Position([0.077115, 0.757017, 0.0], "Viewport"),
    ),
    ("", "player1ShowHints", TextAlignment(["Right", "Top"])),
    (
        "",
        "player1ShowObjectiveDetails",
        Position([0.086988, 0.701683, 0.0], "Viewport"),
    ),
    (
        "",
        "player1ShowObjectiveDetails",
        TextAlignment(["Right", "Top"]),
    ),
    (
        "",
        "player1energybar",
        Position([0.064532, 0.854368, 0.0], "Viewport"),
    ),
    (
        "",
        "player1energy_fill",
        Position([3.1e-05, 2.6e-05, 0.0], "Viewport"),
    ),
    (
        "",
        "player1flags",
        Position([0.865404, 0.295744, 0.0], "Viewport"),
    ),
    (
        "",
        "player1healthbar",
        Position([0.064863, 0.827526, 0.0], "Viewport"),
    ),
    (
        "",
        "player1health_fill",
        Position([0.0, 0.0, 0.0], "Pixels"),
    ),
    (
        "",
        "player1herobar",
        Position([0.062993, 0.826381, 0.0], "Viewport"),
    ),
    (
        "",
        "player1herobar_COLOR",
        Position([0.147668, -0.009484, 0.0], "Viewport"),
    ),
    (
        "",
        "player1herobar_INNERWHITE",
        Position([0.145533, 0.009719, 0.0], "Viewport"),
    ),
    (
        "",
        "player1jetpackbar",
        Position([0.063165, 0.47552, 0.0], "Viewport"),
    ),
    (
        "",
        "player1jetpack_fill",
        Position([-0.65708, 0.094735, 0.0], "Viewport"),
    ),
    (
        "",
        "player1jetpack_interiorfield",
        Position([0.0, -0.000237, 0.0], "Viewport"),
    ),
    (
        "",
        "player1jetpack_threshold",
        Position([-0.65708, 0.095022, 0.0], "Viewport"),
    ),
    (
        "",
        "player1lockoninfo",
        Position([0.771629, 0.741757, 0.0], "Viewport"),
    ),
    (
        "",
        "player1lockon.className",
        Position([0.006355, 0.004276, 0.0], "Viewport"),
    ),
    (
        "",
        "player1lockon.directionIconGroup",
        Position([-0.03162, 0.109253, 0.0], "Viewport"),
    ),
    (
        "",
        "player1lockon.distance",
        Position([-0.015466, 0.089712, 0.0], "Viewport"),
    ),
    ("", "player1lockon.distance", TextAlignment(["Left", "Top"])),
    (
        "",
        "player1map",
        Position([0.856385, 0.186248, 0.004111], "Viewport"),
    ),
    ("", "player1map", Scale([0.9, 0.9, 0.9])),
    (
        "",
        "player1mapkey",
        Position([0.142071, 0.682009, 0.0], "Viewport"),
    ),
    (
        "",
        "player1missilelock",
        Position([0.5, 0.819279, 0.0], "Viewport"),
    ),
    (
        "",
        "player1missilelockdistance",
        Position([0.5, 0.84756, 0.0], "Viewport"),
    ),
    (
        "",
        "player1reticule",
        Position([0.5, 0.410624, 0.0], "Viewport"),
    ),
    ("", "player1reticule", Scale([1.0, 1.0, 1.0])),
    ("", "player1reticule_bullseye", Scale([0.26, 0.26, 0.26])),
    ("", "player1weapon1clipbar", AngleEnd(144.79582)),
    ("", "player1weapon1clipbar", AngleStart(-135.29686)),
    ("", "player1weapon1heatbar", AngleEnd(149.99976)),
    ("", "player1weapon1heatbar", AngleStart(-149.99976)),
    ("", "player1weapon1refire_leftbit", AngleEnd(-139.99991)),
    ("", "player1weapon1refire_rightbit", AngleEnd(-139.99991)),
    ("", "player1weapon2refirebar", AngleEnd(240.09558)),
    ("", "player1weapon2refirebar", AngleStart(-119.999954)),
    (
        "",
        "player1spaceassaultgroup",
        Position([0.737986, 0.071701, 0.0], "Viewport"),
    ),
    (
        "",
        "player1stats",
        Position([0.098092, 1.155109, 0.0], "Viewport"),
    ),
    (
        "",
        "player1pointsdeltagroup",
        Position([0.107202, -0.149014, 0.0], "Viewport"),
    ),
    (
        "",
        "player1pointsgroup",
        Position([0.0, -0.02552, 0.0], "Viewport"),
    ),
    (
        "",
        "player1statspoints",
        Position([0.049872, -0.121355, 0.0], "Viewport"),
    ),
    (
        "",
        "player1statskillsdeltagroup",
        Position([0.106831, -0.201207, 0.0], "Viewport"),
    ),
    (
        "",
        "player1statskillsDelta",
        Position([1.7e-05, 1.3e-05, 0.0], "Viewport"),
    ),
    (
        "",
        "player1statskillsgroup",
        Position([0.0, 0.000789, 0.0], "Viewport"),
    ),
    (
        "",
        "player1statskills",
        Position([0.048524, -0.201406, 0.0], "Viewport"),
    ),
    (
        "",
        "player1statspoints.icon",
        Position([4.4e-05, -0.154318, 0.0], "Viewport"),
    ),
    (
        "",
        "player1systemMessages",
        Position([0.079938, 0.191374, 0.0], "Viewport"),
    ),
    (
        "",
        "player1systemMessages",
        Rect([0.5, 0.25], ["Left", "Top", "Viewport"]),
    ),
    (
        "",
        "player1targethealthbar",
        Position([0.5, 0.059272, 0.0], "Viewport"),
    ),
    (
        "",
        "player1targethealth_fill",
        Position([0.0, 0.227078, 0.0], "Viewport"),
    ),
    (
        "",
        "player1targetnamegroup",
        Position([0.5, 0.012799, 0.0], "Viewport"),
    ),
    (
        "",
        "player1team1_group",
        Position([-0.174945, 0.027542, 0.0], "Viewport"),
    ),
    (
        "",
        "player1teams_team1icongroup",
        Position([0.0, 0.0, 0.0], "Pixels"),
    ),
    (
        "",
        "player1teams_team1icon",
        Rotation([0.004578, 180.01282, 0.0]),
    ),
    ("", "player1teams_team1icon", Scale([0.75, 0.75, 0.75])),
    (
        "",
        "player1teams_team1pointsgroup",
        Position([0.004896, 0.030506, 0.0], "Viewport"),
    ),
    (
        "",
        "player1teams_team1reinforcementsgroup",
        Position([-0.051935, -0.019856, 0.0], "Viewport"),
    ),
    (
        "",
        "player1team2_group",
        Position([0.173132, 0.030501, 0.0], "Viewport"),
    ),
    (
        "",
        "player1teams_team2icongroup",
        Position([0.0, 0.0, 0.0], "Pixels"),
    ),
    ("", "player1teams_team2icon", Scale([0.75, 0.75, 0.75])),
    (
        "",
        "player1teams_team2pointsgroup",
        Position([0.003999, 0.030949, 0.0], "Viewport"),
    ),
    (
        "",
        "player1teams_team2points",
        Position([5.1e-05, 0.0, 0.0], "Viewport"),
    ),
    (
        "",
        "player1teams_team2reinforcementsgroup",
        Position([0.052427, -0.020526, 0.0], "Viewport"),
    ),
    (
        "",
        "player1tooltipsGroup",
        Position([0.0, 0.868316, 0.0], "Viewport"),
    ),
    (
        "",
        "player1tooltipsGroup",
        Rect([0.0, 0.0], ["Left", "Top", "Pixels"]),
    ),
    (
        "",
        "player1tooltipsTEXT",
        Position([0.075, 0.002578, 0.0], "Viewport"),
    ),
    (
        "",
        "player1vehiclehealthbar",
        Position([0.064694, 0.827526, 0.0], "Viewport"),
    ),
    (
        "",
        "player1vehiclehealth_empty",
        BitmapRect([0.211292, 0.029295], ["Left", "Top", "Viewport"]),
    ),
    (
        "",
        "player1vehiclehealth_fill",
        BitmapRect([0.201996, 0.014658], ["Left", "Top", "Viewport"]),
    ),
    (
        "",
        "player1vehicleseatinggroup",
        Position([0.129043, 0.596705, 0.0], "Viewport"),
    ),
    (
        "",
        "player1weapon1lockon",
        Position([0.483591, 0.319782, 0.0], "Viewport"),
    ),
    (
        "",
        "player1weaponinformation",
        Position([0.068639, 0.427927, 0.0], "Viewport"),
    ),
    (
        "",
        "player1weapon1ammo",
        Position([0.075284, 0.340939, 0.0], "Viewport"),
    ),
    (
        "",
        "player1weapon1ammoInfinite",
        Position([0.084926, 0.36047, 0.0], "Viewport"),
    ),
    (
        "",
        "player1weapon1backdrop",
        Position([0.207763, 0.356201, -0.002803], "Viewport"),
    ),
    (
        "",
        "player1weapon1group",
        Position([0.403714, 0.005189, 0.484404], "Viewport"),
    ),
    (
        "",
        "player1weapon1mesh",
        Position([-0.128227, 0.353206, 0.007854], "Viewport"),
    ),
    (
        "",
        "player1weapon1mesh",
        Rotation([359.7993, 88.687515, 0.0]),
    ),
    (
        "",
        "player1weapon1mesh",
        Scale([0.350001, 0.350001, 0.350001]),
    ),
    (
        "player1weapon1mesh",
        "all_weap_inf_bowcaster",
        Position([-0.197062, 0.357451, -0.00189], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "all_weap_inf_bowcaster",
        Scale([0.83, 0.83, 0.83]),
    ),
    (
        "player1weapon1mesh",
        "all_weap_inf_launcher",
        Position([-0.204363, 0.367216, 0.000847], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "all_weap_inf_pistol",
        Position([-0.258881, 0.361796, 0.013887], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "all_weap_inf_rifle",
        Position([-0.190385, 0.361293, 0.01459], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "all_weap_inf_seeker",
        Position([-0.215594, 0.3595, 0.00024], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "all_weap_inf_seeker",
        Rotation([358.81155, 99.14022, 1.132509]),
    ),
    (
        "player1weapon1mesh",
        "all_weap_inf_sniperrifle",
        Position([-0.214412, 0.355448, 0.003653], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "cis_weap_inf_fusioncutter",
        Position([-0.234983, 0.370699, -0.003261], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "cis_weap_inf_launcher",
        Position([-0.195014, 0.366254, 0.003622], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "cis_weap_inf_pistol",
        Position([-0.230665, 0.353164, -0.002834], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "cis_weap_inf_rifle",
        Position([-0.217397, 0.359879, 0.001903], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "cis_weap_inf_sniperrifle",
        Position([-0.21129, 0.362247, -0.003271], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "cis_weap_inf_sniperrifle",
        Rotation([0.804269, 90.74727, 359.9997]),
    ),
    (
        "player1weapon1mesh",
        "com_weap_inf_torpedo",
        Position([-0.220738, 0.360748, 0.009504], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "ewk_spear",
        Position([-0.059835, 0.249738, 199.96967], "Viewport"),
    ),
    ("player1weapon1mesh", "ewk_spear", Scale([1.0, 1.0, 1.0])),
    (
        "player1weapon1mesh",
        "hud_cis_trishot",
        Position([-0.227755, 0.350278, 0.003377], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "hud_cis_wristblaster",
        Position([-0.225014, 0.350327, 0.002719], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "hud_cis_wristblaster",
        Rotation([359.9672, 359.76343, 119.96609]),
    ),
    (
        "player1weapon1mesh",
        "hud_imp_flamethrower",
        Position([-0.139062, 0.358333, 0.0], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "hud_lightsaber",
        Position([-0.221242, 0.356256, 0.002719], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "hud_lightsaber",
        Scale([1.093337, 1.093337, 1.093337]),
    ),
    (
        "player1weapon1mesh",
        "hud_stealth",
        Position([-0.251257, 0.352591, 0.0], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "hud_stealth",
        Scale([0.320001, 0.320001, 0.320001]),
    ),
    (
        "player1weapon1mesh",
        "hud_vehicle_flak",
        Position([-0.261789, 0.357042, 0.00628], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "hud_vehicle_flak",
        Scale([1.0, 1.0, 1.0]),
    ),
    (
        "player1weapon1mesh",
        "hud_vehicle_guided_rocket",
        Position([-0.203592, 0.354333, 0.00628], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "hud_vehicle_guided_rocket",
        Scale([1.909999, 1.909999, 1.909999]),
    ),
    (
        "player1weapon1mesh",
        "hud_vehicle_lasers",
        Position([-0.18155, 0.354928, -0.000509], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "hud_vehicle_missile",
        Position([-0.238161, 0.353195, 0.00628], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "hud_vehicle_mortar",
        Position([-0.278276, 0.250243, 0.00628], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "hud_vehicle_mortar",
        Scale([1.0, 1.0, 1.0]),
    ),
    (
        "player1weapon1mesh",
        "imp_weap_inf_pistol",
        Position([-0.246548, 0.356163, -0.003873], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "imp_weap_inf_pistol",
        Scale([1.0, 1.0, 1.0]),
    ),
    (
        "player1weapon1mesh",
        "imp_weap_inf_rifle",
        Position([-0.216959, 0.360546, 0.003365], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "rep_weap_inf_chaingun",
        Position([-0.180229, 0.371106, 0.007124], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "rep_weap_inf_chaingun",
        Rotation([0.804269, 90.74727, 359.9997]),
    ),
    (
        "player1weapon1mesh",
        "rep_weap_inf_chaingun",
        Scale([0.83, 0.83, 0.83]),
    ),
    (
        "player1weapon1mesh",
        "rep_weap_inf_launcher",
        Position([-0.198116, 0.373565, 0.007124], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "rep_weap_inf_pistol",
        Position([-0.231578, 0.359076, 0.007261], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "rep_weap_inf_rifle",
        Position([-0.190026, 0.359797, 0.003263], "Viewport"),
    ),
    (
        "player1weapon1mesh",
        "rep_weap_inf_sniperrifle",
        Position([-0.215316, 0.36723, -0.007488], "Viewport"),
    ),
    (
        "",
        "player1weapon1name",
        Position([0.076942, 0.226998, 0.0], "Viewport"),
    ),
    (
        "",
        "player1weapon2ammo",
        Position([0.076532, 0.286563, 0.0], "Viewport"),
    ),
    (
        "",
        "player1weapon2ammoInfinite",
        Position([0.085056, 0.306351, 0.035392], "Viewport"),
    ),
    (
        "",
        "player1weapon2backdrop",
        Position([0.207606, 0.369806, -0.025075], "Viewport"),
    ),
    (
        "",
        "player1weapon2group",
        Position([0.403714, 0.047426, -0.000447], "Viewport"),
    ),
    (
        "",
        "player1weapon2mesh",
        Position([-0.252965, 0.255739, 0.0], "Viewport"),
    ),
    (
        "",
        "player1weapon2mesh",
        Rotation([338.53748, 206.99374, 5.118684]),
    ),
    ("", "player1weapon2mesh", Scale([0.67, 0.67, 0.67])),
    (
        "player1weapon2mesh",
        "com_item_powerup_dual",
        Position([-0.276563, 0.270833, 0.0], "Viewport"),
    ),
    (
        "player1weapon2mesh",
        "com_weap_inf_detpack",
        Position([-0.266703, 0.248956, 0.0], "Viewport"),
    ),
    (
        "player1weapon2mesh",
        "com_weap_inf_dropturret",
        Position([-0.280506, 0.366832, 0.0], "Viewport"),
    ),
    (
        "player1weapon2mesh",
        "com_weap_inf_dropturret",
        Rotation([15.79687, 158.63916, 14.374387]),
    ),
    (
        "player1weapon2mesh",
        "com_weap_inf_dropturret",
        Scale([0.600187, 0.600187, 0.600187]),
    ),
    (
        "player1weapon2mesh",
        "com_weap_inf_landmine",
        Position([-0.268781, 0.236343, 0.0], "Viewport"),
    ),
    (
        "player1weapon2mesh",
        "com_weap_inf_landmine",
        Scale([1.0, 1.0, 1.0]),
    ),
    (
        "player1weapon2mesh",
        "com_weap_inf_seismic_mine",
        Position([-0.247479, 0.255733, 0.0], "Viewport"),
    ),
    (
        "player1weapon2mesh",
        "com_weap_inf_thermaldetonator",
        Position([0.0, 0.0, 0.0], "Pixels"),
    ),
    (
        "player1weapon2mesh",
        "com_weap_inf_thermaldetonator",
        Rotation([353.35867, 99.14022, 1.132509]),
    ),
    (
        "player1weapon2mesh",
        "com_weap_inf_thermaldetonator",
        Scale([1.0, 1.0, 1.0]),
    ),
    (
        "player1weapon2mesh",
        "com_weap_probedroid",
        Rotation([351.88266, 181.73657, 0.0]),
    ),
    (
        "player1weapon2mesh",
        "ewk_rock",
        Position([-0.0791, 0.141669, 276.44482], "Viewport"),
    ),
    ("player1weapon2mesh", "ewk_rock", Scale([1.0, 1.0, 1.0])),
    (
        "player1weapon2mesh",
        "gun_weap_inf_grenadethermal",
        Position([-0.2396, 0.245459, 0.0], "Viewport"),
    ),
    (
        "player1weapon2mesh",
        "hud_cis_wristrocket",
        Position([-0.231045, 0.260428, 0.0], "Viewport"),
    ),
    (
        "player1weapon2mesh",
        "hud_cis_wristrocket",
        Scale([0.88, 0.88, 0.88]),
    ),
    (
        "player1weapon2mesh",
        "hud_detpack_plunger",
        Position([-0.271784, 0.242515, 0.039169], "Viewport"),
    ),
    (
        "player1weapon2mesh",
        "hud_droideka_shield",
        Position([-0.252854, 0.259983, 0.013074], "Viewport"),
    ),
    (
        "player1weapon2mesh",
        "hud_force_choke",
        Position([-0.269269, 0.260038, 0.326666], "Viewport"),
    ),
    (
        "player1weapon2mesh",
        "hud_force_choke",
        Scale([1.299999, 1.299999, 1.299999]),
    ),
    (
        "player1weapon2mesh",
        "hud_force_lightning",
        Position([-0.237166, 0.258013, 0.06], "Viewport"),
    ),
    (
        "player1weapon2mesh",
        "hud_force_lightning",
        Scale([0.830001, 0.830001, 0.830001]),
    ),
    (
        "player1weapon2mesh",
        "hud_force_pull",
        Position([-0.230219, 0.262752, 0.06], "Viewport"),
    ),
    (
        "player1weapon2mesh",
        "hud_force_pull",
        Scale([1.43, 1.43, 1.43]),
    ),
    (
        "player1weapon2mesh",
        "hud_force_push",
        Position([-0.230219, 0.262752, 0.06], "Viewport"),
    ),
    (
        "player1weapon2mesh",
        "hud_force_push",
        Scale([1.43, 1.43, 1.43]),
    ),
    (
        "player1weapon2mesh",
        "hud_force_saberthrow",
        Position([-0.272209, 0.25225, 0.06], "Viewport"),
    ),
    (
        "player1weapon2mesh",
        "hud_force_saberthrow",
        Scale([0.160001, 0.160001, 0.160001]),
    ),
    (
        "player1weapon2mesh",
        "hud_health_regen",
        Position([-0.284336, 0.25477, 0.0], "Viewport"),
    ),
    (
        "player1weapon2mesh",
        "hud_health_regen",
        Rotation([350.75073, 12.551803, 0.86993]),
    ),
    (
        "player1weapon2mesh",
        "hud_health_regen",
        Scale([0.550001, 0.550001, 0.550001]),
    ),
    (
        "player1weapon2mesh",
        "hud_poison",
        Rotation([0.020765, 359.92514, 359.9997]),
    ),
    (
        "player1weapon2mesh",
        "hud_rage",
        Position([-0.251899, 0.248431, 0.0], "Viewport"),
    ),
    (
        "player1weapon2mesh",
        "hud_rage",
        Rotation([0.020765, 359.92514, 359.9997]),
    ),
    (
        "player1weapon2mesh",
        "hud_rep_weap_defense_buff_icon",
        Position([-0.269786, 0.257026, 0.013074], "Viewport"),
    ),
    (
        "player1weapon2mesh",
        "hud_thermaldetonator",
        Position([-0.278276, 0.250243, 0.00628], "Viewport"),
    ),
    (
        "player1weapon2mesh",
        "hud_vehicle_bombs",
        Position([-0.274918, 0.256127, 0.00628], "Viewport"),
    ),
    (
        "player1weapon2mesh",
        "hud_vehicle_bombs",
        Scale([0.7, 0.7, 0.7]),
    ),
    (
        "player1weapon2mesh",
        "hud_vehicle_guided_rocket",
        Position([-0.278276, 0.250243, 0.00628], "Viewport"),
    ),
    (
        "player1weapon2mesh",
        "hud_vehicle_missile",
        Position([-0.224235, 0.25043, 0.00628], "Viewport"),
    ),
    (
        "player1weapon2mesh",
        "hud_vehicle_missile",
        Scale([1.549995, 1.549995, 1.549995]),
    ),
    (
        "player1weapon2mesh",
        "hud_vehicle_recoiless",
        Position([-0.230713, 0.260922, 0.00628], "Viewport"),
    ),
    (
        "player1weapon2mesh",
        "wok_weap_inf_thermaldetonator",
        Position([-0.269904, 0.260036, 0.00628], "Viewport"),
    ),
    (
        "player1weapon2mesh",
        "wok_weap_inf_thermaldetonator",
        Rotation([0.056783, 359.99658, 359.9997]),
    ),
    (
        "",
        "player1weapon2name",
        Position([0.07513, 0.186985, 0.0], "Viewport"),
    ),
];
