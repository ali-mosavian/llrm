//! Adjacent loads or stores of two registers to neighbouring addresses become
//! one `ldp`/`stp` (`AArch64LoadStoreOptimizer`'s `findMatchingInsn` and
//! `mergePairedInsns`), when nothing between them touches the registers, the
//! base or memory. This version looks at most `WINDOW` instructions ahead;
//! LLVM's `LdStLimit` is 20.

use crate::frame::Lowered;
use crate::mc::inst::*;

const WINDOW: usize = 8;

/// A plain load or store of one register at `[base, #imm]`.
struct Access {
    load: bool,
    mem: Memory,
    base: Gpr,
    imm: i64,
    bytes: i64,
}

fn access(inst: &Inst) -> Option<Access> {
    let (load, mem, addr) = match inst {
        Inst::Load { mem, addr } => (true, *mem, addr),
        Inst::Store { mem, addr } => (false, *mem, addr),
        _ => return None,
    };
    let Addr::Offset { base, imm } = addr else { return None };
    let (bytes, signed) = match mem {
        Memory::Int { bytes, sign, .. } => (i64::from(bytes), sign),
        Memory::Fp { bytes, .. } => (i64::from(bytes), false),
    };
    // Only the 32-, 64- and 128-bit forms pair, and only at a multiple of the
    // size.
    if signed || !matches!(bytes, 4 | 8 | 16) || imm % bytes != 0 {
        return None;
    }
    if let Memory::Int { width, bytes, .. } = mem
        && (bytes == 4) != (width == Width::W)
    {
        return None;
    }
    Some(Access { load: load, mem, base: *base, imm: *imm, bytes })
}

fn register(mem: &Memory) -> (bool, u32) {
    match mem {
        Memory::Int { reg, .. } => (false, reg.0),
        Memory::Fp { reg, .. } => (true, reg.0),
    }
}

fn defines_or_uses(
    inst: &Inst,
    float: bool,
    number: u32,
    defs_only: bool,
) -> bool {
    let (defs, uses) = inst.defs_and_uses();
    let hit = |one: &crate::mc::visit::Reg| match one {
        crate::mc::visit::Reg::Gpr(gpr) => !float && gpr.0 == number,
        crate::mc::visit::Reg::Fpr(fpr) => float && fpr.0 == number,
    };
    defs.iter().any(hit) || (!defs_only && uses.iter().any(hit))
}

fn is_memory(inst: &Inst) -> bool {
    matches!(
        inst,
        Inst::Load { .. } | Inst::Store { .. } | Inst::LoadPair { .. } | Inst::StorePair { .. }
    )
}

fn is_store(inst: &Inst) -> bool {
    matches!(inst, Inst::Store { .. } | Inst::StorePair { .. })
}

fn leaves_the_block(inst: &Inst) -> bool {
    matches!(
        inst,
        Inst::Branch { .. }
            | Inst::BranchCond { .. }
            | Inst::CompareBranch { .. }
            | Inst::TestBranch { .. }
            | Inst::BranchReg { .. }
            | Inst::Ret { .. }
            | Inst::Svc { .. }
            | Inst::Brk { .. }
    )
}

fn pairs_in(insts: Vec<Inst>) -> Vec<Inst> {
    let mut insts: Vec<Option<Inst>> = insts.into_iter().map(Some).collect();
    for i in 0..insts.len() {
        let Some(first) = insts[i].as_ref().and_then(access) else { continue };
        let limit = (i + 1 + WINDOW).min(insts.len());
        for j in i + 1..limit {
            let Some(inst) = insts[j].as_ref() else { continue };
            if let Some(second) = access(inst)
                && let Some(pair) = pair_of(&first, &second)
                // The second access moves up to the first: what lies between
                // must not care. The registers it carries and the base are
                // checked at each step.
                && (-64..64).contains(&(pair.low.imm / pair.low.bytes))
                && insts[i + 1..j].iter().flatten().all(|between| {
                    let (float, number) = register(&second.mem);
                    // A load moves up over them: they must not meet its
                    // register. A store moves up: they must not change the
                    // register it stores.
                    !defines_or_uses(between, float, number, !first.load)
                })
            {
                insts[i] = Some(pair.inst);
                insts[j] = None;
                break;
            }
            // Passing `inst` is allowed only when it neither is control flow
            // nor changes the base, a store passes nothing in
            // memory, and a load passes only loads.
            let stop = leaves_the_block(inst)
                || defines_or_uses(inst, false, first.base.0, true)
                || (is_memory(inst) && (is_store(inst) || !first.load))
                || (is_memory(inst) && first.load && access(inst).is_none());
            if stop {
                break;
            }
        }
    }
    insts.into_iter().flatten().collect()
}

struct Pair {
    low: Access,
    inst: Inst,
}

/// The pair `first` and `second` make, if they are neighbours of one kind.
fn pair_of(
    first: &Access,
    second: &Access,
) -> Option<Pair> {
    let (first_float, first_number) = register(&first.mem);
    let (second_float, second_number) = register(&second.mem);
    let same_kind = second.load == first.load
        && second.base == first.base
        && second.bytes == first.bytes
        && std::mem::discriminant(&second.mem) == std::mem::discriminant(&first.mem)
        && (second.imm == first.imm + first.bytes || second.imm == first.imm - first.bytes)
        && (second_float, second_number) != (first_float, first_number);
    // A load into the base register would change the address of the other.
    let base_loaded = first.load
        && ((!second_float && second_number == first.base.0) || (!first_float && first_number == first.base.0));
    if !same_kind || base_loaded {
        return None;
    }
    let (low, high) = if second.imm > first.imm { (first, second) } else { (second, first) };
    let addr = Addr::Offset { base: low.base, imm: low.imm };
    let inst = if first.load {
        Inst::LoadPair { first: low.mem, second: high.mem, addr }
    } else {
        Inst::StorePair { first: low.mem, second: high.mem, addr }
    };
    Some(Pair { low: Access { load: low.load, mem: low.mem, base: low.base, imm: low.imm, bytes: low.bytes }, inst })
}

/// `blocks` with their adjacent accesses paired.
pub fn fold(blocks: Vec<Lowered>) -> Vec<Lowered> {
    blocks.into_iter().map(|block| Lowered { label: block.label, insts: pairs_in(block.insts) }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(
        reg: u32,
        base: u32,
        imm: i64,
    ) -> Inst {
        Inst::Load {
            mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg: Gpr(reg) },
            addr: Addr::Offset { base: Gpr(base), imm },
        }
    }

    fn store(
        reg: u32,
        base: u32,
        imm: i64,
    ) -> Inst {
        Inst::Store {
            mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg: Gpr(reg) },
            addr: Addr::Offset { base: Gpr(base), imm },
        }
    }

    fn folded(insts: Vec<Inst>) -> Vec<Inst> {
        pairs_in(insts)
    }

    /// Two loads of neighbouring doublewords were two instructions.
    #[test]
    fn neighbouring_loads_become_a_pair() {
        let out = folded(vec![load(1, 0, 8), load(2, 0, 0)]);
        assert_eq!(out.len(), 1, "{out:?}");
        assert!(matches!(
            &out[0],
            Inst::LoadPair {
                first: Memory::Int { reg: Gpr(2), .. },
                second: Memory::Int { reg: Gpr(1), .. },
                addr: Addr::Offset { imm: 0, .. }
            }
        ));
    }

    /// `ldp x16, x15, [sp]` over `sub w15, w16, #1`: the second load moved
    /// above an instruction that writes its register, and the fib recursion
    /// printed `0 1 1 4 5 12` instead of the Fibonacci numbers.
    #[test]
    fn a_load_does_not_move_over_a_write_of_its_register() {
        let sub = Inst::AddSubImm {
            op: AddSub { subtract: true, set_flags: false },
            width: Width::W,
            rd: Gpr(15),
            rn: Gpr(16),
            imm12: 1,
            shift12: false,
        };
        let out = folded(vec![load(16, 31, 0), sub, load(15, 31, 8)]);
        assert_eq!(out.len(), 3, "{out:?}");
    }

    #[test]
    fn a_store_between_stops_a_load_pair() {
        let out = folded(vec![load(1, 0, 0), store(3, 4, 0), load(2, 0, 8)]);
        assert_eq!(out.len(), 3, "{out:?}");
    }

    /// `ldp x0, x1, [x0]` would reload its own base before the second read.
    #[test]
    fn a_load_into_the_base_is_not_paired() {
        let out = folded(vec![load(0, 0, 0), load(1, 0, 8)]);
        assert_eq!(out.len(), 2, "{out:?}");
    }

    #[test]
    fn neighbouring_stores_become_a_pair_past_an_arithmetic_instruction() {
        let add = Inst::AddSubImm {
            op: AddSub { subtract: false, set_flags: false },
            width: Width::X,
            rd: Gpr(9),
            rn: Gpr(9),
            imm12: 1,
            shift12: false,
        };
        let out = folded(vec![store(1, 0, 0), add, store(2, 0, 8)]);
        assert_eq!(out.len(), 2, "{out:?}");
    }
}
