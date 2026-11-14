//! `WebAssemblyRegStackify`
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/WebAssembly/WebAssemblyRegStackify.cpp#L841)):
//! a register whose definition can sit right before its use moves onto the
//! value stack, and the walk continues through that definition's operands.
//!
//! # How it works
//! Instructions are visited last to first. Each one's register operands are
//! popped last operand first. When the operand's definition is in the same
//! block and nothing between it and the instruction depends on it, a value read
//! once is moved down to the instruction and stackified; a constant is cloned
//! to it (`rematerializeCheapDef`); a value read more than once whose first
//! reader is the instruction gets a `TEE` that keeps the value in its register
//! for the rest. When an operand cannot be stackified the instruction's
//! commutable operands are swapped once to try the other order.
//!
//! Without optimization (`-O0`) only the definition right above the use
//! qualifies and a register must be read once.
//!
//! DEVIATE 1: no `LiveIntervals`, no dominator tree. The queries the pin asks
//! of them are answered from the registers' definition counts and the block
//! liveness (`liveness::liveness`, computed once; moves, tees and clones add
//! block-local registers only, so it stays valid, and a register whose last
//! outside use a clone removed stays live-out, which only withholds a
//! stackification): the definition reaching a use is the nearest one above it
//! in the block, or the only one in the function; a value is read once when
//! the uses after its definition up to the next definition are one and it is
//! dead past them; a use dominates the others when every other use of the value
//! is later in the block, in another block that the value reaches (the
//! definition dominates them), or is stackified into it. A multi-definition
//! register defined outside the block is not stackified (the pin asks
//! `LiveIntervals` for its reaching value). Exit: the shared live intervals
//! (S4), no later than 2026-11-20.
//!
//! DEVIATE 2: a load always counts as a read of memory
//! (`isDereferenceableInvariantLoad` is false): MIR has no invariant flag.
//! Exit: carry it from the access (2026-11-13).
//!
//! # See also
//! `WebAssemblyRegStackify.cpp`, pin d1106deb71cc: `regStackify` (L841),
//! `isSafeToMove` (L352), `hasSingleUse` (L296), `query` (L172),
//! `oneUseDominatesOtherUses` (L457), `moveForSingleUse` (L554),
//! `rematerializeCheapDef` (L605), `moveAndTeeForMultiUse` (L669),
//! `TreeWalkerState` (L725), `CommutingState` (L793).

use llrm_support::hash::HashMap;

use crate::dagisel::generated::ids::opcode as op;
use crate::dagisel::generated::instrinfo::{desc, flag};
use crate::mir::{Function, IMPLICIT_DEF, Inst, Operand, VReg, ValType, is_argument};
use crate::spike_coalesce::name_of;

/// `TreeWalkerState`: ranges of the operands still to visit, last first.
struct TreeWalker {
    /// The block the walk is in.
    b: usize,
    /// (instruction, one past the next operand to pop).
    worklist: Vec<(usize, usize)>,
}

impl TreeWalker {
    fn new(
        f: &Function,
        b: usize,
        at: usize,
    ) -> TreeWalker {
        let mut walker = TreeWalker { b, worklist: Vec::new() };
        walker.push_operands(f, at);
        walker
    }

    fn defs(
        &self,
        f: &Function,
        at: usize,
    ) -> usize {
        f.blocks[self.b][at].operands.iter().take_while(|o| matches!(o, Operand::Def(_))).count()
    }

    fn push_operands(
        &mut self,
        f: &Function,
        at: usize,
    ) {
        let end = f.blocks[self.b][at].operands.len();
        if end > self.defs(f, at) {
            self.worklist.push((at, end));
        }
    }

    /// An instruction was inserted (`delta` 1) or removed (-1) at `from`: the
    /// ranges naming instructions at or after it follow.
    fn shift(
        &mut self,
        from: usize,
        delta: isize,
    ) {
        for (at, _) in &mut self.worklist {
            if *at >= from {
                *at = (*at as isize + delta) as usize;
            }
        }
    }

    fn done(&self) -> bool {
        self.worklist.is_empty()
    }

    /// The next operand: its instruction and index.
    fn pop(
        &mut self,
        f: &Function,
    ) -> (usize, usize) {
        let (at, end) = self.worklist.pop().expect("a range remains");
        if end - 1 > self.defs(f, at) {
            self.worklist.push((at, end - 1));
        }
        (at, end - 1)
    }

    fn has_remaining_operands(
        &self,
        at: usize,
    ) -> bool {
        self.worklist.last().is_some_and(|&(top, _)| top == at)
    }

    fn reset_top_operands(
        &mut self,
        f: &Function,
        at: usize,
    ) {
        *self.worklist.last_mut().expect("an operand remains") = (at, f.blocks[self.b][at].operands.len());
    }

    fn is_on_stack(
        &self,
        f: &Function,
        reg: VReg,
    ) -> bool {
        self.worklist.iter().any(|&(at, end)| f.blocks[self.b][at].operands[..end].contains(&Operand::Use(reg)))
    }
}

/// `CommutingState`.
#[derive(Default)]
struct Commuting {
    tentative: Option<(usize, usize)>,
    declined: bool,
}

impl Commuting {
    fn maybe_commute(
        &mut self,
        f: &mut Function,
        at: usize,
        walker: &mut TreeWalker,
    ) {
        if let Some((a, b)) = self.tentative.take() {
            f.blocks[walker.b][at].operands.swap(a, b);
            self.declined = true;
        } else if !self.declined && walker.has_remaining_operands(at) {
            if let Some((a, b)) = commutable_operands(&f.blocks[walker.b][at]) {
                f.blocks[walker.b][at].operands.swap(a, b);
                walker.reset_top_operands(f, at);
                self.tentative = Some((a, b));
                self.declined = false;
            }
        }
    }

    fn reset(&mut self) {
        *self = Commuting::default();
    }
}

/// `findCommutedOpIndices`: the first two source registers of a commutable
/// instruction.
fn commutable_operands(inst: &Inst) -> Option<(usize, usize)> {
    if desc(inst.opcode)?.flags & flag::COMMUTABLE == 0 {
        return None;
    }
    let uses: Vec<usize> =
        inst.operands.iter().enumerate().filter(|(_, o)| matches!(o, Operand::Use(_))).map(|(i, _)| i).collect();
    (uses.len() >= 2).then(|| (uses[0], uses[1]))
}

/// What `query` found an instruction to do.
#[derive(Default)]
struct Effects {
    read: bool,
    write: bool,
    effects: bool,
    stack_pointer: bool,
}

/// The names `query` lets move: they trap on overflow and invalid, which is
/// undefined behaviour, so stackifying may reorder them.
fn is_trapping_arithmetic(name: &str) -> bool {
    matches!(
        name,
        "DIV_S_I32"
            | "DIV_S_I64"
            | "REM_S_I32"
            | "REM_S_I64"
            | "DIV_U_I32"
            | "DIV_U_I64"
            | "REM_U_I32"
            | "REM_U_I64"
            | "I32_TRUNC_S_F32"
            | "I64_TRUNC_S_F32"
            | "I32_TRUNC_S_F64"
            | "I64_TRUNC_S_F64"
            | "I32_TRUNC_U_F32"
            | "I64_TRUNC_U_F32"
            | "I32_TRUNC_U_F64"
            | "I64_TRUNC_U_F64"
    )
}

/// `queryCallee`.
fn query_callee(
    f: &Function,
    inst: &Inst,
    found: &mut Effects,
) {
    found.stack_pointer = true;
    if let Some(&Operand::Sym(at)) = inst.operands.iter().find(|o| matches!(o, Operand::Sym(_))) {
        let name = &f.symbols[at as usize].0;
        if let Some(&(reads, writes)) = f.callee_memory.get(name) {
            if !f.nothrow.contains(name) {
                found.effects = true;
            }
            if !reads && !writes {
                return;
            }
            if reads && !writes {
                found.read = true;
                return;
            }
        }
    }
    found.read = true;
    found.write = true;
    found.effects = true;
}

/// `query`: whether `inst` reads memory, writes it, has side effects or uses
/// the stack pointer.
fn query(
    f: &Function,
    inst: &Inst,
) -> Effects {
    let mut found = Effects::default();
    let Some(info) = desc(inst.opcode) else {
        // `CALL` and `CALL_INDIRECT` are in no table (variadic defs): a call.
        if crate::branch::is_call(inst) {
            query_callee(f, inst, &mut found);
        }
        // Otherwise a marker: it does nothing to memory.
        return found;
    };
    if info.has(flag::TERMINATOR) {
        return Effects { read: true, write: true, effects: true, stack_pointer: true };
    }
    let is_call = info.has(flag::CALL);
    let may_load = info.has(flag::MAY_LOAD);
    let may_store = info.has(flag::MAY_STORE);
    let unmodeled = info.has(flag::UNMODELED_SIDE_EFFECTS);
    let name = if unmodeled || (may_load && !may_store) { name_of(inst.opcode) } else { "" };
    if may_load {
        found.read = true;
    }
    if may_store {
        found.write = true;
    } else if !is_call && (may_load || unmodeled) {
        // `hasOrderedMemoryRef`: no memory operand, or an ordered one.
        let ordered = inst.mem.is_none_or(|mem| mem.ordered);
        if ordered && !is_trapping_arithmetic(name) {
            found.write = true;
            found.effects = true;
        }
    }
    if unmodeled && !is_trapping_arithmetic(name) {
        found.effects = true;
    }
    if matches!(name_of(inst.opcode), "GLOBAL_SET_I32" | "GLOBAL_SET_I64")
        && inst.operands.first().is_some_and(|o| {
            matches!(
                o,
                Operand::Sym(at) if f.symbols[*at as usize].0 == "__stack_pointer"
            )
        })
    {
        found.stack_pointer = true;
    }
    if is_call {
        if inst.operands.iter().any(|o| {
            matches!(
                o,
                Operand::Sym(at) if f.symbols[*at as usize].0 == "__wasm_get_stack_pointer"
            )
        }) {
            found.stack_pointer = true;
        }
        query_callee(f, inst, &mut found);
    }
    found
}

/// `WebAssembly::isCatch`.
fn is_catch(opcode: u16) -> bool {
    matches!(
        name_of(opcode),
        "CATCH" | "CATCH_REF" | "CATCH_ALL" | "CATCH_ALL_REF" | "CATCH_LEGACY" | "CATCH_ALL_LEGACY"
    )
}

/// What the pass knows about the registers: how many definitions and uses
/// each has, where its first definition is, and the liveness of the blocks.
struct Registers {
    optimize: bool,
    live_out: Vec<llrm_mir::dense::IdSet<VReg>>,
    defs: Vec<u32>,
    uses: Vec<u32>,
    /// The block and opcode of the first definition.
    first_def: Vec<Option<(usize, u16)>>,
    /// The index of each register's definition in a block, for the blocks
    /// searched; dropped when the block changes.
    positions: HashMap<usize, HashMap<VReg, usize>>,
    /// For a register with several definitions, per block: the one block whose
    /// definition is the value live on entry, if the entry has one value.
    entering: HashMap<VReg, Vec<Option<usize>>>,
}

impl Registers {
    fn new(
        f: &Function,
        optimize: bool,
    ) -> Registers {
        let n = f.vreg_count();
        let (mut defs, mut uses) = (vec![0; n], vec![0; n]);
        let mut first_def = vec![None; n];
        for (b, block) in f.blocks.iter().enumerate() {
            for inst in block {
                for operand in &inst.operands {
                    match *operand {
                        Operand::Def(r) => {
                            defs[r as usize] += 1;
                            first_def[r as usize].get_or_insert((b, inst.opcode));
                        }
                        Operand::Use(r) => uses[r as usize] += 1,
                        _ => {}
                    }
                }
            }
        }
        let live_out = if optimize { crate::liveness::liveness(f).1 } else { Vec::new() };
        Registers { optimize, live_out, defs, uses, first_def, positions: HashMap::default(), entering: HashMap::default() }
    }

    /// The block holding the one definition of `reg` whose value is live on
    /// entry to `block`: `getVNInfoBefore` at the block's start, where it is no
    /// merge of two values (a `PHIDef`, which `getVRegDef` answers with no
    /// instruction).
    fn entering_def_block(
        &mut self,
        f: &Function,
        reg: VReg,
        block: usize,
    ) -> Option<usize> {
        if !self.entering.contains_key(&reg) {
            let n = f.blocks.len();
            let defines: Vec<bool> = f.blocks.iter().map(|b| b.iter().any(|inst| inst.defs().any(|r| r == reg))).collect();
            // The definitions (by block) that reach each block's entry.
            let mut reaching: Vec<Vec<usize>> = vec![Vec::new(); n];
            let mut work: Vec<usize> = (0..n).filter(|&b| defines[b]).collect();
            while let Some(b) = work.pop() {
                let out: Vec<usize> = if defines[b] { vec![b] } else { reaching[b].clone() };
                for &s in &f.succs[b] {
                    let mut grew = false;
                    for &d in &out {
                        if !reaching[s].contains(&d) {
                            reaching[s].push(d);
                            grew = true;
                        }
                    }
                    if grew && !defines[s] {
                        work.push(s);
                    }
                }
            }
            let found = reaching.iter().map(|r| if let [one] = r[..] { Some(one) } else { None }).collect();
            self.entering.insert(reg, found);
        }
        self.entering[&reg][block]
    }

    /// `MRI.createVirtualRegister` with one definition and one use.
    fn new_reg(
        &mut self,
        f: &mut Function,
        ty: ValType,
        block: usize,
        opcode: u16,
    ) -> VReg {
        let reg = f.new_vreg(ty);
        self.defs.push(1);
        self.uses.push(1);
        self.first_def.push(Some((block, opcode)));
        reg
    }

    fn live_out(
        &self,
        block: usize,
        reg: VReg,
    ) -> bool {
        self.live_out[block].contains(&reg)
    }

    /// The index of the definition of `reg` in `block`.
    fn position(
        &mut self,
        f: &Function,
        block: usize,
        reg: VReg,
    ) -> Option<usize> {
        let found = self
            .positions
            .entry(block)
            .or_insert_with(
                || {
                    let mut found = HashMap::default();
                    for (i, inst) in f.blocks[block].iter().enumerate() {
                        for r in inst.defs() {
                            found.entry(r).or_insert(i);
                        }
                    }
                    found
                },
            );
        found.get(&reg).copied()
    }
}

/// A definition: its block, index and opcode.
struct Site {
    block: usize,
    index: usize,
}

/// `getVRegDef`: the definition `reg` has at instruction `insert` of `block`.
fn reaching_def(
    f: &Function,
    regs: &mut Registers,
    block: usize,
    reg: VReg,
    insert: usize,
) -> Option<Site> {
    let above = || f.blocks[block][..insert].iter().rposition(|inst| inst.defs().any(|r| r == reg));
    if regs.defs[reg as usize] == 1 {
        let (def_block, _) = regs.first_def[reg as usize]?;
        if def_block == block {
            return above().map(|index| Site { block, index });
        }
        let index = regs.position(f, def_block, reg)?;
        return Some(Site { block: def_block, index });
    }
    // `LiveIntervals` finds the value of a register with several definitions.
    if !regs.optimize {
        return None;
    }
    if let Some(index) = above() {
        return Some(Site { block, index });
    }
    // None above in this block: the value that enters it, when one definition
    // reaches (`LiveIntervals::getVNInfoBefore` crosses blocks). A definition
    // further down this block reaches round a loop and merges with the entry's.
    let def_block = regs.entering_def_block(f, reg, block).filter(|&d| d != block)?;
    let index = f.blocks[def_block].iter().rposition(|inst| inst.defs().any(|r| r == reg))?;
    Some(Site { block: def_block, index })
}

/// `hasSingleUse`: `reg`, defined at `def` of `block` (where the pass is
/// working), is read once.
fn has_single_use(
    f: &Function,
    regs: &Registers,
    block: usize,
    reg: VReg,
    def: Option<usize>,
) -> bool {
    if !regs.optimize {
        return regs.uses[reg as usize] == 1;
    }
    if regs.uses[reg as usize] == 1 {
        return true;
    }
    let Some(def) = def else { return false };
    let mut reads = 0;
    let mut redefined = false;
    for inst in &f.blocks[block][def + 1..] {
        reads += inst.operands.iter().filter(|o| **o == Operand::Use(reg)).count();
        if inst.defs().any(|r| r == reg) {
            redefined = true;
            break;
        }
    }
    reads == 1 && (redefined || !regs.live_out(block, reg))
}

/// `isSafeToMove`: the definition at `def` can move down to just before
/// `insert`.
fn is_safe_to_move(
    f: &Function,
    regs: &Registers,
    block: usize,
    def: usize,
    insert: usize,
) -> bool {
    let inst = &f.blocks[block][def];
    if inst.defs().count() > 1 {
        return false;
    }
    if def + 1 == insert {
        return true;
    }
    if !regs.optimize || is_catch(inst.opcode) {
        return false;
    }
    // A register with several definitions has different values at different
    // times: no definition of it may come in between.
    let mutable: Vec<VReg> = inst
        .operands
        .iter()
        .filter_map(|o| if let Operand::Use(r) = o { Some(*r) } else { None })
        .filter(|r| regs.defs[*r as usize] != 1)
        .collect();
    let moved = query(f, inst);
    if !moved.read && !moved.write && !moved.effects && !moved.stack_pointer && mutable.is_empty() {
        return true;
    }
    for between in &f.blocks[block][def + 1..insert] {
        let other = query(f, between);
        if (moved.effects && other.effects)
            || (moved.read && other.write)
            || (moved.write && (other.read || other.write))
            || (moved.stack_pointer && other.stack_pointer)
        {
            return false;
        }
        if mutable.iter().any(|r| between.defs().any(|d| d == *r)) {
            return false;
        }
    }
    true
}

/// `oneUseDominatesOtherUses`: the use at operand `index` of `at`, of `reg`
/// defined at `def`, comes first among the uses of that value in stack order.
fn one_use_dominates_other_uses(
    f: &Function,
    regs: &Registers,
    block: usize,
    reg: VReg,
    (at, index): (usize, usize),
    def: usize,
) -> bool {
    let insts = &f.blocks[block];
    // Another use in the instruction itself must come after this one.
    if insts[at].operands[..index].contains(&Operand::Use(reg)) {
        return false;
    }
    let end = insts[def + 1..].iter().position(|i| i.defs().any(|r| r == reg)).map_or(insts.len(), |p| def + 1 + p);
    for earlier in def + 1..at.min(end) {
        if !insts[earlier].operands.contains(&Operand::Use(reg)) {
            continue;
        }
        // Only a use that is stackified into this one comes after it.
        let mut user = earlier;
        loop {
            let [Operand::Def(result), ..] = insts[user].operands[..] else { return false };
            if !f.is_stackified(result) {
                return false;
            }
            let Some(next) = (user + 1..insts.len()).find(|&i| insts[i].operands.contains(&Operand::Use(result)))
            else {
                return false;
            };
            if next == at {
                if insts[at].operands.iter().position(|o| *o == Operand::Use(result)).is_some_and(|p| p < index) {
                    return false;
                }
                break;
            }
            if next > at {
                break;
            }
            user = next;
        }
    }
    // The value reaches other blocks only through the definition, which
    // dominates them, and so does this use.
    let _ = regs;
    true
}

/// `shouldRematerialize`: a constant.
fn should_rematerialize(inst: &Inst) -> bool {
    let Some(info) = desc(inst.opcode) else { return false };
    info.has(flag::CHEAP_AS_A_MOVE)
        && info.has(flag::REMATERIALIZABLE)
        && (matches!(
            inst.opcode,
            op::CONST_I32 | op::CONST_I64 | op::CONST_F32 | op::CONST_F64
        )
            || (!info.has(flag::MAY_STORE)
                && !info.has(flag::MAY_LOAD)
                && !info.has(flag::UNMODELED_SIDE_EFFECTS)
                && !inst.operands.iter().any(|o| matches!(o, Operand::Use(_)))))
}

/// Whether anything was stackified. `optimize` is the pin's `Optimize`: every
/// level but `-O0`.
pub fn reg_stackify(
    f: &mut Function,
    optimize: bool,
) -> bool {
    let mut regs = Registers::new(f, optimize);
    let mut changed = false;
    for b in 0..f.blocks.len() {
        changed |= stackify_block(f, &mut regs, b);
    }
    changed
}

fn stackify_block(
    f: &mut Function,
    regs: &mut Registers,
    b: usize,
) -> bool {
    let mut changed = false;
    let mut mii = f.blocks[b].len() as isize - 1;
    while mii >= 0 {
        let mut insert = mii as usize;
        let mut stackified = false;
        let mut commuting = Commuting::default();
        let mut walker = TreeWalker::new(f, b, insert);
        while !walker.done() {
            let (at, index) = walker.pop(f);
            let Operand::Use(reg) = f.blocks[b][at].operands[index] else { continue };
            let Some(def) = reaching_def(f, regs, b, reg, insert) else { continue };
            if is_argument(f.blocks[def.block][def.index].opcode) {
                continue;
            }
            let same_block = def.block == b;
            let can_move = same_block && is_safe_to_move(f, regs, b, def.index, insert) && !walker.is_on_stack(f, reg);
            let single = can_move && has_single_use(f, regs, b, reg, Some(def.index));
            if single {
                insert = move_for_single_use(f, regs, b, &mut walker, reg, (at, index), def.index, insert);
            } else if regs.optimize && should_rematerialize(&f.blocks[def.block][def.index]) {
                insert = rematerialize(f, regs, b, &mut walker, reg, (at, index), &def, insert);
            } else if regs.optimize && can_move && one_use_dominates_other_uses(f, regs, b, reg, (at, index), def.index)
            {
                insert = move_and_tee(f, regs, b, &mut walker, reg, (at, index), def.index, insert);
            } else {
                if !can_move && same_block {
                    commuting.maybe_commute(f, insert, &mut walker);
                }
                continue;
            }
            stackified = true;
            // The pin then stackifies the defs after the first of a multivalue
            // def; `isSafeToMove` refuses every instruction with several, so
            // there are none here.
            if f.blocks[b][insert].opcode == IMPLICIT_DEF {
                convert_implicit_def_to_const_zero(f, b, insert);
            }
            commuting.reset();
            walker.push_operands(f, insert);
        }
        if stackified {
            mii = insert as isize;
            changed = true;
        }
        mii -= 1;
    }
    changed
}

/// `convertImplicitDefToConstZero`: the push must be explicit.
fn convert_implicit_def_to_const_zero(
    f: &mut Function,
    b: usize,
    at: usize,
) {
    let [Operand::Def(reg)] = f.blocks[b][at].operands[..] else { return };
    let (opcode, zero) = match f.vreg_type(reg) {
        ValType::I32 => (op::CONST_I32, Operand::Imm(0)),
        ValType::I64 => (op::CONST_I64, Operand::Imm(0)),
        ValType::F32 => (op::CONST_F32, Operand::F32(0)),
        ValType::F64 => (op::CONST_F64, Operand::F64(0)),
    };
    f.blocks[b][at] = Inst::new(opcode, &[Operand::Def(reg), zero]);
}

/// Replace the `Def` of `reg` in `inst` by `new`.
fn redefine(
    inst: &mut Inst,
    reg: VReg,
    new: VReg,
) {
    for operand in &mut inst.operands {
        if *operand == Operand::Def(reg) {
            *operand = Operand::Def(new);
        }
    }
}

/// `moveForSingleUse`: the definition at `def` moves down to `insert`.
#[allow(clippy::too_many_arguments)]
fn move_for_single_use(
    f: &mut Function,
    regs: &mut Registers,
    b: usize,
    _walker: &mut TreeWalker,
    reg: VReg,
    (at, index): (usize, usize),
    def: usize,
    insert: usize,
) -> usize {
    f.blocks[b][def..insert].rotate_left(1);
    regs.positions.remove(&b);
    if regs.defs[reg as usize] == 1 && regs.uses[reg as usize] == 1 {
        f.stackify(reg);
    } else {
        // The register has unrelated uses or definitions: this definition and
        // use get a register of their own.
        let ty = f.vreg_type(reg);
        let opcode = f.blocks[b][insert - 1].opcode;
        let new = regs.new_reg(f, ty, b, opcode);
        f.blocks[b][at].operands[index] = Operand::Use(new);
        redefine(&mut f.blocks[b][insert - 1], reg, new);
        regs.defs[reg as usize] -= 1;
        regs.uses[reg as usize] -= 1;
        f.stackify(new);
    }
    insert - 1
}

/// `rematerializeCheapDef`: a clone of the constant `def` goes before `insert`.
#[allow(clippy::too_many_arguments)]
fn rematerialize(
    f: &mut Function,
    regs: &mut Registers,
    b: usize,
    walker: &mut TreeWalker,
    reg: VReg,
    (at, index): (usize, usize),
    def: &Site,
    insert: usize,
) -> usize {
    let (ty, opcode) = (f.vreg_type(reg), f.blocks[def.block][def.index].opcode);
    let new = regs.new_reg(f, ty, b, opcode);
    let mut clone = f.blocks[def.block][def.index].clone();
    redefine(&mut clone, reg, new);
    f.blocks[b].insert(insert, clone);
    walker.shift(insert, 1);
    f.blocks[b][at + 1].operands[index] = Operand::Use(new);
    f.stackify(new);
    regs.positions.remove(&b);
    regs.uses[reg as usize] -= 1;
    let mut insert = insert;
    if regs.defs[reg as usize] == 1 && regs.uses[reg as usize] == 0 {
        // The last use of the original: it goes.
        let index = def.index;
        f.blocks[def.block].remove(index);
        regs.positions.remove(&def.block);
        regs.defs[reg as usize] = 0;
        if def.block == b {
            walker.shift(index, -1);
            insert -= 1;
        }
    }
    insert
}

/// `moveAndTeeForMultiUse`: the definition at `def` moves down to `insert`,
/// and a `TEE` after it keeps the value in `reg` for the other uses.
#[allow(clippy::too_many_arguments)]
fn move_and_tee(
    f: &mut Function,
    regs: &mut Registers,
    b: usize,
    walker: &mut TreeWalker,
    reg: VReg,
    (at, index): (usize, usize),
    def: usize,
    insert: usize,
) -> usize {
    let ty = f.vreg_type(reg);
    let opcode = f.blocks[b][def].opcode;
    let tee_reg = regs.new_reg(f, ty, b, ty.tee());
    let def_reg = regs.new_reg(f, ty, b, opcode);
    f.blocks[b][def..insert].rotate_left(1);
    redefine(&mut f.blocks[b][insert - 1], reg, def_reg);
    let tee = Inst::new(ty.tee(), &[Operand::Def(tee_reg), Operand::Def(reg), Operand::Use(def_reg)]);
    f.blocks[b].insert(insert, tee);
    walker.shift(insert, 1);
    f.blocks[b][at + 1].operands[index] = Operand::Use(tee_reg);
    regs.uses[reg as usize] -= 1;
    regs.positions.remove(&b);
    f.stackify(def_reg);
    f.stackify(tee_reg);
    insert - 1
}
