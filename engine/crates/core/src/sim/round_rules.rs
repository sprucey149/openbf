//! Space and ground decisions derived from the active round.

use super::state::Sim;

#[derive(Clone, Copy)]
enum Space {
    Assault,
    OneFlag,
    Landing,
}

#[derive(Clone, Copy)]
pub(crate) struct RoundRules {
    space: Option<Space>,
}

impl RoundRules {
    pub(crate) fn ground(self) -> bool {
        self.space.is_none()
    }

    pub(crate) fn space_assault(self) -> bool {
        matches!(self.space, Some(Space::Assault))
    }
}

impl Sim {
    pub(crate) fn round_rules(&self) -> RoundRules {
        let space = if self.space_assault.is_some() {
            Some(Space::Assault)
        } else if self
            .ctf
            .as_ref()
            .is_some_and(super::ctf::CtfState::is_space)
        {
            Some(Space::OneFlag)
        } else if !self.landing_regions.is_empty() {
            Some(Space::Landing)
        } else {
            None
        };
        RoundRules { space }
    }
}
