//! A two-register target and block helpers the tests share.

#![allow(dead_code)]

use llrm_lir::registers::RegId;
use llrm_spike_mf::regalloc::Target;
use llrm_spike_mf::{Inst, Jump, Kind, Operand, Reg};

pub struct Two;

impl Target for Two {
    type Class = ();

    fn meet(
        _: (),
        _: (),
    ) -> Option<()> {
        Some(())
    }

    fn any() {}

    fn candidates(_: ()) -> Vec<RegId> {
        vec![RegId::from_raw(0), RegId::from_raw(1)]
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

    fn reserved(_: RegId) -> bool {
        false
    }

    fn caller_saved() -> Vec<RegId> {
        vec![]
    }
}

impl Jump for Two {
    fn jump(block: u32) -> Inst {
        let mut inst = Inst::new(Kind::Op("jump"), vec![Operand::Block(block)]);
        inst.terminator = true;
        inst.barrier = true;
        inst
    }

    fn reverse(branch: &Inst) -> Option<Inst> {
        let Kind::Op(name) = branch.kind else { return None };
        let opposite = match name {
            "branch" => "branch_not",
            "branch_not" => "branch",
            _ => return None,
        };
        Some(Inst { kind: Kind::Op(opposite), ..branch.clone() })
    }
}

pub fn v(n: u32) -> Reg {
    Reg::Virt(n)
}

pub fn op(
    name: &'static str,
    operands: Vec<Operand>,
) -> Inst {
    Inst::new(Kind::Op(name), operands)
}

pub fn branch(to: u32) -> Inst {
    let mut inst = op("branch", vec![Operand::Block(to)]);
    inst.terminator = true;
    inst
}
