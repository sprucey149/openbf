//! What a tick's thinks have already asked the world. The thinks read a sim no
//! one moves until the write-back ([`Sim::step`]), so a question asked twice
//! among them has one answer, and the second need not be cast. Bits are the key:
//! only the same question counts. Open only during the thinks; anything else
//! that asks gets a cast. Per thread: the sweep runs a map per worker.
//!
//! [`Sim::step`]: crate::sim::Sim::step

use glam::{Vec2, Vec3};
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;

/// A lane ray's two ends.
type LaneKey = ([u32; 3], [u32; 3]);
/// A floor walk's start, its end and whether it may jump.
type WalkKey = ([u32; 3], [u32; 2], bool);

#[derive(Default)]
struct Memo {
    open: bool,
    lanes: HashMap<LaneKey, bool>,
    walks: HashMap<WalkKey, Option<f32>>,
}

thread_local! {
    static MEMO: RefCell<Memo> = RefCell::new(Memo::default());
}

/// Run `thinks` with the memo open, and forget it after.
pub(crate) fn during_thinks<T>(thinks: impl FnOnce() -> T) -> T {
    /// Closes on the way out, a panic's too: answers left open would be taken
    /// for another world's on this thread.
    struct Close;
    impl Drop for Close {
        fn drop(&mut self) {
            MEMO.with_borrow_mut(|m| {
                m.open = false;
                m.lanes.clear();
                m.walks.clear();
            });
        }
    }
    MEMO.with_borrow_mut(|m| m.open = true);
    let _close = Close;
    thinks()
}

/// The one lookup: `cast` runs outside the borrow, since it may ask again.
fn remembered<K: Hash + Eq, V: Copy>(
    map: impl Fn(&mut Memo) -> &mut HashMap<K, V>,
    key: K,
    cast: impl FnOnce() -> V,
) -> V {
    let known = MEMO.with_borrow_mut(|m| m.open.then(|| map(m).get(&key).copied()));
    match known {
        None => cast(),
        Some(Some(v)) => v,
        Some(None) => {
            let v = cast();
            MEMO.with_borrow_mut(|m| map(m).insert(key, v));
            v
        }
    }
}

/// Whether a lane ray from `eye` reaches `to`.
pub(super) fn lane(eye: Vec3, to: Vec3, cast: impl FnOnce() -> bool) -> bool {
    let key = (bits(eye), bits(to));
    remembered(|m| &mut m.lanes, key, cast)
}

/// The floor a walk from `from` to `to` arrives at.
pub(crate) fn walk(
    from: Vec3,
    to: Vec2,
    jump: bool,
    cast: impl FnOnce() -> Option<f32>,
) -> Option<f32> {
    let key = (bits(from), to.to_array().map(f32::to_bits), jump);
    remembered(|m| &mut m.walks, key, cast)
}

fn bits(v: Vec3) -> [u32; 3] {
    v.to_array().map(f32::to_bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_answers_from_memory_only_while_open() {
        let eye = Vec3::new(1.0, 2.0, 3.0);
        let casts = std::cell::Cell::new(0);
        let ask = || {
            lane(eye, Vec3::ZERO, || {
                casts.set(casts.get() + 1);
                true
            })
        };
        ask();
        ask();
        assert_eq!(casts.get(), 2, "closed: every question is cast");
        during_thinks(|| {
            ask();
            ask();
        });
        assert_eq!(casts.get(), 3, "open: the second is not");
        during_thinks(ask);
        assert_eq!(casts.get(), 4, "and the next tick starts over");
    }

    #[test]
    fn a_panicking_think_leaves_it_closed_and_empty() {
        let eye = Vec3::new(4.0, 5.0, 6.0);
        let caught = std::panic::catch_unwind(|| {
            during_thinks(|| {
                lane(eye, Vec3::ZERO, || true);
                panic!("a think");
            })
        });
        assert!(caught.is_err());
        let casts = std::cell::Cell::new(0);
        let ask = || {
            lane(eye, Vec3::ZERO, || {
                casts.set(casts.get() + 1);
                false
            })
        };
        assert!(!ask(), "not the panicked tick's answer");
        assert!(!ask());
        assert_eq!(casts.get(), 2, "closed: every question is cast");
        assert!(!during_thinks(ask), "and the next tick starts empty");
    }
}
