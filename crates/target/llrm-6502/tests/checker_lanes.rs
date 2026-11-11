//! `LLRM_6502_CHECK`'s liveness follows a physical pair's bytes.

use llrm_6502::alloc::{Class, Mos};
use llrm_6502::reg;
use llrm_lir::registers::RegId;
use llrm_spike_mf::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
use llrm_spike_mf::{Assignment, GraphColoring};

fn def(r: RegId) -> Operand {
    Operand::Def(Reg::Phys(r))
}

fn used(r: RegId) -> Operand {
    Operand::Use(Reg::Phys(r))
}

/// A byte in `rc3`, dead before `rc2` and `rc3` are written, then the pair
/// `rs1` (`rc2:rc3`) read. The pair was a node nothing defined, so it was live
/// from the entry and "overlapped" the byte: every `hand.c` (a pointer built by
/// `UNPACK`/`PACK` lane copies, read through `rs1`) failed the check with
/// "live together in overlapping registers" though its code is right.
#[test]
fn a_pair_written_by_its_bytes_is_not_live_from_the_entry() {
    let mut function = MFunction::<Class>::new("f");
    let byte = Operand::Def(Reg::Virt(0));
    let read = Operand::Use(Reg::Virt(0));
    function.classes = vec![Class::ANY];
    function.blocks = vec![Block {
        id: 0,
        insts: vec![
            Inst::new(Kind::Copy, vec![byte, used(reg::A)]),
            Inst::new(Kind::Copy, vec![def(reg::A), read]),
            Inst::new(Kind::Copy, vec![def(reg::RC2), used(reg::A)]),
            Inst::new(Kind::Copy, vec![def(reg::RC3), used(reg::A)]),
            Inst::new(Kind::Op("STPY"), vec![used(reg::A), used(reg::RS1), Operand::Imm(0)]),
            Inst::new(Kind::Op("RTS"), vec![]),
        ],
        phis: Vec::new(),
    }];
    let checked = GraphColoring::<Mos<false>>::by_frequency(false).check(&function, &Assignment(vec![reg::RC3]));
    assert_eq!(checked, Ok(()));
}
