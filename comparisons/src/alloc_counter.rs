//! A dependency-free counting global allocator for the RAM metric.
//!
//! It wraps the system allocator and tracks live (currently-allocated) bytes and
//! the peak seen. The `bin/size` tool installs it as the `#[global_allocator]`
//! and measures one engine per process so counters are never contaminated.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

/// Counting allocator. Use as `#[global_allocator] static A: Counting = Counting;`.
pub struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(live, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            if new_size >= layout.size() {
                let d = new_size - layout.size();
                let live = LIVE.fetch_add(d, Ordering::Relaxed) + d;
                PEAK.fetch_max(live, Ordering::Relaxed);
            } else {
                LIVE.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        new_ptr
    }
}

/// Currently-allocated bytes.
pub fn live() -> usize {
    LIVE.load(Ordering::Relaxed)
}

/// Highest `live()` seen so far.
pub fn peak() -> usize {
    PEAK.load(Ordering::Relaxed)
}

/// Reset the peak watermark down to the current live value (call before a build
/// to measure that build's peak in isolation).
pub fn reset_peak() {
    PEAK.store(LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
}
