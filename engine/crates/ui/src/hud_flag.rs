use crate::hud_tree::{EventValue, Hud};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagStatus {
    Carried {
        friendly: bool,
        player_first_person: bool,
    },
    Dropped,
}

pub fn feed(hud: &mut Hud, status: Option<FlagStatus>, has_mesh: &dyn Fn(&str) -> bool) {
    let last = match hud.last("openbf.flag.state") {
        Some(EventValue::Number(n)) => *n as u8,
        _ => 0,
    };
    let next = match status {
        None => 0,
        Some(FlagStatus::Carried {
            player_first_person: true,
            ..
        }) => 5,
        Some(FlagStatus::Carried { friendly: true, .. }) => 1,
        Some(FlagStatus::Carried {
            friendly: false, ..
        }) => 2,
        Some(FlagStatus::Dropped) if matches!(last, 2 | 4) => 4,
        Some(FlagStatus::Dropped) => 3,
    };
    if next == last {
        return;
    }
    for event in [
        "player1.flag.friend.carried.disable",
        "player1.flag.enemy.carried.disable",
        "player1.flag.friend.dropped.disable",
        "player1.flag.enemy.dropped.disable",
        "player1.flag.player.disable",
    ] {
        hud.send(event, EventValue::None, has_mesh);
    }
    let event = match next {
        1 => Some("player1.flag.enemy.carried"),
        2 => Some("player1.flag.friend.carried"),
        3 => Some("player1.flag.enemy.dropped"),
        4 => Some("player1.flag.friend.dropped"),
        5 => Some("player1.flag.player.carried"),
        _ => None,
    };
    if let Some(event) = event {
        hud.send(event, EventValue::None, has_mesh);
    }
    if next == 5 {
        hud.send("player1.flag.enemy.carried", EventValue::None, has_mesh);
    }
    hud.publish(
        "openbf.flag.state",
        EventValue::Number(next.into()),
        has_mesh,
    );
}

pub const TEAM_COLORS: [egui::Color32; 2] = [
    egui::Color32::from_rgb(242, 100, 40),
    egui::Color32::from_rgb(80, 220, 90),
];
