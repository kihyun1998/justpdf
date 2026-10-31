//! `Journal::from_bytes` allocates in proportion to what it reads, not to the
//! counts its input states. This binary counts every allocation, so it holds
//! a single test.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use justpdf_core::journal::Journal;

struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let live = LIVE.fetch_add(layout.size(), Ordering::SeqCst) + layout.size();
        PEAK.fetch_max(live, Ordering::SeqCst);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::SeqCst);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const TAG_ADD: u8 = 1;
const TAG_MODIFY: u8 = 2;
const TAG_BATCH: u8 = 4;
const OBJ_NULL: u8 = 0;
const OBJ_ARRAY: u8 = 6;
const OBJ_DICT: u8 = 7;

fn header(undo: u32) -> Vec<u8> {
    let mut b = b"JRNL".to_vec();
    b.extend(1u32.to_le_bytes());
    b.extend(undo.to_le_bytes());
    b
}

/// The most bytes live at once while `from_bytes` reads `input`, above
/// what was live before.
fn peak_while_reading(input: &[u8]) -> usize {
    let before = LIVE.load(Ordering::SeqCst);
    PEAK.store(before, Ordering::SeqCst);
    let journal = Journal::from_bytes(input);
    assert!(journal.is_none());
    PEAK.load(Ordering::SeqCst) - before
}

#[test]
fn a_stated_count_reserves_nothing_beyond_what_is_read() {
    let max = u32::MAX.to_le_bytes();
    let mut cases: Vec<(&str, Vec<u8>)> = Vec::new();

    cases.push(("undo", header(u32::MAX)));

    let mut redo = header(0);
    redo.extend(max);
    cases.push(("redo", redo));

    let mut batch = header(1);
    batch.push(TAG_BATCH);
    batch.extend(0u32.to_le_bytes());
    batch.extend(max);
    batch.push(TAG_ADD);
    batch.extend(1u32.to_le_bytes());
    cases.push(("batch", batch));

    let mut array = header(1);
    array.push(TAG_MODIFY);
    array.extend(7u32.to_le_bytes());
    array.push(OBJ_ARRAY);
    array.extend(max);
    cases.push(("array", array));

    let mut dict = header(1);
    dict.push(TAG_MODIFY);
    dict.extend(7u32.to_le_bytes());
    dict.push(OBJ_DICT);
    dict.extend(max);
    cases.push(("dict", dict));

    // 256 arrays, each stating u32::MAX items, then 4000 nulls: a bound per
    // level would reserve for the 4000 bytes at every one of them.
    let mut nested = header(1);
    nested.push(TAG_MODIFY);
    nested.extend(7u32.to_le_bytes());
    for _ in 0..256 {
        nested.push(OBJ_ARRAY);
        nested.extend(max);
    }
    nested.extend(std::iter::repeat_n(OBJ_NULL, 4000));
    cases.push(("nested", nested));

    for (name, input) in cases {
        let peak = peak_while_reading(&input);
        assert!(
            peak < 2 << 20,
            "{name}: {peak} bytes live for {} input bytes",
            input.len()
        );
    }
}
