//! A returning tail copied into a predecessor that branches elsewhere first
//! stays a block of its own.

use llrm_z80::compile::tail_duplicate_returns;
use llrm_z80::mfunc::{Block, Class, Inst, Kind, MFunction, Operand, Phi, Reg};

fn terminator(
    name: &'static str,
    target: Option<u32>,
    barrier: bool,
) -> Inst {
    let mut inst = Inst::new(Kind::Op(name), target.map(|id| vec![Operand::Block(id)]).unwrap_or_default());
    inst.terminator = true;
    inst.barrier = barrier;
    inst
}

/// `jp nz, 2; jp 1` with block 1 a call and `ret` (`sibling_call_callees`'
/// `g`): the copy went after the `jp nz` in the same block, a branch in the
/// middle that branch folding cannot see (its guard fires on three tests).
#[test]
fn a_returning_tail_copied_after_a_conditional_branch_is_a_block_of_its_own() {
    let call = Inst::new(Kind::Op("CALL"), vec![Operand::Symbol("h".to_owned(), 0)]);
    let mut f = MFunction::new("g");
    f.blocks = vec![
        Block {
            id: 0,
            insts: vec![terminator("JPNZ", Some(2), false), terminator("JP", Some(1), true)],
            phis: Vec::new(),
        },
        Block { id: 1, insts: vec![call, terminator("RET", None, true)], phis: Vec::new() },
        Block { id: 2, insts: vec![terminator("RET", None, true)], phis: Vec::new() },
    ];
    tail_duplicate_returns(&mut f);
    for block in &f.blocks {
        let first = block.insts.iter().position(|inst| inst.terminator);
        let after = first.is_some_and(|at| block.insts[at..].iter().any(|inst| !inst.terminator));
        assert!(!after, "block {} has an instruction after a branch: {:?}", block.id, block.insts);
    }
    let kinds: Vec<Vec<Kind>> =
        f.blocks.iter().map(|block| block.insts.iter().map(|inst| inst.kind).collect()).collect();
    assert!(kinds.iter().any(|block| block == &[Kind::Op("CALL"), Kind::Op("RET")]), "the copy is gone: {kinds:?}");
}

fn defs(block: &Block) -> Vec<Reg> {
    block
        .insts
        .iter()
        .flat_map(|inst| inst.operands.iter())
        .filter_map(|operand| if let Operand::Def(reg @ Reg::Virt(_)) = operand { Some(*reg) } else { None })
        .collect()
}

/// `switch_peeled_table` -Os returned `i32` constants from four cases through
/// one block of four copies and `ret`: the cap counted the copies, so the block
/// stayed (`ld bc,#3; ld de,#0; jr; ex de,hl; ld e,c; ld d,b; ret`, 132 B
/// against 122 B). A tail of copies and a `ret` is copied into its
/// predecessors, which then each hold their own registers: one register defined
/// in both held one range for both paths.
#[test]
fn a_tail_of_copies_and_a_ret_is_copied_with_its_own_registers() {
    let load = |reg, value| Inst::new(Kind::Op("LD16ri"), vec![Operand::Def(Reg::Virt(reg)), Operand::Imm(value)]);
    let mut f = MFunction::new("pick");
    f.classes = vec![Class::Gr16; 8];
    let copies = (4..8).map(|n| Inst::copy(Reg::Virt(n), Reg::Virt(if n == 4 { 0 } else { n - 1 })));
    f.blocks = vec![
        Block {
            id: 0,
            insts: vec![terminator("JPNZ", Some(2), false), terminator("JP", Some(1), true)],
            phis: Vec::new(),
        },
        Block { id: 1, insts: vec![load(1, 3), terminator("JP", Some(3), true)], phis: Vec::new() },
        Block { id: 2, insts: vec![load(2, 4), terminator("JP", Some(3), true)], phis: Vec::new() },
        Block {
            id: 3,
            insts: copies.chain([terminator("RET", None, true)]).collect(),
            phis: vec![Phi { result: Reg::Virt(0), incoming: vec![(1, Reg::Virt(1)), (2, Reg::Virt(2))] }],
        },
    ];
    tail_duplicate_returns(&mut f);
    assert_eq!(f.blocks.len(), 3, "the tail was not copied: {:?}", f.blocks);
    for block in &f.blocks[1..] {
        assert_eq!(block.insts.last().map(|inst| inst.kind), Some(Kind::Op("RET")), "{:?}", block.insts);
    }
    let (one, two) = (defs(&f.blocks[1]), defs(&f.blocks[2]));
    assert!(one.iter().all(|reg| !two.contains(reg)), "both copies define {one:?} / {two:?}");
}

/// The same shape through the driver: a constant `i32` from one path and a
/// sign-extended `i16` from the other came back as `ld bc,#0; ld de,#3; ex
/// de,hl; ld e,c; ld d,b; ret` (the constant built in the wrong pair and
/// swapped).
#[test]
fn a_constant_i32_result_is_built_in_hl_and_de() {
    use llrm_z80::compile::text;
    use llrm_z80::driver::{Level, Request};
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"
define i32 @f(i16 %a, i16 %b) {
  %c = icmp eq i16 %a, 5
  br i1 %c, label %x, label %y
x:
  ret i32 3
y:
  %s = sext i16 %b to i32
  ret i32 %s
}
";
    let out = text(ir, "f", 0, &Request::new(Level::Os)).expect("compiles");
    assert!(out.contains("ld hl, #3") && !out.contains("ld bc, #"), "{out}");
}
