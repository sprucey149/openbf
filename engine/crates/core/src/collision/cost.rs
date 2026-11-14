//! What the rays of a tick cost, for the game's slow-frame log. Counted here
//! rather than timed: a count is the same on every machine and every run, so
//! two logs can be compared, and an increment costs next to nothing on a path
//! this hot. A measurement only: nothing in the sim reads it and `Sim::hash`
//! leaves it out. One thread runs a sim, and each thread counts its own.

use std::cell::Cell;

/// The rays since the last [`take`].
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RayCost {
    /// World ray queries (each may test many bodies).
    pub queries: u32,
    /// Casts against a triangle mesh.
    pub mesh_casts: u32,
    /// Triangles those casts tested.
    pub triangles: u32,
    /// The cast that tested the most triangles.
    pub worst: Option<WorstCast>,
}

/// The dearest single cast: which map instance it was, and how many triangles.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WorstCast {
    pub triangles: u32,
    /// The body's map instance, where it has one.
    pub instance: Option<u32>,
    pub origin: [f32; 3],
    pub range: f32,
}

impl RayCost {
    /// `self` with `other` added: the rays of two stretches of a frame.
    pub fn plus(mut self, other: RayCost) -> RayCost {
        self.queries += other.queries;
        self.mesh_casts += other.mesh_casts;
        self.triangles += other.triangles;
        if other.worst.map(|w| w.triangles) > self.worst.map(|w| w.triangles) {
            self.worst = other.worst;
        }
        self
    }
}

thread_local! {
    static QUERIES: Cell<u32> = const { Cell::new(0) };
    static MESH_CASTS: Cell<u32> = const { Cell::new(0) };
    static TRIANGLES: Cell<u32> = const { Cell::new(0) };
    static LAST_CAST: Cell<u32> = const { Cell::new(0) };
    static WORST: Cell<Option<WorstCast>> = const { Cell::new(None) };
}

/// The rays counted since the last call, and a fresh count.
pub fn take() -> RayCost {
    RayCost {
        queries: QUERIES.with(|c| c.replace(0)),
        mesh_casts: MESH_CASTS.with(|c| c.replace(0)),
        triangles: TRIANGLES.with(|c| c.replace(0)),
        worst: WORST.with(|c| c.replace(None)),
    }
}

/// Add `rays` (a think thread's, `sim::think_pool`) to this thread's count.
pub(crate) fn add(rays: RayCost) {
    let mine = take().plus(rays);
    QUERIES.with(|c| c.set(mine.queries));
    MESH_CASTS.with(|c| c.set(mine.mesh_casts));
    TRIANGLES.with(|c| c.set(mine.triangles));
    WORST.with(|c| c.set(mine.worst));
}

/// The world queries counted so far, the count going on.
pub fn queries() -> u32 {
    QUERIES.with(Cell::get)
}

#[cfg(feature = "brain-shadow")]
pub(crate) fn preserve(f: impl FnOnce()) {
    struct Restore {
        rays: RayCost,
        last_cast: u32,
    }
    impl Drop for Restore {
        fn drop(&mut self) {
            QUERIES.with(|c| c.set(self.rays.queries));
            MESH_CASTS.with(|c| c.set(self.rays.mesh_casts));
            TRIANGLES.with(|c| c.set(self.rays.triangles));
            LAST_CAST.with(|c| c.set(self.last_cast));
            WORST.with(|c| c.set(self.rays.worst));
        }
    }
    let _restore = Restore {
        rays: take(),
        last_cast: LAST_CAST.with(|c| c.replace(0)),
    };
    f();
}

#[inline]
pub(super) fn query() {
    QUERIES.with(|c| c.set(c.get() + 1));
}

/// A mesh cast tested `triangles`; the caller says whose it was
/// ([`mesh_cast_of`]).
#[inline]
pub(super) fn mesh_cast(triangles: u32) {
    MESH_CASTS.with(|c| c.set(c.get() + 1));
    TRIANGLES.with(|c| c.set(c.get() + triangles));
    LAST_CAST.with(|c| c.set(triangles));
}

/// The mesh cast just made was of the body of map instance `instance`, from
/// `origin` over `range` metres. A cast that tested nothing, or a body that is
/// no mesh, costs a thread-local read.
#[inline]
pub(super) fn mesh_cast_of(instance: Option<u32>, origin: [f32; 3], range: f32) {
    let triangles = LAST_CAST.with(|c| c.replace(0));
    if triangles == 0 {
        return;
    }
    WORST.with(|c| {
        if c.get().is_none_or(|w| triangles > w.triangles) {
            c.set(Some(WorstCast {
                triangles,
                instance,
                origin,
                range,
            }));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_returns_the_count_and_starts_a_new_one() {
        take();
        query();
        mesh_cast(10);
        mesh_cast_of(Some(4), [1.0, 2.0, 3.0], 50.0);
        mesh_cast(3);
        mesh_cast_of(Some(5), [0.0; 3], 1.0);
        let got = take();
        assert_eq!((got.queries, got.mesh_casts, got.triangles), (1, 2, 13));
        let worst = got.worst.unwrap();
        assert_eq!(
            (worst.triangles, worst.instance, worst.range),
            (10, Some(4), 50.0)
        );
        assert_eq!(take(), RayCost::default());
    }

    #[test]
    fn two_stretches_add_and_keep_the_worse_cast() {
        let a = RayCost {
            queries: 2,
            mesh_casts: 1,
            triangles: 5,
            worst: Some(WorstCast {
                triangles: 5,
                ..WorstCast::default()
            }),
        };
        let b = RayCost {
            queries: 1,
            mesh_casts: 1,
            triangles: 9,
            worst: Some(WorstCast {
                triangles: 9,
                instance: Some(2),
                ..WorstCast::default()
            }),
        };
        let sum = a.plus(b);
        assert_eq!((sum.queries, sum.triangles), (3, 14));
        assert_eq!(sum.worst.unwrap().instance, Some(2));
        assert_eq!(a.plus(RayCost::default()).worst, a.worst);
    }
}
