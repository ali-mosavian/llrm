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
//! Blockers (named, with the date each lands, 2026-11-04 unless said):
//! the copy and its restore in different blocks, flags live into successors,
//! and the hoisting of the test position up the dominator tree (the loop in
//! `runOnMachineFunction` over `TestMBB`) are an error with a message that
//! says so; the split of a block with a second `jcc` on the flags is too.
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

use std::collections::HashMap;
use std::sync::Arc;

use crate::backend::liveness;
use crate::model::ir::{Held, Imm, Loc, Operation, Semantics};
use crate::model::lir::{Insn, LirBody};
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

/// Fresh values and the renames the pass makes (`replaceRegWith`).
struct State {
    regs: llrm_lir::registers::Regs,
    bits: u32,
    next: u32,
    renames: HashMap<u32, u32>,
}

impl State {
    fn fresh(&mut self) -> u32 {
        self.next += 1;
        self.next - 1
    }
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L721
/// `collectCondsInRegs`: the conditions `setcc`ed into registers before
/// `test_pos` and after the last instruction that defines the flags.
fn collect_conds_in_regs(
    insns: &[Arc<Insn>],
    test_pos: usize,
    state: &State,
) -> CondRegs {
    let mut conds = CondRegs::new();
    for one in insns[..test_pos].iter().rev() {
        if let Some(Reads::Set(code, false)) = reads(one)
            && let [Loc::Held(into)] = one.what.as_ref().expect("semantics").dests.as_slice()
        {
            conds.insert(code, into.value);
        }
        // The first definition of the flags: before it the state would be
        // wrong.
        if modifies_flags(one, state.regs, state.bits) {
            break;
        }
    }
    conds
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L744
/// `promoteCondToReg`: a byte holding `cond`, `setcc`ed before `test_pos`.
fn promote_cond_to_reg(
    insns: &mut Vec<Arc<Insn>>,
    test_pos: &mut usize,
    state: &mut State,
    cond: &str,
) -> u32 {
    let value = state.fresh();
    let at = insns[*test_pos].at;
    insns.insert(
        *test_pos,
        make(at, &format!("set{cond}"), Operation::Unary, vec![byte(value)], vec![], vec![value], vec![]),
    );
    *test_pos += 1;
    value
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L761
/// `getCondOrInverseInReg`: the byte of `cond`, else the byte of its opposite
/// and `true`; neither: `cond` is saved.
fn get_cond_or_inverse_in_reg(
    insns: &mut Vec<Arc<Insn>>,
    test_pos: &mut usize,
    state: &mut State,
    cond: &'static str,
    conds: &mut CondRegs,
) -> (u32, bool) {
    if !conds.contains_key(cond) && !conds.contains_key(opposite(cond)) {
        let value = promote_cond_to_reg(insns, test_pos, state, cond);
        conds.insert(cond, value);
    }
    match conds.get(cond) {
        Some(&value) => (value, false),
        None => (conds[opposite(cond)], true),
    }
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L775
/// `insertTest`: `test reg, reg` before `at`.
fn insert_test(
    insns: &mut Vec<Arc<Insn>>,
    at: usize,
    value: u32,
) {
    let place = insns[at].at;
    insns.insert(
        at,
        make(place, "test", Operation::Compare, vec![], vec![byte(value), byte(value)], vec![], vec![value]),
    );
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L785
/// `rewriteSetCC`: the `setcc` at `at` takes the byte of its condition; the
/// opposite is not used (it would need an analysis of the byte's readers).
/// To memory, it becomes a byte store of the saved byte.
fn rewrite_set_cc(
    insns: &mut Vec<Arc<Insn>>,
    test_pos: &mut usize,
    at: usize,
    state: &mut State,
    conds: &mut CondRegs,
    code: &'static str,
    store: bool,
) {
    let before = insns.len();
    let saved = match conds.get(code) {
        Some(&value) => value,
        None => {
            let value = promote_cond_to_reg(insns, test_pos, state, code);
            conds.insert(code, value);
            value
        }
    };
    // A byte saved at `test_pos`, which precedes `at`, moves the reader down.
    let at = at + insns.len() - before;
    let one = Arc::clone(&insns[at]);
    let what = one.what.as_ref().expect("semantics");
    if !store {
        let [Loc::Held(old)] = what.dests.as_slice() else { return };
        state.renames.insert(old.value, saved);
        insns.remove(at);
        return;
    }
    // A byte store of the saved byte to the same address.
    let mut store_what = Semantics { name: Some("mov".into()), ..Semantics::new(Operation::Move) };
    store_what.dests = what.dests.clone();
    store_what.sources = vec![byte(saved)];
    let mut made = (*one).clone();
    made.what = Some(store_what);
    made.defines = vec![];
    made.uses.push(saved);
    insns[at] = Arc::new(made);
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L824
/// `rewriteArithmetic`: a reader of the carry gets it back by `add byte, 255`
/// of the saved `b` byte.
fn rewrite_arithmetic(
    insns: &mut Vec<Arc<Insn>>,
    test_pos: &mut usize,
    at: &mut usize,
    state: &mut State,
    conds: &mut CondRegs,
) {
    let saved = match conds.get("b") {
        Some(&value) => value,
        None => {
            let value = promote_cond_to_reg(insns, test_pos, state, "b");
            *at += 1;
            conds.insert("b", value);
            value
        }
    };
    let tmp = state.fresh();
    let place = insns[*at].at;
    insns.insert(
        *at,
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
    *at += 1;
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FlagsCopyLowering.cpp#L899
/// `rewriteMI`: a reader with a condition tests the saved byte first and reads
/// `ne` (`e` for the opposite condition's byte).
fn rewrite_mi(
    insns: &mut Vec<Arc<Insn>>,
    test_pos: &mut usize,
    at: &mut usize,
    state: &mut State,
    conds: &mut CondRegs,
    code: &'static str,
) {
    let before = insns.len();
    let (saved, inverted) = get_cond_or_inverse_in_reg(insns, test_pos, state, code, conds);
    // A byte saved at `test_pos`, which precedes `at`, moves the reader down.
    *at += insns.len() - before;
    insert_test(insns, *at, saved);
    *at += 1;
    let mut changed = (*insns[*at]).clone();
    let what = changed.what.as_mut().expect("semantics");
    let name = what.name.clone().expect("a name");
    let stem = ["cmov", "set", "j"].into_iter().find(|stem| name.starts_with(stem)).expect("a conditional");
    what.name = Some(format!("{stem}{}", if inverted { "e" } else { "ne" }).into());
    insns[*at] = Arc::new(changed);
}

/// The blocks in reverse post-order from the entry, as indices into
/// `body.blocks` (`ReversePostOrderTraversal`).
fn reverse_post_order(body: &LirBody) -> Vec<usize> {
    let index: HashMap<i64, usize> = body.blocks.iter().enumerate().map(|(at, block)| (block.at, at)).collect();
    let mut seen = vec![false; body.blocks.len()];
    let mut order = Vec::new();
    let mut stack = vec![(0usize, 0usize)];
    if body.blocks.is_empty() {
        return order;
    }
    seen[0] = true;
    while let Some(&mut (block, ref mut next)) = stack.last_mut() {
        let succ = &body.blocks[block].succ;
        if let Some(&to) = succ.iter().nth(*next).and_then(|to| index.get(to)) {
            *next += 1;
            if !seen[to] {
                seen[to] = true;
                stack.push((to, 0));
            }
        } else if *next < succ.len() {
            *next += 1;
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
    let (regs, bits) = (body.regs(), body.bits);
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
    let mut state = State { regs, bits, next, renames: HashMap::new() };
    let mut out = body.clone();
    for at in reverse_post_order(body) {
        let mut insns: Vec<Arc<Insn>> = out.blocks[at].insns.iter().cloned().collect();
        while let Some(restore_at) = insns.iter().position(|one| name_of(one) == Some("flags_restore")) {
            lower_copy(&out, at, &mut insns, restore_at, &mut state)?;
        }
        out.blocks[at] = out.blocks[at].with_insns(insns);
    }
    // The renames (`replaceRegWith`), chains followed.
    let resolve = |mut value: u32| {
        while let Some(&to) = state.renames.get(&value) {
            value = to;
        }
        value
    };
    if !state.renames.is_empty() {
        for block in &mut out.blocks {
            let insns: Vec<Arc<Insn>> = block.insns.iter().map(|one| one.renamed(&resolve)).collect();
            *block = block.with_insns(insns);
            block.phis = block
                .phis
                .iter()
                .map(|phi| crate::model::lir::Phi {
                    result: resolve(phi.result),
                    incoming: phi.incoming.iter().map(|&(at, value)| (at, resolve(value))).collect(),
                })
                .collect();
        }
    }
    // A copy none reads (`use_empty`) goes; one that remains is read by what
    // was not lowered.
    let mut used: HashMap<u32, usize> = HashMap::new();
    for block in &out.blocks {
        for one in block.insns.iter() {
            for &value in &one.uses {
                *used.entry(value).or_default() += 1;
            }
        }
        for phi in block.phis.iter() {
            for &(_, value) in &phi.incoming {
                *used.entry(value).or_default() += 1;
            }
        }
    }
    for block in &mut out.blocks {
        let insns: Vec<Arc<Insn>> = block
            .insns
            .iter()
            .filter(|one| name_of(one) != Some("flags_copy") || one.defines.iter().any(|v| used.contains_key(v)))
            .cloned()
            .collect();
        *block = block.with_insns(insns);
    }
    if out.blocks.iter().any(|b| b.insns.iter().any(|one| name_of(one) == Some("flags_copy"))) {
        return Err("an unlowered flags copy".to_owned());
    }
    Ok(out)
}

/// Whether a block reads the flags before it writes them: they live in.
fn flags_live_in(
    block: &crate::model::lir::Block,
    state: &State,
) -> bool {
    for one in block.insns.iter() {
        if reads(one).is_some() && name_of(one) != Some("flags_copy") {
            return true;
        }
        if modifies_flags(one, state.regs, state.bits) {
            return false;
        }
    }
    false
}

/// One `flags_restore` at `restore_at` of the block `block` and the code after
/// it that reads the flags it restores.
fn lower_copy(
    body: &LirBody,
    block: usize,
    insns: &mut Vec<Arc<Insn>>,
    restore_at: usize,
    state: &mut State,
) -> Result<(), String> {
    let value = *insns[restore_at].uses.first().ok_or("a flags_restore reads no copy")?;
    let Some(def_at) =
        insns[..restore_at].iter().position(|one| name_of(one) == Some("flags_copy") && one.defines == [value])
    else {
        return Err(format!(
            "the flags copy v{value} is not in the block of its restore (cross-block lowering lands 2026-11-04)"
        ));
    };
    // The test position is the copy.
    let mut test_pos = def_at;
    let mut conds = collect_conds_in_regs(insns, test_pos, state);
    let mut jumps: Vec<usize> = Vec::new();
    let mut at = restore_at + 1;
    let mut killed = false;
    while at < insns.len() {
        let one = Arc::clone(&insns[at]);
        killed = modifies_flags(&one, state.regs, state.bits);
        let Some(reading) = reads(&one) else {
            if killed {
                break;
            }
            at += 1;
            continue;
        };
        match reading {
            // Once a branch is met the rest are branches too: they are handled
            // below, as splitting the block perturbs the walk.
            Reads::Jump(_) => {
                jumps.push(at);
                let mut next = at + 1;
                while next < insns.len() && matches!(reads(&insns[next]), Some(Reads::Jump(_))) {
                    jumps.push(next);
                    next += 1;
                }
                break;
            }
            // Just replace this copy with the original copy def.
            Reads::Copy => {
                let [copied] = one.defines[..] else { return Err("a flags copy defines one value".to_owned()) };
                state.renames.insert(copied, value);
                insns.remove(at);
                continue;
            }
            Reads::Set(code, store) => {
                let before = insns.len();
                rewrite_set_cc(insns, &mut test_pos, at, state, &mut conds, code, store);
                // A register set leaves, a store stays; a byte saved before
                // the copy moves the rest down.
                let saved = (insns.len() + usize::from(!store)).saturating_sub(before);
                at += saved + usize::from(store);
            }
            Reads::Arith => {
                rewrite_arithmetic(insns, &mut test_pos, &mut at, state, &mut conds);
                at += 1;
            }
            Reads::Cond(code) => {
                rewrite_mi(insns, &mut test_pos, &mut at, state, &mut conds, code);
                at += 1;
            }
        }
        if killed {
            break;
        }
    }
    // Flags not killed in the block may be read in a successor that was not
    // visited.
    let reaches_end = !killed && (jumps.len() + at >= insns.len() || !jumps.is_empty());
    if reaches_end {
        for to in body.blocks[block].succ.iter() {
            if let Some(next) = body.blocks.iter().find(|b| b.at == *to)
                && next.at != body.blocks[block].at
                && flags_live_in(next, state)
            {
                return Err(format!("flags live into the block at {to} (cross-block lowering lands 2026-11-04)"));
            }
        }
    }
    // Now the jumps: the first in place; past it the block is split.
    if let Some((&first, rest)) = jumps.split_first() {
        if !rest.is_empty() {
            return Err("a second jcc on the restored flags needs the block split (lands 2026-11-04)".to_owned());
        }
        let Reads::Jump(code) = reads(&insns[first]).expect("a jump") else { unreachable!("found as a jump") };
        let mut at = first;
        rewrite_mi(insns, &mut test_pos, &mut at, state, &mut conds, code);
    }
    // All uses are rewritten: the restore goes, and the copy if unread (the
    // sweep in `lowered`).
    let gone = insns.iter().position(|one| name_of(one) == Some("flags_restore") && one.uses == [value]);
    insns.remove(gone.expect("the restore"));
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

    #[test]
    fn a_restore_in_another_block_is_refused() {
        let insns = vec![pseudo("flags_restore", vec![], vec![1])];
        let body = LirBody::new("f", 0, vec![LirBlock::new(0, insns).into()], IndexMap::default(), IndexMap::default());
        assert!(lowered(&body).is_err());
    }
}
