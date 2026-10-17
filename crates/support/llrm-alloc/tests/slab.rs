//! The slab hands a freed block of a class back before it takes memory, keeps
//! the bytes through a `realloc` (across its 1 KiB limit too), leaves the large
//! and the over-aligned to mimalloc, and survives blocks freed by another
//! thread.

use std::alloc::{GlobalAlloc, Layout};
use std::sync::mpsc;

use llrm_alloc::Slab;

fn layout(
    size: usize,
    align: usize,
) -> Layout {
    Layout::from_size_align(size, align).expect("a valid layout")
}

fn fill(
    ptr: *mut u8,
    len: usize,
    tag: u8,
) {
    for at in 0..len {
        unsafe { ptr.add(at).write(tag.wrapping_add(at as u8)) };
    }
}

fn holds(
    ptr: *const u8,
    len: usize,
    tag: u8,
) -> bool {
    (0..len).all(|at| unsafe { ptr.add(at).read() } == tag.wrapping_add(at as u8))
}

/// A block freed is the next block of its class: a loop of 1 M allocations and
/// frees takes the memory once, where an allocator that cut a new block each
/// time would take a chunk for every 32,768 of 32 bytes.
#[test]
fn a_freed_block_is_handed_out_again_before_new_memory_is_taken() {
    let before = Slab::chunks();
    let first = unsafe { Slab.alloc(layout(40, 8)) };
    unsafe { Slab.dealloc(first, layout(40, 8)) };
    for _ in 0..1_000_000 {
        let again = unsafe { Slab.alloc(layout(48, 8)) };
        assert_eq!(again, first, "40 and 48 bytes are one class");
        unsafe { Slab.dealloc(again, layout(48, 8)) };
    }
    assert!(Slab::chunks() <= before + 1, "{} chunks for one live block", Slab::chunks() - before);
}

#[test]
fn another_class_does_not_take_the_block() {
    let one = unsafe { Slab.alloc(layout(16, 8)) };
    unsafe { Slab.dealloc(one, layout(16, 8)) };
    let other = unsafe { Slab.alloc(layout(32, 8)) };
    assert_ne!(one, other);
    let same = unsafe { Slab.alloc(layout(16, 8)) };
    assert_eq!(same, one);
    unsafe {
        Slab.dealloc(other, layout(32, 8));
        Slab.dealloc(same, layout(16, 8));
    }
}

#[test]
fn a_block_of_a_class_is_aligned_to_16_and_zeroed_when_asked() {
    for size in [1, 15, 16, 17, 100, 1023, 1024] {
        let dirty = unsafe { Slab.alloc(layout(size, 1)) };
        fill(dirty, size, 0xAB);
        unsafe { Slab.dealloc(dirty, layout(size, 1)) };
        let zeroed = unsafe { Slab.alloc_zeroed(layout(size, 1)) };
        assert_eq!(zeroed as usize % 16, 0, "{size} bytes");
        assert!((0..size).all(|at| unsafe { zeroed.add(at).read() } == 0), "{size} bytes were not zeroed");
        unsafe { Slab.dealloc(zeroed, layout(size, 1)) };
    }
}

#[test]
fn realloc_keeps_the_bytes_inside_a_class_across_classes_and_over_the_limit() {
    let mut size = 1;
    let mut ptr = unsafe { Slab.alloc(layout(size, 8)) };
    fill(ptr, size, 7);
    for next in [2, 15, 16, 17, 31, 500, 1023, 1024, 1025, 3000, 1100, 1024, 900, 17, 16, 1] {
        let moved = unsafe { Slab.realloc(ptr, layout(size, 8), next) };
        assert!(holds(moved, size.min(next), 7), "{size} -> {next}");
        if size.div_ceil(16) == next.div_ceil(16) && size <= 1024 && next <= 1024 {
            assert_eq!(moved, ptr, "{size} -> {next} is one class");
        }
        ptr = moved;
        size = next;
        fill(ptr, size, 7);
    }
    unsafe { Slab.dealloc(ptr, layout(size, 8)) };
}

#[test]
fn a_large_or_over_aligned_request_is_served_and_freed_whole() {
    for (size, align) in [(4096, 8), (1 << 20, 16), (64, 64), (16, 4096), (1, 128), (1025, 16)] {
        let ptr = unsafe { Slab.alloc(layout(size, align)) };
        assert!(!ptr.is_null());
        assert_eq!(ptr as usize % align, 0, "{size} aligned to {align}");
        fill(ptr, size, 3);
        let bigger = unsafe { Slab.realloc(ptr, layout(size, align), size * 2) };
        assert!(holds(bigger, size, 3));
        assert_eq!(bigger as usize % align, 0);
        unsafe { Slab.dealloc(bigger, layout(size * 2, align)) };
    }
}

#[test]
fn a_zero_size_request_is_served() {
    let ptr = unsafe { Slab.alloc(layout(0, 1)) };
    assert!(!ptr.is_null());
    unsafe { Slab.dealloc(ptr, layout(0, 1)) };
}

/// A block made on one thread and freed on another joins the freeing thread's
/// list of its class and is handed out there; the first thread's lists are
/// untouched.
#[test]
fn a_block_freed_by_another_thread_is_reused_there_and_nothing_is_corrupted() {
    let (to_other, from_first) = mpsc::channel::<usize>();
    let (back, answer) = mpsc::channel::<(usize, usize)>();
    let other = std::thread::spawn(move || {
        let ptr = from_first.recv().expect("a block") as *mut u8;
        assert!(holds(ptr, 64, 9));
        unsafe { Slab.dealloc(ptr, layout(64, 8)) };
        let again = unsafe { Slab.alloc(layout(64, 8)) };
        back.send((ptr as usize, again as usize)).expect("sent");
        unsafe { Slab.dealloc(again, layout(64, 8)) };
    });
    let ptr = unsafe { Slab.alloc(layout(64, 8)) };
    fill(ptr, 64, 9);
    to_other.send(ptr as usize).expect("sent");
    let (freed, again) = answer.recv().expect("an answer");
    assert_eq!(freed, again, "the freeing thread hands the block out");
    other.join().expect("the other thread ends");
    let mine = unsafe { Slab.alloc(layout(64, 8)) };
    assert_ne!(mine, ptr, "the first thread's own list does not hold the block it gave away");
    unsafe { Slab.dealloc(mine, layout(64, 8)) };
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

struct Held {
    ptr: usize,
    layout: Layout,
    tag: u8,
}

/// Random sizes (zero to 3,000, a few over-aligned), allocations, frees,
/// reallocs growing and shrinking across the 1 KiB limit, and blocks handed to
/// the next thread to free, from eight threads; every block's bytes are
/// checked whenever it is freed or moved.
#[test]
fn eight_threads_allocating_reallocating_and_freeing_each_others_blocks() {
    const THREADS: usize = 8;
    const STEPS: usize = 30_000;
    let (senders, receivers): (Vec<_>, Vec<_>) = (0..THREADS).map(|_| mpsc::channel::<Held>()).unzip();
    let workers: Vec<_> = receivers
        .into_iter()
        .enumerate()
        .map(|(at, inbox)| {
            let next = senders[(at + 1) % THREADS].clone();
            std::thread::spawn(move || {
                let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ (at as u64 + 1) * 0x1234_5679);
                let mut held: Vec<Held> = Vec::new();
                let check = |one: &Held| {
                    assert!(holds(one.ptr as *const u8, one.layout.size(), one.tag), "a block's bytes changed");
                };
                for _ in 0..STEPS {
                    while let Ok(one) = inbox.try_recv() {
                        check(&one);
                        unsafe { Slab.dealloc(one.ptr as *mut u8, one.layout) };
                    }
                    match rng.next() % 8 {
                        0..=3 => {
                            let size = (rng.next() % 3001) as usize;
                            let align = [1, 8, 16, 16, 16, 64][(rng.next() % 6) as usize];
                            let layout = layout(size, align);
                            let ptr = unsafe { Slab.alloc(layout) };
                            assert!(!ptr.is_null() && ptr as usize % align == 0);
                            let tag = rng.next() as u8;
                            fill(ptr, size, tag);
                            held.push(Held { ptr: ptr as usize, layout, tag });
                        }
                        4 | 5 if !held.is_empty() => {
                            let one = held.swap_remove((rng.next() as usize) % held.len());
                            check(&one);
                            unsafe { Slab.dealloc(one.ptr as *mut u8, one.layout) };
                        }
                        6 if !held.is_empty() => {
                            let index = (rng.next() as usize) % held.len();
                            let one = &mut held[index];
                            check(one);
                            let size = (rng.next() % 3001) as usize;
                            let moved = unsafe { Slab.realloc(one.ptr as *mut u8, one.layout, size.max(1)) };
                            assert!(holds(moved, one.layout.size().min(size.max(1)), one.tag));
                            one.layout = layout(size.max(1), one.layout.align());
                            one.ptr = moved as usize;
                            fill(moved, one.layout.size(), one.tag);
                        }
                        _ if !held.is_empty() => {
                            let one = held.swap_remove((rng.next() as usize) % held.len());
                            // A thread that has finished takes no more: free it
                            // here.
                            if let Err(mpsc::SendError(one)) = next.send(one) {
                                check(&one);
                                unsafe { Slab.dealloc(one.ptr as *mut u8, one.layout) };
                            }
                        }
                        _ => {}
                    }
                }
                for one in held {
                    check(&one);
                    unsafe { Slab.dealloc(one.ptr as *mut u8, one.layout) };
                }
                // The blocks still sent to this thread are freed here; the
                // inbox closes when every thread has dropped its sender.
                drop(next);
                for one in inbox {
                    check(&one);
                    unsafe { Slab.dealloc(one.ptr as *mut u8, one.layout) };
                }
            })
        })
        .collect();
    drop(senders);
    for worker in workers {
        worker.join().expect("a worker finished without a corrupted block");
    }
}
