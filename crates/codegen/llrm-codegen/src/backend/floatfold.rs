//! A float load that only one arithmetic instruction reads becomes that
//! instruction's memory operand: `fld [m]; faddp` is `fadd [m]`.
//!
//! DEVIATE FF1 (owner m16, exit 2026-11-17): a volatile cell folds too; the
//! fused instruction stays volatile and ordered against every other volatile
//! access. The pin keeps `fld [m]` for it. Cost rule: the fold drops an `fld`
//! and a stack slot and keeps the one memory read, so it never costs a byte or
//! a clock; the exit is the pin's own fold, or this stays with the exit moved.
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
//!   plain `fld` or `fild` of a memory cell, with no clobbers and not part of a
//!   group;
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

use crate::backend::constpool;
use crate::backend::floatalloc::{_loads_memory, _reversed, _two_values, cell_of, name_is, semantics};
use crate::backend::select;
use crate::backend::storedhomes::may_write;
use crate::model::ir::{Addr, Loc, Mem, Operation, Space};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::{HashMap, HashSet, IndexMap, IndexSet};

pub struct FloatFold<'a> {
    pub cpu: &'a crate::backend::cpu::Profile,
    /// Where a constant folded into an operand is kept (`fold_constants`);
    /// none: constants stay loaded.
    pub pool: Option<std::rc::Rc<std::cell::RefCell<crate::backend::constpool::Pool>>>,
    /// How many registers an integer value may take: the pressure a refold
    /// may not take a block over.
    pub registers: usize,
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
        let mut pool = self.pool.as_ref().map(|pool| pool.borrow_mut());
        Ok(folded_with(&body, self.cpu, self.registers, pool.as_deref_mut()))
    }
}

/// `body` with each float load its one reader takes from memory read there.
pub fn folded(
    body: &LirBody,
    cpu: &crate::backend::cpu::Profile,
    registers: usize,
) -> LirBody {
    folded_with(body, cpu, registers, None)
}

/// [`folded`], with a pool to keep the constants it folds in.
pub fn folded_with(
    body: &LirBody,
    cpu: &crate::backend::cpu::Profile,
    registers: usize,
    pool: Option<&mut crate::backend::constpool::Pool>,
) -> LirBody {
    let mut uses: HashMap<u32, usize> = HashMap::default();
    let mut defs: HashMap<u32, usize> = HashMap::default();
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
                *defs.entry(held.value).or_default() += usize::from(!read);
            }
        }
    }
    let constants = pool.and_then(|pool| fold_constants(body, &uses, &defs, cpu, pool));
    let body = constants.as_ref().unwrap_or(body);
    let mut blocks = Vec::new();
    let mut changed = constants.is_some();
    for block in &body.blocks {
        let mut insns: Vec<Arc<Insn>> = block.insns.to_vec();
        let mut at = 0;
        while at < insns.len() {
            let one = Arc::clone(&insns[at]);
            if let Some((home, loaded, read)) = _repaired(&insns, at, &uses, &floating, cpu) {
                insns[home] = loaded;
                insns[at] = read;
                changed = true;
                at += 1;
                continue;
            }
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
            let fused = _fused(&one, &load, value, fused_what, &floating);
            insns[at] = fused;
            insns.remove(home);
            changed = true;
            // `at` now names the instruction after the fused one's old place.
        }
        blocks.push(block.with_insns(insns));
    }
    let folded = if changed { body.with_blocks(blocks) } else { body.clone() };
    _refolded(&folded, &uses, &defs, &floating, cpu, registers).unwrap_or(folded)
}

/// A constant `fldz` or `fld1` that one arithmetic instruction reads becomes
/// that instruction's memory operand, the constant kept in the pool: `fld1;
/// faddp` is `fadd dword [K]`.
///
/// DEVIATE FF2 (owner x87, exit 2026-11-28): the pin keeps `fld1`/`fldz` for
/// every use (LLVM's legal x87 immediates) and gcc folds them when the memory
/// form is cheaper. The same rule as a load: the clocks the fold saves
/// (`memory_form`, per CPU) and the bytes at the target's exchange rate; the
/// memory form is the longer (6 bytes against 4 at m32) and the pool keeps the
/// constant besides, so code whose bytes are strict (-O1, -Os, -Oz) keeps the
/// constant: -O1 m16 x_ld_sum grew 9 B with the 486 rows, two new pool cells.
/// x_ld_sum -O2 m32: `x[r] += 1` ran `fld1; faddp` (12 clocks) against `fadd
/// dword [K]` (8); in the loops of x_ld_poly 129 of them.
fn fold_constants(
    body: &LirBody,
    uses: &HashMap<u32, usize>,
    defs: &HashMap<u32, usize>,
    cpu: &crate::backend::cpu::Profile,
    pool: &mut crate::backend::constpool::Pool,
) -> Option<LirBody> {
    if cpu.bytes_strict() {
        return None;
    }
    let mut blocks = Vec::new();
    let mut changed = false;
    for block in &body.blocks {
        let mut insns: Vec<Arc<Insn>> = block.insns.to_vec();
        let mut at = 0;
        while at < insns.len() {
            let one = Arc::clone(&insns[at]);
            let constant = one
                .what
                .as_ref()
                .filter(
                    |what| what.op == Operation::FloatLoad
                        && what.sources.is_empty()
                        && matches!(what.dests.as_slice(), [Loc::Held(held)] if held.width == 10),
                );
            let Some(((value, one_value), what)) = constant.and_then(|what| {
                let Loc::Held(held) = &what.dests[0] else { return None };
                let one_value = match what.name.as_deref()? {
                    "fldz" => false,
                    "fld1" => true,
                    _ => return None,
                };
                Some(((held.value, one_value), what))
            }) else {
                at += 1;
                continue;
            };
            let _ = what;
            if uses.get(&value) != Some(&1)
                || defs.get(&value) != Some(&1)
                || !one.clobbers.is_empty()
                || one.group.is_some()
            {
                at += 1;
                continue;
            }
            // Its reader: the next instruction in the block that names it.
            let Some(reader) = (at + 1..insns.len()).find(|&next| {
                insns[next]
                    .what
                    .as_ref()
                    .is_some_and(|w| w.sources.iter().any(|s| matches!(s, Loc::Held(h) if h.value == value)))
            }) else {
                at += 1;
                continue;
            };
            let Some(arith) =
                insns[reader].what.as_ref().filter(|w| w.op == Operation::FloatArith && w.dests.len() == 1)
            else {
                at += 1;
                continue;
            };
            let Some((name, left, right)) =
                _two_values(Some(arith)).filter(|(_, left, right)| left.value != right.value)
            else {
                at += 1;
                continue;
            };
            let cell_is_left = left.value == value;
            // The memory form reads and writes `st(0)`: the other operand must
            // be the float the block defined last, or an exchange (or the
            // constant loaded again) follows, which ties in clocks and costs
            // bytes (x_nbody_f -O3 m32: +378 clocks, a nest laid out again;
            // x_ld_axpy -O2: `fld [K]` for `fld1`).
            let other = if cell_is_left { right.value } else { left.value };
            // Only a value still read at or after the reader is on the stack:
            // one a `fistp` or a popping operation consumed is gone (x_ldconv
            // -O2: the folds counted as exchanges lost 3,999 clocks).
            let newest = insns[..reader]
                .iter()
                .enumerate()
                .rev()
                .find_map(
                    |(index, earlier)| {
                        if index == at {
                            return None;
                        }
                        let what = earlier.what.as_ref()?;
                        what.dests.iter().find_map(|dest| match dest {
                            Loc::Held(held) if held.width == 10 && earlier.defines.contains(&held.value) => {
                                let read_between = insns[index + 1..reader]
                                    .iter()
                                    .filter_map(|between| between.what.as_ref())
                                    .flat_map(|what| &what.sources)
                                    .filter(|source| matches!(source, Loc::Held(read) if read.value == held.value))
                                    .count();
                                (uses.get(&held.value).copied().unwrap_or(0) > read_between).then_some(held.value)
                            }
                            _ => None,
                        })
                    },
                );
            if newest != Some(other) {
                at += 1;
                continue;
            }
            let bytes = constpool::narrowest(if one_value { 1.0 } else { 0.0 });
            let Some((_, saved)) = memory_form_of(&name, cell_is_left, false, bytes.len() as u32, cpu) else {
                at += 1;
                continue;
            };
            // Gone: the constant's load and the register form's operand
            // encoding; added: the memory form's.
            let probe = Mem {
                disp_width: 2,
                ..Mem::new(Some(Addr { index: 0, ..Addr::new(Space::Segment, 0) }), bytes.len() as u32)
            };
            // A constant the pool does not hold yet is a cell of its own, which
            // the fold pays for once (the fold of a repeated constant reuses
            // it).
            let held = pool.holds(&bytes);
            if cpu.pool_data_strict && !held {
                at += 1;
                continue;
            }
            let cell_bytes = if held { 0 } else { bytes.len() as i64 };
            let gained = 2 + REGISTER_FORM - _memory_operation_bytes(&probe, body.bits) - cell_bytes;
            let rate = cpu.speed_milliclocks_per_byte();
            let pays = if rate == 0 { saved > 0 && gained >= 0 } else { saved * 1000 + gained * rate > 0 };
            if !pays {
                at += 1;
                continue;
            }
            let cell = pool.cell(bytes);
            let Some(fused) = read_from_cell(&insns[reader], value, &cell) else {
                at += 1;
                continue;
            };
            insns[reader] = Arc::new(fused);
            insns.remove(at);
            changed = true;
        }
        blocks.push(block.with_insns(insns));
    }
    changed.then(|| body.with_blocks(blocks))
}

/// Bytes of an x87 register form (`fmul st, st(i)`, `fld st(i)`).
const REGISTER_FORM: i64 = 2;

/// Bytes of an x87 instruction naming `cell` in a `bits`-bit body: opcode and
/// mode, the displacement (none from a register alone, a byte where it fits,
/// a word or a dword otherwise, for a symbol or a far one), a scale-index byte
/// for an indexed 32-bit address and a segment override for a far pointer.
fn _memory_operation_bytes(
    cell: &Mem,
    bits: u32,
) -> i64 {
    let model = llrm_target::object::CodeModel::of(bits);
    let register = cell.base.is_some() || cell.index.is_some();
    let wide = model.displacement_bytes() as i64;
    let displacement = match cell.addr {
        Some(addr) if addr.space == crate::model::ir::Space::Frame => 1,
        Some(_) => wide,
        None if cell.offset == 0 && register => 0,
        None if (-128..128).contains(&cell.offset) => 1,
        None => wide,
    };
    2 + displacement + i64::from(model.flat() && cell.index.is_some()) + i64::from(cell.selector.is_some())
}

/// The loads a held cell takes: its own, and one more at each read a call
/// separates from the one before it (or from the load), as a call empties the
/// x87 stack and the cell is loaded again where it is read next.
fn _loads_needed(
    body: &LirBody,
    home: (usize, usize),
    places: &[(usize, usize)],
) -> i64 {
    let mut loads = 1;
    for (index, place) in places.iter().enumerate() {
        let previous = places[..index].iter().rev().find(|earlier| earlier.0 == place.0).map(|earlier| earlier.1);
        let from = match previous {
            Some(at) => at + 1,
            None if place.0 == home.0 => home.1 + 1,
            None => 0,
        };
        let calls =
            |one: &Arc<Insn>| one.call.is_some() || one.what.as_ref().is_some_and(|what| what.op == Operation::Call);
        if body.blocks[place.0].insns[from..place.1].iter().any(calls) {
            loads += 1;
        }
    }
    loads
}

/// A load that arithmetic instructions read, each taking the cell as its
/// memory operand and the load deleted: gcc's `REG_EQUIV` through x87's memory
/// forms, where LLVM folds only a load with one use in its block.
///
/// Holding the value costs its `fld` and a register-form operation at each
/// read; the memory forms cost each their encoding (`_memory_operation_bytes`,
/// which the unplaced cell is priced by, as `float_memory` cannot place it).
/// The fold is taken when the clocks it saves, summed over the reads
/// (`memory_form`, per CPU), and the bytes at the target's exchange rate are
/// positive; in code built for size the bytes alone must be. Every read must
/// fold, and the cell must hold from the load to each read along every path
/// (`bodyfacts::_unchanged_near`: writes, calls and the optimizer's `spares`
/// pairs); the value has one definition and no reader but those, and the
/// pointer the cell is read through must not be kept live over a block that has
/// no register for it (`_fits_registers`). DEVIATE (beyond LLVM): the pin folds
/// a single use only. One proof a candidate, each a pass over the body.
fn _refolded(
    body: &LirBody,
    uses: &HashMap<u32, usize>,
    defs: &HashMap<u32, usize>,
    floating: &HashSet<u32>,
    cpu: &crate::backend::cpu::Profile,
    registers: usize,
) -> Option<LirBody> {
    let mut loads: IndexMap<u32, (usize, usize)> = IndexMap::default();
    for (block, made) in body.blocks.iter().enumerate() {
        for (at, one) in made.insns.iter().enumerate() {
            let value = one.defines.first().copied().unwrap_or(0);
            if one.defines.len() == 1
                && _loads_memory(one.what.as_ref())
                && defs.get(&value) == Some(&1)
                && uses.get(&value).is_some_and(|reads| *reads >= 1)
                && one.clobbers.is_empty()
                && one.group.is_none()
            {
                loads.insert(value, (block, at));
            }
        }
    }
    if loads.is_empty() {
        return None;
    }
    let mut readers: HashMap<u32, Vec<(usize, usize)>> = HashMap::default();
    for (block, made) in body.blocks.iter().enumerate() {
        for (at, one) in made.insns.iter().enumerate() {
            for arg in one.what.iter().flat_map(|what| &what.sources) {
                if let Loc::Held(held) = arg
                    && loads.contains_key(&held.value)
                {
                    readers.entry(held.value).or_default().push((block, at));
                }
            }
        }
    }
    let flow = std::cell::OnceCell::new();
    let integers = std::cell::OnceCell::new();
    let runs = std::cell::OnceCell::new();
    let mut insns: Vec<Vec<Arc<Insn>>> = body.blocks.iter().map(|block| block.insns.to_vec()).collect();
    let mut deleted: Vec<(usize, usize)> = Vec::new();
    // An instruction has one memory operand: the first load to take it keeps
    // it.
    let mut taken: HashSet<(usize, usize)> = HashSet::default();
    for (value, home) in &loads {
        let value = *value;
        let load = Arc::clone(&body.blocks[home.0].insns[home.1]);
        let Some(places) = readers
            .get(&value)
            .filter(|places| places.len() == uses[&value] && places.iter().all(|place| !taken.contains(place)))
        else {
            continue;
        };
        let mut plan = Vec::new();
        let (mut extra, mut held) = (0.0, 0);
        let load_clocks = load_clocks(&load, cpu);
        let loaded = _memory_operation_bytes(cell_of(&load), body.bits);
        let (mut folded_bytes, mut foldable) = (0, true);
        for place in places {
            let one = &body.blocks[place.0].insns[place.1];
            let Some((name, left, right)) = one
                .what
                .as_ref()
                .filter(|what| what.op == Operation::FloatArith && what.dests.len() == 1)
                .and_then(|what| _two_values(Some(what)))
                .filter(|(_, left, right)| left.value != right.value)
            else {
                foldable = false;
                break;
            };
            let (cell_is_left, other) = if right.value == value { (false, left) } else { (true, right) };
            let Some((operation, gain)) = memory_form(&name, cell_is_left, &load, cpu) else {
                foldable = false;
                break;
            };
            let what = one.what.as_ref().expect("an arithmetic instruction");
            held += REGISTER_FORM;
            folded_bytes += _memory_operation_bytes(cell_of(&load), body.bits);
            // `gain` counts the load saved once per read, as if the read ran as
            // often as the load: a read in a hotter block pays its operation
            // again on every run and saves the load only as often as the
            // load ran.
            let runs = runs.get_or_init(|| crate::analysis::frequency::Frequency::of(body));
            let (read, made) = (runs.block(body.blocks[place.0].at), runs.block(body.blocks[home.0].at));
            extra += if made > 0.0 { read / made } else { 1.0 } * (gain - load_clocks) as f64;
            plan.push((
                *place,
                semantics(
                    Operation::FloatArith,
                    &operation,
                    what.dests.clone(),
                    vec![Loc::Held(other), Loc::Mem(cell_of(&load).clone())],
                ),
            ));
        }
        if !foldable {
            continue;
        }
        let bytes = loaded * _loads_needed(body, *home, places) + held - folded_bytes;
        // Clocks saved per run of the load, in thousandths.
        let saved = ((load_clocks as f64 + extra) * 1000.0).round() as i64;
        // Built for size the bytes decide, on every target and with no
        // exchange rate; clocks only break a tie. At speed a target with a
        // rate takes clocks for bytes at it, and one without takes neither
        // worse.
        let pays = if cpu.size || cpu.min_size {
            bytes > 0 || (bytes == 0 && saved > 0)
        } else {
            let rate = cpu.speed_milliclocks_per_byte();
            if rate == 0 { saved >= 0 && bytes >= 0 && (saved > 0 || bytes > 0) } else { saved + bytes * rate > 0 }
        };
        if !pays {
            continue;
        }
        let integers = integers.get_or_init(|| Integers::of(body, floating));
        if !integers.fit(body, *home, places, &load, floating, registers) {
            continue;
        }
        let flow = flow.get_or_init(|| crate::backend::bodyfacts::Flow::of(body));
        let proof = crate::backend::bodyfacts::_unchanged_near(body, flow, *home, cell_of(&load), places);
        if !proof {
            continue;
        }
        for (place, what) in plan {
            let one = Arc::clone(&body.blocks[place.0].insns[place.1]);
            insns[place.0][place.1] = _fused(&one, &load, value, what, floating);
            taken.insert(place);
        }
        deleted.push(*home);
    }
    if deleted.is_empty() {
        return None;
    }
    deleted.sort_unstable_by(|a, b| b.cmp(a));
    for (block, at) in deleted {
        insns[block].remove(at);
    }
    let blocks = body.blocks.iter().zip(insns).map(|(block, insns)| block.with_insns(insns)).collect();
    Some(body.with_blocks(blocks))
}

/// Where the integer values live, by block, and how the blocks follow each
/// other: what a refold's reach is checked against.
struct Integers {
    into: crate::backend::liverows::Live,
    out: crate::backend::liverows::Live,
    successors: Vec<Vec<usize>>,
    predecessors: Vec<Vec<usize>>,
}

impl Integers {
    fn of(
        body: &LirBody,
        floating: &HashSet<u32>,
    ) -> Self {
        let (into, out) = crate::backend::liverows::live_among(body, |value| !floating.contains(&value));
        let position: HashMap<i64, usize> = body.blocks.iter().enumerate().map(|(at, block)| (block.at, at)).collect();
        let successors: Vec<Vec<usize>> = body
            .blocks
            .iter()
            .map(|block| block.succ.iter().filter_map(|next| position.get(next).copied()).collect())
            .collect();
        let mut predecessors = vec![Vec::new(); successors.len()];
        for (at, next) in successors.iter().enumerate() {
            for &to in next {
                predecessors[to].push(at);
            }
        }
        Self { into, out, successors, predecessors }
    }

    /// The blocks from `from` along `edges`, `from` included.
    fn reach(
        edges: &[Vec<usize>],
        from: impl IntoIterator<Item = usize>,
    ) -> HashSet<usize> {
        let mut seen: HashSet<usize> = HashSet::default();
        let mut work: Vec<usize> = from.into_iter().collect();
        while let Some(at) = work.pop() {
            if seen.insert(at) {
                work.extend(&edges[at]);
            }
        }
        seen
    }

    /// Whether the registers the pointer of `load` is made of can be kept live
    /// to every reader, in `places`, without taking a block it crosses over
    /// `registers` live integer values. Read from its cell, a load keeps its
    /// pointer where a register-form read would not: a block with no register
    /// to spare spills for it, which the memory form does not repay. The
    /// pressure of a block is the most live at any of its points (a bound, as
    /// the pointer may be live over part of it already).
    fn fit(
        &self,
        body: &LirBody,
        home: (usize, usize),
        places: &[(usize, usize)],
        load: &Insn,
        floating: &HashSet<u32>,
        registers: usize,
    ) -> bool {
        let pointer: Vec<u32> = load.uses.iter().copied().filter(|value| !floating.contains(value)).collect();
        if pointer.is_empty() {
            return true;
        }
        let after = Self::reach(&self.successors, [home.0]);
        let before = Self::reach(&self.predecessors, places.iter().map(|place| place.0));
        let span = after.intersection(&before).copied().chain([home.0]).chain(places.iter().map(|place| place.0));
        let span: IndexSet<usize> = span.collect();
        span.into_iter().all(|at| {
            let (entering, leaving) = (&self.into[&body.blocks[at].at], &self.out[&body.blocks[at].at]);
            let missing = pointer.iter().filter(|value| !(entering.contains(value) && leaving.contains(value))).count();
            missing == 0 || self.pressure(body, at, floating) + missing <= registers
        })
    }

    /// The most integer values live at once in the block at `at`.
    fn pressure(
        &self,
        body: &LirBody,
        at: usize,
        floating: &HashSet<u32>,
    ) -> usize {
        let block = &body.blocks[at];
        let mut live: HashSet<u32> = self.out[&block.at].iter().collect();
        let mut most = live.len();
        for one in block.insns.iter().rev() {
            for value in &one.defines {
                live.remove(value);
            }
            live.extend(one.uses.iter().copied().filter(|value| !floating.contains(value)));
            most = most.max(live.len());
        }
        most
    }
}

/// `one` with `value`, read from `load`'s cell, replaced by the cell: what the
/// fused instruction takes over from the load and drops of the value.
fn _fused(
    one: &Arc<Insn>,
    load: &Arc<Insn>,
    value: u32,
    what: crate::model::ir::Semantics,
    floating: &HashSet<u32>,
) -> Arc<Insn> {
    let mut fused = (**one).clone();
    fused.what = Some(what);
    // A volatile cell is still read once, by the fused instruction, and kept
    // in order against every other volatile access (`_home_of`).
    fused.volatile |= load.volatile;
    let foreign = |values: &[u32]| values.iter().copied().filter(|one| !floating.contains(one)).collect::<Vec<u32>>();
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
    fused.requires = load.requires.iter().chain(&one.requires).copied().collect::<IndexSet<_>>().into_iter().collect();
    fused.symbol = if one.covers.is_some_and(|(start, end)| start != end) { Some(false) } else { one.symbol };
    Arc::new(fused)
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

/// An arithmetic instruction that reads a float cell and an integer `fild`
/// nothing else reads: `fild [i]; fmul dword [K]` becomes `fld dword [K];
/// fimul [i]` where the target prices the integer memory form further below
/// its pair than the float one (486: 13 + 16 - 23 = 6 against 3 + 16 - 14 = 5).
/// The cells are the same two, so the bytes tie (also for size, where the
/// clocks it saves are free); the pin keeps the float form (the DAG folds the
/// cell before the integer load is seen), gcc takes the integer one. DEVIATE
/// (owner x87, exit 2026-11-28).
///
/// Returns the load's place, the `fld` that replaces it and the instruction
/// that reads the integer cell.
fn _repaired(
    insns: &[Arc<Insn>],
    at: usize,
    uses: &HashMap<u32, usize>,
    floating: &HashSet<u32>,
    cpu: &crate::backend::cpu::Profile,
) -> Option<(usize, Arc<Insn>, Arc<Insn>)> {
    let arith = &insns[at];
    let what = arith.what.as_ref().filter(|what| what.op == Operation::FloatArith && what.dests.len() == 1)?;
    let [Loc::Held(held), Loc::Mem(float_cell)] = what.sources.as_slice() else { return None };
    if held.width != 10 || arith.volatile || !matches!(float_cell.width, 4 | 8) || uses.get(&held.value) != Some(&1) {
        return None;
    }
    let home = _home_of(insns, at, held.value)?;
    let load = &insns[home];
    if load.volatile || !name_is(load.what.as_ref()?, "fild") {
        return None;
    }
    // The float cell is read at the load's place now: no write to it, and no
    // register of its address changed, in between.
    if insns[home + 1..at].iter().any(|between| {
        between.defines.iter().any(|defined| arith.uses.contains(defined)) || may_write(between, float_cell, false)
    }) {
        return None;
    }
    let full = what.name.as_deref()?;
    let integer_is_left = !matches!(full, "fsubr" | "fdivr");
    let base = full.strip_suffix('r').filter(|_| !integer_is_left).unwrap_or(full);
    let (operation, saved_integer) = memory_form_of(base, integer_is_left, true, cell_of(load).width, cpu)?;
    let (_, saved_float) = memory_form_of(base, !integer_is_left, false, float_cell.width, cpu)?;
    if saved_integer <= saved_float {
        return None;
    }
    let foreign = |values: &[u32]| values.iter().copied().filter(|one| !floating.contains(one)).collect::<Vec<u32>>();
    let mut loaded = (**load).clone();
    loaded.what = Some(semantics(
        Operation::FloatLoad,
        "fld",
        load.what.as_ref()?.dests.clone(),
        vec![Loc::Mem(float_cell.clone())],
    ));
    loaded.uses = foreign(&arith.uses);
    loaded.widths = arith.widths.iter().copied().filter(|(one, _)| !floating.contains(one)).collect();
    let mut read = (**arith).clone();
    read.what = Some(semantics(
        Operation::FloatArith,
        &operation,
        what.dests.clone(),
        vec![Loc::Held(*held), Loc::Mem(cell_of(load).clone())],
    ));
    read.uses = arith
        .uses
        .iter()
        .copied()
        .filter(|one| floating.contains(one))
        .chain(foreign(&load.uses))
        .collect::<IndexSet<u32>>()
        .into_iter()
        .collect();
    read.widths = arith
        .widths
        .iter()
        .chain(&load.widths)
        .copied()
        .filter(|(one, _)| floating.contains(one) || load.uses.contains(one))
        .collect::<IndexSet<(u32, u32)>>()
        .into_iter()
        .collect();
    Some((home, Arc::new(loaded), Arc::new(read)))
}

/// The load that defines `value`, if it may be read again at `at` as a memory
/// operand: the nearest definition in the block, a plain load of a cell, with
/// no clobbers and not part of a group, and nothing between it
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
    if !_loads_memory(load.what.as_ref()) || !load.clobbers.is_empty() || load.group.is_some() {
        return None;
    }
    let cell = cell_of(load);
    if insns[home + 1..at].iter().any(|between| {
        between.defines.iter().any(|defined| load.uses.contains(defined))
            || between.volatile
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
    memory_form_of(name, cell_is_left, name_is(load.what.as_ref()?, "fild"), cell_of(load).width, cpu)
}

/// The clocks `load` takes: what folding it into its reader saves besides the
/// reader's own operation.
fn load_clocks(
    load: &Insn,
    cpu: &crate::backend::cpu::Profile,
) -> i64 {
    let key = if load.what.as_ref().is_some_and(|what| name_is(what, "fild")) { "x87_load_i" } else { "x87_load" };
    cpu.cost(key).unwrap_or(0)
}

/// [`memory_form`] for a cell of `width` bytes, an integer one if `integer`.
fn memory_form_of(
    name: &str,
    cell_is_left: bool,
    integer: bool,
    width: u32,
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
    if integer {
        let (integer, float) = match name.as_str() {
            "fadd" => ("x87_add_i", "x87_add"),
            "fsub" | "fsubr" => ("x87_add_i", "x87_add"),
            "fmul" => ("x87_mul_i", "x87_mul"),
            "fdiv" | "fdivr" => ("x87_div_i", "x87_div"),
            _ => return None,
        };
        let price = |key: &str| cpu.cost(key).ok();
        let replaced = price("x87_load_i")? + price(float)?;
        if !matches!(width, 2 | 4) || price(integer)? >= replaced {
            return None;
        }
        return Some((format!("fi{}", &name[1..]), replaced - price(integer)?));
    }
    if !matches!(width, 4 | 8) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ir::{Addr, Held, Semantics, Space};

    /// Integer registers of the m16 target.
    const REGISTERS: usize = 6;

    /// The named profile with pool data free, so a test of the price sees the
    /// price alone.
    fn speed_profile(name: &str) -> Result<&'static crate::backend::cpu::Profile, String> {
        let mut profile = crate::backend::cpu::named(name)?.clone();
        profile.pool_data_strict = false;
        Ok(Box::leak(Box::new(profile)))
    }

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 10, offset: 0 })
    }

    fn global(
        symbol: i64,
        disp: i64,
    ) -> Mem {
        Mem::new(Some(Addr { index: symbol, ..Addr::new(Space::External, disp) }), 4)
    }

    fn frame(disp: i64) -> Mem {
        Mem::new(Some(Addr::new(Space::Frame, disp)), 4)
    }

    fn insn(
        at: i64,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
    ) -> Arc<Insn> {
        let value = |args: &[Loc]| -> Vec<u32> {
            args.iter()
                .filter_map(|arg| match arg {
                    Loc::Held(held) => Some(held.value),
                    _ => None,
                })
                .collect()
        };
        let (defines, uses) = (value(&dests), value(&sources));
        let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) };
        Arc::new(Insn::new(at, None, Some(what), defines, uses))
    }

    fn fld(
        at: i64,
        value: u32,
        cell: Mem,
    ) -> Arc<Insn> {
        insn(at, Operation::FloatLoad, "fld", vec![held(value)], vec![Loc::Mem(cell)])
    }

    fn arith(
        at: i64,
        name: &str,
        result: u32,
        left: u32,
        right: u32,
    ) -> Arc<Insn> {
        insn(at, Operation::FloatArith, name, vec![held(result)], vec![held(left), held(right)])
    }

    fn fstp(
        at: i64,
        value: u32,
        cell: Mem,
    ) -> Arc<Insn> {
        insn(at, Operation::FloatStore, "fstp", vec![Loc::Mem(cell)], vec![held(value)])
    }

    fn one_block(insns: Vec<Arc<Insn>>) -> LirBody {
        LirBody::new(
            "fold",
            0,
            vec![crate::model::lir::Block::from(crate::model::lir::LirBlock::new(0, insns))],
            Default::default(),
            Default::default(),
        )
    }

    fn names(body: &LirBody) -> Vec<String> {
        body.blocks
            .iter()
            .flat_map(|block| block.insns.iter())
            .filter(|one| one.what.is_some())
            .map(|one| {
                let what = one.what.as_ref().expect("semantics");
                let cells = what.sources.iter().filter(|arg| matches!(arg, Loc::Mem(_))).count();
                let cells = if what.op == Operation::FloatLoad || what.op == Operation::FloatStore { 0 } else { cells };
                format!("{}{}", what.name.as_deref().unwrap_or(""), if cells > 0 { "+m" } else { "" })
            })
            .collect()
    }

    /// qmove -Os: GVN forwarded two loads of a float to every reader, so the
    /// value was held on the stack (`fld st(1)`, `fxch`, `fstp st(i)`) where
    /// `fmul m32` reads the cell for nothing. A load that several arithmetic
    /// instructions read is read from its cell by each.
    #[test]
    fn test_a_load_read_by_several_arithmetic_instructions_is_read_from_its_cell_by_each() {
        let cpu = speed_profile("486").expect("a profile");
        let body = one_block(vec![
            fld(0, 1, global(0, 0)),
            insn(1, Operation::FloatLoad, "fld1", vec![held(2)], vec![]),
            arith(2, "fmul", 3, 2, 1),
            arith(3, "fadd", 4, 3, 1),
            fstp(4, 4, frame(-4)),
        ]);
        assert_eq!(names(&folded(&body, cpu, REGISTERS)), ["fld1", "fmul+m", "fadd+m", "fstp"]);
    }

    /// `fold_constants` over a pool of its own.
    fn constants_folded(
        body: &LirBody,
        cpu: &crate::backend::cpu::Profile,
    ) -> LirBody {
        let mut pool = crate::backend::constpool::Pool::new(0);
        folded_with(body, cpu, REGISTERS, Some(&mut pool))
    }

    /// x_ld_sum -O2 m32: `x[r] += 1` was `fld1; faddp` (12 clocks, 4 bytes)
    /// where `fadd dword [K]` is 8 clocks and 6 bytes: 129 executed in the
    /// loops of x_ld_poly, 60 clocks over gcc in x_ld_sum. The constant one
    /// arithmetic instruction reads is its memory operand where the target
    /// prices the clocks above the extra bytes.
    #[test]
    fn test_a_constant_one_instruction_reads_is_its_memory_operand_at_speed() {
        let cpu = speed_profile("486").expect("a profile");
        let body = one_block(vec![
            fld(0, 1, global(0, 0)),
            insn(1, Operation::FloatLoad, "fld1", vec![held(2)], vec![]),
            arith(2, "fadd", 3, 1, 2),
            fstp(3, 3, global(0, 0)),
        ]);
        assert_eq!(names(&constants_folded(&body, cpu)), ["fld", "fadd+m", "fstp"]);
        assert_eq!(names(&folded(&body, cpu, REGISTERS)), ["fld1", "fadd+m", "fstp"], "no pool: only the load folds");
    }

    /// x_ld_sum -O1 m16 grew 9 B (x_nbody_f 21) once the table priced `fld1;
    /// faddp` above `fadd [K]`: the code ties and the pool cell is the growth.
    /// Where bytes are strict the constant stays loaded.
    #[test]
    fn test_a_constant_stays_loaded_where_bytes_are_strict() {
        let mut cpu = speed_profile("486").expect("a profile").clone();
        let body = one_block(vec![
            fld(0, 1, global(0, 0)),
            insn(1, Operation::FloatLoad, "fld1", vec![held(2)], vec![]),
            arith(2, "fadd", 3, 1, 2),
            fstp(3, 3, global(0, 0)),
        ]);
        assert_eq!(names(&constants_folded(&body, &cpu)), ["fld", "fadd+m", "fstp"], "-O2: the control");
        cpu.speed_rate = false;
        assert_eq!(names(&constants_folded(&body, &cpu)), ["fld1", "fadd+m", "fstp"], "-O1: the constant stays");
    }

    /// x_jpeg_single -O1 m16: one pool constant read three times, a call
    /// between the reads. A call empties the x87 stack, so the held constant
    /// is loaded again at each read (`fld K; faddp`, 6 B) where the memory form
    /// is 4 B: 3 x 4 + 6 - 3 x 4 = 6 B in favour of folding. The cell is a pool
    /// constant, which the call cannot write.
    #[test]
    fn test_a_pool_constant_read_across_calls_is_read_from_its_cell_where_bytes_are_strict() {
        let mut cpu = speed_profile("486").expect("a profile").clone();
        cpu.speed_rate = false;
        let call = |at: i64| insn(at, Operation::Call, "call", vec![], vec![]);
        let invariant = |one: Arc<Insn>| Arc::new(Insn { invariant: true, ..Insn::clone(&one) });
        let body = |calls: bool| {
            let mut insns = vec![invariant(fld(0, 1, global(0, 0)))];
            for (index, value) in [(1, 10), (3, 11), (5, 12)] {
                insns.push(insn(index, Operation::FloatLoad, "fld1", vec![held(value)], vec![]));
                insns.push(arith(index + 1, "fadd", value + 10, value, 1));
                insns.push(fstp(index + 1, value + 10, frame(-4)));
                if calls {
                    insns.push(call(index + 1));
                }
            }
            one_block(insns)
        };
        let adds = |done: &LirBody| names(done).iter().filter(|name| *name == "fadd+m").count();
        assert_eq!(adds(&folded(&body(true), &cpu, REGISTERS)), 3, "calls split the reads: three loads held");
        assert_eq!(adds(&folded(&body(false), &cpu, REGISTERS)), 0, "no call: one load and three register forms win");
    }

    /// A fold of `fld1` makes a pool cell the code does not show: 4 B for a
    /// float. The 486 saves 3 clocks (`fld1; faddp` 11, `fadd [K]` 8) and
    /// pays it; the Pentium saves 1 (1 + 3 against 3) and does not, unless
    /// another reader already put the constant in the pool.
    #[test]
    fn test_a_constant_fold_pays_for_the_pool_cell_it_makes() {
        let body = one_block(vec![
            fld(0, 1, global(0, 0)),
            insn(1, Operation::FloatLoad, "fld1", vec![held(2)], vec![]),
            arith(2, "fadd", 3, 1, 2),
            fstp(3, 3, global(0, 0)),
        ]);
        let run = |name: &str, held_before: bool| {
            let cpu = speed_profile(name).expect("a profile");
            let mut pool = crate::backend::constpool::Pool::new(0);
            if held_before {
                pool.cell(crate::backend::constpool::narrowest(1.0));
            }
            names(&folded_with(&body, cpu, REGISTERS, Some(&mut pool)))
        };
        assert_eq!(run("486", false), ["fld", "fadd+m", "fstp"], "486: 3 clocks for 4 B");
        assert_eq!(run("P5", false), ["fld1", "fadd+m", "fstp"], "P5: 1 clock does not pay for 4 B");
        assert_eq!(run("P5", true), ["fld", "fadd+m", "fstp"], "P5 with the cell already there");
    }

    /// m16 recfloat -O2: `fld1; faddp` became `fadd dword [K]` with a new 1.0f
    /// cell, +4 B of data against bcc, and data is a strict axis of the
    /// ranking. Where pool data is strict a fold makes no new cell; an
    /// existing one is free.
    #[test]
    fn test_a_fold_makes_no_pool_cell_where_pool_data_is_strict() {
        let body = one_block(vec![
            fld(0, 1, global(0, 0)),
            insn(1, Operation::FloatLoad, "fld1", vec![held(2)], vec![]),
            arith(2, "fadd", 3, 1, 2),
            fstp(3, 3, global(0, 0)),
        ]);
        let run = |held_before: bool| {
            let mut strict = speed_profile("486").expect("a profile").clone();
            strict.pool_data_strict = true;
            let cpu = &strict;
            let mut pool = crate::backend::constpool::Pool::new(0);
            if held_before {
                pool.cell(crate::backend::constpool::narrowest(1.0));
            }
            names(&folded_with(&body, cpu, REGISTERS, Some(&mut pool)))
        };
        assert!(run(false).contains(&"fld1".to_owned()), "a new cell: {:?}", run(false));
        assert!(run(true).iter().any(|n| n.ends_with("+m")), "a held cell: {:?}", run(true));
        assert!(!run(true).contains(&"fld1".to_owned()), "a held cell: {:?}", run(true));
    }

    /// x_nbody_f -O3 m32 and x_ld_axpy -O2: the other operand is not the last
    /// float defined, so `fadd [K]` needs an exchange (or the constant loaded
    /// again): a tie in clocks and 4 bytes more, and in x_nbody_f a nest laid
    /// out again (+378 clocks). The constant stays `fld1`.
    #[test]
    fn test_a_constant_stays_loaded_where_its_memory_form_would_need_an_exchange() {
        let cpu = speed_profile("486").expect("a profile");
        let body = one_block(vec![
            fld(0, 1, global(0, 0)),
            fld(1, 4, global(8, 0)),
            insn(2, Operation::FloatLoad, "fld1", vec![held(2)], vec![]),
            arith(3, "fadd", 3, 1, 2),
            fstp(4, 3, global(0, 0)),
            fstp(5, 4, global(8, 0)),
        ]);
        let done = names(&constants_folded(&body, cpu));
        assert!(done.contains(&"fld1".to_owned()), "{done:?}");
    }

    /// x_ldconv -O2 m32: a float defined after the other operand but already
    /// consumed (stored) is not on the stack, so the operand is the top and the
    /// memory form needs no exchange.
    #[test]
    fn test_a_constant_folds_past_a_newer_value_already_consumed() {
        let cpu = speed_profile("486").expect("a profile");
        let body = one_block(vec![
            fld(0, 1, global(0, 0)),
            fld(1, 4, global(8, 0)),
            fstp(2, 4, global(16, 0)),
            insn(3, Operation::FloatLoad, "fld1", vec![held(2)], vec![]),
            arith(4, "fadd", 3, 1, 2),
            fstp(5, 3, global(0, 0)),
        ]);
        let done = names(&constants_folded(&body, cpu));
        assert!(!done.contains(&"fld1".to_owned()), "{done:?}");
    }

    /// x_ldconv -O2 m32: `fadd v, v` reads a value twice; the count of reads
    /// between its definition and the constant's reader counted the
    /// instruction once, so a value already consumed looked live and the
    /// constant stayed `fld1` (+3,999 clocks against the memory form).
    #[test]
    fn test_a_constant_folds_past_a_newer_value_one_instruction_read_twice() {
        let cpu = speed_profile("486").expect("a profile");
        let body = one_block(vec![
            fld(0, 1, global(0, 0)),
            fld(1, 4, global(8, 0)),
            arith(2, "fadd", 5, 4, 4),
            fstp(3, 5, global(16, 0)),
            insn(4, Operation::FloatLoad, "fld1", vec![held(2)], vec![]),
            arith(5, "fadd", 3, 1, 2),
            fstp(6, 3, global(0, 0)),
        ]);
        let done = names(&constants_folded(&body, cpu));
        assert!(!done.contains(&"fld1".to_owned()), "{done:?}");
    }

    /// x_ld_matmul -O2 m32: the DAG folds `0.5L` into `fmul dword [K]` and
    /// leaves `fild [i]`; the integer memory form saves more (13 + 16 - 23
    /// against 3 + 16 - 14), as gcc has it: `fld [K]; fimul [i]`, +576 clocks.
    #[test]
    fn test_an_integer_load_and_a_float_cell_read_the_integer_one_from_memory_at_speed() {
        let cpu = speed_profile("486").expect("a profile");
        let body = one_block(vec![
            insn(0, Operation::FloatLoad, "fild", vec![held(1)], vec![Loc::Mem(frame(-4))]),
            insn(1, Operation::FloatArith, "fmul", vec![held(3)], vec![held(1), Loc::Mem(global(8, 0))]),
            fstp(2, 3, global(0, 0)),
        ]);
        let done = names(&folded(&body, cpu, REGISTERS));
        assert_eq!(done, ["fld", "fimul+m", "fstp"], "{done:?}");
    }

    /// x_matmul_f -Os m32: `fild [i]; fmul dword [K]` was kept for size, where
    /// gcc has `fld [K]; fimul [i]`: the same two cells, so the same bytes, and
    /// 6 clocks fewer for 288 sites (69,492 against 69,449 clk, the best's).
    #[test]
    fn test_an_integer_load_and_a_float_cell_read_the_integer_one_from_memory_for_size() {
        let mut size = crate::backend::cpu::named("486").expect("a profile").clone();
        size.pool_data_strict = false;
        size.size = true;
        let cpu: &'static crate::backend::cpu::Profile = Box::leak(Box::new(size));
        let body = one_block(vec![
            insn(0, Operation::FloatLoad, "fild", vec![held(1)], vec![Loc::Mem(frame(-4))]),
            insn(1, Operation::FloatArith, "fmul", vec![held(3)], vec![held(1), Loc::Mem(global(8, 0))]),
            fstp(2, 3, global(0, 0)),
        ]);
        let done = names(&folded(&body, cpu, REGISTERS));
        assert_eq!(done, ["fld", "fimul+m", "fstp"], "{done:?}");
    }

    /// The `fild` of a loop-invariant integer, before a loop, and the `fadd`
    /// that reads it each trip.
    fn invariant_integer(looping: bool) -> LirBody {
        let block = |at, insns, succ: Vec<i64>| {
            crate::model::lir::Block::from(crate::model::lir::LirBlock {
                succ: succ.into(),
                ..crate::model::lir::LirBlock::new(at, insns)
            })
        };
        let blocks = vec![
            block(0, vec![insn(0, Operation::FloatLoad, "fild", vec![held(1)], vec![Loc::Mem(frame(-4))])], vec![16]),
            block(
                16,
                vec![
                    insn(16, Operation::FloatLoad, "fld1", vec![held(2)], vec![]),
                    arith(17, "fadd", 3, 2, 1),
                    fstp(18, 3, global(0, 0)),
                ],
                if looping { vec![16, 32] } else { vec![32] },
            ),
            block(32, Vec::new(), vec![]),
        ];
        LirBody::new("fold", 0, blocks, Default::default(), Default::default())
    }

    /// x_ld_to_ulong -O1 m32: the `fild` of `r * 67108864` sat before the
    /// inner loop and `fadd st, st(1)` (8 clocks) read it on every trip; read
    /// from its cell as `fiadd` (20) on every trip it cost 1,719 clocks, the
    /// fold having counted the load's 13 clocks as saved once per read. The
    /// pin and gcc keep the load once. A read in a hotter block than the load
    /// keeps the load; the same code without the loop is folded.
    #[test]
    fn test_an_integer_load_is_kept_for_a_read_in_a_hotter_block() {
        let cpu = speed_profile("486").expect("a profile");
        let looped = invariant_integer(true);
        let runs = crate::analysis::frequency::Frequency::of(&looped);
        assert!(runs.block(16) > runs.block(0), "premise: the read runs more often than the load");
        let straight = names(&folded(&invariant_integer(false), cpu, REGISTERS));
        assert_eq!(straight, ["fld1", "fiadd+m", "fstp"], "premise: the fold is taken without the loop: {straight:?}");
        let kept = names(&folded(&looped, cpu, REGISTERS));
        assert_eq!(kept, ["fild", "fld1", "fadd", "fstp"], "{kept:?}");
    }

    /// The 386 column priced the integer memory form at the 387's numbers
    /// (`fimul` 35 against `fild` 8 + `fmul` 27), where the pair saves
    /// nothing and the pair stays: x87rotate-class m16 code was priced at
    /// fxch 18 and fstp 8 while the bar is the 486 table (fxch 4, `fimul` 23
    /// against 13 + 16).
    #[test]
    fn test_the_386_column_prices_the_integer_memory_form_at_the_486_numbers() {
        let cpu = speed_profile("386").expect("a profile");
        let body = one_block(vec![
            insn(0, Operation::FloatLoad, "fild", vec![held(1)], vec![Loc::Mem(frame(-4))]),
            insn(1, Operation::FloatArith, "fmul", vec![held(3)], vec![held(1), Loc::Mem(global(8, 0))]),
            fstp(2, 3, global(0, 0)),
        ]);
        let done = names(&folded(&body, cpu, REGISTERS));
        assert_eq!(done, ["fld", "fimul+m", "fstp"], "{done:?}");
    }

    /// A constant that two instructions read stays loaded: one `fld1` serves
    /// both, and each memory form would cost its bytes again.
    #[test]
    fn test_a_constant_read_twice_stays_loaded() {
        let cpu = speed_profile("486").expect("a profile");
        let body = one_block(vec![
            fld(0, 1, global(0, 0)),
            insn(1, Operation::FloatLoad, "fld1", vec![held(2)], vec![]),
            arith(2, "fadd", 3, 1, 2),
            arith(3, "fmul", 4, 3, 2),
            fstp(4, 4, global(0, 0)),
        ]);
        assert_eq!(names(&constants_folded(&body, cpu)), ["fld1", "fadd+m", "fmul", "fstp"]);
    }

    /// int_compared_with_float -Os: a volatile float read once by a compare was
    /// `fld [m]; fxch; fcompp` (+24 B over six sites); the compare reads the
    /// cell itself, and stays volatile.
    #[test]
    fn test_a_volatile_load_a_compare_reads_once_is_its_memory_operand() {
        let cpu = speed_profile("486").expect("a profile");
        let volatile = {
            let mut load = (*fld(1, 2, global(0, 0))).clone();
            load.volatile = true;
            Arc::new(load)
        };
        let compare = insn(2, Operation::Compare, "fcom", vec![], vec![held(1), held(2)]);
        let body = one_block(vec![fld(0, 1, frame(-4)), volatile, compare]);
        let done = folded(&body, cpu, REGISTERS);
        assert_eq!(names(&done), ["fld", "fcom+m"]);
        assert!(done.blocks[0].insns.last().expect("an instruction").volatile);
    }

    /// A store to the cell between two reads changes the value the second one
    /// reads: the load stays.
    #[test]
    fn test_a_load_is_not_read_from_its_cell_where_the_cell_is_written_between_the_reads() {
        let cpu = speed_profile("486").expect("a profile");
        let body = one_block(vec![
            fld(0, 1, global(0, 0)),
            insn(1, Operation::FloatLoad, "fld1", vec![held(2)], vec![]),
            arith(2, "fmul", 3, 2, 1),
            fstp(3, 3, global(0, 0)),
            arith(4, "fadd", 4, 2, 1),
            fstp(5, 4, frame(-4)),
        ]);
        assert!(
            names(&folded(&body, cpu, REGISTERS)).contains(&"fld".to_owned()),
            "{:?}",
            names(&folded(&body, cpu, REGISTERS))
        );
    }

    /// One reader that cannot take a memory operand (a store of the value)
    /// keeps the load for all.
    #[test]
    fn test_a_load_one_reader_cannot_fold_is_kept() {
        let cpu = speed_profile("486").expect("a profile");
        let body = one_block(vec![
            fld(0, 1, global(0, 0)),
            insn(1, Operation::FloatLoad, "fld1", vec![held(2)], vec![]),
            arith(2, "fmul", 3, 2, 1),
            fstp(3, 3, frame(-4)),
            fstp(4, 1, frame(-8)),
        ]);
        assert!(
            names(&folded(&body, cpu, REGISTERS)).contains(&"fld".to_owned()),
            "{:?}",
            names(&folded(&body, cpu, REGISTERS))
        );
    }

    /// A load through a pointer (an unplaced cell) read in two blocks.
    fn two_blocks(
        between: Vec<Arc<Insn>>,
        crowd: &[u32],
    ) -> LirBody {
        let pointer = Mem { base: Some(Held { value: 9, width: 2, offset: 0 }), ..Mem::new(None, 4) };
        two_blocks_through(pointer, between, crowd)
    }

    fn two_blocks_through(
        pointer: Mem,
        between: Vec<Arc<Insn>>,
        crowd: &[u32],
    ) -> LirBody {
        let mut load = (*fld(0, 1, pointer)).clone();
        load.uses = vec![9];
        let first = [
            vec![
                Arc::new(load),
                insn(1, Operation::FloatLoad, "fld1", vec![held(2)], vec![]),
                arith(2, "fmul", 3, 2, 1),
            ],
            between,
        ]
        .concat();
        let blocks = vec![
            crate::model::lir::Block::from(crate::model::lir::LirBlock {
                succ: vec![16].into(),
                ..crate::model::lir::LirBlock::new(0, first)
            }),
            crate::model::lir::Block::from(crate::model::lir::LirBlock::new(
                16,
                [
                    vec![Arc::new(Insn::new(15, None, None, Vec::new(), crowd.to_vec()))],
                    vec![arith(16, "fadd", 4, 3, 1), fstp(17, 4, frame(-4))],
                ]
                .concat(),
            )),
        ];
        LirBody::new("fold", 0, blocks, Default::default(), Default::default())
    }

    /// `body` and `fillers` blocks of twenty instructions each that nothing
    /// reaches.
    fn with_fillers(
        body: &LirBody,
        fillers: i64,
    ) -> LirBody {
        let mut blocks = body.blocks.to_vec();
        for block in 1..=fillers {
            let plain = (0..20).map(|n| Arc::new(Insn::new(1000 * block + n, None, None, vec![], vec![]))).collect();
            blocks.push(crate::model::lir::Block::from(crate::model::lir::LirBlock::new(1000 * block, plain)));
        }
        body.with_blocks(blocks)
    }

    /// d_faces -m32 -O2: the proof that a cell holds from a load to its reads
    /// asked `_keeps` of every instruction of the function for each fold
    /// candidate, 41 M of 1,324 M instructions. It is asked of the blocks
    /// between the load and its reads alone: 600 filler instructions cost none.
    #[test]
    fn test_the_proof_a_cell_holds_asks_only_the_blocks_between_the_load_and_its_reads() {
        let cpu = speed_profile("486").expect("a profile");
        let body = with_fillers(&two_blocks(Vec::new(), &[]), 30);
        crate::backend::bodyfacts::KEEPS.with(|asked| asked.set(0));
        let got = names(&folded(&body, cpu, REGISTERS));
        assert_eq!(got, ["fld1", "fmul+m", "fadd+m", "fstp"], "the fold is taken: {got:?}");
        let asked = crate::backend::bodyfacts::KEEPS.with(|asked| asked.get());
        assert!(asked < 30, "{asked} instructions asked of for two blocks of a few instructions");
    }

    /// The same body with a store through another pointer on the way to the
    /// second reader keeps the load: the window finds what the whole body did.
    #[test]
    fn test_a_store_on_the_way_to_a_reader_keeps_the_load_among_unrelated_blocks() {
        let cpu = speed_profile("486").expect("a profile");
        let other = Mem { base: Some(Held { value: 8, width: 2, offset: 0 }), ..Mem::new(None, 4) };
        let mut store = (*fstp(3, 2, other)).clone();
        store.uses = vec![8];
        let body = with_fillers(&two_blocks(vec![Arc::new(store)], &[]), 30);
        let got = names(&folded(&body, cpu, REGISTERS));
        assert!(got.contains(&"fld".to_owned()), "{got:?}");
    }

    /// qmove -Os: the readers of a forwarded load sat in two blocks, so the
    /// value stayed on the stack from one to the other (`fxch`, `fld st(i)`)
    /// and 489 clocks were spent where the cells could be read for 463. A
    /// load nothing writes before any of its readers, in whatever block, is
    /// read from its cell by each.
    #[test]
    fn test_a_load_read_in_two_blocks_is_read_from_its_cell_by_each() {
        let cpu = speed_profile("486").expect("a profile");
        let got = names(&folded(&two_blocks(Vec::new(), &[]), cpu, REGISTERS));
        assert_eq!(got, ["fld1", "fmul+m", "fadd+m", "fstp"], "{got:?}");
    }

    /// A write through another pointer on the way to the second reader may
    /// change the cell: the load stays.
    #[test]
    fn test_a_load_is_kept_where_a_store_through_a_pointer_may_reach_its_cell_before_a_reader() {
        let cpu = speed_profile("486").expect("a profile");
        let other = Mem { base: Some(Held { value: 8, width: 2, offset: 0 }), ..Mem::new(None, 4) };
        let mut store = (*fstp(3, 2, other)).clone();
        store.uses = vec![8];
        let got = names(&folded(&two_blocks(vec![Arc::new(store)], &[]), cpu, REGISTERS));
        assert!(got.contains(&"fld".to_owned()), "{got:?}");
    }

    /// qmove's refold read from the cell kept the pointer live into a block
    /// whose registers were all taken: stft_single bq32 -Os grew 12 bytes (a
    /// spill) for 8 clocks. A pointer that would be live over a block with no
    /// register left for it keeps the load.
    #[test]
    fn test_a_load_is_kept_where_its_pointer_would_be_live_over_a_block_with_no_register_left() {
        let cpu = speed_profile("486").expect("a profile");
        let crowded = two_blocks(Vec::new(), &[20, 21]);
        let got = names(&folded(&crowded, cpu, 3));
        assert!(got.contains(&"fld".to_owned()), "{got:?}");
        let got = names(&folded(&crowded, cpu, 4));
        assert!(!got.contains(&"fld".to_owned()), "{got:?}");
    }

    /// fpbench m32 -O3: `fsub dword ptr [esp+edx+320]` was priced 4 bytes (a
    /// word displacement, no scale-index byte) and is 7, so a refold of
    /// loads through such cells was weighed against the wrong bytes. The
    /// cell's bytes follow the body's width.
    #[test]
    fn test_a_cell_with_an_index_and_a_dword_displacement_costs_seven_bytes_at_32_bits() {
        let pointer = Mem {
            base: Some(Held { value: 9, width: 4, offset: 0 }),
            index: Some(Held { value: 10, width: 4, offset: 0 }),
            offset: 320,
            ..Mem::new(None, 4)
        };
        assert_eq!(_memory_operation_bytes(&pointer, 16), 4);
        assert_eq!(_memory_operation_bytes(&pointer, 32), 7);
    }

    /// A load through a pointer that is written before a reader was read from
    /// the cell at the reader, where the pointer then names another cell: the
    /// reader took the value of `[p + 8]` for that of `[p]`.
    #[test]
    fn test_a_load_is_kept_where_its_pointer_is_written_before_a_reader() {
        let cpu = speed_profile("486").expect("a profile");
        let step = Arc::new(Insn::new(
            9,
            None,
            Some(Semantics {
                name: Some("add".into()),
                dests: vec![Loc::Held(Held { value: 9, width: 2, offset: 0 })],
                sources: vec![
                    Loc::Held(Held { value: 9, width: 2, offset: 0 }),
                    Loc::Imm(llrm_lir::Imm { value: 8, width: 2, address: None }),
                ],
                ..Semantics::new(Operation::Binary)
            }),
            vec![9],
            vec![9],
        ));
        let got = names(&folded(&two_blocks(vec![step], &[]), cpu, REGISTERS));
        assert!(got.contains(&"fld".to_owned()), "{got:?}");
    }

    /// The same inside one block: `fld [p]; p := p + 8; fadd` took `[p + 8]`.
    #[test]
    fn test_a_load_is_kept_where_its_pointer_is_written_before_its_one_reader_in_the_block() {
        let cpu = speed_profile("486").expect("a profile");
        let pointer = Mem { base: Some(Held { value: 9, width: 2, offset: 0 }), ..Mem::new(None, 4) };
        let mut load = (*fld(0, 1, pointer)).clone();
        load.uses = vec![9];
        let step = Arc::new(Insn::new(
            1,
            None,
            Some(Semantics {
                name: Some("add".into()),
                dests: vec![Loc::Held(Held { value: 9, width: 2, offset: 0 })],
                sources: vec![
                    Loc::Held(Held { value: 9, width: 2, offset: 0 }),
                    Loc::Imm(llrm_lir::Imm { value: 8, width: 2, address: None }),
                ],
                ..Semantics::new(Operation::Binary)
            }),
            vec![9],
            vec![9],
        ));
        let body = one_block(vec![
            Arc::new(load),
            insn(0, Operation::FloatLoad, "fld1", vec![held(2)], vec![]),
            step,
            arith(2, "fmul", 3, 2, 1),
            fstp(3, 3, frame(-4)),
        ]);
        let got = names(&folded(&body, cpu, REGISTERS));
        assert!(got.contains(&"fld".to_owned()), "{got:?}");
    }

    /// A size profile of a target with no size rate (m32), the memory form of
    /// `fadd` priced above the pair it replaces (`fld` 6 + `fadd` 6).
    fn priced_memory_form(memory: i64) -> crate::backend::cpu::Profile {
        let mut cpu = crate::backend::cpu::tuned_for(&llrm_x86_m32::M32, "486", true).expect("a profile").clone();
        for (key, price) in [("x87_load", 6), ("x87_add", 6), ("x87_add_m", memory)] {
            std::sync::Arc::make_mut(&mut cpu._costs).retain(|(name, _)| name != key);
            std::sync::Arc::make_mut(&mut cpu._costs).push((key.to_owned(), price));
        }
        cpu
    }

    /// A load read by `readers` arithmetic instructions: the several-reader
    /// refold, the only fold that weighs clocks against bytes (a single reader
    /// folds as the pin does, whatever the clocks).
    fn read_by(
        readers: usize,
        cell: Mem,
    ) -> LirBody {
        let mut insns = vec![fld(0, 1, cell), insn(1, Operation::FloatLoad, "fld1", vec![held(2)], vec![])];
        let mut left = 2;
        for reader in 0..readers {
            let result = 3 + reader as u32;
            insns.push(arith(2 + reader as i64, "fadd", result, left, 1));
            left = result;
        }
        insns.push(fstp(10, left, frame(-4)));
        one_block(insns)
    }

    /// Built for size the bytes decide: two readers of a 3-byte cell save a
    /// byte and, at `fadd m64` 20 against 12, cost 16 clocks; the fold is made
    /// on every target. m32 -Os refused it (no size rate: neither metric may
    /// grow) and m16 took it only through a fitted 10_000 rate.
    #[test]
    fn test_a_refold_that_saves_bytes_and_costs_clocks_is_made_for_size() {
        let cpu = priced_memory_form(20);
        let got = names(&folded(&read_by(2, frame(-8)), &cpu, REGISTERS));
        assert!(!got.contains(&"fld".to_owned()), "{got:?}");
    }

    /// The same refold with the memory form cheaper than the pair.
    #[test]
    fn test_a_refold_that_saves_bytes_and_clocks_is_made_for_size() {
        let cpu = priced_memory_form(6);
        let got = names(&folded(&read_by(2, frame(-8)), &cpu, REGISTERS));
        assert!(!got.contains(&"fld".to_owned()), "{got:?}");
    }

    /// A guard, green before and after: three readers of a 4-byte cell save
    /// clocks and add two bytes; code built for size does not grow.
    #[test]
    fn test_a_refold_that_saves_clocks_and_grows_code_is_not_made_for_size() {
        let cpu = priced_memory_form(6);
        let got = names(&folded(&read_by(3, global(0, 0)), &cpu, REGISTERS));
        assert!(got.contains(&"fld".to_owned()), "{got:?}");
    }
}
