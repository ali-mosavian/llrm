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
use crate::support::hash::{HashMap, HashSet, IndexMap, IndexSet};

pub struct FloatFold<'a> {
    pub cpu: &'a crate::backend::cpu::Profile,
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
        Ok(folded(&body, self.cpu, self.registers))
    }
}

/// `body` with each float load its one reader takes from memory read there.
pub fn folded(
    body: &LirBody,
    cpu: &crate::backend::cpu::Profile,
    registers: usize,
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
/// (`bodyfacts::_unchanged`: writes, calls and the optimizer's `spares` pairs);
/// the value has one definition and no reader but those, and the pointer the
/// cell is read through must not be kept live over a block that has no register
/// for it (`_fits_registers`). DEVIATE (beyond LLVM): the pin folds a single
/// use only. One proof a candidate, each a pass over the body.
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
                && !one.volatile
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
        let (mut saved, mut held) = (0, 0);
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
            saved += gain;
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
        let bytes = loaded + held - folded_bytes;
        let rate = if cpu.size || cpu.min_size {
            cpu.size_milliclocks_per_byte()
        } else {
            cpu.target().tuning().speed_milliclocks_per_byte
        };
        // A target with an exchange rate takes clocks for bytes at it, and
        // never grows code built for size; one without takes neither worse.
        let pays = if rate == 0 {
            saved >= 0 && bytes >= 0 && (saved > 0 || bytes > 0)
        } else if cpu.size || cpu.min_size {
            bytes > 0
        } else {
            saved * 1000 + bytes * rate > 0
        };
        if !pays {
            continue;
        }
        let readers_of: Vec<Arc<Insn>> =
            places.iter().map(|place| Arc::clone(&body.blocks[place.0].insns[place.1])).collect();
        let integers = integers.get_or_init(|| Integers::of(body, floating));
        if !integers.fit(body, *home, places, &load, floating, registers) {
            continue;
        }
        let flow = flow.get_or_init(|| crate::backend::bodyfacts::Flow::of(body));
        let proof = crate::backend::bodyfacts::_unchanged(body, flow, &load, cell_of(&load), &readers_of);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ir::{Addr, Held, Semantics, Space};

    /// Integer registers of the m16 target.
    const REGISTERS: usize = 6;

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 10 })
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
        let cpu = crate::backend::cpu::profile("486").expect("a profile");
        let body = one_block(vec![
            fld(0, 1, global(0, 0)),
            insn(1, Operation::FloatLoad, "fld1", vec![held(2)], vec![]),
            arith(2, "fmul", 3, 2, 1),
            arith(3, "fadd", 4, 3, 1),
            fstp(4, 4, frame(-4)),
        ]);
        assert_eq!(names(&folded(&body, cpu, REGISTERS)), ["fld1", "fmul+m", "fadd+m", "fstp"]);
    }

    /// A store to the cell between two reads changes the value the second one
    /// reads: the load stays.
    #[test]
    fn test_a_load_is_not_read_from_its_cell_where_the_cell_is_written_between_the_reads() {
        let cpu = crate::backend::cpu::profile("486").expect("a profile");
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
        let cpu = crate::backend::cpu::profile("486").expect("a profile");
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
        let pointer = Mem { base: Some(Held { value: 9, width: 2 }), ..Mem::new(None, 4) };
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

    /// qmove -Os: the readers of a forwarded load sat in two blocks, so the
    /// value stayed on the stack from one to the other (`fxch`, `fld st(i)`)
    /// and 489 clocks were spent where the cells could be read for 463. A
    /// load nothing writes before any of its readers, in whatever block, is
    /// read from its cell by each.
    #[test]
    fn test_a_load_read_in_two_blocks_is_read_from_its_cell_by_each() {
        let cpu = crate::backend::cpu::profile("486").expect("a profile");
        let got = names(&folded(&two_blocks(Vec::new(), &[]), cpu, REGISTERS));
        assert_eq!(got, ["fld1", "fmul+m", "fadd+m", "fstp"], "{got:?}");
    }

    /// A write through another pointer on the way to the second reader may
    /// change the cell: the load stays.
    #[test]
    fn test_a_load_is_kept_where_a_store_through_a_pointer_may_reach_its_cell_before_a_reader() {
        let cpu = crate::backend::cpu::profile("486").expect("a profile");
        let other = Mem { base: Some(Held { value: 8, width: 2 }), ..Mem::new(None, 4) };
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
        let cpu = crate::backend::cpu::profile("486").expect("a profile");
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
            base: Some(Held { value: 9, width: 4 }),
            index: Some(Held { value: 10, width: 4 }),
            offset: 320,
            ..Mem::new(None, 4)
        };
        assert_eq!(_memory_operation_bytes(&pointer, 16), 4);
        assert_eq!(_memory_operation_bytes(&pointer, 32), 7);
    }
}
