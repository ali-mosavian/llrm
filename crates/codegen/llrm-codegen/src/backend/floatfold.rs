//! A float load that only one arithmetic instruction reads becomes that
//! instruction's memory operand: `fld [m]; faddp` is `fadd [m]`.
//!
//! # How it works
//! x87 arithmetic has a memory form for a float or a double (`fadd qword ptr
//! [m]`). Folding saves the load and the x87 register the loaded value held.
//! LLVM folds a single-use load the same way
//! (`X86InstrInfo::foldMemoryOperand`). `floatassign` folds only the reload of
//! a spilled value; this folds any load, before values are placed.
//!
//! For each two-operand `FloatArith` on two different values, the right operand
//! is tried first (the left stays on the stack top, as `fadd st, [m]` has it),
//! then the left. An operand qualifies when:
//!
//! - it is read exactly once in the body;
//! - its defining instruction is the nearest earlier definition in the block, a
//!   plain `fld` or `fild` of a memory cell, not volatile, with no clobbers and
//!   not part of a group;
//! - nothing between the load and the arithmetic is volatile, a call, a barrier
//!   or an escape, or may write the cell (`storedhomes::may_write`);
//! - the cell is 4 or 8 bytes, or a 2 or 4 byte integer the target prices below
//!   the pair it replaces: `fimul word [m]` for `fild word [m]` and `fmul` (23
//!   clocks against 13 and 16 on a 486; the P5 has no cheaper form, so it keeps
//!   the pair), per operation by the cost keys `x87_load_i`, `x87_add_i`,
//!   `x87_mul_i` and `x87_div_i`. Where both operands qualify, the fold that
//!   saves more clocks is taken: gcc and clang fold the integer load the same
//!   way (`fimul`, `ficomp`). A compare whose right operand is such a load is
//!   `ficom [m]` under the same rule, key `x87_cmp_i`; `floatalloc` emits the
//!   popping `ficomp` where the left dies.
//!
//! Where the loaded cell is the left operand the operation is reversed (`fsub`
//! becomes `fsubr`, `fdiv` becomes `fdivr`). The arithmetic then reads the
//! other value and the cell, and the load is deleted.
//!
//! ```text
//!  before                          after
//!  v1 = fld  [m]                   v2 = fadd v0, [m]
//!  ...  (nothing writes [m])       ...
//!  v2 = fadd v0, v1
//! ```
//!
//! # Example
//! `p.x = p.x * k + p.y; return p.x - k`: each load of a field is read once, so
//! it becomes the memory operand of the arithmetic that reads it. The `fsub`
//! had the loaded cell on its left, so it is the reversed `fsubr`.
//!
//! Before (after `PhiElimination`):
//!
//! ```lir
//! mix: 11 instructions
//!   block 0x0000
//!     0x0000     ; inserted; delivers v1@EAX
//!     0x0000  v2 := fld [[bp+0x8]]  ; inserted
//!     0x0000  v3 := fld [None v1@unplaced]  ; inserted
//!     0x0001  v4 := fmul v3, v2  ; inserted
//!     0x0003  v5 := fld [None v1@unplaced]  ; inserted
//!     0x0004  v6 := fadd v4, v5  ; inserted
//!     0x0005  [None v1@unplaced] := fstp v6  ; inserted
//!     0x0006  v7 := fld [None v1@unplaced]  ; inserted
//!     0x0007  v8 := fsub v7, v2  ; inserted
//!     0x0008   v8  ; inserted
//!     0x0008     ; inserted
//! ```
//!
//! After (`FloatFold`):
//!
//! ```lir
//! mix: 8 instructions
//!   block 0x0000
//!     0x0000     ; inserted; delivers v1@EAX
//!     0x0000  v2 := fld [[bp+0x8]]  ; inserted
//!     0x0001  v4 := fmul v2, [None v1@unplaced]  ; inserted
//!     0x0004  v6 := fadd v4, [None v1@unplaced]  ; inserted
//!     0x0005  [None v1@unplaced] := fstp v6  ; inserted
//!     0x0007  v8 := fsubr v2, [None v1@unplaced]  ; inserted
//!     0x0008   v8  ; inserted
//!     0x0008     ; inserted
//! ```
//!
//! From `ISEL_DUMP=1` of the nib function `mix(p: &mut P, k: f64)` with `-m32
//! -O0`.
//!
//! # Runs
//! Machine phase after `machinesched` and before `floatassign`, once per
//! function. Not gated by an -O level.
//!
//! # Needs
//! [`crate::backend::storedhomes`] for the may-write test; helpers from
//! [`crate::backend::floatalloc`].
//!
//! # See also
//! `floatassign::fused`; LLVM `X86InstrInfo::foldMemoryOperand`.

use std::sync::Arc;

use crate::backend::floatalloc::{_loads_memory, _reversed, _two_values, cell_of, name_is, semantics};
use crate::backend::select;
use crate::backend::storedhomes::may_write;
use crate::model::ir::{Loc, Mem, Operation};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::{HashMap, HashSet, IndexSet};

pub struct FloatFold<'a> {
    pub cpu: &'a crate::backend::cpu::Profile,
}

impl LIRTransform for FloatFold<'_> {
    fn class_name(&self) -> &'static str {
        "FloatFold"
    }

    fn name(&self) -> &str {
        "floatfold"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(folded(&body, self.cpu))
    }
}

/// `body` with each float load its one reader takes from memory read there.
pub fn folded(
    body: &LirBody,
    cpu: &crate::backend::cpu::Profile,
) -> LirBody {
    let mut uses: HashMap<u32, usize> = HashMap::default();
    let mut floating: HashSet<u32> = HashSet::default();
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        for (arg, read) in one.what.iter().flat_map(|what| {
            what.sources.iter().map(|arg| (arg, true)).chain(what.dests.iter().map(|arg| (arg, false)))
        }) {
            if let Loc::Held(held) = arg
                && held.width == 10
            {
                floating.insert(held.value);
                *uses.entry(held.value).or_default() += usize::from(read);
            }
        }
    }
    let mut blocks = Vec::new();
    let mut changed = false;
    for block in &body.blocks {
        let mut insns: Vec<Arc<Insn>> = block.insns.to_vec();
        let mut at = 0;
        while at < insns.len() {
            let one = Arc::clone(&insns[at]);
            let compared = _compare_with_integer(&insns, at, &uses, cpu);
            let arithmetic = one
                .what
                .as_ref()
                .filter(|what| what.op == Operation::FloatArith && what.dests.len() == 1)
                .and_then(|what| _two_values(Some(what)))
                .filter(|(_, left, right)| left.value != right.value);
            if compared.is_none() && arithmetic.is_none() {
                at += 1;
                continue;
            }
            // The right operand's load first: the left is on the stack top, as
            // `fadd st, [m]` has it.
            // Each operand that qualifies, and what folding it saves in clocks;
            // the dearest to leave unfolded goes, the right operand
            // first where they tie.
            let found = arithmetic
                .iter()
                .flat_map(|(name, left, right)| {
                    [(name, right.value, false, *left, *right), (name, left.value, true, *right, *left)]
                })
                .filter_map(|(name, value, cell_is_left, other, _)| {
                    if uses.get(&value) != Some(&1) {
                        return None;
                    }
                    let home = _home_of(&insns, at, value)?;
                    let load = &insns[home];
                    let (operation, saved) = memory_form(name, cell_is_left, load, cpu)?;
                    let what = one.what.as_ref().expect("an arithmetic instruction");
                    let cell = Loc::Mem(cell_of(load).clone());
                    let fused =
                        semantics(Operation::FloatArith, &operation, what.dests.clone(), vec![Loc::Held(other), cell]);
                    Some((saved, (home, value, fused)))
                })
                .reduce(|best, next| if next.0 > best.0 { next } else { best })
                .map(|(_, one)| one)
                .or(compared);
            let Some((home, value, fused_what)) = found else {
                at += 1;
                continue;
            };
            let load = Arc::clone(&insns[home]);
            let mut fused = (*one).clone();
            fused.what = Some(fused_what);
            let foreign =
                |values: &[u32]| values.iter().copied().filter(|one| !floating.contains(one)).collect::<Vec<u32>>();
            fused.uses = one
                .uses
                .iter()
                .copied()
                .filter(|one| *one != value)
                .chain(foreign(&load.uses))
                .collect::<IndexSet<u32>>()
                .into_iter()
                .collect();
            fused.widths = one
                .widths
                .iter()
                .chain(&load.widths)
                .copied()
                .filter(|(one, _)| *one != value && !(floating.contains(one) && load.defines.contains(one)))
                .collect::<IndexSet<(u32, u32)>>()
                .into_iter()
                .collect();
            fused.requires =
                load.requires.iter().chain(&one.requires).copied().collect::<IndexSet<_>>().into_iter().collect();
            fused.symbol = if one.covers.is_some_and(|(start, end)| start != end) { Some(false) } else { one.symbol };
            insns[at] = Arc::new(fused);
            insns.remove(home);
            changed = true;
            // `at` now names the instruction after the fused one's old place.
        }
        blocks.push(block.with_insns(insns));
    }
    if changed { body.with_blocks(blocks) } else { body.clone() }
}

/// `one`, an arithmetic instruction reading the float `value`, with the cell
/// `cell` where it spilled as its memory operand, if x87 has that form: the
/// reload folded (D-RFP2 of the inline spiller). The value is the right
/// operand, or the left with the operation reversed.
pub fn read_from_cell(
    one: &Insn,
    value: u32,
    cell: &Mem,
) -> Option<Insn> {
    let what = one.what.as_ref().filter(|what| what.op == Operation::FloatArith && what.dests.len() == 1)?;
    let (name, left, right) = _two_values(Some(what)).filter(|(_, left, right)| left.value != right.value)?;
    let (cell_is_left, other) = match value {
        _ if value == right.value => (false, left),
        _ if value == left.value => (true, right),
        _ => return None,
    };
    let name = if cell_is_left { _reversed(&name) } else { name.as_str() };
    select::float_memory(name, cell, select::At::bits16(0))?;
    let fused =
        semantics(Operation::FloatArith, name, what.dests.clone(), vec![Loc::Held(other), Loc::Mem(cell.clone())]);
    let mut made = one.clone();
    made.what = Some(fused);
    made.uses.retain(|used| *used != value);
    made.undefs.retain(|used| *used != value);
    made.widths.retain(|(used, _)| *used != value);
    Some(made)
}

/// The load that defines `value`, if it may be read again at `at` as a memory
/// operand: the nearest definition in the block, a plain load of a cell, not
/// volatile, with no clobbers and not part of a group, and nothing between it
/// and `at` volatile, a call, a barrier or an escape, or able to write the
/// cell.
fn _home_of(
    insns: &[Arc<Insn>],
    at: usize,
    value: u32,
) -> Option<usize> {
    let home = (0..at)
        .rev()
        .find(
            |&back| insns[back]
                .what
                .as_ref()
                .is_some_and(|made| made.dests.iter().any(|d| matches!(d, Loc::Held(held) if held.value == value))),
        )?;
    let load = &insns[home];
    if !_loads_memory(load.what.as_ref()) || load.volatile || !load.clobbers.is_empty() || load.group.is_some() {
        return None;
    }
    let cell = cell_of(load);
    if insns[home + 1..at].iter().any(|between| {
        between.volatile
            || between.call.is_some()
            || may_write(between, cell, false)
            || between
                .what
                .as_ref()
                .is_none_or(|w| matches!(w.op, Operation::Call | Operation::Barrier | Operation::Escape))
    }) {
        return None;
    }
    Some(home)
}

/// A compare of a value with a load nothing else reads is `fcom [m]`; with an
/// integer load, `ficom [m]` where the target prices it below the `fild` and
/// the compare it replaces: gcc's and clang's `ficomp`. The cell is the right
/// operand, as `fcom st, [m]` has it. DEVIATE: the pin pushes a float cell
/// and compares the pair (`fld m; fcomp`), 2 to 8 bytes more.
fn _compare_with_integer(
    insns: &[Arc<Insn>],
    at: usize,
    uses: &HashMap<u32, usize>,
    cpu: &crate::backend::cpu::Profile,
) -> Option<(usize, u32, crate::model::ir::Semantics)> {
    let what = insns[at].what.as_ref().filter(|what| what.op == Operation::Compare && what.dests.is_empty())?;
    let [Loc::Held(left), Loc::Held(right)] = what.sources.as_slice() else { return None };
    if left.value == right.value || uses.get(&right.value) != Some(&1) {
        return None;
    }
    let home = _home_of(insns, at, right.value)?;
    let load = &insns[home];
    let integer = name_is(load.what.as_ref()?, "fild");
    if integer {
        if !matches!(cell_of(load).width, 2 | 4) {
            return None;
        }
        let price = |key: &str| cpu.cost(key).ok();
        if price("x87_cmp_i")? >= price("x87_load_i")? + price("x87_add")? {
            return None;
        }
    } else if !matches!(cell_of(load).width, 4 | 8) {
        return None;
    }
    let mut made = what.clone();
    made.name = Some(if integer { "ficom" } else { "fcom" }.into());
    made.sources = vec![Loc::Held(*left), Loc::Mem(cell_of(load).clone())];
    Some((home, right.value, made))
}

/// The memory form of `name` reading `load`'s cell: `fadd qword ptr [m]`,
/// `fsubr dword ptr [m]`; none for an integer or a cell that is not a float or
/// double. Where the cell is the left operand, the operation reversed.
fn memory_form(
    name: &str,
    cell_is_left: bool,
    load: &Insn,
    cpu: &crate::backend::cpu::Profile,
) -> Option<(String, i64)> {
    let reversed = |name: &str| -> String {
        match name {
            "fsub" => "fsubr".to_owned(),
            "fdiv" => "fdivr".to_owned(),
            other => other.to_owned(),
        }
    };
    let name = if cell_is_left { reversed(name) } else { name.to_owned() };
    // An integer load folds into the `fi` form where the target prices that
    // form below the `fild` and the operation it replaces, per operation:
    // the 486 has `fimul` at 23 clocks against 13 and 16, `fiadd` at 20 against
    // 13 and 8.
    if name_is(load.what.as_ref()?, "fild") {
        let (integer, float) = match name.as_str() {
            "fadd" => ("x87_add_i", "x87_add"),
            "fsub" | "fsubr" => ("x87_add_i", "x87_add"),
            "fmul" => ("x87_mul_i", "x87_mul"),
            "fdiv" | "fdivr" => ("x87_div_i", "x87_div"),
            _ => return None,
        };
        let price = |key: &str| cpu.cost(key).ok();
        let replaced = price("x87_load_i")? + price(float)?;
        if !matches!(cell_of(load).width, 2 | 4) || price(integer)? >= replaced {
            return None;
        }
        return Some((format!("fi{}", &name[1..]), replaced - price(integer)?));
    }
    if !matches!(cell_of(load).width, 4 | 8) {
        return None;
    }
    // A float or double cell: the load is saved, and the memory form may cost
    // more than the register one.
    let class = match name.as_str() {
        "fadd" | "fsub" | "fsubr" => "add",
        "fmul" => "mul",
        _ => "div",
    };
    let price = |key: String| cpu.cost(&key).ok();
    let saved = match (price("x87_load".to_owned()), price(format!("x87_{class}")), price(format!("x87_{class}_m"))) {
        (Some(load), Some(register), Some(memory)) => load + register - memory,
        _ => 1,
    };
    Some((name, saved))
}
