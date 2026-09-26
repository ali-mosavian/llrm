//! Port of `qbopt/backend/regthrash.py`: renaming a definition's register so
//! a copy after it is redundant.
//!
//! Open Watcom's `RegThrash` (`bld/cg/c/scthrash.c`), and it is the inverse of
//! coalescing. Registers are already chosen, so it takes
//!
//!     OP(...) -> Y        ...        mov Z, Y     (and Y dies there)
//!
//! and writes the producer's result into Z instead, which leaves the move
//! copying Z to itself. Between the definition and the move, Y has to stay
//! live and untouched and Z has to be dead, and the rewritten instruction has
//! to still exist: `select.emit` is that check here.

use std::sync::Arc;

use iced_x86::Register;

use crate::backend::liveness;
use crate::backend::peephole::{DeadAfter, Lanes, _lanes, _register_effects, _register_operand, id};
use crate::backend::select;
use crate::model::ir::{Loc, Operation, Reg, Semantics};
use crate::model::lir::{self, Insn, LirBlock, LirBody};
use crate::model::passes::LIRTransform;

// Enough to settle. Each pass makes at most one rename per block, because a
// rename changes the liveness every later candidate is judged against, and
// recomputing it per candidate costs more than going round again.
pub const ROUNDS: usize = 8;

pub struct RegThrash;

impl LIRTransform for RegThrash {
    fn class_name(&self) -> &'static str {
        "RegThrash"
    }

    fn name(&self) -> &str {
        "regthrash"
    }

    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        Ok(thrashed(body))
    }
}

pub fn thrashed(body: LirBody) -> LirBody {
    let mut body = body;
    for _round in 0..ROUNDS {
        // `after is body`: `_once` answers None where it returned `body` itself.
        match _once(&body) {
            None => return body,
            Some(after) => body = after,
        }
    }
    body
}

/// `body` with one copy thrashed in each block that has one, or None.
///
/// A rename leaves its block's live-in as it was, so every block is judged
/// against the same exit liveness.
fn _once(body: &LirBody) -> Option<LirBody> {
    let exits = liveness::dead_at_exit(body);
    let mut blocks = body.blocks.clone();
    let mut changed = false;
    for block in &mut blocks {
        if let Some(done) = _block(block, exits[&block.at].clone()) {
            *block = done;
            changed = true;
        }
    }
    changed.then(|| body.with_blocks(blocks))
}

/// Per instruction, the register lanes dead once it has run.
pub fn _dead_after(block: &LirBlock, dead: Lanes) -> DeadAfter {
    let mut dead = dead;
    let mut out = DeadAfter::default();
    for one in block.insns.iter().rev() {
        out.insert(id(one), dead);
        dead = liveness::effect(one).map_or_else(Lanes::new, |effect| effect.dead_before(&dead));
    }
    out
}

/// This block with one copy thrashed away, or None where none can be.
fn _block(block: &LirBlock, dead: Lanes) -> Option<LirBlock> {
    let after = _dead_after(block, dead);
    for (position, one) in block.insns.iter().enumerate() {
        let Some((into, out_of)) = _plain_copy(one) else {
            continue;
        };
        // Y has to die at the move. That is the whole licence for the
        // rename: if anything later reads Y, its definition still has to
        // land in Y and there is nothing to rewrite.
        if !_lanes(out_of.register).is_subset(&after[&id(one)]) {
            continue;
        }
        let Some((at, tied)) = _producer(block, position, &out_of, &into, &after) else {
            continue;
        };
        let Some(rewritten) = _renamed(&block.insns[at], out_of.register, into.register, !tied) else {
            continue;
        };
        if !tied {
            // The producer never read Y, so writing Z instead is the whole
            // physical operation. Keep the copy's virtual definition as an
            // anchor: later opaque operands can still name that SSA value.
            let insns = block
                .insns
                .iter()
                .enumerate()
                .map(|(index, insn)| {
                    if index == at {
                        Arc::clone(&rewritten)
                    } else if index == position {
                        lir::anchor(Arc::clone(insn))
                    } else {
                        Arc::clone(insn)
                    }
                })
                .collect();
            return Some(block.with_insns(insns));
        }
        // Two-address: the producer reads Y as well as writing it, so the
        // renamed form reads Z and Z has to arrive first. Watcom's
        // `PrefixIns` -- the copy is relocated, not removed, and what it
        // buys is Y's range ending here instead of at the old move.
        let mut insns = Vec::new();
        for (one, insn) in block.insns.iter().enumerate() {
            if one == position {
                continue;
            }
            if one == at {
                insns.push(Arc::clone(&block.insns[position]));
                insns.push(Arc::clone(&rewritten));
            } else {
                insns.push(Arc::clone(insn));
            }
        }
        return Some(block.with_insns(insns));
    }
    None
}

/// `(written, read)` where this is a register-to-register move of one width.
fn _plain_copy(one: &Insn) -> Option<(Reg, Reg)> {
    let what = one.what.as_ref()?;
    if !one.clobbers.is_empty() || one.symbol == Some(true) || one.group.is_some() {
        return None;
    }
    if !one.requires.is_empty() || !one.delivers.is_empty() || !one.spread.is_empty() {
        return None;
    }
    match (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice()) {
        (Operation::Move, Some("mov"), [Loc::Reg(into)], [Loc::Reg(out_of)]) => {
            if into.width != out_of.width || into.register == out_of.register {
                return None;
            }
            Some((*into, *out_of))
        }
        _ => None,
    }
}

/// Where `out_of` was defined, and whether that definition reads it too.
///
/// Watcom's backward walk. Every step has to leave Y live and untouched --
/// otherwise the definition found is not the one the move reads -- and has
/// to leave Z dead, or renaming into it destroys a value something else
/// still wants.
fn _producer(block: &LirBlock, position: usize, out_of: &Reg, into: &Reg, after: &DeadAfter) -> Option<(usize, bool)> {
    let (mine, theirs) = (_lanes(out_of.register), _lanes(into.register));
    for at in (0..position).rev() {
        let one = &block.insns[at];
        let what = one.what.as_ref()?;
        if !one.clobbers.is_empty() || one.symbol == Some(true) || one.group.is_some() {
            return None;
        }
        if liveness::_terminator(Some(what)) || what.op == Operation::Barrier {
            return None;
        }
        let (reads, writes) = _register_effects(one, false, true)?;
        // Z dead here, or the rename overwrites a live value.
        if !theirs.is_subset(&after[&id(one)]) {
            return None;
        }
        if what.op == Operation::Move && one.spill_store {
            return None;
        }
        if _writes(what, &mine) {
            // It must own the whole of Y: a partial write leaves lanes
            // belonging to some earlier definition, and renaming this one
            // alone would split the value in two.
            if !mine.is_subset(&writes) {
                return None;
            }
            // And it must not read Z: the rename would merge Z's value into
            // Y's operand, and a tied producer's relocated copy overwrites it.
            if !reads.is_disjoint(&theirs) {
                return None;
            }
            return Some((at, !reads.is_disjoint(&mine)));
        }
        if !reads.is_disjoint(&mine) || !writes.is_disjoint(&theirs) {
            return None;
        }
    }
    None
}

/// Whether this operation names those lanes as its own destination.
fn _writes(what: &Semantics, lanes: &Lanes) -> bool {
    what.dests.iter().any(|dest| matches!(dest, Loc::Reg(dest) if !_lanes(dest.register).is_disjoint(lanes)))
}

/// `one` computing into `after` instead, or None where it cannot.
///
/// `result_only` is Watcom's `ChangeIns(oth,Z,&oth->result)` against its
/// `CantChange(&oth,Y,Z)`: a definition that never read Y only needs its
/// destination moved, and rewriting its sources too would change what it
/// reads.
///
/// The encodability re-check is Watcom's, and it is not a formality: the
/// substitution can name an operand the machine has no form for, and
/// `select.emit` answering None is exactly `FindGenEntry` returning
/// `G_UNKNOWN` there.
fn _renamed(one: &Insn, before: Register, after: Register, result_only: bool) -> Option<Arc<Insn>> {
    let what = one.what.as_ref().expect("a producer has semantics");
    let changed = Semantics {
        dests: what.dests.iter().map(|dest| _register_operand(dest, before, after)).collect(),
        sources: if result_only {
            what.sources.clone()
        } else {
            what.sources.iter().map(|source| _register_operand(source, before, after)).collect()
        },
        ..what.clone()
    };
    if changed == *what {
        return None;
    }
    select::emit(&changed, 0, None, false, false, None)?;
    // Encodable is not renamed: an operand the instruction fixes -- `idiv`'s
    // EDX -- emits the same bytes under any name. The decoded effects have to
    // move from `before` to `after`, or the rename exists only in the LIR.
    let renamed = Insn { what: Some(changed), ..one.clone() };
    let (was, now) = (_register_effects(one, false, false), _register_effects(&renamed, false, false));
    let (Some(was), Some(now)) = (was, now) else {
        return None;
    };
    let (mine, theirs) = (_lanes(before), _lanes(after));
    let reads: Lanes = if result_only || was.0.is_disjoint(&mine) {
        was.0.clone()
    } else {
        was.0.minus(&mine).or(&theirs)
    };
    let writes: Lanes = was.1.minus(&mine).or(&theirs);
    if now != (reads, writes) {
        return None;
    }
    Some(Arc::new(renamed))
}

#[cfg(test)]
mod tests {
    use crate::support::hash::HashMap;
    use std::sync::Arc;

    use iced_x86::{Decoder, DecoderOptions, Mnemonic, OpKind, Register};
    use crate::support::hash::IndexMap;

    use super::{_plain_copy, thrashed, ROUNDS};
    use crate::backend::{select, verify};
    use crate::model::ir::{Imm, Loc, Operation, Reg, Semantics};
    use crate::model::lir::{Insn, LirBlock, LirBody};

    const MASK: u64 = 0xFFFF_FFFF;

    fn _reg(register: Register) -> Loc {
        Loc::Reg(Reg { register, width: 4 })
    }

    fn _insn(at: i64, name: &str, operation: Operation, dests: Vec<Loc>, sources: Vec<Loc>) -> Insn {
        Insn::new(
            at,
            Some((at, at)),
            Some(Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(operation) }),
            vec![],
            vec![],
        )
    }

    /// What the emitted bytes of `block` leave in the registers.
    fn _run(block: &LirBlock, state: &HashMap<Register, u64>) -> HashMap<Register, u64> {
        let mut state = state.clone();
        for one in &block.insns {
            let code = select::emit(one.what.as_ref().unwrap(), 0, None, false, false, None).unwrap().code;
            for insn in &mut Decoder::new(16, &code, DecoderOptions::NONE) {
                let into = insn.op0_register();
                match insn.mnemonic() {
                    Mnemonic::Mov => {
                        let value = if insn.op1_kind() == OpKind::Register {
                            state[&insn.op1_register()]
                        } else {
                            insn.immediate(1)
                        };
                        state.insert(into, value & MASK);
                    }
                    Mnemonic::Add => {
                        let value = (state[&into] + state[&insn.op1_register()]) & MASK;
                        state.insert(into, value);
                    }
                    Mnemonic::Cdq => {
                        let value = if state[&Register::EAX] & 0x8000_0000 != 0 { MASK } else { 0 };
                        state.insert(Register::EDX, value);
                    }
                    Mnemonic::Idiv => {
                        let signed = |value: i128, bits: u32| {
                            if value >> (bits - 1) != 0 { value - (1i128 << bits) } else { value }
                        };
                        let dividend = signed(
                            i128::from(state[&Register::EDX]) << 32 | i128::from(state[&Register::EAX]),
                            64,
                        );
                        let divisor = signed(i128::from(state[&into]), 32);
                        assert!(divisor != 0, "divide by zero");
                        let sign = if (dividend < 0) == (divisor < 0) { 1 } else { -1 };
                        let quotient = dividend.abs() / divisor.abs() * sign;
                        state.insert(Register::EAX, (quotient & i128::from(MASK)) as u64);
                        state.insert(Register::EDX, ((dividend - quotient * divisor) & i128::from(MASK)) as u64);
                    }
                    other => panic!("NotImplementedError: {other:?}"),
                }
            }
        }
        state
    }

    /// `insns`, then a block that reads only `read` and overwrites the rest.
    fn _body(insns: Vec<Insn>, read: Register) -> LirBody {
        let killed = [Register::EAX, Register::EBX, Register::ECX, Register::EDX, Register::ESI, Register::EDI]
            .into_iter()
            .enumerate()
            .map(|(index, one)| {
                Arc::new(_insn(
                    10 + index as i64,
                    "mov",
                    Operation::Move,
                    vec![_reg(one)],
                    vec![Loc::Imm(Imm { value: 0, width: 4, address: None })],
                ))
            });
        let after = LirBlock::new(
            1,
            std::iter::once(Arc::new(_insn(9, "push", Operation::Push, vec![], vec![_reg(read)])))
                .chain(killed)
                .collect(),
        );
        let first = LirBlock { succ: vec![1], ..LirBlock::new(0, insns.into_iter().map(Arc::new).collect()) };
        LirBody::new("thrash", 0, vec![first, after], IndexMap::default(), IndexMap::default())
    }

    #[test]
    fn test_oimad_remainder_is_not_renamed_out_of_edx() {
        // `idiv`'s remainder is fixed in EDX: renaming it to ESI emitted the
        // same `idiv esi` behind a `mov esi,edx` that overwrote the divisor.
        for divisor in [Register::ESI, Register::ECX] {
            let (edi, esi, eax, edx) =
                (_reg(Register::EDI), _reg(Register::ESI), _reg(Register::EAX), _reg(Register::EDX));
            let body = _body(
                vec![
                    _insn(0, "mov", Operation::Move, vec![_reg(divisor)], vec![Loc::Imm(Imm {
                        value: 0xFFF1,
                        width: 4,
                        address: None,
                    })]),
                    _insn(1, "mov", Operation::Move, vec![eax.clone()], vec![edi]),
                    _insn(2, "cdq", Operation::Extend, vec![edx.clone()], vec![eax.clone()]),
                    _insn(3, "idiv", Operation::Divide, vec![eax.clone(), edx.clone()], vec![
                        edx.clone(),
                        eax,
                        _reg(divisor),
                    ]),
                    _insn(4, "mov", Operation::Move, vec![esi], vec![edx]),
                ],
                Register::ESI,
            );
            let mut start: HashMap<Register, u64> =
                [Register::EAX, Register::ECX, Register::EDX, Register::ESI].into_iter().map(|one| (one, 0)).collect();
            start.insert(Register::EDI, 100_000);
            let result = thrashed(body);
            assert_eq!(_run(&result.blocks[0], &start)[&Register::ESI], 100_000 % 0xFFF1, "{divisor:?}");
        }
    }

    #[test]
    fn test_a_rename_does_not_merge_the_producers_other_operand() {
        // `ecx += edx; edx = ecx` renamed to `edx = ecx; edx += edx` doubles ecx.
        let (ecx, edx) = (_reg(Register::ECX), _reg(Register::EDX));
        let body = _body(
            vec![
                _insn(0, "add", Operation::Binary, vec![ecx.clone()], vec![ecx.clone(), edx.clone()]),
                _insn(1, "mov", Operation::Move, vec![edx], vec![ecx]),
            ],
            Register::EDX,
        );
        let result = thrashed(body);
        let start = HashMap::from_iter([(Register::ECX, 5), (Register::EDX, 7)]);
        assert_eq!(_run(&result.blocks[0], &start)[&Register::EDX], 12);
    }

    #[test]
    fn test_every_block_is_thrashed_past_the_round_limit() {
        // One rename per body per round stopped after ROUNDS blocks: deedlines'
        // PLASMA loop kept `mov bx,dx; sub bx,k; shl bx,1` once earlier blocks
        // had used the budget.
        let (ecx, edx) = (_reg(Register::ECX), _reg(Register::EDX));
        let count = ROUNDS as i64 + 1;
        let tail = _body(vec![], Register::EDX).blocks[1].clone();
        let mut blocks: Vec<LirBlock> = (0..count)
            .map(|at| {
                let insns = vec![
                    Arc::new(_insn(100 * at, "mov", Operation::Move, vec![ecx.clone()], vec![Loc::Imm(Imm {
                        value: at,
                        width: 4,
                        address: None,
                    })])),
                    Arc::new(_insn(100 * at + 1, "mov", Operation::Move, vec![edx.clone()], vec![ecx.clone()])),
                ];
                LirBlock { succ: vec![if at + 1 == count { 1 } else { at + 2 }], ..LirBlock::new(if at == 0 { 0 } else { at + 1 }, insns) }
            })
            .collect();
        blocks.push(tail);
        let body = LirBody::new("thrash", 0, blocks, IndexMap::default(), IndexMap::default());

        let result = thrashed(body);

        let copies = result.blocks.iter().flat_map(|block| &block.insns).filter(|one| _plain_copy(one).is_some()).count();
        assert_eq!(copies, 0);
    }

    #[test]
    fn test_removed_copy_keeps_its_virtual_definition() {
        // mdl_draw_tris lost the selector value used by later far-memory reads
        // when regthrash removed its physical copy after allocation.
        let (ax, di) = (_reg(Register::EAX), _reg(Register::EDI));
        let producer = Insn {
            defines: vec![1],
            .._insn(0, "mov", Operation::Move, vec![ax.clone()], vec![Loc::Imm(Imm { value: 7, width: 4, address: None })])
        };
        let copy = Insn { defines: vec![2], uses: vec![1], .._insn(1, "mov", Operation::Move, vec![di], vec![ax]) };
        let body = _body(vec![producer, copy], Register::EDI);
        let following = body.blocks[1].clone();
        let mut insns = following.insns.clone();
        insns[0] = Arc::new(Insn { uses: vec![2], ..(*insns[0]).clone() });
        let body = LirBody { blocks: vec![body.blocks[0].clone(), LirBlock { insns, ..following }], ..body };

        let result = thrashed(body);

        assert!(verify::verify(&result, false).is_empty());
        let anchor = result.blocks[0].insns.iter().find(|one| one.defines == [2]).unwrap();
        assert!(anchor.what.as_ref().unwrap().op == Operation::Nothing && anchor.uses == [1]);
    }
}
