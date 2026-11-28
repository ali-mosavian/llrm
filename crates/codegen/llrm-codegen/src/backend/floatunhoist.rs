//! A hoisted float load that a loop only reads is made again where it is read.
//!
//! # How it works
//! LICM hoists `v = fld [cell]` out of a loop whose cell nothing in the loop
//! writes. On the x87 stack `v` then lies in a slot for every trip, and its
//! price is one `fld` and one `fstp` per entry of the loop. A loop that runs
//! two trips on average pays that for two uses that a memory operand reads at
//! the same price: `fld [x_j]; fsub st, st(2)` against `fld [x_j]; fsub [xi]`.
//!
//! A value is made again in the loop when
//!
//! - the registers its address names are read in the loop already (no live
//!   range grows: m16 has four address registers);
//! - one `fld` of a plain cell defines it, in the loop's only outside
//!   predecessor, with nothing after it there or in the loop that may write the
//!   cell, call, or change what the address reads;
//! - every read of it is in the loop, at most two, each the right operand of
//!   `fadd`/`fsub`/`fmul`/`fdiv` (the shape `FloatFold` folds to a memory
//!   operand), the register operand of such an operation that already has its
//!   memory operand (each read copies the value: `fld st(i)`), or a copy of it;
//! - no read costs more from the cell than from the stack, per the target's
//!   `x87_*` rows (a copy: `x87_load` against `x87_copy`; an operation:
//!   `x87_*_m` against `x87_*`).
//!
//! Two reads at most keep the bytes: the memory forms add at most as many as
//! the hoisted `fld` and the `fstp` that ends it take.
//!
//! DEVIATE M16-NBODY-O2: the pin keeps the value on the stack. Reason: the
//! entry's `fld` and `fstp` cost 6 clocks on the 486 per entry of the inner
//! loop (m16 nbody -O2: 600 entries of 2 trips). Owner llrm-target-x87, exit
//! 2026-11-20.
//!
//! # Example
//! ```text
//!   pre:  fld [xi]               pre:
//!   loop: fld [xj]               loop: fld [xj]
//!         fsub st, st(2)    =>         fsubr [xi]
//!   exit: fstp st(1)             exit:
//! ```
//!
//! # Runs
//! After phi elimination, before `FloatFold` (which folds the new loads).

use std::sync::Arc;

use crate::analysis::loops as loopy;
use crate::backend::bodyfacts::_may_write;
use crate::backend::floatalloc::{_float_copy, _loads_memory, _two_values, cell_of, name_is};
use crate::backend::overlap;
use crate::model::ir::{Loc, Operation};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::{HashMap, HashSet};

/// The target's clocks for the stack form and the memory form of a read.
struct Prices {
    load: i64,
    copy: i64,
    /// `(stack, memory)` for add/sub, mul, div.
    arith: [(i64, i64); 3],
}

impl Prices {
    fn of(cpu: &crate::backend::cpu::Profile) -> Option<Self> {
        let get = |name: &str| cpu.cost(name).ok();
        Some(Self {
            load: get("x87_load")?,
            copy: get("x87_copy")?,
            arith: [
                (get("x87_add")?, get("x87_add_m")?),
                (get("x87_mul")?, get("x87_mul_m")?),
                (get("x87_div")?, get("x87_div_m")?),
            ],
        })
    }

    /// Whether reading from the cell costs no more than from the stack.
    fn memory_is_no_dearer(
        &self,
        read: Read,
    ) -> bool {
        match read {
            Read::Copy => self.load <= self.copy,
            Read::Arith(class) => self.arith[class].1 <= self.arith[class].0,
        }
    }
}

#[derive(Clone, Copy)]
enum Read {
    Copy,
    Arith(usize),
}

pub struct FloatUnhoist<'a> {
    pub cpu: &'a crate::backend::cpu::Profile,
    pub basic_semantics: bool,
}

impl LIRTransform for FloatUnhoist<'_> {
    fn class_name(&self) -> &'static str {
        "FloatUnhoist"
    }

    fn name(&self) -> &str {
        "floatunhoist"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        if self.basic_semantics {
            return Ok(body);
        }
        let Some(prices) = Prices::of(self.cpu) else { return Ok(body) };
        Ok(unhoisted_if_any(body, &prices))
    }
}

/// `unhoisted`, for a body that has a memory `fld` to move. The scan accepts
/// nothing else, so without one the loops and the read counts are not worth
/// computing.
fn unhoisted_if_any(
    body: LirBody,
    prices: &Prices,
) -> LirBody {
    let has_candidate = body
        .blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .any(|one| one.what.as_ref().is_some_and(|what| name_is(what, "fld")) && _loads_memory(one.what.as_ref()));
    if has_candidate { unhoisted(&body, prices) } else { body }
}

/// How `one` reads `value`, when it is a read the memory form can take.
fn read_of(
    one: &Insn,
    value: u32,
) -> Option<Read> {
    if let Some((_, source)) = _float_copy(one) {
        return (source == value).then_some(Read::Copy);
    }
    // `op r := v, [m]` (memory form): `v` is the register operand and stays,
    // so each read copies it (`fld st(i)`); from the cell it is a load.
    if let Some(what) = one.what.as_ref()
        && what.op == Operation::FloatArith
        && matches!(
            what.name.as_deref(),
            Some("fadd" | "fsub" | "fsubr" | "fmul" | "fdiv" | "fdivr")
        )
        && let [Loc::Held(a), Loc::Mem(_)] | [Loc::Mem(_), Loc::Held(a)] = what.sources.as_slice()
    {
        return (a.value == value).then_some(Read::Copy);
    }
    let (name, left, right) = _two_values(one.what.as_ref())?;
    let class = match name.as_str() {
        "fadd" | "fsub" => 0,
        "fmul" => 1,
        "fdiv" => 2,
        _ => return None,
    };
    (right.value == value && left.value != value).then_some(Read::Arith(class))
}

/// Whether `one` may change what the load reads or where it reads it.
fn disturbs(
    one: &Insn,
    load: &Insn,
) -> bool {
    one.defines.iter().any(|defined| load.uses.contains(defined))
        || one.volatile
        || one.call.is_some()
        || _may_write(one, cell_of(load), false)
        || one.what.as_ref().is_some_and(|what| {
            let cell = cell_of(load);
            what.dests
                .iter()
                .any(
                    |dest| matches!(
                        dest,
                        Loc::Mem(other) if overlap::may_overlap(cell.addr, cell.width, other.addr, other.width)
                    ),
                )
        })
        || one
            .what
            .as_ref()
            .is_none_or(|what| matches!(
                what.op,
                Operation::Call | Operation::Barrier | Operation::Escape
            ))
}

#[cfg(test)]
thread_local! {
    /// Bodies `unhoisted` has analysed on this thread.
    static ANALYSED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// `body` with each hoisted float load a loop only reads made again at its
/// reads.
fn unhoisted(
    body: &LirBody,
    prices: &Prices,
) -> LirBody {
    #[cfg(test)]
    ANALYSED.with(|count| count.set(count.get() + 1));
    let loops = loopy::loops(&body.blocks, Some(body.entry));
    if loops.is_empty() {
        return body.clone();
    }
    // Where each float value is defined and read, over the whole function.
    let mut reads: HashMap<u32, usize> = HashMap::default();
    let mut defs: HashMap<u32, usize> = HashMap::default();
    for one in body.blocks.iter().flat_map(|block| block.insns.iter()) {
        for value in &one.uses {
            *reads.entry(*value).or_default() += 1;
        }
        for value in &one.defines {
            *defs.entry(*value).or_default() += 1;
        }
    }
    let mut fresh = body.next_value();
    let mut insns: HashMap<i64, Vec<Arc<Insn>>> =
        body.blocks.iter().map(|block| (block.at, block.insns.to_vec())).collect();
    let mut changed = false;
    let mut done: HashSet<u32> = HashSet::default();
    for one in &loops {
        let inside = one.body.to_set();
        let outside: Vec<i64> = body
            .blocks
            .iter()
            .filter(|block| !inside.contains(&block.at) && block.succ.contains(&one.header))
            .map(|block| block.at)
            .collect();
        let [pre] = outside.as_slice() else { continue };
        let Some(pre_block) = body.blocks.iter().find(|block| block.at == *pre) else { continue };
        if pre_block.succ.as_ref() != [one.header] {
            continue;
        }
        let members: Vec<i64> = body.blocks.iter().filter(|b| inside.contains(&b.at)).map(|b| b.at).collect();
        'scan: loop {
            let pre_insns = insns[pre].clone();
            for (position, load) in pre_insns.iter().enumerate() {
                if !_loads_memory(load.what.as_ref())
                    || !load.what.as_ref().is_some_and(|what| name_is(what, "fld"))
                    || load.volatile
                    || !load.clobbers.is_empty()
                    || load.group.is_some()
                    || !matches!(cell_of(load).width, 4 | 8)
                {
                    continue;
                }
                let [value] = load.defines.as_slice() else { continue };
                let value = *value;
                if defs.get(&value) != Some(&1) || done.contains(&value) {
                    continue;
                }
                let in_loop: Vec<(i64, usize)> = members
                    .iter()
                    .flat_map(|at| insns[at].iter().enumerate().map(move |(index, _)| (*at, index)))
                    .filter(|(at, index)| insns[at][*index].uses.contains(&value))
                    .collect();
                let total_reads = reads.get(&value).copied().unwrap_or(0);
                if in_loop.is_empty() || in_loop.len() > 2 || in_loop.len() != total_reads {
                    continue;
                }
                // A reload keeps its address registers alive through the loop:
                // only where the loop already reads them (m16 has four to
                // spare: an index used by the load alone costs a `mov` a trip).
                let address_in_loop = load
                    .uses
                    .iter()
                    .all(|used| members.iter().any(|at| insns[at].iter().any(|one| one.uses.contains(used))));
                if !address_in_loop {
                    continue;
                }
                let read_kinds: Option<Vec<Read>> =
                    in_loop.iter().map(|(at, index)| read_of(&insns[at][*index], value)).collect();
                let Some(read_kinds) = read_kinds else { continue };
                if !read_kinds.iter().all(|read| prices.memory_is_no_dearer(*read)) {
                    continue;
                }
                if pre_insns[position + 1..].iter().any(|later| disturbs(later, load))
                    || members.iter().any(|at| insns[at].iter().any(|later| disturbs(later, load)))
                {
                    continue;
                }
                // Made again at each read, the original gone.
                let mut sites = in_loop.clone();
                sites.sort_by(|a, b| b.cmp(a));
                for (at, index) in sites {
                    let made = fresh;
                    fresh += 1;
                    let read = insns[&at][index].renamed(&|name| if name == value { made } else { name });
                    let again = load.renamed(&|name| if name == value { made } else { name });
                    let list = insns.get_mut(&at).expect("a loop block");
                    list[index] = read;
                    list.insert(index, again);
                }
                let list = insns.get_mut(pre).expect("the preheader");
                let at_now = list.iter().position(|one| Arc::ptr_eq(one, load)).expect("the load stays until now");
                list.remove(at_now);
                done.insert(value);
                changed = true;
                // The positions are stale after an edit: scan again.
                continue 'scan;
            }
            break;
        }
    }
    if !changed {
        return body.clone();
    }
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            let list = insns[&block.at].clone();
            let same =
                list.len() == block.insns.len() && list.iter().zip(block.insns.iter()).all(|(a, b)| Arc::ptr_eq(a, b));
            if same { block.clone() } else { block.with_insns(list) }
        })
        .collect();
    body.with_blocks(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ir::{Addr, Held, Loc, Mem, Semantics, Space};
    use crate::model::lir::{Block, LirBlock};

    const I486: Prices = Prices { load: 3, copy: 4, arith: [(8, 8), (16, 14), (73, 73)] };

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 10, offset: 0 })
    }

    fn cell(symbol: i64) -> Mem {
        Mem::new(Some(Addr::new(Space::Frame, symbol * 16)), 8)
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
        symbol: i64,
    ) -> Arc<Insn> {
        insn(at, Operation::FloatLoad, "fld", vec![held(value)], vec![Loc::Mem(cell(symbol))])
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

    /// `result := v op [cell]`: the memory form, `v` the register operand.
    fn arith_m(
        at: i64,
        name: &str,
        result: u32,
        left: u32,
        symbol: i64,
    ) -> Arc<Insn> {
        insn(at, Operation::FloatArith, name, vec![held(result)], vec![held(left), Loc::Mem(cell(symbol))])
    }

    fn fstp(
        at: i64,
        value: u32,
        symbol: i64,
    ) -> Arc<Insn> {
        insn(at, Operation::FloatStore, "fstp", vec![Loc::Mem(cell(symbol))], vec![held(value)])
    }

    /// `pre: fld [0] -> v1; jmp`, the loop `body`, `exit: ret`.
    fn looped(body: Vec<Arc<Insn>>) -> LirBody {
        let block = |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| -> Block {
            LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) }.into()
        };
        let mut inside = body;
        inside.push(insn(90, Operation::Branch, "jne", vec![], vec![]));
        LirBody::new(
            "loop",
            0,
            vec![
                block(0, vec![1], vec![fld(0, 1, 0), insn(1, Operation::Jump, "jmp", vec![], vec![])]),
                block(1, vec![1, 2], inside),
                block(2, vec![], vec![insn(91, Operation::Return, "ret", vec![], vec![])]),
            ],
            Default::default(),
            Default::default(),
        )
    }

    fn loads_in(
        body: &LirBody,
        block: usize,
    ) -> usize {
        body.blocks[block].insns.iter().filter(|one| _loads_memory(one.what.as_ref())).count()
    }

    /// m16 nbody -O2: `xi` loaded before the inner loop and popped after it,
    /// 6 clocks per entry of a loop that runs twice, for one `fsub` that a
    /// memory operand reads at the same price. The load moves to the read.
    #[test]
    fn test_a_hoisted_load_one_operation_reads_is_made_again_at_the_read() {
        let body = looped(vec![fld(10, 2, 1), arith(11, "fsub", 3, 2, 1), fstp(12, 3, 2)]);
        let done = unhoisted(&body, &I486);
        assert_eq!((loads_in(&done, 0), loads_in(&done, 1)), (0, 2));
    }

    /// m16 nbody -O2 at the tip: `xi` is the register operand of `fsubr r :=
    /// xi, [x_j]`, so each read copies it (`fld st(i)`, 4 clocks on the 486; a
    /// load is 3). The shape the first matcher missed: the pass changed
    /// nothing there.
    #[test]
    fn test_a_hoisted_load_copied_by_a_memory_form_read_is_made_again() {
        let body = looped(vec![arith_m(10, "fsubr", 3, 1, 1), fstp(11, 3, 2)]);
        let done = unhoisted(&body, &I486);
        assert_eq!((loads_in(&done, 0), loads_in(&done, 1)), (0, 1));
    }

    /// Three reads cost more bytes as memory operands than the load and the
    /// pop they save: the value stays on the stack.
    #[test]
    fn test_a_load_read_three_times_stays_hoisted() {
        let body = looped(vec![
            fld(10, 2, 1),
            arith(11, "fsub", 3, 2, 1),
            arith(12, "fadd", 4, 3, 1),
            arith(13, "fmul", 5, 4, 1),
            fstp(14, 5, 2),
        ]);
        assert_eq!(loads_in(&unhoisted(&body, &I486), 0), 1);
    }

    /// A store to the cell in the loop changes what a read finds.
    #[test]
    fn test_a_load_of_a_cell_the_loop_writes_stays_hoisted() {
        let body = looped(vec![fld(10, 2, 1), arith(11, "fsub", 3, 2, 1), fstp(12, 3, 0)]);
        assert_eq!(loads_in(&unhoisted(&body, &I486), 0), 1);
    }

    /// A read that costs more from the cell than from the stack keeps the
    /// value on the stack.
    #[test]
    fn test_a_read_dearer_from_the_cell_stays_hoisted() {
        let body = looped(vec![fld(10, 2, 1), arith(11, "fsub", 3, 2, 1), fstp(12, 3, 2)]);
        let dearer = Prices { arith: [(8, 9), (16, 14), (73, 73)], ..I486 };
        assert_eq!(loads_in(&unhoisted(&body, &dearer), 0), 1);
    }

    /// The value as the left operand has no memory form to fold to.
    #[test]
    fn test_a_load_read_as_the_left_operand_stays_hoisted() {
        let body = looped(vec![fld(10, 2, 1), arith(11, "fsub", 3, 1, 2), fstp(12, 3, 2)]);
        assert_eq!(loads_in(&unhoisted(&body, &I486), 0), 1);
    }

    /// Compile time: `unhoisted` built loop info and read maps for every
    /// function (93 M Ir of 8,863 M on memexp -O2). A loop without a memory
    /// `fld` must not be analysed.
    #[test]
    fn test_a_function_with_no_memory_fld_is_not_analysed() {
        let body = looped(vec![arith(11, "fsub", 3, 2, 1), fstp(12, 3, 2)]);
        let without: Vec<Arc<Insn>> =
            body.blocks[0].insns.iter().filter(|one| !_loads_memory(one.what.as_ref())).cloned().collect();
        let mut body = body;
        body.blocks[0] = LirBlock { succ: body.blocks[0].succ.clone(), ..LirBlock::new(0, without) }.into();
        ANALYSED.with(|count| count.set(0));
        let kept = unhoisted_if_any(body.clone(), &I486);
        assert_eq!(ANALYSED.with(|count| count.get()), 0);
        assert_eq!(kept.blocks.len(), body.blocks.len());
        let with = looped(vec![fld(10, 2, 1), arith(11, "fsub", 3, 2, 1), fstp(12, 3, 2)]);
        unhoisted_if_any(with, &I486);
        assert_eq!(ANALYSED.with(|count| count.get()), 1);
    }
}
