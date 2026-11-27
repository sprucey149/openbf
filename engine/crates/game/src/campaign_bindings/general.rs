//! Identity, message/voice/music/movie and audio-stream bindings: the calls
//! that queue something for the HUD or the audio player.

use super::*;

impl Ctx<'_> {
    pub(super) fn general(&mut self, _it: &mut Interp, name: &str, a: &[Value]) -> Option<Ret> {
        let w = &mut *self.w;
        Some(match name {
            "IsCampaign" => one(Value::Bool(w.is_campaign)),
            "IsMultiplayerGametype" | "ScriptCB_InMultiplayer" | "ScriptCB_InNetGame" => {
                one(Value::Bool(false))
            }
            "ScriptCB_GetPlatform" => one(Value::str("PC")),
            "ScriptCB_GetNumCameras" => one(Value::Num(1.0)),
            "ScriptCB_GetDateStr" => one(Value::str("01/01/2005")),
            "ScriptCB_IsMissionSetupSaved" => one(Value::Bool(false)),
            "GetWorldFilename" => one(Value::str(&w.map)),
            "ScriptCB_GetMissionTime" => one(Value::Num(w.time)),

            "ShowMessageText" => {
                if w.for_player(arg(a, 1)) {
                    if let Some(t) = arg(a, 0).as_str() {
                        w.messages.push(t.to_owned());
                    }
                }
                nothing()
            }
            "BroadcastVoiceOver" => {
                if w.for_player(arg(a, 1)) {
                    if let Some(t) = arg(a, 0).as_str() {
                        w.voice.push(t.to_owned());
                    }
                }
                nothing()
            }
            // B2 `FUN_0019e458`: one argument plays the property 2-D with
            // the queue flag set (`FUN_0043ceb0(h, 0, 0, _, 1)`), so a
            // stream line waits its turn on its stream's slot
            // (re-announcer.md §1 step 2). The battle routes it by kind.
            "ScriptCB_SndPlaySound" => {
                if let Some(t) = arg(a, 0).as_str() {
                    w.sounds.push(t.to_owned());
                }
                nothing()
            }
            "ScriptCB_PlayInGameMusic" => {
                // FUN_001a04a0 (45241): the name, then the replay delay when
                // the call has a second argument (lua_gettop >= 2), else 15.
                if let Some(name) = arg(a, 0).as_str() {
                    let replay = if a.len() < 2 {
                        crate::music::IN_GAME_REPLAY
                    } else {
                        num(a, 1).unwrap_or(0.0)
                    };
                    w.music.push(crate::music::InGameMusic::Play {
                        name: name.to_owned(),
                        replay,
                    });
                }
                nothing()
            }
            "ScriptCB_StopInGameMusic" => {
                w.music.push(crate::music::InGameMusic::Stop);
                nothing()
            }
            "ScriptCB_PlayInGameMovie" => {
                if let Some(t) = arg(a, 1).as_str().or(arg(a, 0).as_str()) {
                    w.movies.push(t.to_owned());
                }
                nothing()
            }
            "SetMissionEndMovie" => {
                w.end_movie = arg(a, 1).as_str().map(str::to_owned);
                nothing()
            }

            "OpenAudioStream" => {
                let slot = arg(a, 1).as_str().map_or(0, fnv1a_lowercase);
                if slot != 0 {
                    w.stream_slots.entry(slot).or_insert(slot);
                }
                one(Value::Ud(Ud::Stream(slot)))
            }
            "AudioStreamAppendSegments" => {
                if let (Some(name), Value::Ud(Ud::Stream(slot))) = (arg(a, 1).as_str(), arg(a, 2)) {
                    if *slot != 0 {
                        w.stream_slots.entry(fnv1a_lowercase(name)).or_insert(*slot);
                    }
                }
                nothing()
            }
            "AudioStreamComplete" => one(Value::Num(if w.voice_playing { 0.0 } else { 1.0 })),

            _ => return None,
        })
    }
}
