//! `[ours]` The one scrolling list behaviour, configured once.
//!
//! Every scrolling menu list (the Game Menu and settings stack, the binding
//! list, Instant Action's lists and option groups, the profile picker) keeps
//! a [`ScrollView`]: where its view starts, in rows (fractional), and where
//! that start is heading.
//!
//! - The wheel and a trackpad move the view, never the selection, by their
//!   own amount (no rounding to whole rows); the view eases there over a few
//!   frames ([`EASE_SECONDS`]).
//! - Keys and the pad move the selection; the view then moves only as far as
//!   it must to keep the selection [`MARGIN`] rows inside it ([`ScrollView::
//!   reveal`]). It never recentres.
//! - A click selects the row under the pointer and leaves the view where it
//!   is; only a row the window cuts off is brought fully in
//!   ([`ScrollView::reveal`] with no margin).
//!
//! Rows are drawn at their fractional place and cut at the list's window.

/// Seconds for the view to cover 63 % of the way to its target.
pub const EASE_SECONDS: f32 = 0.07;
/// Rows the view moves per wheel line (a notch; a trackpad's pixels arrive
/// as fractions of a line).
pub const WHEEL_ROWS: f32 = 1.0;
/// A trackpad's pixels per wheel line.
pub const PX_PER_LINE: f32 = 32.0;
/// Rows kept between a key-moved selection and the window's edge (none at
/// the list's ends).
pub const MARGIN: usize = 1;
/// Closer than this (rows), the view lands on its target.
const SNAP: f32 = 0.004;

/// A list's view: its first shown row (fractional) and its target.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ScrollView {
    offset: f32,
    target: f32,
}

// Offsets are finite (every write is clamped), so equality is total.
impl Eq for ScrollView {}

/// The furthest a view of `shown` rows can start in `len` rows.
pub fn max_offset(len: usize, shown: usize) -> f32 {
    len.saturating_sub(shown) as f32
}

impl ScrollView {
    /// A view resting at `offset` rows.
    pub fn at(offset: f32) -> Self {
        let offset = if offset.is_finite() { offset } else { 0.0 };
        Self {
            offset,
            target: offset,
        }
    }

    /// The first shown row, fractional, as drawn now.
    pub fn offset(&self) -> f32 {
        self.offset
    }

    /// Where the view is heading.
    pub fn target(&self) -> f32 {
        self.target
    }

    /// The first row at least partly shown.
    pub fn first(&self) -> usize {
        self.offset.max(0.0).floor() as usize
    }

    /// The rows at least partly shown by a window of `shown` rows.
    pub fn visible(&self, len: usize, shown: usize) -> std::ops::Range<usize> {
        let first = self.first().min(len);
        let end = ((self.offset + shown as f32 - 1.0e-3).ceil().max(0.0) as usize).min(len);
        first..end.max(first)
    }

    /// The wheel: `lines` > 0 scrolls towards the top.
    pub fn wheel(&mut self, lines: f32, len: usize, shown: usize) {
        if lines.is_finite() {
            self.target -= lines * WHEEL_ROWS;
        }
        self.clamp(len, shown);
    }

    /// Move the target as little as keeps `row` and `margin` rows either
    /// side of it (fewer at the list's ends) inside the window.
    pub fn reveal(&mut self, row: usize, len: usize, shown: usize, margin: usize) {
        if len == 0 || shown == 0 {
            return;
        }
        let row = row.min(len - 1);
        let margin = margin.min(shown.saturating_sub(1) / 2);
        let top = row.saturating_sub(margin) as f32;
        let bottom = (row + margin).min(len - 1) as f32 + 1.0;
        if top < self.target {
            self.target = top;
        }
        if bottom > self.target + shown as f32 {
            self.target = bottom - shown as f32;
        }
        self.clamp(len, shown);
    }

    /// Keep the view and its target inside the list.
    pub fn clamp(&mut self, len: usize, shown: usize) {
        let max = max_offset(len, shown);
        self.target = self.target.clamp(0.0, max);
        self.offset = self.offset.clamp(0.0, max);
    }

    /// Land on the target now (a page opening).
    pub fn snap(&mut self) {
        self.offset = self.target;
    }

    /// Ease towards the target over `dt` seconds of UI time.
    pub fn advance(&mut self, dt: f32) {
        let gap = self.target - self.offset;
        if gap.abs() <= SNAP || !dt.is_finite() {
            self.offset = self.target;
            return;
        }
        self.offset += gap * (1.0 - (-dt.max(0.0) / EASE_SECONDS).exp());
        if (self.target - self.offset).abs() <= SNAP {
            self.offset = self.target;
        }
    }

    /// Whether the view is still on its way.
    pub fn moving(&self) -> bool {
        self.offset != self.target
    }
}

/// Wheel lines for one frame from a raw delta: a mouse notch is one line,
/// a trackpad's pixels `px_per_line` pixels a line, unrounded.
pub fn wheel_lines(line_delta: f32, pixel_delta: f32, px_per_line: f32) -> f32 {
    line_delta + pixel_delta / px_per_line.max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settle(v: &mut ScrollView) {
        for _ in 0..120 {
            v.advance(1.0 / 60.0);
        }
    }

    #[test]
    fn the_wheel_moves_the_view_smoothly_by_its_own_amount() {
        let mut v = ScrollView::default();
        v.wheel(-1.0, 20, 6);
        assert_eq!(v.target(), 1.0);
        v.advance(1.0 / 60.0);
        let first = v.offset();
        assert!(first > 0.0 && first < 1.0, "eases, does not jump: {first}");
        v.advance(1.0 / 60.0);
        assert!(v.offset() > first && v.offset() < 1.0);
        settle(&mut v);
        assert_eq!(v.offset(), 1.0);
        // A trackpad's fraction of a line stays a fraction.
        v.wheel(-0.3, 20, 6);
        settle(&mut v);
        assert!((v.offset() - 1.3).abs() < 1e-5, "{}", v.offset());
        // Never past the ends.
        v.wheel(100.0, 20, 6);
        settle(&mut v);
        assert_eq!(v.offset(), 0.0);
        v.wheel(-100.0, 20, 6);
        settle(&mut v);
        assert_eq!(v.offset(), 14.0);
    }

    #[test]
    fn a_key_moved_selection_scrolls_only_as_far_as_needed_with_a_margin() {
        let mut v = ScrollView::default();
        // Rows 0..6 shown; moving down inside the window does not scroll.
        for row in 0..=4 {
            v.reveal(row, 20, 6, MARGIN);
            assert_eq!(v.target(), 0.0, "row {row}");
        }
        // Row 5 is the last shown: keep one row below it.
        v.reveal(5, 20, 6, MARGIN);
        assert_eq!(v.target(), 1.0);
        v.reveal(6, 20, 6, MARGIN);
        assert_eq!(v.target(), 2.0, "one row per step, never recentred");
        // Back up: no move until the margin is reached.
        v.reveal(4, 20, 6, MARGIN);
        assert_eq!(v.target(), 2.0);
        v.reveal(2, 20, 6, MARGIN);
        assert_eq!(v.target(), 1.0);
        // The list's ends need no margin.
        v.reveal(19, 20, 6, MARGIN);
        assert_eq!(v.target(), 14.0);
        v.reveal(0, 20, 6, MARGIN);
        assert_eq!(v.target(), 0.0);
    }

    #[test]
    fn a_click_on_a_shown_row_leaves_the_view_alone() {
        let mut v = ScrollView::at(3.0);
        for row in 3..9 {
            v.reveal(row, 20, 6, 0);
            assert_eq!(v.target(), 3.0, "row {row}");
            assert_eq!(v.offset(), 3.0);
        }
        // A row the window cuts in half comes fully in, no further.
        let mut v = ScrollView::at(3.5);
        v.reveal(3, 20, 6, 0);
        assert_eq!(v.target(), 3.0);
        let mut v = ScrollView::at(3.5);
        v.reveal(9, 20, 6, 0);
        assert_eq!(v.target(), 4.0);
    }

    #[test]
    fn visible_rows_include_the_partly_shown_ones() {
        assert_eq!(ScrollView::at(0.0).visible(20, 6), 0..6);
        assert_eq!(ScrollView::at(2.5).visible(20, 6), 2..9);
        assert_eq!(ScrollView::at(0.0).visible(3, 6), 0..3);
    }

    #[test]
    fn trackpad_pixels_are_fractions_of_a_line() {
        assert_eq!(wheel_lines(1.0, 0.0, PX_PER_LINE), 1.0);
        assert_eq!(wheel_lines(0.0, 8.0, PX_PER_LINE), 0.25);
    }
}
