//! The allocator weighs an instruction in each register its operand may take
//! (`Target::operand_cost`): every call built a list of registers, and for an
//! `inc` or a compare printed the form and read it back through the timing
//! tables, 4.6% of the instructions of `idioms` -O2. A form is read from the
//! tables once, and an instruction with no register-dependent form costs
//! nothing to ask.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use llrm_6502::alloc::Mos;
use llrm_6502::reg;
use llrm_spike_mf::regalloc::Target;
use llrm_spike_mf::{Inst, Kind, Operand, Reg};

struct Counting;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(
        &self,
        layout: Layout,
    ) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
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

fn made(f: impl FnOnce()) -> usize {
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    f();
    ALLOCATIONS.load(Ordering::Relaxed) - before
}

/// One test: the allocation counter is the process's.
#[test]
fn asking_what_an_instruction_costs_in_a_register_allocates_nothing() {
    let cell = llrm_lir::registers::RegId::from_raw(reg::RC0.index() as u8 + 3);
    let step = Inst::new(Kind::Op("INC"), vec![Operand::Def(Reg::Virt(0)), Operand::Use(Reg::Virt(0))]);
    let compare = Inst::new(
        Kind::Op("CMP"),
        vec![Operand::Def(Reg::Phys(reg::FLAGS)), Operand::Use(Reg::Virt(0)), Operand::Imm(7)],
    );
    let load = Inst::new(Kind::Op("LDI"), vec![Operand::Def(Reg::Virt(0)), Operand::Imm(7)]);
    // the table's answers, and the first read of each form
    for (register, bytes, cycles) in [(reg::X, 1, 2), (reg::Y, 1, 2), (cell, 2, 5), (reg::A, 3, 4)] {
        assert_eq!(Mos::<false>::operand_cost(&step, 0, register), bytes + cycles);
        Mos::<false>::operand_cost(&compare, 1, register);
    }
    let again = made(|| {
        for _ in 0..1000 {
            for register in [reg::X, reg::Y, cell, reg::A] {
                Mos::<false>::operand_cost(&step, 0, register);
                Mos::<false>::operand_cost(&compare, 1, register);
                Mos::<false>::operand_cost(&load, 0, register);
            }
        }
    });
    assert_eq!(again, 0, "allocations for 12000 questions");
}
