//! `register_extra_bytes` named the register by formatting its `Debug` into a
//! `String` on every call: 42,678 calls and 800 instructions each (1.1% of
//! the compile of memory_expansion at -O2).

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

use llrm_lir::registers::RegId;
use llrm_lir::{Held, Loc, Operation, Semantics};
use llrm_x86::register_cost::register_extra_bytes;

struct Counting;
// Per thread: the harness's own thread and any other test allocate meanwhile,
// and a process-wide count failed the gate of #10317 under load.
thread_local! { static ALLOCATIONS: Cell<usize> = const { Cell::new(0) }; }

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(
        &self,
        layout: Layout,
    ) -> *mut u8 {
        ALLOCATIONS.with(|count| count.set(count.get() + 1));
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(
        &self,
        pointer: *mut u8,
        layout: Layout,
    ) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

fn allocations_pricing_registers(repeats: usize) -> usize {
    let held = Loc::Held(Held { value: 7, width: 1, offset: 0 });
    let what = Semantics {
        op: Operation::Move,
        name: None,
        dests: vec![held],
        sources: vec![],
        target: None,
        indirect: false,
        table: vec![],
        table_id: None,
        computed: false,
        repeat: false,
        lock: false,
        pops: None,
        tied: None,
    };
    let registers: Vec<RegId> = RegId::values().collect();
    let sum = |registers: &[RegId]| registers.iter().map(|&reg| register_extra_bytes(&what, 7, reg)).sum::<i64>();
    let first = sum(&registers);
    let before = ALLOCATIONS.with(Cell::get);
    for _ in 0..repeats {
        assert_eq!(sum(&registers), first);
    }
    ALLOCATIONS.with(Cell::get) - before
}

#[test]
fn pricing_a_register_allocates_nothing() {
    let made = allocations_pricing_registers(1);
    assert_eq!(made, 0, "{made} allocations pricing the registers");
}

/// A process-wide allocation counter counted the harness thread and failed the
/// gate of #10317 under load. Another thread allocating in a loop all through
/// the measured window is not this thread's allocations.
#[test]
fn another_thread_allocating_is_not_counted() {
    static STOP: AtomicBool = AtomicBool::new(false);
    static STARTED: AtomicBool = AtomicBool::new(false);
    let noise = std::thread::spawn(|| {
        while !STOP.load(Ordering::Relaxed) {
            std::hint::black_box(Box::new([0u8; 64]));
            STARTED.store(true, Ordering::Relaxed);
        }
    });
    while !STARTED.load(Ordering::Relaxed) {
        std::thread::yield_now();
    }
    let made = allocations_pricing_registers(2000);
    STOP.store(true, Ordering::Relaxed);
    noise.join().unwrap();
    assert_eq!(made, 0, "{made} allocations counted from another thread");
}
