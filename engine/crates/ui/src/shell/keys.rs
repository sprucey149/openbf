//! One frame of the screen's input: `InstantAction::step`.

use super::*;

impl InstantAction {
    /// One frame: `dt` seconds and the presses.
    pub fn step(&mut self, input: IaInput, dt: f32) -> Option<IaCommand> {
        self.on_page += dt;
        let i = input.menu;
        match self.page {
            Page::Missions => {}
            Page::Overview(focus) => {
                let live: Vec<bool> = self.overview().iter().map(|(_, l)| *l).collect();
                let wrap = |step: isize| {
                    let n = live.len() as isize;
                    let mut k = focus as isize;
                    for _ in 0..n {
                        k = (k + step).rem_euclid(n);
                        if live[k as usize] {
                            return k as usize;
                        }
                    }
                    focus
                };
                if i.back {
                    self.click(MenuSound::Exit);
                    self.go(Page::Missions);
                } else if i.confirm && live[focus] {
                    self.click(MenuSound::Accept);
                    let g = GROUPS[focus];
                    let rows: Vec<_> = self
                        .options
                        .rows(g)
                        .into_iter()
                        .map(|o| self.options.option_enabled(o))
                        .collect();
                    self.go(Page::Group(g, ListCursor::first(&rows, OPTION_ROWS)));
                } else if i.up {
                    self.click(MenuSound::Select);
                    self.page = Page::Overview(wrap(-1));
                } else if i.down {
                    self.click(MenuSound::Select);
                    self.page = Page::Overview(wrap(1));
                }
                return None;
            }
            Page::Group(g, mut cur) => {
                if i.back {
                    self.click(MenuSound::Exit);
                    let at = GROUPS.iter().position(|x| *x == g).unwrap_or(0);
                    self.go(Page::Overview(at));
                    return Some(IaCommand::Prefs);
                }
                let rows = self.options.rows(g);
                let live: Vec<_> = rows
                    .iter()
                    .map(|o| self.options.option_enabled(*o))
                    .collect();
                let mut moved = false;
                if i.up {
                    moved |= cur.step(&live, -1, OPTION_ROWS);
                }
                if i.down {
                    moved |= cur.step(&live, 1, OPTION_ROWS);
                }
                if moved {
                    self.click(MenuSound::Select);
                }
                let (dir, mult) = match (i.left, i.right, input.l1, input.r1) {
                    (true, ..) => (-1, 1),
                    (_, true, ..) => (1, 1),
                    (_, _, true, _) => (-1, 10),
                    (_, _, _, true) => (1, 10),
                    _ => (0, 0),
                };
                if let Some(o) = rows
                    .get(cur.at)
                    .filter(|o| dir != 0 && self.options.option_enabled(**o))
                {
                    // Select on every step, also at a clamp (menuref).
                    self.click(MenuSound::Select);
                    self.options.adjust(*o, dir, mult);
                    // The hero rows come and go with Heroes On / Off.
                    let n = self.options.rows(g).len();
                    cur.at = cur.at.min(n - 1);
                    cur.show(n, OPTION_ROWS);
                }
                self.page = Page::Group(g, cur);
                return None;
            }
        }
        if self.full {
            // "Selected maplist is full." closes with accept, Triangle too
            // (menuref `ia_listfull`).
            if i.confirm || i.back {
                self.click(MenuSound::Accept);
                self.full = false;
            }
            return None;
        }
        // A list press: Up / Down one row, L1 / R1 a page; whether the
        // cursor moved (a click only then, menuref).
        let moves = |c: &mut ListCursor, live: &[bool], shown: usize| {
            let mut moved = false;
            if i.up {
                moved |= c.step(live, -1, shown);
            }
            if i.down {
                moved |= c.step(live, 1, shown);
            }
            if input.l1 || input.r1 {
                let page = if shown == LIST_ROWS { LIST_PAGE } else { shown } as isize;
                moved |= c.step(live, if input.l1 { -page } else { page }, shown);
            }
            moved
        };
        match self.state {
            State::Command => {
                if i.back {
                    return Some(IaCommand::Back);
                }
                let live = COMMANDS.map(|c| self.command_live(c));
                let wrap = |from: usize, step: isize| {
                    let mut k = from as isize;
                    for _ in 0..COMMANDS.len() {
                        k = (k + step).rem_euclid(COMMANDS.len() as isize);
                        if live[k as usize] {
                            return k as usize;
                        }
                    }
                    from
                };
                if i.up {
                    self.command = wrap(self.command, -1);
                }
                if i.down {
                    self.command = wrap(self.command, 1);
                }
                if i.up || i.down {
                    self.click(MenuSound::Select);
                }
                if i.confirm && self.command_live(COMMANDS[self.command]) {
                    self.click(MenuSound::Accept);
                    match COMMANDS[self.command] {
                        Cmd::Launch => return Some(IaCommand::Launch(self.launch())),
                        Cmd::Add => {
                            self.state = State::Map;
                            let live = self.live(&self.map_rows());
                            if !live.get(self.map_list.at).copied().unwrap_or(false) {
                                self.map_list = ListCursor::first(&live, LIST_ROWS);
                            }
                        }
                        // The cursor on the last row (menuref §1.3,
                        // `ia_remove`).
                        Cmd::Remove => {
                            self.state = State::Playlist;
                            let n = self.playlist.len() + 1;
                            self.play_list.at = n - 1;
                            self.play_list.show(n, LIST_ROWS);
                        }
                        Cmd::Options => self.go(Page::Overview(0)),
                    }
                }
            }
            State::Map => {
                let live = self.live(&self.map_rows());
                let moved = if i.up != i.down && !input.l1 && !input.r1 {
                    self.map_list.step_wrapping(&live, i.down, LIST_ROWS)
                } else {
                    moves(&mut self.map_list, &live, LIST_ROWS)
                };
                if moved {
                    self.click(MenuSound::Select);
                }
                if i.back {
                    self.click(MenuSound::Exit);
                    self.state = State::Command;
                } else if i.confirm && live.get(self.map_list.at) == Some(&true) {
                    self.click(MenuSound::Accept);
                    self.picked_map = self.map_picks()[self.map_list.at];
                    self.state = State::Mode;
                    let live = self.live(&self.mode_rows());
                    self.mode_list = ListCursor::first(&live, LIST_ROWS);
                }
            }
            State::Mode => {
                let live = self.live(&self.mode_rows());
                if moves(&mut self.mode_list, &live, LIST_ROWS) {
                    self.click(MenuSound::Select);
                }
                if i.back {
                    self.click(MenuSound::Exit);
                    self.state = State::Map;
                } else if i.confirm && live.get(self.mode_list.at) == Some(&true) {
                    self.click(MenuSound::Accept);
                    self.picked_mode = self.mode_picks()[self.mode_list.at];
                    self.state = State::Era;
                    let live = self.live(&self.side_rows());
                    self.era_list = ListCursor::first(&live, ERA_ROWS);
                }
            }
            State::Era => {
                let live = self.live(&self.side_rows());
                if moves(&mut self.era_list, &live, ERA_ROWS) {
                    self.click(MenuSound::Select);
                }
                if i.back {
                    self.click(MenuSound::Exit);
                    self.state = State::Mode;
                } else if i.confirm && live.get(self.era_list.at) == Some(&true) {
                    // The full popup opens silently over this accept.
                    self.click(MenuSound::Accept);
                    let side = self.side_picks()[self.era_list.at];
                    self.add(self.picked_map, self.picked_mode, side);
                    self.state = State::Command;
                    self.focus_command(Cmd::Launch);
                }
            }
            State::Playlist => {
                let live = vec![true; self.playlist.len() + 1];
                if moves(&mut self.play_list, &live, LIST_ROWS) {
                    self.click(MenuSound::Select);
                }
                if i.back {
                    self.click(MenuSound::Exit);
                    self.state = State::Command;
                } else if i.confirm {
                    match self.play_list.at {
                        // Cross on "Remove All": the error click, the list
                        // cleared (menuref `ia_remove`).
                        0 => {
                            self.click(MenuSound::Error);
                            self.playlist.clear();
                        }
                        k => {
                            self.click(MenuSound::Accept);
                            self.playlist.remove(k - 1);
                        }
                    }
                    let n = self.playlist.len() + 1;
                    self.play_list.at = self.play_list.at.min(n - 1);
                    self.play_list.show(n, LIST_ROWS);
                    self.state = State::Command;
                    self.focus_command(if self.playlist.is_empty() {
                        Cmd::Add
                    } else {
                        Cmd::Launch
                    });
                }
            }
        }
        None
    }
}
