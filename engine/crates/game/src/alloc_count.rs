//! Heap traffic for `--bench`. With the `alloc-count` feature every allocation in
//! the process is counted, so a detailed bench can say what a tick or a frame
//! allocates. Without it nothing changes: the feature is for measuring builds.

#[cfg(feature = "alloc-count")]
mod on {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

    pub(super) static COUNT: AtomicU64 = AtomicU64::new(0);
    pub(super) static BYTES: AtomicU64 = AtomicU64::new(0);

    struct Counting;

    // SAFETY: every call forwards to `System` unchanged.
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            COUNT.fetch_add(1, Relaxed);
            BYTES.fetch_add(l.size() as u64, Relaxed);
            unsafe { System.alloc(l) }
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            unsafe { System.dealloc(p, l) }
        }
        unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
            COUNT.fetch_add(1, Relaxed);
            BYTES.fetch_add(l.size() as u64, Relaxed);
            unsafe { System.alloc_zeroed(l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
            COUNT.fetch_add(1, Relaxed);
            BYTES.fetch_add(new as u64, Relaxed);
            unsafe { System.realloc(p, l, new) }
        }
    }

    #[global_allocator]
    static COUNTING: Counting = Counting;
}

/// Allocations and bytes asked for so far, or `None` in a normal build.
pub fn so_far() -> Option<(u64, u64)> {
    #[cfg(feature = "alloc-count")]
    {
        use std::sync::atomic::Ordering::Relaxed;
        Some((on::COUNT.load(Relaxed), on::BYTES.load(Relaxed)))
    }
    #[cfg(not(feature = "alloc-count"))]
    {
        None
    }
}

#[cfg(all(test, feature = "alloc-count"))]
mod tests {
    #[test]
    fn a_box_is_counted() {
        let before = super::so_far().unwrap();
        let b = std::hint::black_box(Box::new([0u8; 64]));
        let after = super::so_far().unwrap();
        drop(b);
        assert!(after.0 > before.0 && after.1 >= before.1 + 64);
    }
}
