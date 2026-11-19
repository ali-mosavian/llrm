//! `to_lir` asks the target for a class's registers once, not once per virtual
//! register.

use std::sync::atomic::{AtomicUsize, Ordering};

use llrm_lir::registers::{RegId, Regs};
use llrm_spike_mf::regalloc::Target;
use llrm_spike_mf::tolir::to_lir;
use llrm_spike_mf::{Block, Inst, Kind, MFunction, Operand, Reg};

struct Counting;

static ASKED: AtomicUsize = AtomicUsize::new(0);
const REGISTERS: [RegId; 3] = [RegId::EBX, RegId::ECX, RegId::EDX];

impl Target for Counting {
    type Class = ();

    fn meet(
        _: (),
        _: (),
    ) -> Option<()> {
        Some(())
    }

    fn any() {}

    fn candidates(_: ()) -> Vec<RegId> {
        ASKED.fetch_add(1, Ordering::Relaxed);
        REGISTERS.to_vec()
    }

    fn view(
        root: RegId,
        _: (),
    ) -> RegId {
        root
    }

    fn root(register: RegId) -> RegId {
        register
    }

    fn reserved(root: RegId) -> bool {
        root == RegId::ESP
    }

    fn caller_saved() -> Vec<RegId> {
        REGISTERS.to_vec()
    }

    fn spill_store(
        _: (),
        register: Reg,
        slot: i32,
    ) -> Option<Inst> {
        Some(Inst::new(Kind::Op("st"), vec![Operand::Frame(slot), Operand::Use(register)]))
    }

    fn spill_load(
        _: (),
        register: Reg,
        slot: i32,
    ) -> Option<Inst> {
        Some(Inst::new(Kind::Op("ld"), vec![Operand::Def(register), Operand::Frame(slot)]))
    }
}

/// `to_lir` called `candidates` once per virtual register for the orders and
/// again per register for the callee-saved list: 1,804 of shellsort's 4,274
/// calls, each a fresh vector (arm64: 3.5 M of 583 M). 60 registers of one
/// class asked 300 times.
#[test]
fn the_converter_asks_for_a_class_once() {
    let mut f = MFunction::<()>::new("many");
    f.classes = vec![(); 60];
    let mut block = Block { id: 0, ..Block::default() };
    for n in 0..60 {
        block.insts.push(Inst::new(Kind::Op("load"), vec![Operand::Def(Reg::Virt(n))]));
        block.insts.push(Inst::new(Kind::Op("store"), vec![Operand::Use(Reg::Virt(n))]));
    }
    let mut ret = Inst::new(Kind::Op("ret"), vec![]);
    ret.terminator = true;
    ret.barrier = true;
    block.insts.push(ret);
    f.blocks = vec![block];
    ASKED.store(0, Ordering::Relaxed);
    let converted = to_lir::<Counting>(&f, Regs(&llrm_x86_m16::REGISTER_INFO)).expect("converts");
    assert_eq!(converted.body.blocks[0].insns.len(), 121, "the premise: the whole function converted");
    let asked = ASKED.load(Ordering::Relaxed);
    // A few stages each ask once per class; none asks per register.
    assert!(asked <= 4, "{asked} calls for one class");
}

/// A branch found its target block by walking the function's blocks from the
/// first: a function of 300 blocks, each ending in a branch, walked about
/// 45,000 blocks (quadratic). The ids are read once.
#[test]
fn branch_targets_are_found_without_a_walk_per_branch() {
    let mut f = MFunction::<()>::new("chain");
    let count = 300u32;
    for id in 0..count {
        let mut block = Block { id, ..Block::default() };
        let mut last = Inst::new(Kind::Op(if id + 1 < count { "branch" } else { "ret" }), vec![]);
        if id + 1 < count {
            last.operands.push(Operand::Block(id + 1));
        }
        last.terminator = true;
        last.barrier = id + 1 == count;
        block.insts.push(last);
        f.blocks.push(block);
    }
    let before = llrm_spike_mf::tolir::block_walks();
    let converted = to_lir::<Counting>(&f, Regs(&llrm_x86_m16::REGISTER_INFO)).expect("converts");
    assert_eq!(converted.body.blocks.len(), count as usize, "the premise: every block converted");
    let walked = llrm_spike_mf::tolir::block_walks() - before;
    assert!(walked <= 2 * count as usize, "{walked} blocks walked for {count} branches");
}
