//! Rise of the Empire campaign step, driven from WalkSim::step_tick.
//! Takes the runtime out of the walk, feeds it the tick player and class
//! data, routes its voice/sounds/camera exclusions, then puts it back.
//! The fixed tick belongs to the caller: the campaign runtime gets exactly
//! core::TICK_DT. No wall clock and no sim inputs are created here.

use super::*;

pub(super) fn step(walk: &mut WalkSim) {
    if let Some(mut campaign) = walk.campaign.take() {
        let player_ids = walk
            .sim
            .unit(walk.player)
            .map(|u| (u.team, usize::from(u.spec.class)));
        let player_class = player_ids.and_then(|(team, cls)| {
            walk.team_template(team)
                .classes
                .get(cls)
                .map(|c| c.name.clone())
        });
        let team_classes: [Vec<String>; 2] = [0, 1].map(|t| {
            walk.teams[t]
                .classes
                .iter()
                .map(|c| c.name.clone())
                .collect()
        });
        let native_classes: std::collections::BTreeMap<u8, Vec<String>> = walk
            .native_teams
            .iter()
            .map(|(&team, tmpl)| (team, tmpl.classes.iter().map(|c| c.name.clone()).collect()))
            .collect();
        campaign.set_voice_playing(walk.voice.as_ref().is_some_and(|v| v.voice_playing()));
        campaign.step(
            &mut walk.sim,
            walk.player,
            player_class.as_deref(),
            &team_classes,
            &native_classes,
            &walk.vehicle_classes,
            core::TICK_DT,
        );
        walk.camera_blockers
            .set_excluded(campaign.excluded_camera_instances());
        // `BroadcastVoiceOver` lines join the commander's voice
        // queue as commander lines. [H] the binding's queue key
        // is not traced. trace: BroadcastVoiceOver's binding.
        let mut lines = campaign.take_voice_overs();
        lines.extend(campaign.take_sounds());
        if let Some(v) = walk.voice.as_mut() {
            v.route_streams(campaign.stream_slots());
            v.enable_post_vo(campaign.command_post_vo());
            v.ask_script_sounds(lines);
        }
        walk.campaign = Some(campaign);
    }
}
