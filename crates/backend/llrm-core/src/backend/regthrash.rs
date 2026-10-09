//! Port of `qbopt/backend/regthrash.py`: renaming a definition's register so
//! a copy after it is redundant.
//!
//! Open Watcom's `RegThrash` (`bld/cg/c/scthrash.c`), and it is the inverse of
//! coalescing. Registers are already chosen, so it takes
//!
//! ```text
//! OP(...) -> Y        ...        mov Z, Y     (and Y dies there)
//! ```
//!
//! and writes the producer's result into Z instead, which leaves the move
//! copying Z to itself. Between the definition and the move, Y has to stay
//! live and untouched and Z has to be dead, and the rewritten instruction has
//! to still exist: `select.emit` is that check here.

use std::sync::Arc;

use iced_x86::Register;

use crate::backend::liveness;
use crate::backend::peephole::{_lanes, _register_effects, _register_operand, DeadAfter, Lanes, id};
use crate::backend::select;
use crate::model::ir::{Loc, Operation, Reg, Semantics};
use crate::model::lir::{self, Insn, LirBlock, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::IndexMap;

// Enough to settle. Each pass makes at most one rename per block, because a
// rename changes the liveness every later candidate is judged against, and
// recomputing it per candidate costs more than going round again.
pub const ROUNDS: usize = 8;

pub fn thrashed(body: LirBody) -> LirBody {
    // A rename leaves its block's live-in as it was, so what is dead at every
    // block's exit is the same whatever is renamed, and the blocks do not
    // depend on one another: each is renamed to a fixed point of its own.
    let exits = liveness::dead_at_exit(&body);
    let mut changed = false;
    let blocks: Vec<LirBlock> = body
        .blocks
        .iter()
        .map(|block| match _thrash_block(body.bits, block, exits[&block.at]) {
            Some(done) => {
                changed = true;
                done
            }
            None => block.clone(),
        })
        .collect();
    // `after is body`: the same body where nothing was renamed.
    if changed { body.with_blocks(blocks) } else { body }
}

/// Per instruction, the register lanes dead once it has run.
pub fn _dead_after(
    bits: u32,
    block: &LirBlock,
    dead: Lanes,
) -> DeadAfter {
    let by_position = _dead_after_by(bits, &block.insns, dead);
    block.insns.iter().zip(by_position).map(|(one, dead)| (id(one), dead)).collect()
}

/// `_dead_after` by position.
fn _dead_after_by(
    bits: u32,
    insns: &[Arc<Insn>],
    dead: Lanes,
) -> Vec<Lanes> {
    let mut dead = dead;
    let mut out = vec![Lanes::new(); insns.len()];
    for (position, one) in insns.iter().enumerate().rev() {
        out[position] = dead;
        dead = liveness::with_effect(bits, one, |effect| {
            effect.map_or_else(Lanes::new, |effect| effect.dead_before(&dead))
        });
    }
    out
}

/// One rename found: the copy at `position` of what the instruction at `at`
/// made, which is `rewritten` writing the copy's register.
struct Rename {
    position: usize,
    at: usize,
    tied: bool,
    rewritten: Arc<Insn>,
}

/// This block with copies thrashed away, at most ROUNDS of them, or None where
/// none can be.
///
/// Each rename is the first the block now offers, and changes the liveness
/// between the producer and the copy, and before the producer as far back as it
/// moves; after the copy nothing moved. So what is dead after each instruction
/// is worked out again for that stretch alone, and the next copy to try is no
/// earlier than the first instruction whose liveness moved: an earlier one saw
/// the same instructions and the same liveness, and failed.
fn _thrash_block(
    bits: u32,
    block: &LirBlock,
    dead: Lanes,
) -> Option<LirBlock> {
    let mut insns: Vec<Arc<Insn>> = block.insns.to_vec();
    let exit = dead;
    let mut after = _dead_after_by(bits, &insns, exit);
    let (mut from, mut renamed) = (0, 0);
    while renamed < ROUNDS {
        let Some(Rename { position, at, tied, rewritten }) = _first_rename(bits, &insns, &after, from) else { break };
        if !tied {
            // The producer never read Y, so writing Z instead is the whole
            // physical operation. Keep the copy's virtual definition as an
            // anchor: later opaque operands can still name that SSA value.
            insns[at] = rewritten;
            insns[position] = lir::anchor(Arc::clone(&insns[position]));
        } else {
            // Two-address: the producer reads Y as well as writing it, so the
            // renamed form reads Z and Z has to arrive first. Watcom's
            // `PrefixIns` -- the copy is relocated, not removed, and what it
            // buys is Y's range ending here instead of at the old move.
            let copy = insns.remove(position);
            insns.insert(at, copy);
            insns[at + 1] = rewritten;
        }
        let mut dead = after[position];
        for index in (at..=position).rev() {
            after[index] = dead;
            dead = liveness::with_effect(bits, &insns[index], |effect| {
                effect.map_or_else(Lanes::new, |effect| effect.dead_before(&dead))
            });
        }
        // Before the producer, what is dead moves only where the producer read
        // less of Y than the copy does (a shift by a byte reads half of
        // it): the liveness is worked out backwards until it is what it was.
        from = at;
        for index in (0..at).rev() {
            if after[index] == dead {
                break;
            }
            after[index] = dead;
            from = index;
            dead = liveness::with_effect(bits, &insns[index], |effect| {
                effect.map_or_else(Lanes::new, |effect| effect.dead_before(&dead))
            });
        }
        if cfg!(test) || llrm_support::env_set("LLRM_CHECK_THRASH") {
            assert!(
                after == _dead_after_by(bits, &insns, exit),
                "a rename changed what is dead beyond what was worked out again"
            );
        }
        renamed += 1;
    }
    (renamed > 0).then(|| block.with_insns(insns))
}

/// The first copy at or after `from` that can be renamed.
fn _first_rename(
    bits: u32,
    insns: &[Arc<Insn>],
    after: &[Lanes],
    from: usize,
) -> Option<Rename> {
    for position in from..insns.len() {
        let Some((into, out_of)) = _plain_copy(&insns[position]) else {
            continue;
        };
        // Y has to die at the move. That is the whole licence for the
        // rename: if anything later reads Y, its definition still has to
        // land in Y and there is nothing to rewrite.
        if !_lanes(out_of.register).is_subset(&after[position]) {
            continue;
        }
        let Some((at, tied)) = _producer(bits, insns, position, &out_of, &into, after) else {
            continue;
        };
        let Some(rewritten) = _renamed(bits, &insns[at], out_of.register, into.register, !tied) else {
            continue;
        };
        return Some(Rename { position, at, tied, rewritten });
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
fn _producer(
    bits: u32,
    insns: &[Arc<Insn>],
    position: usize,
    out_of: &Reg,
    into: &Reg,
    after: &[Lanes],
) -> Option<(usize, bool)> {
    let (mine, theirs) = (_lanes(out_of.register), _lanes(into.register));
    for at in (0..position).rev() {
        let one = &insns[at];
        let what = one.what.as_ref()?;
        if !one.clobbers.is_empty() || one.symbol == Some(true) || one.group.is_some() {
            return None;
        }
        if liveness::_terminator(Some(what)) || what.op == Operation::Barrier {
            return None;
        }
        let (reads, writes) = _register_effects(bits, one, false, true)?;
        // Z dead here, or the rename overwrites a live value.
        if !theirs.is_subset(&after[at]) {
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
fn _writes(
    what: &Semantics,
    lanes: &Lanes,
) -> bool {
    what.dests
        .iter()
        .any(
            |dest| matches!(
                dest,
                Loc::Reg(dest) if !_lanes(dest.register).is_disjoint(lanes)
            ),
        )
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
fn _renamed(
    bits: u32,
    one: &Insn,
    before: Register,
    after: Register,
    result_only: bool,
) -> Option<Arc<Insn>> {
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
    select::priced_in(bits, &changed, 0, None, false, false, None)?;
    // Encodable is not renamed: an operand the instruction fixes -- `idiv`'s
    // EDX -- emits the same bytes under any name. The decoded effects have to
    // move from `before` to `after`, or the rename exists only in the LIR.
    let renamed = Insn { what: Some(changed), ..one.clone() };
    let (was, now) = (_register_effects(bits, one, false, false), _register_effects(bits, &renamed, false, false));
    let (Some(was), Some(now)) = (was, now) else {
        return None;
    };
    let (mine, theirs) = (_lanes(before), _lanes(after));
    let reads: Lanes =
        if result_only || was.0.is_disjoint(&mine) { was.0.clone() } else { was.0.minus(&mine).or(&theirs) };
    let writes: Lanes = was.1.minus(&mine).or(&theirs);
    if now != (reads, writes) {
        return None;
    }
    Some(Arc::new(renamed))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iced_x86::{Decoder, DecoderOptions, Mnemonic, OpKind, Register};

    use super::{_plain_copy, ROUNDS, thrashed};
    use crate::backend::{select, verify};
    use crate::model::ir::{Imm, Loc, Operation, Reg, Semantics};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::support::hash::HashMap;
    use crate::support::hash::IndexMap;

    const MASK: u64 = 0xFFFF_FFFF;

    fn _reg(register: Register) -> Loc {
        Loc::Reg(Reg { register, width: 4 })
    }

    fn _insn(
        at: i64,
        name: &str,
        operation: Operation,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
    ) -> Insn {
        Insn::new(
            at,
            Some((at, at)),
            Some(Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(operation) }),
            vec![],
            vec![],
        )
    }

    /// What the emitted bytes of `block` leave in the registers.
    fn _run(
        block: &LirBlock,
        state: &HashMap<Register, u64>,
    ) -> HashMap<Register, u64> {
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
                        let dividend =
                            signed(i128::from(state[&Register::EDX]) << 32 | i128::from(state[&Register::EAX]), 64);
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
    fn _body(
        insns: Vec<Insn>,
        read: Register,
    ) -> LirBody {
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
                    _insn(
                        0,
                        "mov",
                        Operation::Move,
                        vec![_reg(divisor)],
                        vec![Loc::Imm(Imm { value: 0xFFF1, width: 4, address: None })],
                    ),
                    _insn(1, "mov", Operation::Move, vec![eax.clone()], vec![edi]),
                    _insn(2, "cdq", Operation::Extend, vec![edx.clone()], vec![eax.clone()]),
                    _insn(
                        3,
                        "idiv",
                        Operation::Divide,
                        vec![eax.clone(), edx.clone()],
                        vec![edx.clone(), eax, _reg(divisor)],
                    ),
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
        // `ecx += edx; edx = ecx` renamed to `edx = ecx; edx += edx` doubles
        // ecx.
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
                    Arc::new(_insn(
                        100 * at,
                        "mov",
                        Operation::Move,
                        vec![ecx.clone()],
                        vec![Loc::Imm(Imm { value: at, width: 4, address: None })],
                    )),
                    Arc::new(_insn(100 * at + 1, "mov", Operation::Move, vec![edx.clone()], vec![ecx.clone()])),
                ];
                LirBlock {
                    succ: vec![if at + 1 == count { 1 } else { at + 2 }],
                    ..LirBlock::new(if at == 0 { 0 } else { at + 1 }, insns)
                }
            })
            .collect();
        blocks.push(tail);
        let body = LirBody::new("thrash", 0, blocks, IndexMap::default(), IndexMap::default());

        let result = thrashed(body);

        let copies =
            result.blocks.iter().flat_map(|block| &block.insns).filter(|one| _plain_copy(one).is_some()).count();
        assert_eq!(copies, 0);
    }

    #[test]
    fn test_removed_copy_keeps_its_virtual_definition() {
        // mdl_draw_tris lost the selector value used by later far-memory reads
        // when regthrash removed its physical copy after allocation.
        let (ax, di) = (_reg(Register::EAX), _reg(Register::EDI));
        let producer = Insn {
            defines: vec![1],
            .._insn(
                0,
                "mov",
                Operation::Move,
                vec![ax.clone()],
                vec![Loc::Imm(Imm { value: 7, width: 4, address: None })],
            )
        };
        let copy = Insn { defines: vec![2], uses: vec![1], .._insn(1, "mov", Operation::Move, vec![di], vec![ax]) };
        let body = _body(vec![producer, copy], Register::EDI);
        let following = body.blocks[1].clone();
        let mut insns = following.insns.to_vec();
        insns[0] = Arc::new(Insn { uses: vec![2], ..(*insns[0]).clone() });
        let body =
            LirBody { blocks: vec![body.blocks[0].clone(), LirBlock { insns: insns.into(), ..following }], ..body };

        let result = thrashed(body);

        assert!(verify::verify(&result, false).is_empty());
        let anchor = result.blocks[0].insns.iter().find(|one| one.defines == [2]).unwrap();
        assert!(anchor.what.as_ref().unwrap().op == Operation::Nothing && anchor.uses == [1]);
    }

    /// The rename of a copy asks what is dead after each instruction of its
    /// block every round, and each ask worked out the instruction's effect
    /// again: a straight run of 1600 statements spent 250 Minstr of 340 in the
    /// rounds, and the cost of a body grew faster than its size while the
    /// rounds did. An instruction's effect is worked out once: those of the
    /// instructions a round changed are new ones.
    #[test]
    fn test_an_instructions_effect_is_worked_out_once_however_many_rounds_ask() {
        let mut insns = Vec::new();
        for at in 0..6_i64 {
            insns.push(_insn(
                at * 3,
                "mov",
                Operation::Move,
                vec![_reg(Register::EAX)],
                vec![Loc::Imm(Imm { value: at, width: 4, address: None })],
            ));
            insns.push(_insn(at * 3 + 1, "mov", Operation::Move, vec![_reg(Register::EDX)], vec![_reg(Register::EAX)]));
            insns.push(_insn(
                at * 3 + 2,
                "add",
                Operation::Binary,
                vec![_reg(Register::EBX)],
                vec![_reg(Register::EBX), _reg(Register::EDX)],
            ));
        }
        let body = _body(insns, Register::EBX);
        let size = body.insns().len();
        let before = crate::backend::liveness::effects_worked_out();
        let after = thrashed(body.clone());
        let worked = crate::backend::liveness::effects_worked_out() - before;
        let changed = after.insns().iter().zip(body.insns()).filter(|(new, old)| !Arc::ptr_eq(new, old)).count();
        assert!(changed >= 4, "premise: several rounds rename ({changed} instructions changed)");
        assert!(
            worked <= size + changed + 2,
            "{worked} effects worked out for {size} instructions and {changed} changed"
        );
    }

    /// Moving a copy ahead of a producer that reads half of what it copies
    /// makes the other half live before it: `shr cx, 8` reads CX's high
    /// byte alone, `mov ax, cx` reads both. What is dead after the instruction
    /// before the producer is not what it was, and the rename that worked
    /// it out for the window between producer and copy alone left the old
    /// answer there.
    #[test]
    fn test_a_copy_moved_ahead_of_a_shift_by_a_byte_changes_what_is_dead_before_it() {
        let word = |register| Loc::Reg(Reg { register, width: 2 });
        let insns = vec![
            _insn(
                0,
                "mov",
                Operation::Move,
                vec![word(Register::CX)],
                vec![Loc::Imm(Imm { value: 0x1234, width: 2, address: None })],
            ),
            _insn(
                1,
                "shr",
                Operation::Binary,
                vec![word(Register::CX)],
                vec![word(Register::CX), Loc::Imm(Imm { value: 8, width: 1, address: None })],
            ),
            _insn(2, "mov", Operation::Move, vec![word(Register::AX)], vec![word(Register::CX)]),
        ];
        let body = _body(insns, Register::EAX);
        let after = thrashed(body.clone());
        // Whether or not it renamed, `thrashed` checked the liveness it kept
        // against working it out whole; a rename is the premise.
        assert!(
            after.insns().iter().zip(body.insns().iter()).any(|(new, old)| !Arc::ptr_eq(new, &old)),
            "premise: the copy was renamed"
        );
    }
}
