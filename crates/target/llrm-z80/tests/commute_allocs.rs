//! `commute_to_the_dying_source` solved the virtual registers live out of each
//! block in hash sets rebuilt per block per round: 4.6% of idioms -O2, 3.5% of
//! it in `HashSet<u32>` growth. The solution takes a few allocations a block.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use llrm_spike_mf::{Block, Inst, Kind, Operand, Reg};
use llrm_z80::mfunc::MFunction;
use llrm_z80::regalloc::Class;

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

/// A chain of `count` blocks: the first makes 24 words, each of the others
/// reads them all, so each is live out of every block before the last.
fn chain(count: usize) -> MFunction {
    const WORDS: u32 = 24;
    let mut function = MFunction::new("f");
    function.classes = vec![Class::Gr16; WORDS as usize];
    for at in 0..count {
        let insts = if at == 0 {
            (0..WORDS)
                .map(|v| Inst::new(Kind::Op("LD16ri"), vec![Operand::Def(Reg::Virt(v)), Operand::Imm(i64::from(v))]))
                .collect()
        } else {
            vec![Inst::new(Kind::Op("USE"), (0..WORDS).map(|v| Operand::Use(Reg::Virt(v))).collect())]
        };
        function.blocks.push(Block { id: at as u32, insts, phis: Vec::new() });
    }
    function
}

#[test]
fn the_liveness_of_a_chain_of_blocks_takes_a_few_allocations_a_block() {
    let mut function = chain(400);
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    llrm_z80::commute::commute_to_the_dying_source(&mut function);
    let made = ALLOCATIONS.load(Ordering::Relaxed) - before;
    assert!(made < 400 * 12, "{made} allocations for 400 blocks");
}
