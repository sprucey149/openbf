//! Where each local player's view goes in the window (issue #194,
//! `analysis/plan/SPLITSCREEN.md` section 7).
//!
//! One player fills the window. Two players share it side by side (the default) or one over the
//! other. Three and four players use a 2x2 grid; with three, the fourth cell is simply unused.
//!
//! Every cell of one layout has exactly the same width and height, because the renderer makes its
//! size-keyed targets once per present (`render::ViewSlot`). Sizes round down and a thin border
//! of [`BORDER`] pixels separates the cells; any odd leftover pixels are split around the whole
//! block so it stays centred. Pure, no window needed.

/// The gap between two cells, in pixels.
pub const BORDER: u32 = 2;

/// How two players share the window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TwoPlayer {
    /// One left, one right (the default).
    #[default]
    SideBySide,
    /// One over the other (the original's layout).
    TopBottom,
}

/// A cell as `[x, y, width, height]` in window pixels.
pub type Cell = [u32; 4];

/// The cells for `players` (clamped to 1..=4) in a `width` x `height` window, in player order.
/// Empty for a window with no area or `players == 0`.
pub fn cells(players: usize, width: u32, height: u32, two: TwoPlayer) -> Vec<Cell> {
    if players == 0 || width == 0 || height == 0 {
        return Vec::new();
    }
    let players = players.min(4);
    if players == 1 {
        return vec![[0, 0, width, height]];
    }
    // Columns and rows of the block.
    let (cols, rows) = match (players, two) {
        (2, TwoPlayer::SideBySide) => (2, 1),
        (2, TwoPlayer::TopBottom) => (1, 2),
        _ => (2, 2),
    };
    let fit = |len: u32, n: u32| (len.saturating_sub(BORDER * (n - 1)) / n).max(1);
    let (cw, ch) = (fit(width, cols), fit(height, rows));
    let used_w = cw * cols + BORDER * (cols - 1);
    let used_h = ch * rows + BORDER * (rows - 1);
    let (x0, y0) = (
        width.saturating_sub(used_w) / 2,
        height.saturating_sub(used_h) / 2,
    );
    (0..players as u32)
        .map(|i| {
            let (col, row) = (i % cols, i / cols);
            [x0 + col * (cw + BORDER), y0 + row * (ch + BORDER), cw, ch]
        })
        .collect()
}

/// Is `order` a permutation of 0..4?
fn is_permutation(order: [u8; 4]) -> bool {
    let mut seen = [false; 4];
    order.iter().all(|&c| {
        let fresh = usize::from(c) < 4 && !seen[usize::from(c)];
        if fresh {
            seen[usize::from(c)] = true;
        }
        fresh
    })
}

/// The cells for `players` local players with the Splitscreen options' order (issue #194,
/// `SPLITSCREEN.md` 18.1 item 4), indexed by local player. `order[seat]` is the 2x2 grid
/// position (0 top-left to 3 bottom-right) of Players-screen seat P1..P4, and `seat_of(i)` is
/// local player `i`'s seat number.
///
/// - One player: the whole window.
/// - Two: the two players ranked by their seats' `order` values; the lower gets the left (or
///   top) cell of [`cells`]`(2, ..)`.
/// - Three and four: the 2x2 grid, each player in their seat's grid position, so with three the
///   blank cell can be any of the four.
///
/// With the identity order and `seat_of(i) == i` it equals [`cells`] for every count. An order
/// that is not a permutation (or seats that collide) falls back to [`cells`]' own order.
pub fn arranged(
    players: usize,
    width: u32,
    height: u32,
    two: TwoPlayer,
    order: [u8; 4],
    seat_of: impl Fn(usize) -> usize,
) -> Vec<Cell> {
    let n = players.min(4);
    let base = cells(n, width, height, two);
    if n < 2 || base.len() != n || !is_permutation(order) {
        return base;
    }
    let pos = |i: usize| order.get(seat_of(i)).copied().map(usize::from);
    let Some(positions) = (0..n).map(pos).collect::<Option<Vec<usize>>>() else {
        return base;
    };
    let mut distinct = positions.clone();
    distinct.sort_unstable();
    distinct.dedup();
    if distinct.len() != n {
        return base;
    }
    if n == 2 {
        let first = usize::from(positions[1] < positions[0]);
        return vec![base[first], base[1 - first]];
    }
    let grid = cells(4, width, height, two);
    positions.iter().map(|&p| grid[p]).collect()
}

/// The Splitscreen options' screen row step (18.1 item 4) on seat `seat`: the new order.
/// `seated` says which seats sit in the match.
///
/// - Two seated: any step on either seated row swaps the two seated seats' values, so every
///   press changes the screen whatever the absent seats hold.
/// - Three or four: the seat moves to the next grid position (`right`) or the one before, and
///   swaps with whoever (or the blank) held it.
///
/// Unchanged when `seat` is not seated or fewer than two are. A bad order is reset to the
/// identity first.
pub fn move_seat(order: [u8; 4], seated: [bool; 4], seat: usize, right: bool) -> [u8; 4] {
    let mut order = if is_permutation(order) {
        order
    } else {
        [0, 1, 2, 3]
    };
    let count = seated.iter().filter(|s| **s).count();
    if seat >= 4 || !seated[seat] || count < 2 {
        return order;
    }
    let other = if count == 2 {
        (0..4).find(|&j| j != seat && seated[j])
    } else {
        let to = (order[seat] + if right { 1 } else { 3 }) % 4;
        (0..4).find(|&j| order[j] == to)
    };
    if let Some(j) = other {
        order.swap(seat, j);
    }
    order
}

/// A view's 4:3 band inside its `cell`: the rectangle the view's projected UI (the reticle,
/// lock-ons, markers: `ui::hud_draw::Frame::cell`'s `proj`), the spawn and stats pages and the
/// class page's 3D line-up ([`ui_view`]) are laid out on. The view's UI itself is laid out for
/// and painted over the whole cell, its HUD items at the cell's edges (`game::split_ui`,
/// `SPLITSCREEN.md` 18.1 item 5).
///
/// - A cell 4:3 or wider is its own band.
/// - A cell narrower than 4:3 (two side by side in a window less than 8:3) gets a 4:3 band the
///   cell's full width, centred up and down: the part of the view's widened field
///   ([`fit_fov_y`]) the unwidened camera covers.
///
/// PROVISIONAL `[ours]`; replace: the maintainer's look at the B6 captures (off-screen marker
/// clamping to the cell instead of the band is deferred until then, `SPLITSCREEN.md` 18.3).
pub fn ui_rect(cell: Cell) -> Cell {
    let [x, y, w, h] = cell;
    // 4:3 or wider: the cell itself.
    if u64::from(w) * 3 >= u64::from(h) * 4 {
        return cell;
    }
    let band = (u64::from(w) * 3 / 4).max(1) as u32;
    [x, y + (h - band) / 2, w, band]
}

/// The vertical field of view for a view of `aspect` (width / height) whose camera's own is
/// `fov_y`: the same, unless the cell is narrower than 4:3, where it widens so the view still
/// sees a 4:3 screen's width (the plan's rule: horizontal FOV is kept). Never stretched; capped
/// below 120 degrees.
pub fn fit_fov_y(fov_y: f32, aspect: f32) -> f32 {
    const FOUR_THREE: f32 = 4.0 / 3.0;
    if aspect.is_nan() || aspect <= 0.0 || aspect >= FOUR_THREE {
        return fov_y;
    }
    let wide = 2.0 * ((fov_y * 0.5).tan() * FOUR_THREE / aspect).atan();
    wide.min(120f32.to_radians()).max(fov_y)
}

/// The vertical field of view and aspect a view's 3D UI pieces (the spawn screens' soldiers) are
/// laid out for: those of its UI rectangle ([`ui_rect`]) seen through the view's camera, whose
/// vertical field of view `fov_y` spans the whole `cell`. A cell 4:3 or wider gives back its own
/// field and aspect; a narrower cell's centred band gives the band's 4:3 and the part of the field
/// the band covers (the camera's field before [`fit_fov_y`] widened it), so the soldiers stand in
/// the band as they stand on a single player's 4:3 screen.
pub fn ui_view(fov_y: f32, cell: Cell) -> (f32, f32) {
    let r = ui_rect(cell);
    let share = r[3] as f32 / cell[3].max(1) as f32;
    let fov = 2.0 * ((fov_y * 0.5).tan() * share).atan();
    (fov, r[2] as f32 / r[3].max(1) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOWS: [(u32, u32); 5] = [
        (1280, 720),
        (1920, 1080),
        (1281, 721),
        (800, 600),
        (2561, 1441),
    ];

    fn overlap(a: Cell, b: Cell) -> bool {
        a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
    }

    #[test]
    fn one_player_gets_the_whole_window() {
        for (w, h) in WINDOWS {
            for two in [TwoPlayer::SideBySide, TwoPlayer::TopBottom] {
                assert_eq!(cells(1, w, h, two), vec![[0, 0, w, h]]);
            }
        }
    }

    #[test]
    fn every_cell_has_the_same_size_and_lies_in_the_window() {
        for (w, h) in WINDOWS {
            for n in 1..=4 {
                for two in [TwoPlayer::SideBySide, TwoPlayer::TopBottom] {
                    let c = cells(n, w, h, two);
                    assert_eq!(c.len(), n);
                    for cell in &c {
                        assert_eq!((cell[2], cell[3]), (c[0][2], c[0][3]), "n={n} {w}x{h}");
                        assert!(cell[2] > 0 && cell[3] > 0);
                        assert!(cell[0] + cell[2] <= w && cell[1] + cell[3] <= h);
                    }
                }
            }
        }
    }

    #[test]
    fn no_two_cells_overlap_and_a_border_separates_them() {
        for (w, h) in WINDOWS {
            for n in 2..=4 {
                for two in [TwoPlayer::SideBySide, TwoPlayer::TopBottom] {
                    let c = cells(n, w, h, two);
                    for i in 0..c.len() {
                        for j in i + 1..c.len() {
                            assert!(!overlap(c[i], c[j]), "{i} and {j} overlap, n={n} {w}x{h}");
                        }
                    }
                }
            }
        }
        let c = cells(2, 1280, 720, TwoPlayer::SideBySide);
        assert_eq!(c[1][0] - (c[0][0] + c[0][2]), BORDER);
    }

    #[test]
    fn two_players_split_by_the_chosen_axis() {
        let side = cells(2, 1280, 720, TwoPlayer::SideBySide);
        assert_eq!((side[0][1], side[1][1]), (0, 0));
        assert!(side[1][0] > side[0][0]);
        assert_eq!(side[0][3], 720);
        let top = cells(2, 1280, 720, TwoPlayer::TopBottom);
        assert_eq!((top[0][0], top[1][0]), (0, 0));
        assert!(top[1][1] > top[0][1]);
        assert_eq!(top[0][2], 1280);
    }

    #[test]
    fn three_players_use_the_first_three_cells_of_the_grid() {
        let three = cells(3, 1280, 720, TwoPlayer::SideBySide);
        let four = cells(4, 1280, 720, TwoPlayer::SideBySide);
        assert_eq!(three, four[..3]);
        assert_eq!(four[2][0], four[0][0], "third cell under the first");
        assert_eq!(four[3][1], four[2][1]);
    }

    #[test]
    fn a_tiny_or_empty_window_never_panics() {
        assert!(cells(0, 100, 100, TwoPlayer::default()).is_empty());
        assert!(cells(2, 0, 100, TwoPlayer::default()).is_empty());
        for n in 1..=4 {
            let c = cells(n, 1, 1, TwoPlayer::default());
            assert_eq!(c.len(), n);
        }
        assert_eq!(cells(9, 640, 480, TwoPlayer::default()).len(), 4);
    }
    #[test]
    fn the_ui_fits_inside_every_cell_and_keeps_its_shape() {
        for (w, h) in WINDOWS
            .iter()
            .copied()
            .chain([(3839, 2372), (1920, 1080), (3840, 2160)])
        {
            for n in 2..=4 {
                for two in [TwoPlayer::SideBySide, TwoPlayer::TopBottom] {
                    for cell in cells(n, w, h, two) {
                        let r = ui_rect(cell);
                        assert!(r[0] >= cell[0] && r[1] >= cell[1], "{cell:?} {r:?}");
                        assert!(r[0] + r[2] <= cell[0] + cell[2]);
                        assert!(r[1] + r[3] <= cell[1] + cell[3]);
                        // The HUD's layout needs 4:3 or wider.
                        assert!(u64::from(r[2]) * 3 + 3 >= u64::from(r[3]) * 4, "{r:?}");
                    }
                }
            }
        }
        // The maintainer's window, two side by side: portrait cells get a centred 4:3 band.
        let c = cells(2, 3839, 2372, TwoPlayer::SideBySide);
        assert_eq!(c[0], [0, 0, 1918, 2372]);
        assert_eq!(ui_rect(c[0]), [0, 467, 1918, 1438]);
        assert_eq!(ui_rect(c[1]), [1920, 467, 1918, 1438]);
        // Wide cells are their own screen.
        let grid = cells(4, 3839, 2372, TwoPlayer::default());
        assert_eq!(ui_rect(grid[3]), grid[3]);
        let tb = cells(2, 3839, 2372, TwoPlayer::TopBottom);
        assert_eq!(ui_rect(tb[1]), tb[1]);
    }

    /// Every permutation of 0..4.
    fn permutations() -> Vec<[u8; 4]> {
        let mut out = Vec::new();
        for a in 0..4u8 {
            for b in 0..4u8 {
                for c in 0..4u8 {
                    for d in 0..4u8 {
                        let p = [a, b, c, d];
                        if is_permutation(p) {
                            out.push(p);
                        }
                    }
                }
            }
        }
        assert_eq!(out.len(), 24);
        out
    }

    /// Every set of `n` seats out of four, as seat numbers in order.
    fn seat_sets(n: usize) -> Vec<Vec<usize>> {
        (0u8..16)
            .filter(|m| m.count_ones() as usize == n)
            .map(|m| (0..4).filter(|s| m & (1 << s) != 0).collect())
            .collect()
    }

    #[test]
    fn the_identity_order_equals_cells() {
        for (w, h) in WINDOWS {
            for two in [TwoPlayer::SideBySide, TwoPlayer::TopBottom] {
                for n in 0..=4 {
                    assert_eq!(
                        arranged(n, w, h, two, [0, 1, 2, 3], |i| i),
                        cells(n, w, h, two),
                        "n={n} {w}x{h} {two:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_order_is_a_bijection_onto_equal_cells() {
        for (w, h) in WINDOWS {
            for two in [TwoPlayer::SideBySide, TwoPlayer::TopBottom] {
                for n in 2..=4 {
                    let grid = if n == 2 {
                        cells(2, w, h, two)
                    } else {
                        cells(4, w, h, two)
                    };
                    for seats in seat_sets(n) {
                        for order in permutations() {
                            let c = arranged(n, w, h, two, order, |i| seats[i]);
                            assert_eq!(c.len(), n);
                            for (i, cell) in c.iter().enumerate() {
                                assert!(grid.contains(cell), "{order:?} {seats:?}");
                                assert_eq!((cell[2], cell[3]), (grid[0][2], grid[0][3]));
                                assert!(!c[..i].contains(cell), "two players share {cell:?}");
                            }
                            if n > 2 {
                                for (i, cell) in c.iter().enumerate() {
                                    assert_eq!(*cell, grid[usize::from(order[seats[i]])]);
                                }
                            }
                        }
                    }
                }
            }
        }
        // Three players with P1 moved to the last place: the blank cell is top-left.
        let grid = cells(4, 1280, 720, TwoPlayer::SideBySide);
        let c = arranged(3, 1280, 720, TwoPlayer::SideBySide, [3, 1, 2, 0], |i| i);
        assert_eq!(c, vec![grid[3], grid[1], grid[2]]);
        assert!(!c.contains(&grid[0]));
        // A bad order falls back to the plain cells.
        assert_eq!(
            arranged(3, 1280, 720, TwoPlayer::SideBySide, [0, 0, 1, 2], |i| i),
            cells(3, 1280, 720, TwoPlayer::SideBySide)
        );
    }

    #[test]
    fn a_move_swaps_exactly_two_seats() {
        for order in permutations() {
            for n in 3..=4 {
                for seats in seat_sets(n) {
                    let mut seated = [false; 4];
                    for &s in &seats {
                        seated[s] = true;
                    }
                    for &seat in &seats {
                        for right in [false, true] {
                            let next = move_seat(order, seated, seat, right);
                            assert!(is_permutation(next));
                            let changed: Vec<usize> =
                                (0..4).filter(|&j| next[j] != order[j]).collect();
                            assert_eq!(changed.len(), 2, "{order:?} {seat} {right}");
                            assert!(changed.contains(&seat));
                            let step = if right { 1 } else { 3 };
                            assert_eq!(next[seat], (order[seat] + step) % 4);
                            let other = changed.into_iter().find(|&j| j != seat).unwrap();
                            assert_eq!(next[other], order[seat]);
                        }
                    }
                    // A seat not in the match changes nothing.
                    if let Some(absent) = (0..4).find(|s| !seated[*s]) {
                        assert_eq!(move_seat(order, seated, absent, true), order);
                    }
                }
            }
        }
    }

    #[test]
    fn in_two_player_every_step_on_either_row_swaps_the_two_screens() {
        for two in [TwoPlayer::SideBySide, TwoPlayer::TopBottom] {
            for seats in seat_sets(2) {
                let mut seated = [false; 4];
                for &s in &seats {
                    seated[s] = true;
                }
                // Whatever the absent seats hold.
                for order in permutations() {
                    for &seat in &seats {
                        for right in [false, true] {
                            let before = arranged(2, 1280, 720, two, order, |i| seats[i]);
                            let next = move_seat(order, seated, seat, right);
                            let after = arranged(2, 1280, 720, two, next, |i| seats[i]);
                            assert_eq!(after, vec![before[1], before[0]], "{order:?} {seats:?}");
                            for j in 0..4 {
                                if !seated[j] {
                                    assert_eq!(next[j], order[j], "absent seats untouched");
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_narrow_cell_keeps_a_four_three_horizontal_view() {
        let fov = 60f32.to_radians();
        assert_eq!(fit_fov_y(fov, 16.0 / 9.0), fov);
        assert_eq!(fit_fov_y(fov, 4.0 / 3.0), fov);
        let tall = fit_fov_y(fov, 1918.0 / 2372.0);
        let hfov = |fy: f32, a: f32| 2.0 * ((fy * 0.5).tan() * a).atan();
        assert!((hfov(tall, 1918.0 / 2372.0) - hfov(fov, 4.0 / 3.0)).abs() < 1e-4);
        assert!(fit_fov_y(fov, 0.1) <= 120f32.to_radians());
    }

    #[test]
    fn the_spawn_soldiers_are_laid_out_for_the_ui_rectangle() {
        let fov = 50f32.to_radians();
        // Portrait cells: the band's 4:3 under the camera's field before it was widened.
        let cell = cells(2, 3839, 2372, TwoPlayer::SideBySide)[0];
        let wide = fit_fov_y(fov, cell[2] as f32 / cell[3] as f32);
        let (ui, aspect) = ui_view(wide, cell);
        assert!((ui - fov).abs() < 2e-3, "{ui} {fov}");
        assert!((aspect - 4.0 / 3.0).abs() < 2e-3, "{aspect}");
        // A wide cell is its own screen.
        let grid = cells(4, 3839, 2372, TwoPlayer::default())[1];
        let (ui, aspect) = ui_view(fov, grid);
        assert!((ui - fov).abs() < 1e-6);
        assert!((aspect - grid[2] as f32 / grid[3] as f32).abs() < 1e-6);
    }
}
