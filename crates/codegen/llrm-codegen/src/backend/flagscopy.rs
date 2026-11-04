//! Flags kept across an instruction that clobbers them: `X86FlagsCopyLowering`.
//! Translated from
//! [X86FlagsCopyLowering.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp),
//! function by function, in the pin's order.
//!
//! # How it works
//! A copy of the flags is the pin's two `COPY`s as two pseudo instructions:
//! `v := flags_copy` (`%1 = COPY $eflags`) and `flags_restore v`
//! (`$eflags = COPY %1`). For each restore, in reverse post-order, the
//! conditions the code after it reads are saved as bytes by `setcc` at the
//! copy (`promote_cond_to_reg`, [L744]) and each reader is rewritten:
//! a `setcc` takes the saved byte ([`rewriteSetCC`][L785]),
//! `adc`/`sbb`/`rcl`/`rcr` rebuild the carry with `add byte, 255`
//! ([`rewriteArithmetic`][L824]), any other reader (`jcc`, `cmov`) is preceded
//! by `test byte, byte` and reads `ne`, or `e` of the opposite condition's byte
//! ([`rewriteMI`][L899], [`getCondOrInverseInReg`][L761],
//! [`insertTest`][L775]). A reader after the first jump of a block needs the
//! block split ([`splitBlock`][L149]).
//!
//! Pin functions in file order: `isArithmeticOp` ([L144]) is `reads::Arith`;
//! `splitBlock` ([L149]) is `split_block`; `getClobberType` ([L251]) only
//! serves the NF variants, which the target lacks (`hasNF` is APX: N/A);
//! `runOnMachineFunction` ([L263]) is `lowered` and `lower_copy`;
//! `collectCondsInRegs` ([L721]), `promoteCondToReg` ([L744]),
//! `getCondOrInverseInReg` ([L761]), `insertTest` ([L775]), `rewriteSetCC`
//! ([L785]), `rewriteArithmetic` ([L824]), `getImplicitCondFromMI` ([L855],
//! the x87 `fcmov`s: LIR has none before allocation, N/A) and `rewriteMI`
//! ([L899]) keep their names in snake case.
//!
//! [L144]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L144
//! [L149]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L149
//! [L744]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L744
//! [L761]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L761
//! [L775]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L775
//! [L785]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L785
//! [L824]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L824
//! [L899]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L899
//!
//! Cross-block lowering follows the pin: the copy may be in another block than
//! its restore, the test position is hoisted up the dominator tree while the
//! flags live in and nothing clobbers them (`HasEFLAGSClobber`,
//! `HasEFLAGSClobberPath`), the flags live into successors are rewritten
//! where they are read (the copy's block must dominate them; else the pin's
//! fatal error, here an `Err`), and a second `jcc` on the flags goes to a block
//! split off ([`splitBlock`][L149]) with its edges, probabilities and phis.
//! The NF variants (`hasNF` is APX) and the x87 `fcmov` forms have no
//! counterpart: N/A.
//!
//! DEVIATE R2 (no number yet): no isel producer of `flags_copy`/`flags_restore`
//! exists, so tests hand-build LIR; owner w-5, review 2026-11-19, exit an isel
//! producer.
//!
//! # Example
//! ```lir
//! v1 := flags_copy
//! v2 := add v3, 0x1
//! flags_restore v1
//! jl
//! ```
//! becomes
//! ```lir
//! v4 := setl
//! v2 := add v3, 0x1
//! test v4, v4
//! jne
//! ```
//!
//! # Runs
//! After `FixupSetCC`, before the float phases. Every level (the pin's pass is
//! not gated), so it is not in `UNOPTIMIZED`.

use std::sync::Arc;

use llrm_support::hash::HashMap;

use crate::backend::liveness;
use crate::liveness::cfg::{Cfg, DomTree};
use crate::model::ir::{Held, Imm, Loc, Operation, Semantics};
use crate::model::lir::{Block, BlockOdds, Insn, LirBlock, LirBody};
use crate::model::passes::LIRTransform;

pub struct FlagsCopy;

impl LIRTransform for FlagsCopy {
    fn class_name(&self) -> &'static str {
        "FlagsCopy"
    }

    fn name(&self) -> &str {
        "flagscopy"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        lowered(&body)
    }
}

/// The condition codes and their opposites (`GetOppositeBranchCondition`).
const OPPOSITES: [(&str, &str); 8] =
    [("e", "ne"), ("l", "ge"), ("le", "g"), ("b", "ae"), ("be", "a"), ("s", "ns"), ("o", "no"), ("p", "np")];

/// Other spellings of the same code.
const ALIASES: [(&str, &str); 14] = [
    ("z", "e"),
    ("nz", "ne"),
    ("c", "b"),
    ("nc", "ae"),
    ("nae", "b"),
    ("nb", "ae"),
    ("na", "be"),
    ("nbe", "a"),
    ("nl", "ge"),
    ("nge", "l"),
    ("ng", "le"),
    ("nle", "g"),
    ("pe", "p"),
    ("po", "np"),
];

/// A condition code by its canonical name.
fn cond(name: &str) -> Option<&'static str> {
    let name = ALIASES.iter().find(|(from, _)| *from == name).map_or(name, |(_, to)| to);
    OPPOSITES.iter().flat_map(|&(a, b)| [a, b]).find(|one| *one == name)
}

/// `GetOppositeBranchCondition`.
fn opposite(cond: &str) -> &'static str {
    OPPOSITES
        .iter()
        .find_map(|&(a, b)| {
            if a == cond {
                Some(b)
            } else if b == cond {
                Some(a)
            } else {
                None
            }
        })
        .expect("a condition code")
}

/// `cmovCC` as `cmov!CC`: the move that takes the other source on the
/// opposite condition (`X86InstrInfo::commuteInstruction`, CMOV).
pub(crate) fn opposite_cmov(name: &str) -> Option<String> {
    let code = cond(name.strip_prefix("cmov")?)?;
    Some(format!("cmov{}", opposite(code)))
}

/// What an instruction does with the flags it reads.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Reads {
    /// `jcc`: a conditional branch (`getCondFromBranch`).
    Jump(&'static str),
    /// `setcc` into a register, or to memory if `store`.
    Set(&'static str, bool),
    /// Any other instruction with a condition operand (`cmovcc`).
    Cond(&'static str),
    /// `adc`, `sbb`, `rcl`, `rcr`, `setb_c`: the carry (`isArithmeticOp`).
    Arith,
    /// `%x = COPY $eflags`.
    Copy,
}

fn name_of(one: &Insn) -> Option<&str> {
    one.what.as_ref()?.name.as_deref()
}

fn reads(one: &Insn) -> Option<Reads> {
    let what = one.what.as_ref()?;
    let name = what.name.as_deref()?;
    match name {
        "flags_copy" => return Some(Reads::Copy),
        "adc" | "sbb" | "rcl" | "rcr" | "setb_c" => return Some(Reads::Arith),
        _ => {}
    }
    if let Some(code) = name.strip_prefix('j').filter(|_| what.op == Operation::Branch).and_then(cond) {
        return Some(Reads::Jump(code));
    }
    if let Some(code) = name.strip_prefix("set").and_then(cond) {
        return Some(Reads::Set(code, matches!(what.dests.as_slice(), [Loc::Mem(_)])));
    }
    name.strip_prefix("cmov").and_then(cond).map(Reads::Cond)
}

/// Whether the instruction modifies the flags (`modifiesRegister(EFLAGS)`): an
/// unknown effect might.
fn modifies_flags(
    one: &Insn,
    regs: llrm_lir::registers::Regs,
    bits: u32,
) -> bool {
    if name_of(one) == Some("flags_restore") {
        return true;
    }
    // A jump, a `setcc`, a `cmovcc` and the copy out only read.
    if matches!(
        reads(one),
        Some(Reads::Jump(_) | Reads::Set(..) | Reads::Cond(_) | Reads::Copy)
    ) {
        return false;
    }
    liveness::effect(regs, bits, one)
        .is_none_or(|e| e.writes.iter().any(|&(r, _)| r == llrm_lir::registers::RegId::None))
}

fn make(
    at: i64,
    name: &str,
    op: Operation,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
    defines: Vec<u32>,
    uses: Vec<u32>,
) -> Arc<Insn> {
    let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) };
    Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses))
}

fn byte(value: u32) -> Loc {
    Loc::Held(Held { value, width: 1 })
}

/// The bytes already holding a condition, by code (`CondRegArray`).
type CondRegs = HashMap<&'static str, u32>;

/// Where `test_pos` (the copy) is among the marks, and the reader being
/// rewritten.
const TEST: usize = 0;
const AT: usize = 1;
const RESTORE: usize = 2;

/// The function while the copies are lowered: the blocks' instructions apart
/// from the blocks, and positions (`marks`) that follow the insertions and
/// removals made before them.
struct Work {
    regs: llrm_lir::registers::Regs,
    bits: u32,
    next: u32,
    renames: HashMap<u32, u32>,
    shell: Vec<Block>,
    insns: Vec<Vec<Arc<Insn>>>,
    marks: Vec<(usize, usize)>,
    /// The blocks made by `split_block`, after the block each was split from.
    children: Vec<Vec<usize>>,
    odds: BlockOdds,
    /// The dominator tree of the blocks the pass started with (the pin builds
    /// its `MachineDominatorTree` once and does not update it).
    dom: DomTree,
    first: usize,
}

impl Work {
    fn fresh(&mut self) -> u32 {
        self.next += 1;
        self.next - 1
    }

    fn insert(
        &mut self,
        block: usize,
        pos: usize,
        one: Arc<Insn>,
    ) {
        self.insns[block].insert(pos, one);
        for mark in &mut self.marks {
            if mark.0 == block && mark.1 >= pos {
                mark.1 += 1;
            }
        }
    }

    fn remove(
        &mut self,
        block: usize,
        pos: usize,
    ) {
        self.insns[block].remove(pos);
        for mark in &mut self.marks {
            if mark.0 == block && mark.1 > pos {
                mark.1 -= 1;
            }
        }
    }

    fn modifies(
        &self,
        one: &Insn,
    ) -> bool {
        modifies_flags(one, self.regs, self.bits)
    }

    /// The edges of the blocks as the dominator tree and the walk read them.
    fn cfg(&self) -> Cfg {
        let number: HashMap<i64, usize> = self.shell.iter().enumerate().map(|(n, block)| (block.at, n)).collect();
        let succs: Vec<Vec<usize>> = self
            .shell
            .iter()
            .map(|block| block.succ.iter().filter_map(|at| number.get(at).copied()).collect())
            .collect();
        let mut preds = vec![Vec::new(); succs.len()];
        for (block, out) in succs.iter().enumerate() {
            for &to in out {
                if !preds[to].contains(&block) {
                    preds[to].push(block);
                }
            }
        }
        Cfg { preds, succs, entry: 0 }
    }

    /// Whether the flags are read in each block before they are written, from
    /// its entry on (`isLiveIn(EFLAGS)`).
    fn flags_live_in(&self) -> Vec<bool> {
        let cfg = self.cfg();
        let n = self.shell.len();
        let mut live = vec![false; n];
        let mut changed = true;
        while changed {
            changed = false;
            for block in 0..n {
                let mut result = None;
                for one in &self.insns[block] {
                    if reads(one).is_some() {
                        result = Some(true);
                        break;
                    }
                    if self.modifies(one) {
                        result = Some(false);
                        break;
                    }
                }
                let now = result.unwrap_or_else(|| cfg.succs[block].iter().any(|&s| live[s]));
                if now != live[block] {
                    live[block] = now;
                    changed = true;
                }
            }
        }
        live
    }
}

/// The index of the first terminator of a block: where a hoisted test goes.
fn first_terminator(insns: &[Arc<Insn>]) -> usize {
    let is_terminator = |one: &Arc<Insn>| {
        one.what
            .as_ref()
            .is_some_and(
                |what| matches!(
                    what.op,
                    Operation::Branch | Operation::Jump | Operation::Return
                ),
            )
    };
    insns.len() - insns.iter().rev().take_while(|one| is_terminator(one)).count()
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L721
/// `collectCondsInRegs`: the conditions `setcc`ed into registers before the
/// test position and after the last instruction that defines the flags.
fn collect_conds_in_regs(w: &Work) -> CondRegs {
    let (block, test_pos) = w.marks[TEST];
    let mut conds = CondRegs::default();
    for one in w.insns[block][..test_pos].iter().rev() {
        if let Some(Reads::Set(code, false)) = reads(one)
            && let [Loc::Held(into)] = one.what.as_ref().expect("semantics").dests.as_slice()
        {
            conds.insert(code, into.value);
        }
        // The first definition of the flags: before it the state would be
        // wrong.
        if w.modifies(one) {
            break;
        }
    }
    conds
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L744
/// `promoteCondToReg`: a byte holding `cond`, `setcc`ed before the test
/// position.
fn promote_cond_to_reg(
    w: &mut Work,
    cond: &str,
) -> u32 {
    let value = w.fresh();
    let (block, test_pos) = w.marks[TEST];
    let at = w.insns[block][test_pos].at;
    w.insert(
        block,
        test_pos,
        make(at, &format!("set{cond}"), Operation::Unary, vec![byte(value)], vec![], vec![value], vec![]),
    );
    value
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L761
/// `getCondOrInverseInReg`: the byte of `cond`, else the byte of its opposite
/// and `true`; neither: `cond` is saved.
fn get_cond_or_inverse_in_reg(
    w: &mut Work,
    cond: &'static str,
    conds: &mut CondRegs,
) -> (u32, bool) {
    if !conds.contains_key(cond) && !conds.contains_key(opposite(cond)) {
        let value = promote_cond_to_reg(w, cond);
        conds.insert(cond, value);
    }
    match conds.get(cond) {
        Some(&value) => (value, false),
        None => (conds[opposite(cond)], true),
    }
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L775
/// `insertTest`: `test reg, reg` before the reader.
fn insert_test(
    w: &mut Work,
    value: u32,
) {
    let (block, at) = w.marks[AT];
    let place = w.insns[block][at].at;
    w.insert(
        block,
        at,
        make(place, "test", Operation::Compare, vec![], vec![byte(value), byte(value)], vec![], vec![value]),
    );
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L785
/// `rewriteSetCC`: the `setcc` takes the byte of its condition; the opposite
/// is not used (it would need an analysis of the byte's readers). To memory,
/// it becomes a byte store of the saved byte.
fn rewrite_set_cc(
    w: &mut Work,
    conds: &mut CondRegs,
    code: &'static str,
    store: bool,
) {
    let saved = match conds.get(code) {
        Some(&value) => value,
        None => {
            let value = promote_cond_to_reg(w, code);
            conds.insert(code, value);
            value
        }
    };
    let (block, at) = w.marks[AT];
    let one = Arc::clone(&w.insns[block][at]);
    let what = one.what.as_ref().expect("semantics");
    if !store {
        let [Loc::Held(old)] = what.dests.as_slice() else { return };
        w.renames.insert(old.value, saved);
        w.remove(block, at);
        return;
    }
    let mut store_what = Semantics { name: Some("mov".into()), ..Semantics::new(Operation::Move) };
    store_what.dests = what.dests.clone();
    store_what.sources = vec![byte(saved)];
    let mut made = (*one).clone();
    made.what = Some(store_what);
    made.defines = vec![];
    made.uses.push(saved);
    w.insns[block][at] = Arc::new(made);
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L824
/// `rewriteArithmetic`: a reader of the carry gets it back by `add byte, 255`
/// of the saved `b` byte.
fn rewrite_arithmetic(
    w: &mut Work,
    conds: &mut CondRegs,
) {
    let saved = match conds.get("b") {
        Some(&value) => value,
        None => {
            let value = promote_cond_to_reg(w, "b");
            conds.insert("b", value);
            value
        }
    };
    let tmp = w.fresh();
    let (block, at) = w.marks[AT];
    let place = w.insns[block][at].at;
    w.insert(
        block,
        at,
        make(
            place,
            "add",
            Operation::Binary,
            vec![byte(tmp)],
            vec![byte(saved), Loc::Imm(Imm { value: 255, width: 1, address: None })],
            vec![tmp],
            vec![saved],
        ),
    );
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L899
/// `rewriteMI`: a reader with a condition tests the saved byte first and reads
/// `ne` (`e` for the opposite condition's byte).
fn rewrite_mi(
    w: &mut Work,
    conds: &mut CondRegs,
    code: &'static str,
) {
    let (saved, inverted) = get_cond_or_inverse_in_reg(w, code, conds);
    insert_test(w, saved);
    let (block, at) = w.marks[AT];
    let mut changed = (*w.insns[block][at]).clone();
    let what = changed.what.as_mut().expect("semantics");
    let name = what.name.clone().expect("a name");
    let stem = ["cmov", "set", "j"].into_iter().find(|stem| name.starts_with(stem)).expect("a conditional");
    what.name = Some(format!("{stem}{}", if inverted { "e" } else { "ne" }).into());
    w.insns[block][at] = Arc::new(changed);
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L149
/// `splitBlock`: the jumps from the one at `mark` on move to a new block,
/// which the old one reaches by the jump it falls into; the edges, their
/// probabilities and the phis follow.
fn split_block(
    w: &mut Work,
    mark: usize,
) -> Result<(), String> {
    let (block, split_at) = w.marks[mark];
    if split_at == 0 {
        return Err("a split before the first instruction of a block".to_owned());
    }
    // The one jump that stays, and its target.
    let Some(unsplit) = w.insns[block][split_at - 1].what.as_ref().and_then(|what| what.target) else {
        return Err("a split must follow a conditional jump with a target".to_owned());
    };
    // Is an edge split into two: another jump of the tail goes to the same
    // successor?
    let is_edge_split =
        w.insns[block][split_at..].iter().any(|one| one.what.as_ref().and_then(|what| what.target) == Some(unsplit));
    let new_at = w.shell.iter().map(|b| b.at).max().unwrap_or(0) + 1;
    let old_at = w.shell[block].at;
    let new = w.shell.len();

    // The new block takes the tail, and the successors that are not the one
    // staying (all, if the edge is split).
    let tail = w.insns[block].split_off(split_at);
    let old_succ: Vec<i64> = w.shell[block].succ.iter().copied().collect();
    let moved: Vec<i64> = old_succ.iter().copied().filter(|&s| is_edge_split || s != unsplit).collect();
    let mut made = LirBlock::new(new_at, Vec::new());
    made.cold = w.shell[block].cold;
    made.succ = moved.clone().into();
    // Their probabilities are copied; normalized if the edge was not split.
    let stated: Vec<(i64, Option<u32>)> = moved.iter().map(|&s| (s, w.odds.taken.get(&(old_at, s)).copied())).collect();
    let total: u64 = stated.iter().filter_map(|&(_, p)| p).map(u64::from).sum();
    for &(s, p) in &stated {
        if let Some(p) = p {
            let p =
                if is_edge_split || total == 0 { p } else { ((u64::from(p) << 31) / total).min((1 << 31) - 1) as u32 };
            w.odds.taken.insert((new_at, s), p);
        }
    }
    // The old block reaches the new one where it reached the moved ones: their
    // probabilities merge.
    let mut merged: Option<u64> = None;
    let mut kept: Vec<i64> = Vec::new();
    for &s in &old_succ {
        if s == unsplit {
            kept.push(s);
        } else if let Some(p) = w.odds.taken.shift_remove(&(old_at, s)) {
            merged = Some(merged.unwrap_or(0) + u64::from(p));
        }
    }
    kept.push(new_at);
    if let Some(p) = merged {
        w.odds.taken.insert((old_at, new_at), p.min(1 << 31) as u32);
    }
    w.shell[block].succ = kept.into();

    // The phis of the moved successors take the new block as their source; the
    // unsplit one also keeps the old where the edge was split.
    for &s in &moved {
        let Some(to) = w.shell.iter().position(|b| b.at == s) else { continue };
        let phis: Vec<crate::model::lir::Phi> = w.shell[to]
            .phis
            .iter()
            .map(|phi| {
                let mut phi = phi.clone();
                let mut extra = Vec::new();
                for entry in &mut phi.incoming {
                    if entry.0 == old_at {
                        if is_edge_split && s == unsplit {
                            extra.push((new_at, entry.1));
                        } else {
                            entry.0 = new_at;
                        }
                    }
                }
                phi.incoming.extend(extra);
                phi
            })
            .collect();
        w.shell[to].phis = phis.into();
    }

    // The old block falls into the new one by an explicit jump.
    let jump_at = w.insns[block].last().map_or(0, |one| one.at);
    let into = Semantics { name: Some("jmp".into()), target: Some(new_at), ..Semantics::new(Operation::Jump) };
    w.insns[block].push(Arc::new(Insn::new(jump_at, Some((jump_at, jump_at)), Some(into), vec![], vec![])));
    w.shell.push(Block::from(made));
    w.insns.push(tail);
    w.children[block].push(new);
    w.children.push(Vec::new());
    // Positions in the tail follow it.
    for mark in &mut w.marks {
        if mark.0 == block && mark.1 >= split_at {
            *mark = (new, mark.1 - split_at);
        }
    }
    Ok(())
}

/// The blocks in reverse post-order from the entry, as indices into
/// `body.blocks` (`ReversePostOrderTraversal`).
fn reverse_post_order(cfg: &Cfg) -> Vec<usize> {
    let n = cfg.len();
    let mut seen = vec![false; n];
    let mut order = Vec::new();
    if n == 0 {
        return order;
    }
    let mut stack = vec![(cfg.entry, 0usize)];
    seen[cfg.entry] = true;
    while let Some(&(block, next)) = stack.last() {
        if let Some(&to) = cfg.succs[block].get(next) {
            stack.last_mut().expect("a frame").1 += 1;
            if !seen[to] {
                seen[to] = true;
                stack.push((to, 0));
            }
        } else {
            order.push(block);
            stack.pop();
        }
    }
    order.reverse();
    order
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L263
/// `runOnMachineFunction`: every restore of the flags, in reverse post-order,
/// lowered; the copies left unused go.
pub fn lowered(body: &LirBody) -> Result<LirBody, String> {
    let has = |name: &str| body.blocks.iter().any(|block| block.insns.iter().any(|one| name_of(one) == Some(name)));
    // None of the work unless a copy of the flags exists.
    if !has("flags_restore") {
        return if has("flags_copy") { Err("a flags copy with no restore".to_owned()) } else { Ok(body.clone()) };
    }
    let next = body
        .blocks
        .iter()
        .flat_map(|b| b.insns.iter())
        .flat_map(|one| one.defines.iter().chain(&one.uses))
        .copied()
        .max()
        .unwrap_or(0)
        + 1;
    let cfg = Cfg::of(body);
    let order = reverse_post_order(&cfg);
    let mut w = Work {
        regs: body.regs(),
        bits: body.bits,
        next,
        renames: HashMap::default(),
        shell: body.blocks.clone(),
        insns: body.blocks.iter().map(|b| b.insns.iter().cloned().collect()).collect(),
        marks: Vec::new(),
        children: vec![Vec::new(); body.blocks.len()],
        odds: body.odds.clone(),
        dom: DomTree::of(&cfg),
        first: body.blocks.len(),
    };
    // The copies, in reverse post-order.
    let restores: Vec<(usize, Arc<Insn>)> = order
        .iter()
        .flat_map(|&block| {
            w.insns[block]
                .iter()
                .filter(|one| name_of(one) == Some("flags_restore"))
                .map(move |one| (block, Arc::clone(one)))
        })
        .collect();
    for (block, restore) in restores {
        let at = w.insns[block].iter().position(|one| Arc::ptr_eq(one, &restore)).expect("the restore is still there");
        lower_copy(&mut w, block, at)?;
    }
    assemble(body, w)
}

/// The body with the lowered blocks, a split-off block after the one it came
/// from, and the renames (`replaceRegWith`) made; copies nothing reads gone.
fn assemble(
    body: &LirBody,
    mut w: Work,
) -> Result<LirBody, String> {
    let resolve = |mut value: u32, renames: &HashMap<u32, u32>| {
        while let Some(&to) = renames.get(&value) {
            value = to;
        }
        value
    };
    let renames = std::mem::take(&mut w.renames);
    let mut used: HashMap<u32, usize> = HashMap::default();
    let mut count = |value: u32| *used.entry(value).or_default() += 1;
    for (block, insns) in w.insns.iter().enumerate() {
        for one in insns {
            one.uses.iter().for_each(|&v| count(resolve(v, &renames)));
        }
        for phi in w.shell[block].phis.iter() {
            phi.incoming.iter().for_each(|&(_, v)| count(resolve(v, &renames)));
        }
    }
    let mut out = body.clone();
    out.odds = w.odds.clone();
    out.blocks = Vec::new();
    // A block split off is placed after the one it came from.
    fn placed(
        index: usize,
        children: &[Vec<usize>],
        order: &mut Vec<usize>,
    ) {
        order.push(index);
        for &child in &children[index] {
            placed(child, children, order);
        }
    }
    let mut order = Vec::new();
    for index in 0..w.first {
        placed(index, &w.children, &mut order);
    }
    for index in order {
        let insns: Vec<Arc<Insn>> = w.insns[index]
            .iter()
            .map(|one| if renames.is_empty() { Arc::clone(one) } else { one.renamed(&|v| resolve(v, &renames)) })
            .filter(|one| name_of(one) != Some("flags_copy") || one.defines.iter().any(|v| used.contains_key(v)))
            .collect();
        let mut block = w.shell[index].with_insns(insns);
        if !renames.is_empty() {
            block.phis = block
                .phis
                .iter()
                .map(|phi| crate::model::lir::Phi {
                    result: resolve(phi.result, &renames),
                    incoming: phi.incoming.iter().map(|&(at, v)| (at, resolve(v, &renames))).collect(),
                })
                .collect();
        }
        out.blocks.push(block);
    }
    if out.blocks.iter().any(|b| b.insns.iter().any(|one| matches!(name_of(one), Some("flags_copy" | "flags_restore"))))
    {
        return Err("an unlowered flags copy".to_owned());
    }
    Ok(out)
}

/// One `flags_restore` at `restore_at` of `block`: the code that reads the
/// flags it restores, in the block and in those the flags live into.
fn lower_copy(
    w: &mut Work,
    block: usize,
    restore_at: usize,
) -> Result<(), String> {
    let value = *w.insns[block][restore_at].uses.first().ok_or("a flags_restore reads no copy")?;
    // `CopyDefI`: the copy, in this block or another.
    let Some((mut test_block, def_at)) = (0..w.insns.len()).find_map(|b| {
        w.insns[b].iter().position(|one| name_of(one) == Some("flags_copy") && one.defines == [value]).map(|at| (b, at))
    }) else {
        return Err(format!("the restore of v{value} has no flags copy that defines it"));
    };
    let mut live_in = w.flags_live_in();
    w.marks = vec![(test_block, def_at), (block, restore_at + 1), (block, restore_at)];
    let copy_def = Arc::clone(&w.insns[test_block][def_at]);
    let copy_i = Arc::clone(&w.insns[block][restore_at]);
    let other_than_copy = |one: &Arc<Insn>| !Arc::ptr_eq(one, &copy_i);

    // Walk up across live-in flags to find where they were defined: hoist the
    // test position to the nearest common dominator of the predecessors while
    // nothing on the way clobbers the flags.
    let has_clobber = |w: &Work, block: usize, from: usize, to: usize| {
        w.insns[block][from..to].iter().rev().any(|one| other_than_copy(one) && w.modifies(one))
    };
    let cfg = w.cfg();
    while live_in[test_block] && !cfg.preds[test_block].is_empty() && !has_clobber(w, test_block, 0, w.marks[TEST].1) {
        if cfg.preds[test_block].iter().any(|&p| p >= w.first) || test_block >= w.first {
            return Err("test-position hoisting past a block this pass split".to_owned());
        }
        let preds = &cfg.preds[test_block];
        let hoist = preds[1..].iter().fold(preds[0], |lhs, &rhs| w.dom.find_nearest_common_dominator(lhs, rhs));
        // A clobber anywhere on a path from the hoist block down to this one
        // stops the hoist; loops included, as none is allowed.
        let mut visited = vec![false; cfg.len()];
        visited[hoist] = true;
        let mut work = vec![test_block];
        let mut clobbered = false;
        'path: while let Some(b) = work.pop() {
            for &pred in &cfg.preds[b] {
                if std::mem::replace(&mut visited[pred], true) {
                    continue;
                }
                if has_clobber(w, pred, 0, w.insns[pred].len()) {
                    clobbered = true;
                    break 'path;
                }
                work.push(pred);
            }
        }
        if clobbered {
            break;
        }
        // The terminators must not clobber the flags either.
        let terminators = first_terminator(&w.insns[hoist]);
        if has_clobber(w, hoist, terminators, w.insns[hoist].len()) {
            break;
        }
        // Hoist the test position to the hoist block's first terminator.
        test_block = hoist;
        w.marks[TEST] = (hoist, terminators);
    }

    // The conditions already saved in registers at the test position.
    let mut conds = collect_conds_in_regs(w);
    // Jumps are buffered and rewritten last: doing so perturbs the CFG walked
    // to find the uses.
    let mut jumps: Vec<usize> = Vec::new();
    let mut blocks = vec![block];
    let mut visited = vec![false; w.shell.len()];

    while let Some(use_block) = blocks.pop() {
        // In the copy's block the first visit starts after the restore; the
        // second (a cycle) from the beginning, to the restore.
        let start = if use_block == block && !visited[block] { restore_at + 1 } else { 0 };
        w.marks[AT] = (use_block, start);
        let mut flags_killed = false;
        loop {
            let (b, at) = w.marks[AT];
            if at >= w.insns[b].len() {
                break;
            }
            let one = Arc::clone(&w.insns[b][at]);
            // The copy or its def met again: the second pass is done.
            if Arc::ptr_eq(&one, &copy_i) || Arc::ptr_eq(&one, &copy_def) {
                break;
            }
            let reading = reads(&one);
            flags_killed = w.modifies(&one);
            let Some(reading) = reading else {
                if flags_killed {
                    break;
                }
                w.marks[AT].1 += 1;
                continue;
            };
            match reading {
                // Once a branch is met the rest of the block are branches.
                Reads::Jump(_) => {
                    let mut next = at;
                    while next < w.insns[b].len() && matches!(reads(&w.insns[b][next]), Some(Reads::Jump(_))) {
                        jumps.push(w.marks.len());
                        w.marks.push((b, next));
                        next += 1;
                    }
                    break;
                }
                // Just replace this copy with the original copy def.
                Reads::Copy => {
                    let [copied] = one.defines[..] else { return Err("a flags copy defines one value".to_owned()) };
                    w.renames.insert(copied, value);
                    w.remove(b, at);
                    continue;
                }
                Reads::Set(code, store) => {
                    rewrite_set_cc(w, &mut conds, code, store);
                    if store {
                        w.marks[AT].1 += 1;
                    }
                }
                Reads::Arith => {
                    rewrite_arithmetic(w, &mut conds);
                    w.marks[AT].1 += 1;
                }
                Reads::Cond(code) => {
                    rewrite_mi(w, &mut conds, code);
                    w.marks[AT].1 += 1;
                }
            }
            if flags_killed {
                break;
            }
        }
        // The flags were killed here: done with this block. Otherwise the
        // successors the flags live into are next.
        if flags_killed {
            continue;
        }
        for succ in w.cfg().succs[use_block].clone() {
            if live_in[succ] && !std::mem::replace(&mut visited[succ], true) {
                // No phi is made: the test block must dominate every use, and
                // may not be one.
                if succ >= w.first || succ == test_block || !w.dom.dominates(test_block, succ) {
                    return Err(
                        "cannot lower an EFLAGS copy whose copy def does not dominate all uses (it needs phis)"
                            .to_owned(),
                    );
                }
                blocks.push(succ);
                // After this the flags are recreated before each use.
                live_in[succ] = false;
            }
        }
    }

    // Now the jumps: past the first in a block it is split first.
    let mut last_jump_block = None;
    for &mark in &jumps.clone() {
        if last_jump_block == Some(w.marks[mark].0) {
            split_block(w, mark)?;
        } else {
            last_jump_block = Some(w.marks[mark].0);
        }
        w.marks[AT] = w.marks[mark];
        let Reads::Jump(code) = reads(&w.insns[w.marks[AT].0][w.marks[AT].1]).expect("a jump") else {
            unreachable!("found as a jump")
        };
        rewrite_mi(w, &mut conds, code);
    }
    // All uses are rewritten: the restore goes, and the copy if unread (the
    // sweep in `assemble`).
    let (b, at) = w.marks[RESTORE];
    w.remove(b, at);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::lir::LirBlock;
    use crate::support::hash::IndexMap;

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 4 })
    }

    fn pseudo(
        name: &str,
        defines: Vec<u32>,
        uses: Vec<u32>,
    ) -> Arc<Insn> {
        let dests = defines.iter().map(|&v| held(v)).collect();
        let sources = uses.iter().map(|&v| held(v)).collect();
        make(0, name, Operation::Move, dests, sources, defines, uses)
    }

    fn reader(
        name: &str,
        value: u32,
    ) -> Arc<Insn> {
        if name.starts_with('j') {
            return make(0, name, Operation::Branch, vec![], vec![], vec![], vec![]);
        }
        make(0, name, Operation::Unary, vec![byte(value)], vec![], vec![value], vec![])
    }

    fn clobber() -> Arc<Insn> {
        make(0, "add", Operation::Binary, vec![held(2)], vec![held(3), held(3)], vec![2], vec![3])
    }

    /// The names of the instructions after lowering a body: the flags are
    /// copied, `add` clobbers them, they are restored and `readers` read them.
    fn after(readers: &[&str]) -> Vec<String> {
        let mut insns =
            vec![pseudo("flags_copy", vec![1], vec![]), clobber(), pseudo("flags_restore", vec![], vec![1])];
        insns.extend(readers.iter().zip(10..).map(|(&name, value)| reader(name, value)));
        let body = LirBody::new("f", 0, vec![LirBlock::new(0, insns).into()], IndexMap::default(), IndexMap::default());
        let done = lowered(&body).unwrap();
        done.blocks[0].insns.iter().map(|one| name_of(one).unwrap().to_owned()).collect()
    }

    #[test]
    fn a_jump_after_a_clobber_tests_the_saved_byte() {
        assert_eq!(after(&["jl"]), ["setl", "add", "test", "jne"]);
    }

    #[test]
    fn a_condition_is_saved_as_asked_and_its_opposite_reads_it_for_equal() {
        // `cmovge` asks first: `setge` is saved, then `cmovl` reads its
        // opposite.
        assert_eq!(
            after(&["cmovge", "cmovl"]),
            ["setge", "add", "test", "cmovne", "test", "cmove"],
            "the pin tests before every reader"
        );
    }

    #[test]
    fn a_setcc_reader_takes_the_saved_byte_with_no_test() {
        let mut insns =
            vec![pseudo("flags_copy", vec![1], vec![]), clobber(), pseudo("flags_restore", vec![], vec![1])];
        insns.push(reader("setl", 10));
        insns.push(make(0, "mov", Operation::Move, vec![held(20)], vec![byte(10)], vec![20], vec![10]));
        let body = LirBody::new("f", 0, vec![LirBlock::new(0, insns).into()], IndexMap::default(), IndexMap::default());
        let done = lowered(&body).unwrap();
        let names: Vec<_> = done.blocks[0].insns.iter().map(|one| name_of(one).unwrap().to_owned()).collect();
        assert_eq!(names, ["setl", "add", "mov"]);
        // The reader of the old byte reads the saved one.
        let saved = done.blocks[0].insns[0].defines[0];
        assert_eq!(done.blocks[0].insns[2].uses, [saved]);
    }

    #[test]
    fn the_carry_is_saved_as_setb_and_made_again_by_an_add() {
        assert_eq!(after(&["adc"]), ["setb", "add", "add", "adc"]);
    }

    #[test]
    fn a_condition_already_in_a_byte_before_the_copy_is_reused() {
        let mut insns = vec![
            reader("setl", 30),
            pseudo("flags_copy", vec![1], vec![]),
            clobber(),
            pseudo("flags_restore", vec![], vec![1]),
        ];
        insns.push(reader("cmovl", 0));
        let body = LirBody::new("f", 0, vec![LirBlock::new(0, insns).into()], IndexMap::default(), IndexMap::default());
        let done = lowered(&body).unwrap();
        let names: Vec<_> = done.blocks[0].insns.iter().map(|one| name_of(one).unwrap().to_owned()).collect();
        assert_eq!(names, ["setl", "add", "test", "cmovne"], "no second setl");
    }

    fn jump(
        name: &str,
        op: Operation,
        target: i64,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.into()), target: Some(target), ..Semantics::new(op) };
        Arc::new(Insn::new(0, Some((0, 0)), Some(what), vec![], vec![]))
    }

    fn block(
        at: i64,
        insns: Vec<Arc<Insn>>,
        succ: &[i64],
    ) -> LirBlock {
        let mut block = LirBlock::new(at, insns);
        block.succ = succ.to_vec().into();
        block
    }

    fn names(insns: &crate::model::lir::Insns) -> Vec<String> {
        insns.iter().map(|one| name_of(one).unwrap().to_owned()).collect()
    }

    /// The copy in the entry, the flags restored in the next block, as the pin
    /// puts the `setcc` at the copy and the `test` at the reader.
    #[test]
    fn a_copy_in_another_block_saves_at_the_copy_and_tests_at_the_reader() {
        let a =
            block(0, vec![pseudo("flags_copy", vec![1], vec![]), clobber(), jump("jmp", Operation::Jump, 16)], &[16]);
        let b = block(
            16,
            vec![
                pseudo("flags_restore", vec![], vec![1]),
                jump("jl", Operation::Branch, 32),
                jump("jmp", Operation::Jump, 32),
            ],
            &[32],
        );
        let c = block(32, vec![], &[]);
        let body = LirBody::new("f", 0, vec![a.into(), b.into(), c.into()], IndexMap::default(), IndexMap::default());
        let done = lowered(&body).unwrap();
        assert_eq!(names(&done.blocks[0].insns), ["setl", "add", "jmp"]);
        assert_eq!(names(&done.blocks[1].insns), ["test", "jne", "jmp"]);
    }

    /// The flags live into a successor, which reads them: they are tested
    /// there, `test_block` dominating it.
    #[test]
    fn flags_live_into_a_successor_are_tested_there() {
        let a = block(
            0,
            vec![
                pseudo("flags_copy", vec![1], vec![]),
                clobber(),
                pseudo("flags_restore", vec![], vec![1]),
                jump("jmp", Operation::Jump, 16),
            ],
            &[16],
        );
        let b = block(16, vec![reader("cmovl", 0), jump("jmp", Operation::Jump, 32)], &[32]);
        let c = block(32, vec![], &[]);
        let body = LirBody::new("f", 0, vec![a.into(), b.into(), c.into()], IndexMap::default(), IndexMap::default());
        let done = lowered(&body).unwrap();
        assert_eq!(names(&done.blocks[0].insns), ["setl", "add", "jmp"]);
        assert_eq!(names(&done.blocks[1].insns), ["test", "cmovne", "jmp"]);
    }

    /// The pin's `splitBlock` case, from `llc
    /// -run-pass=x86-flags-copy-lowering` on `jl X; jge Y; jmp Z` after the
    /// restore: `setl`, the first jump tested in place as `jne X`, a new
    /// block with a `test`, `je Y` (the opposite of the saved `l`) and the
    /// `jmp Z`.
    #[test]
    fn a_second_jump_on_the_flags_goes_to_a_split_block() {
        let a = block(
            0,
            vec![
                pseudo("flags_copy", vec![1], vec![]),
                clobber(),
                pseudo("flags_restore", vec![], vec![1]),
                jump("jl", Operation::Branch, 16),
                jump("jge", Operation::Branch, 32),
                jump("jmp", Operation::Jump, 48),
            ],
            &[16, 32, 48],
        );
        let body = LirBody::new(
            "f",
            0,
            vec![a.into(), block(16, vec![], &[]).into(), block(32, vec![], &[]).into(), block(48, vec![], &[]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        let done = lowered(&body).unwrap();
        assert_eq!(done.blocks.len(), 5);
        assert_eq!(names(&done.blocks[0].insns), ["setl", "add", "test", "jne", "jmp"]);
        // The split block follows the one it came from.
        assert_eq!(names(&done.blocks[1].insns), ["test", "je", "jmp"]);
        let new_at = done.blocks[1].at;
        assert_eq!(done.blocks[0].succ.iter().copied().collect::<Vec<_>>(), [16, new_at]);
        assert_eq!(done.blocks[1].succ.iter().copied().collect::<Vec<_>>(), [32, 48]);
    }

    /// A copy in a join the flags live into from both arms: the test position
    /// is hoisted to the nearest common dominator of the predecessors, before
    /// its first terminator (`TestMBB = HoistMBB`).
    #[test]
    fn a_test_position_is_hoisted_to_the_dominator_of_the_predecessors() {
        let entry =
            block(0, vec![clobber(), jump("jl", Operation::Branch, 16), jump("jmp", Operation::Jump, 32)], &[16, 32]);
        let left = block(16, vec![jump("jmp", Operation::Jump, 48)], &[48]);
        let right = block(32, vec![jump("jmp", Operation::Jump, 48)], &[48]);
        let join = block(
            48,
            vec![
                pseudo("flags_copy", vec![1], vec![]),
                clobber(),
                pseudo("flags_restore", vec![], vec![1]),
                reader("cmovl", 0),
            ],
            &[],
        );
        let body = LirBody::new(
            "f",
            0,
            vec![entry.into(), left.into(), right.into(), join.into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        let done = lowered(&body).unwrap();
        assert_eq!(names(&done.blocks[0].insns), ["add", "setl", "jl", "jmp"]);
        assert_eq!(names(&done.blocks[3].insns), ["add", "test", "cmovne"]);
    }

    #[test]
    fn a_restore_with_no_copy_is_refused() {
        let insns = vec![pseudo("flags_restore", vec![], vec![1])];
        let body = LirBody::new("f", 0, vec![LirBlock::new(0, insns).into()], IndexMap::default(), IndexMap::default());
        assert!(lowered(&body).is_err());
    }
}
