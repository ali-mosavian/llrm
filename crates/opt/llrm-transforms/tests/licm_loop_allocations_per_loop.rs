//! `Licm` formatted the note for `LLRM_CHECK_LICM_GRAPH` for every loop it
//! visited, check on or off (6.4 M Ir of a memory_expansion -O2 compile:
//! 4297 loops). The note is built only when the check runs.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use llrm_transforms::licm::Licm;

struct Counting;

thread_local! {
    static ALLOCATED: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(
        &self,
        layout: Layout,
    ) -> *mut u8 {
        ALLOCATED.with(|count| count.set(count.get() + 1));
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

/// Allocations of `Licm` over `n` loops of one add and one store each.
fn allocations(n: usize) -> usize {
    let mut body = String::from("@g = global i16 0\ndefine void @f(i16 %n) {\nb0:\n  br label %h0\n");
    for k in 0..n {
        let next = if k + 1 == n { "exit".to_owned() } else { format!("h{}", k + 1) };
        body += &format!(
            "h{k}:\n  %i{k} = phi i16 [ 0, %{} ], [ %j{k}, %h{k} ]\n  %x{k} = add i16 %i{k}, %n\n  store i16 %x{k}, ptr @g\n  %j{k} = add i16 %i{k}, 1\n  %t{k} = icmp slt i16 %j{k}, %n\n  br i1 %t{k}, label %h{k}, label %{next}\n",
            if k == 0 { "b0".to_owned() } else { format!("h{}", k - 1) }
        );
    }
    body += "exit:\n  ret void\n}\n";
    let mut module = llrm_mir::parse::module(&body).unwrap_or_else(|why| panic!("{why:?}"));
    let mut passes = llrm_mir::passes::PassManager::default();
    passes.add(Licm { apply: true, speculate: true });
    let before = ALLOCATED.with(Cell::get);
    passes.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("licm");
    let run = ALLOCATED.with(Cell::get) - before;
    // A debug build also checks phi arity after a pass (`check_phi_arity`, a
    // map per block): 4.0 allocations a loop that are not `Licm`'s, and the
    // bound read 30.25 against 29.0 there. Measured on the result and taken
    // off: debug and release both read 26.25.
    let (_, _, function) = module.functions().find(|(_, _, function)| !function.is_declaration()).expect("f");
    let before = ALLOCATED.with(Cell::get);
    llrm_mir::passes::check_phi_arity("licm", function);
    run - (ALLOCATED.with(Cell::get) - before)
}

#[test]
fn test_each_further_loop_costs_no_formatted_note() {
    allocations(2);
    let (small, large) = (allocations(10), allocations(50));
    let per_loop = (large - small) as f64 / 40.0;
    eprintln!("per loop {per_loop}");
    assert!(per_loop < PER_LOOP_BOUND, "{per_loop} allocations a loop ({small} for 10 loops, {large} for 50)");
}

const PER_LOOP_BOUND: f64 = 29.0;
