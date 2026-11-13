//! A function from IR to sdasz80 text: the shared DAG stages, then the Z80
//! machine function, allocation, expansion and printing.

use llrm_lir::registers::{RegId, Regs};
use llrm_spike_mf::cfg::Cfg;
use llrm_spike_mf::regalloc::{Assignment, Target, eliminate_phis_with, two_address};
use llrm_spike_mf::tolir::allocate_greedy;

use crate::driver::Level;
use crate::driver::{Request, selections_with_alternatives};
use crate::expand::{self, Line, finish};
use crate::mfunc::{Block, Inst, Kind, MFunction, Operand, Phi, Reg, emit};
use crate::regalloc::Z80Registers;
use crate::{REGISTER_INFO, reg};

/// The machine function after selection and emission, before allocation.
pub fn machine_function(
    ir: &str,
    name: &str,
    request: &Request,
) -> Result<MFunction, String> {
    machine_function_with(&cached_selections(ir, request)?, name, true)
}

thread_local! {
    /// Times the mid-end and selection ran since the last
    /// [`take_selection_runs`].
    static SELECTION_RUNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The runs of the mid-end and selection on this thread since the last call.
pub fn take_selection_runs() -> usize {
    SELECTION_RUNS.with(|one| one.replace(0))
}

thread_local! {
    /// Times a module's text was compared with the one held since the last
    /// [`take_text_compares`].
    static TEXT_COMPARES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The comparisons of a module's text with the one held on this thread since
/// the last call: each costs the length of the module.
pub fn take_text_compares() -> usize {
    TEXT_COMPARES.with(|one| one.replace(0))
}

/// A module run through the mid-end and selection once, for the functions of
/// that module in turn ([`text_in`]): no comparison of its text per function.
pub struct Prepared {
    selections: Selections,
    /// The functions that call `__mulsi3`, selected with the multiplies
    /// decomposed.
    alternatives: Selections,
    /// With `-g`: the source file and start line of each function, from one
    /// parse of the module.
    sources: llrm_support::hash::HashMap<String, (String, Option<u32>)>,
    /// The level the module is compiled at: -Os for a module `oz_best_of` found smaller there.
    level: Level,
}

impl Prepared {
    /// The functions the module defines.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.selections.iter().map(|(name, ..)| name.as_str())
    }
}

/// `ir` through the mid-end and selection, for `request`.
pub fn prepare(
    ir: &str,
    request: &Request,
) -> Result<Prepared, String> {
    let best_of = request.level == Level::Oz && request.oz_best_of;
    let request = &effective(ir, request)?;
    SELECTION_RUNS.with(|runs| runs.set(runs.get() + 1));
    let sources = if request.debug { crate::driver::sources_of(ir) } else { Default::default() };
    // -Oz has just compiled the module both ways (`effective`) and holds both: the one it keeps is not selected again
    let (selections, alternatives) = if best_of {
        cached_both(ir, request)?
    } else {
        let (selections, alternatives) = selections_with_alternatives(ir, request)?;
        (Selections::new(selections), Selections::new(alternatives))
    };
    Ok(Prepared { selections, alternatives, sources, level: request.level })
}

type SelectionKey = (String, Vec<(String, String)>);

/// Each function selected with the DAG scheduled by `Source` and by `ILP`.
type Selections = std::rc::Rc<
    Vec<(
        String,
        Result<crate::dagisel::function::Selection, String>,
        Result<crate::dagisel::function::Selection, String>,
    )>,
>;

thread_local! {
    /// The three modules most recently run through the mid-end and selection (three: -Oz compiles a module three ways, as -Oz and as
    /// -Os twice, then reads the one it keeps), most recent first, with what each was run for.
    static SELECTED: std::cell::RefCell<Vec<(String, SelectionKey, Selections, Selections)>> =
        const { std::cell::RefCell::new(Vec::new()) };
    /// The lines `module_bytes` allocated, kept for the module it chose: (selection, function, number) -> lines. The
    /// choice allocates every function at both levels; the one kept is not allocated a third time for its text.
    static KEPT_LINES: std::cell::RefCell<std::collections::HashMap<(usize, String, usize), Vec<Line>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// The request `effective` chose for the module most recently asked about.
    static CHOSEN: std::cell::RefCell<Option<(String, SelectionKey, Request)>> = const { std::cell::RefCell::new(None) };
}

/// What a compile of `ir` reads besides the text: the request and the LLRM_ and Z80_ switches of the environment.
fn selection_key(request: &Request) -> SelectionKey {
    let mut environment: Vec<(String, String)> =
        std::env::vars().filter(|(name, _)| name.starts_with("LLRM_") || name.starts_with("Z80_")).collect();
    environment.sort();
    (
        format!("{:?}", (request.level, &request.passes, request.debug, request.cpu as u8, request.slot_by_conflicts, request.flat_loop_trips, request.oz_best_of, request.optsize_only)),
        environment,
    )
}

/// The request `ir` is compiled with. DEVIATE z80-oz-best-of (see `Request::oz_best_of`): at -Oz, the -Os one where
/// the module is smaller in bytes (the runtime it reaches included), else the request itself.
pub fn effective(
    ir: &str,
    request: &Request,
) -> Result<Request, String> {
    if request.level != Level::Oz || !request.oz_best_of {
        return Ok(request.clone());
    }
    let key = selection_key(request);
    let held = CHOSEN.with(|held| {
        held.borrow().as_ref().filter(|(text, was, _)| text == ir && *was == key).map(|(.., chosen)| chosen.clone())
    });
    let chosen = match held {
        Some(chosen) => chosen,
        None => {
            // -Oz, then -Os as the functions say (`minsize` kept), then -Os as clang makes it (`minsize` dropped): a
            // compile that is refused, or whose lines are, is not a candidate; a tie keeps the earlier
            let candidates = [
                request.clone(),
                Request { level: Level::Os, ..request.clone() },
                Request { level: Level::Os, optsize_only: true, ..request.clone() },
            ];
            let sizes: Vec<Result<u32, String>> = candidates.iter().map(|one| module_bytes(ir, one)).collect();
            if std::env::var_os("LLRM_DEBUG_BESTOF").is_some() {
                eprintln!("best-of oz, os, os without minsize: {sizes:?}");
            }
            let mut best = 0;
            for (at, size) in sizes.iter().enumerate() {
                if let (Ok(size), Ok(held)) = (size, &sizes[best])
                    && size < held
                {
                    best = at;
                }
            }
            let chosen = candidates[best].clone();
            // keep only the lines of the module that stays
            let kept = cached_both(ir, &chosen).map(|(selected, _)| std::rc::Rc::as_ptr(&selected) as usize);
            KEPT_LINES.with(|lines| lines.borrow_mut().retain(|(at, ..), _| kept.as_ref().is_ok_and(|kept| kept == at)));
            CHOSEN.with(|held| *held.borrow_mut() = Some((ir.to_owned(), key, chosen.clone())));
            chosen
        }
    };
    Ok(chosen)
}

/// The module's code bytes at `request.level`: its functions as emitted and the runtime routines they reach, each
/// once.
fn module_bytes(
    ir: &str,
    request: &Request,
) -> Result<u32, String> {
    let (selected, alternatives) = cached_both(ir, request)?;
    let (mut bytes, mut reached) = (0, std::collections::HashSet::new());
    let at = std::rc::Rc::as_ptr(&selected) as usize;
    for (number, (name, ..)) in selected.iter().enumerate() {
        let lines = lines_in(&selected, &alternatives, name, number, request)?;
        bytes += expand::price_emitted(&lines, false).0;
        reached.extend(crate::runtime_size::referenced(&lines));
        KEPT_LINES.with(|kept| kept.borrow_mut().insert((at, name.clone(), number), lines));
    }
    Ok(bytes + crate::runtime_size::reached(&reached) + crate::data::image_bytes(&crate::data::text_at(ir, request)?))
}

/// [`selections`] of `ir`, kept for the next function of the same module:
/// every function was run through the whole mid-end and the selection of every
/// function once for each time it was built (twice, with the return block
/// copied or kept), so a module of N functions cost N squared.
fn cached_selections(
    ir: &str,
    request: &Request,
) -> Result<Selections, String> {
    cached_both(ir, request).map(|(selected, _)| selected)
}

/// [`cached_selections`] and the alternatives of the functions that call
/// `__mulsi3`.
fn cached_both(
    ir: &str,
    request: &Request,
) -> Result<(Selections, Selections), String> {
    // everything the result reads: the text, the request, and the LLRM_ and
    // Z80_ switches of the environment. Compared whole: hashing the text of a
    // large module for each of its functions was a tenth of the compile.
    TEXT_COMPARES.with(|one| one.set(one.get() + 1));
    let key = selection_key(request);
    let held = SELECTED.with(|held| {
        held.borrow().iter().find(|(text, was, ..)| text == ir && *was == key).map(|(_, _, one, other)| (one.clone(), other.clone()))
    });
    if let Some(one) = held {
        return Ok(one);
    }
    SELECTION_RUNS.with(|runs| runs.set(runs.get() + 1));
    let (main, alternatives) = selections_with_alternatives(ir, request)?;
    let (one, other) = (Selections::new(main), Selections::new(alternatives));
    SELECTED.with(|held| {
        let mut held = held.borrow_mut();
        held.insert(0, (ir.to_owned(), key, one.clone(), other.clone()));
        held.truncate(3);
    });
    Ok((one, other))
}

/// [`machine_function`], with or without the return blocks copied into their
/// predecessors.
fn machine_function_with(
    selected: &Selections,
    name: &str,
    duplicate: bool,
) -> Result<MFunction, String> {
    let (_, source, scheduled) =
        selected.iter().find(|(one, ..)| one == name).ok_or_else(|| format!("no function @{name}"))?;
    let selection = if crate::dagisel::pipeline::ILP.with(std::cell::Cell::get) { scheduled } else { source };
    let selection = selection.as_ref().map_err(Clone::clone)?;
    let selection = match &selection.generic {
        Some(generic) if crate::dagisel::pipeline::GENERIC_FORM.with(std::cell::Cell::get) => generic,
        _ => selection,
    };
    let mut function = emit(name, selection)?;
    function.returns_twice = selection.returns_twice;
    expand_cmov(&mut function)?;
    crate::ldir::copy_loops_to_ldir(&mut function);
    if duplicate {
        tail_duplicate_returns(&mut function);
    }
    eliminate_tests(&mut function);
    eliminate_dead(&mut function);
    Ok(function)
}

/// Drops the instructions whose results nothing reads (`ZEXT8` of a byte whose
/// widening an `EXTRACT_SUBREG` bypassed, the sign fill of a word added in
/// 16 bits). Only an instruction that touches no memory and has no other
/// effect goes, and every register it writes is a virtual one nothing reads,
/// or `a` or the flags with no reader before the next write.
///
/// Translation: `DeadMachineInstrElim` (DeadMachineInstructionElim.cpp,
/// `isDead`: a physical register def is dead where it is not live after).
/// DEVIATE: only `a` and the flags are judged, the two registers an
/// accumulator op clobbers without naming a result; every other physical
/// def keeps its instruction. Exit: the pass on the shared flow.
fn eliminate_dead(function: &mut MFunction) {
    // the flags as a register of their own, apart from `a`'s id
    const FLAGS: RegId = reg::FLAGS;
    let judged = |r: RegId| r == reg::A || r == FLAGS;
    loop {
        let mut read = llrm_support::hash::HashSet::default();
        for inst in function.blocks.iter().flat_map(|block| &block.insts) {
            for operand in &inst.operands {
                if let Operand::Use(Reg::Virt(v)) = operand {
                    read.insert(*v);
                }
            }
        }
        for block in &function.blocks {
            for phi in &block.phis {
                for (_, reg) in &phi.incoming {
                    if let Reg::Virt(v) = reg {
                        read.insert(*v);
                    }
                }
            }
        }
        // what `a` and the flags hold for a reader at the start of each block
        let successors = llrm_spike_mf::regalloc::successors(function);
        let mut live_in: Vec<[bool; 2]> = vec![[false; 2]; function.blocks.len()];
        let slot = |r: RegId| usize::from(r == FLAGS);
        let reads_of = |inst: &Inst, r: RegId| {
            inst.operands.iter().any(|o| matches!(o, Operand::Use(Reg::Phys(p)) if *p == r))
                || (r == FLAGS && inst.reads_flags)
        };
        let writes_of = |inst: &Inst, r: RegId| {
            inst.operands.iter().any(|o| matches!(o, Operand::Def(Reg::Phys(p)) if *p == r))
                || (r == FLAGS && inst.sets_flags)
        };
        // a block nothing follows may leave `a` and the flags to its caller
        let exits: Vec<bool> = (0..function.blocks.len())
            .map(|block| function.blocks[block].insts.last().is_none_or(|i| i.terminator) && successors[block].is_empty())
            .collect();
        let live_after = |block: usize, live_in: &[[bool; 2]]| {
            let mut live = [exits[block]; 2];
            for &next in &successors[block] {
                for k in 0..2 {
                    live[k] |= live_in[next][k];
                }
            }
            live
        };
        let mut moved = true;
        while moved {
            moved = false;
            for block in (0..function.blocks.len()).rev() {
                let mut live = live_after(block, &live_in);
                for inst in function.blocks[block].insts.iter().rev() {
                    for r in [reg::A, FLAGS] {
                        if writes_of(inst, r) && !inst.call {
                            live[slot(r)] = false;
                        }
                        if inst.call {
                            live[slot(r)] = true;
                        }
                    }
                    for r in [reg::A, FLAGS] {
                        if reads_of(inst, r) {
                            live[slot(r)] = true;
                        }
                    }
                }
                if live != live_in[block] {
                    live_in[block] = live;
                    moved = true;
                }
            }
        }
        let mut removed = false;
        for block in 0..function.blocks.len() {
            let mut live = live_after(block, &live_in);
            let mut dead = vec![false; function.blocks[block].insts.len()];
            for (k, inst) in function.blocks[block].insts.iter().enumerate().rev() {
                let defs = inst.operands.iter().filter(|o| matches!(o, Operand::Def(_))).count();
                let removable = defs > 0
                    && inst.operands.iter().all(|o| match o {
                        Operand::Def(Reg::Virt(v)) => !read.contains(v),
                        Operand::Def(Reg::Phys(p)) => judged(*p) && !live[slot(*p)],
                        Operand::Def(_) => false,
                        _ => true,
                    })
                    && !(inst.sets_flags && live[slot(FLAGS)])
                    && !(inst.call
                        || inst.terminator
                        || inst.barrier
                        || inst.side_effects
                        || inst.may_store
                        || inst.may_load);
                dead[k] = removable;
                if removable {
                    continue;
                }
                for r in [reg::A, FLAGS] {
                    if writes_of(inst, r) && !inst.call {
                        live[slot(r)] = false;
                    }
                    if inst.call {
                        live[slot(r)] = true;
                    }
                }
                for r in [reg::A, FLAGS] {
                    if reads_of(inst, r) {
                        live[slot(r)] = true;
                    }
                }
            }
            let mut k = 0;
            function.blocks[block].insts.retain(|_| {
                k += 1;
                !dead[k - 1]
            });
            removed |= dead.iter().any(|d| *d);
        }
        if !removed {
            return;
        }
    }
}

/// The function after phi elimination, two-address rewriting and allocation,
/// with the register each virtual register got and the slots spilled to, each
/// with its size in bytes.
pub fn allocated(
    ir: &str,
    name: &str,
    request: &Request,
) -> Result<(MFunction, Assignment, Vec<(i32, usize)>), String> {
    allocated_with(&cached_selections(ir, request)?, name, request, true)
        .map(|(function, assignment, slots, _, _)| (function, assignment, slots))
}

/// What an allocation leaves: the function, the registers, the slots, the slots
/// read again, and the block frequencies where tail merging moved them
/// (`BranchFolder`'s `MBBFreqInfo`).
type Allocated = (
    MFunction,
    Assignment,
    Vec<(i32, usize)>,
    llrm_support::hash::HashMap<u32, usize>,
    Option<llrm_support::hash::HashMap<u32, f64>>,
);

fn allocated_with(
    selected: &Selections,
    name: &str,
    request: &Request,
    duplicate: bool,
) -> Result<Allocated, String> {
    let done = allocated_choosing(selected, name, request, duplicate);
    HOT_LOOP_FOLD.with(|one| one.set(true));
    // what the later prices of this function weigh by: the blocks as the kept
    // allocation has them
    if let Ok((.., Some(own))) = &done {
        expand::replace_frequencies(own.clone());
    }
    done
}

fn allocated_choosing(
    selected: &Selections,
    name: &str,
    request: &Request,
    duplicate: bool,
) -> Result<Allocated, String> {
    SLOT_BY_CONFLICTS.with(|one| one.set(request.slot_by_conflicts));
    expand::BYTES_FIRST.with(|one| one.set(!matches!(request.level, Level::O2 | Level::O3 | Level::Omax)));
    expand::set_frequencies(&frequencies_of(selected, name));
    let mut function = machine_function_with(selected, name, duplicate)?;
    if std::env::var("LLRM_NO_Z80_SHL32").is_err() {
        crate::long_shift::double_and_sink(&mut function);
    }
    if std::env::var("LLRM_Z80_DUMP_SSA").is_ok_and(|one| one == name) {
        dump(&function);
    }
    eliminate_phis_with(&mut function, false);
    merge_lone_constant_stores(&mut function);
    let (joins, plain_first) = repeating_loop_fold_wins(&function, name, request.level);
    HOT_LOOP_FOLD.with(|one| one.set(joins));
    let mut hoisted = function.clone();
    // the third choice: also the constant stores in loops from a register (`split_loop_constant_stores`)
    let mut split = function.clone();
    let splits = split_loop_constant_stores(&mut split) && hoist_constants(&mut split);
    let base = if hoist_constants(&mut hoisted) {
        // the allocator does not see that a byte of a pair is read in place, so a
        // constant kept across a loop can cost more than making it there, or less:
        // the hoisted loop stays unless the plain one is cheaper by the loop's
        // weighted cost. The other choices are explored on both: lz4c -O2 wants
        // the plain one's (exploring only the better first allocation made it 442
        // B against 427).
        match (
            allocate_from(function.clone(), name, request.level, plain_first),
            allocate(hoisted, name, request.level),
        ) {
            (Ok(plain), Ok(with)) => {
                match (priced_in_loops(&plain, request.level), priced_in_loops(&with, request.level)) {
                    (Some(one), Some(other)) if cheaper(request.level, one, other) => Ok(plain),
                    _ => Ok(with),
                }
            }
            (_, Ok(with)) => Ok(with),
            (plain, Err(_)) => plain,
        }
    } else {
        allocate_from(function, name, request.level, plain_first)
    };
    if !splits {
        return base;
    }
    // stays only where it is cheaper than the best of the two
    match (base, allocate(split, name, request.level)) {
        (Ok(base), Ok(split)) => match (priced_in_loops(&base, request.level), priced_in_loops(&split, request.level)) {
            (Some(one), Some(other)) if cheaper(request.level, other, one) => Ok(split),
            _ => Ok(base),
        },
        (_, Ok(split)) => Ok(split),
        (base, Err(_)) => base,
    }
}

/// Bytes and the clocks weighted by loop depth of an allocated function.
fn priced_in_loops(
    one: &Allocated,
    level: Level,
) -> Option<(u32, u64)> {
    let (function, assignment, slots, reads, frequencies) = one;
    expand::PRICING.with(|one| one.set(priced_before_peepholes(level)));
    let lines = finish(function, assignment, 0, slots, reads.clone());
    expand::PRICING.with(|one| one.set(false));
    let kept = frequencies.as_ref().map(|own| expand::replace_frequencies(own.clone()));
    let price = lines.ok().map(|lines| expand::price_emitted(&lines, !matches!(level, Level::Os | Level::Oz)));
    if let Some(kept) = kept {
        expand::replace_frequencies(kept);
    }
    price
}

/// Whether a variant is priced before the printer's peepholes. Only at -O2 and up, where the price is weighted
/// clocks, good to a few tenths of a percent, and a peephole moved a near-tie to the worse variant (copy_runs -O2:
/// 2 B and 600 T). Where the price is bytes (-Os, -Oz) or both axes are strict (-O1) the peephole's bytes and
/// clocks are real: priced before them, clear -O1 chose the allocation that is larger and slower once emitted
/// (96 B / 577 T against 95 B / 550 T at -Os).
fn priced_before_peepholes(level: Level) -> bool {
    matches!(level, Level::O2 | Level::O3)
}

/// Whether the price `one` beats `other`: bytes first at the size levels;
/// otherwise weighted clocks plus 3 clocks a byte (1 B = 3 T), so a variant
/// that is bigger and no faster by that rate does not win on a clock alone.
fn cheaper(
    level: Level,
    one: (u32, u64),
    other: (u32, u64),
) -> bool {
    let priced = |(bytes, clocks): (u32, u64)| clocks + u64::from(bytes) * crate::tti::SPEED_MILLICLOCKS_PER_BYTE as u64 / 1000;
    if matches!(level, Level::Os | Level::Oz) { one < other } else { priced(one) < priced(other) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// call_keeps -O1 was 1876 B for 57813 T (long multiplies and shifts inline) against 1473 B for 64413 T with the
    /// routines: a clock rise of 11% for 21% of the bytes. The price counted each routine call as 17 T, so the form
    /// with the routines looked far faster than it is and no rate was stated for the strict level.
    #[test]
    fn at_o1_the_form_with_the_smaller_relative_regret_wins() {
        let (inline, calls) = ((1916, 11124), (1473, 12449));
        assert!(prefers(Level::O1, calls, inline) && !prefers(Level::O1, inline, calls));
        // a form 1% smaller for 30% more clocks does not
        assert!(!prefers(Level::O1, (990, 14300), (1000, 11000)));
        assert!(prefers(Level::O1, (1000, 11000), (990, 14300)));
    }

    /// div_constants_long -O1 took `call __ashlsi3` for a xorshift step (-14 B, +10% clocks measured): against a
    /// module whose static clocks are one hot loop elsewhere, the call's 500 T were 0.001% and the 14 B 0.08%.
    #[test]
    fn at_o1_a_call_is_priced_against_its_own_function_not_the_module() {
        let routines = std::collections::HashSet::from(["__ashlsi3".to_owned()]);
        let hot_loop = (4000, 40_000_000, std::collections::HashSet::new());
        let step = ("step".to_owned(), (30, 700, routines), Some((44, 120, std::collections::HashSet::new())));
        let variants: Variants = (vec![hot_loop], vec![step]);
        let (calls, inline) = (std::collections::HashSet::new(), std::collections::HashSet::from([0]));
        let module = |form| module_price(&variants, form);
        assert!(prefers(Level::O1, module(&calls), module(&inline)), "the module price cannot see the cost");
        assert!(!prefers(Level::O1, own_price(&variants, &calls, 0), own_price(&variants, &inline, 0)));
    }

    /// Bytes first at the size levels, 1 B = 3 T from O2: a form that is 30% slower for 5% fewer bytes wins the
    /// first and loses the second.
    #[test]
    fn the_size_levels_take_bytes_and_o2_takes_the_rate() {
        let (small, fast) = ((1900, 14300), (2000, 11000));
        assert!(prefers(Level::Os, small, fast) && prefers(Level::Oz, small, fast));
        assert!(!prefers(Level::O2, small, fast) && !prefers(Level::O3, small, fast));
        assert!(prefers(Level::O2, (1000, 11000), (1100, 11000)));
    }

    /// A variant 61 B bigger for 107 clocks fewer won on the clocks alone
    /// (tail_calls -O1, 4444 B to 4505 B); at 1 B = 3 T it loses.
    #[test]
    fn variants_are_priced_with_the_peepholes_only_at_the_size_levels() {
        assert!(priced_before_peepholes(Level::O2) && priced_before_peepholes(Level::O3));
        assert!(!priced_before_peepholes(Level::Os) && !priced_before_peepholes(Level::Oz));
        // -O1 prices what is emitted (`clear` -O1: 96 B / 577 T priced before them, 548 T after)
        assert!(!priced_before_peepholes(Level::O1));
    }

    /// The push and pop form is smaller and dearer in weighted clocks: kept at
    /// the size levels (bytes first), dropped at -O2 (3 T a byte does not pay
    /// 1000 T), kept where it is cheaper by that rate, and a tie stays framed.
    #[test]
    fn push_and_pop_stay_only_where_they_price_lower() {
        let (framed, stacked) = (Some((1000, 50_000)), Some((960, 51_000)));
        assert!(keeps_stacked(Level::Os, framed, stacked) && keeps_stacked(Level::Oz, framed, stacked));
        assert!(!keeps_stacked(Level::O2, framed, stacked) && !keeps_stacked(Level::O3, framed, stacked));
        let cheaper_stack = Some((960, 49_000));
        assert!(keeps_stacked(Level::O2, framed, cheaper_stack));
        assert!(!keeps_stacked(Level::Os, framed, framed) && !keeps_stacked(Level::O2, framed, framed));
        assert!(keeps_stacked(Level::O2, None, stacked));
        // loop_fusion -O1 `same`: 13 B (10%) smaller for 4% more weighted clocks, so the strict level keeps it
        let (framed, stacked) = (Some((124, 6472)), Some((111, 6746)));
        assert!(keeps_stacked(Level::O1, framed, stacked) && !keeps_stacked(Level::O2, framed, stacked));
        assert!(!keeps_stacked(Level::O1, stacked, framed));
    }

    #[test]
    fn a_bigger_variant_does_not_win_on_fewer_clocks_alone() {
        assert!(!cheaper(Level::O2, (4505, 85_000), (4444, 85_107)));
        assert!(cheaper(Level::O2, (4440, 85_000), (4444, 85_107)));
    }

    /// A select side made by an instruction that sets the flags, named in its
    /// operands or not (`XOR16i`), was sunk into the arm ahead of a second
    /// select that still read the compare: i64 `ssub.sat` in a loop gave 0 for
    /// the saturated -1. Neither form may sink.
    #[test]
    fn a_flag_setting_select_side_is_not_sunk_ahead_of_another_select() {
        let flags = Reg::Phys(crate::reg::FLAGS);
        let cmov = |result: u32, on_true: u32| {
            let mut inst = Inst::new(
                Kind::Op("CMOV16"),
                vec![
                    Operand::Def(Reg::Virt(result)),
                    Operand::Use(Reg::Virt(9)),
                    Operand::Use(Reg::Virt(on_true)),
                    Operand::Imm(0),
                    Operand::Use(flags),
                ],
            );
            inst.reads_flags = true;
            inst
        };
        for named in [false, true] {
            let mut compare = Inst::new(Kind::Op("BIT7_16"), vec![Operand::Def(flags), Operand::Use(Reg::Virt(8))]);
            compare.sets_flags = true;
            let mut operands = vec![Operand::Def(Reg::Virt(1)), Operand::Use(Reg::Virt(8))];
            if named {
                operands.push(Operand::Def(flags));
            }
            let mut side = Inst::new(Kind::Op("XOR16i"), operands);
            side.sets_flags = true;
            let mut function = MFunction::new("f");
            function.blocks.push(Block { id: 0, insts: vec![compare, side, cmov(2, 1), cmov(3, 8)], phis: Vec::new() });
            assert!(sunk_operand(&function, 0, 2, Reg::Virt(1)).is_none(), "flag operand named: {named}");
        }
    }

    /// `or a; ...; add hl, de; ...; cp a, 0; jr nz` lost its `or`: the test was dropped across the `add hl`, whose Z
    /// the CPU keeps, but the flag liveness that followed read the `add` as a full flag def and deleted the `or`
    /// chain; the `jr nz` then read the `add`'s flags and `(long)(3.0 / 1.0)` was 4 (f64 divide of a mantissa
    /// at least 1 with fraction bits). The test stays whenever a flag setter lies between.
    #[test]
    fn a_zero_test_is_kept_where_another_flag_setter_lies_between() {
        let (a, flags) = (Reg::Phys(crate::reg::A), Reg::Phys(crate::reg::FLAGS));
        let mut or = Inst::new(Kind::Op("OR8r"), vec![Operand::Def(a), Operand::Use(a), Operand::Use(Reg::Virt(2))]);
        or.sets_flags = true;
        let mut add = Inst::new(Kind::Op("ADD16"), vec![Operand::Def(Reg::Virt(5)), Operand::Use(Reg::Virt(6))]);
        add.sets_flags = true;
        let mut test = Inst::new(Kind::Op("CP8i"), vec![Operand::Def(flags), Operand::Use(a), Operand::Imm(0)]);
        test.sets_flags = true;
        let mut jump = Inst::new(Kind::Op("JPNZ"), vec![Operand::Block(1), Operand::Use(flags)]);
        jump.reads_flags = true;
        jump.terminator = true;
        let insts = vec![
            Inst::new(Kind::Copy, vec![Operand::Def(a), Operand::Use(Reg::Virt(1))]),
            or,
            Inst::new(Kind::Copy, vec![Operand::Def(Reg::Virt(3)), Operand::Use(a)]),
            add,
            Inst::new(Kind::Copy, vec![Operand::Def(a), Operand::Use(Reg::Virt(3))]),
            test,
            jump,
        ];
        let mut function = MFunction::new("f");
        function.blocks.push(Block { id: 0, insts, phis: Vec::new() });
        eliminate_tests(&mut function);
        eliminate_dead(&mut function);
        let insts = &function.blocks[0].insts;
        let reader = insts.iter().position(|inst| inst.reads_flags).expect("the jump stays");
        let setter = insts[..reader].iter().rposition(|inst| inst.sets_flags).expect("something sets the flags it reads");
        assert_ne!(insts[setter].kind, Kind::Op("ADD16"), "the jump reads the add's flags: {insts:?}");
    }
}

/// Allocates `function`; where the first allocation fails, once more with the
/// copies from the argument registers left unjoined. A joined copy pins the
/// argument's pair for the whole of the value's range: a `long long` held in
/// four argument registers has no room left to split (`greedy could not
/// allocate`). The pin does not join them either (`canJoinPhys`).
///
/// Where the choice can allocate better, the other choices are allocated too
/// and the cheaper by the loop's weighted clocks stays, bytes first at the
/// size levels and clocks plus 3 a byte otherwise: the argument copies joined
/// or not (a constant kept in a register of its own, or not), and a byte wanted
/// both in `a` and in a register of its own loaded through `a` or through `hl`.
///
/// DEVIATE Z80-ALLOC-EXPLORE (owner llrm-target-z80, exit 2026-11-30): the pin allocates a function once
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2942>,
/// `RAGreedy::run`). Here a function is allocated up to 12 times: the Source
/// and ILP schedules (Z80S1) x the plain and the constant-hoisted function
/// (`allocated_with`) x this first allocation and its two `improved` choices;
/// the cheapest stays. tests/run/c/fixed_point -O2: 12 allocations are 88% of
/// a 2.0 s compile. Exit: a size-priced budget (Tti), today's behaviour below its size.
fn allocate(
    function: MFunction,
    name: &str,
    level: Level,
) -> Result<Allocated, String> {
    allocate_from(function, name, level, None)
}

/// [`allocate`], from the first allocation of `function` where one is at hand.
fn allocate_from(
    function: MFunction,
    name: &str,
    level: Level,
    first: Option<First>,
) -> Result<Allocated, String> {
    let first = match first {
        Some(first) => first,
        None => allocate_first(function.clone(), name, level)?,
    };
    let best = improved(first, function.clone(), name, level);
    // DEVIATE Z80-FOLD-PRICE (owner z80-b, exit 2026-11-30): the pin's spill
    // slots are all frame cells, so it folds every spilled read. Here a slot
    // can also be a `push` and a `pop` (1 B against 3 B and no frame), chosen
    // after allocation: a spilled read is taken from its cell where the
    // function is cheaper for it, every other choice explored on both. Exit:
    // the stack slots chosen before the fold.
    if !best.0.blocks.iter().flat_map(|block| &block.insts).any(reads_its_cell) {
        return Ok(best);
    }
    crate::regalloc::NO_FOLD.with(|one| one.set(true));
    let other = allocate_first(function.clone(), name, level).map(|first| improved(first, function, name, level));
    crate::regalloc::NO_FOLD.with(|one| one.set(false));
    Ok(match other {
        Ok(other) => match (priced_in_loops(&best, level), priced_in_loops(&other, level)) {
            (Some(now), Some(then)) if cheaper(level, then, now) => other,
            _ => best,
        },
        Err(_) => best,
    })
}

/// Whether joining the guard and latch of the repeating loops stays for this
/// function: decided once, on the first allocation of each, so the variants
/// below all run in the one mode. Joined, the allocator can spill and place
/// slots worse; the function as it was wins where it prices cheaper (DEVIATE
/// Z80T1).
fn repeating_loop_fold_wins(
    function: &MFunction,
    name: &str,
    level: Level,
) -> (bool, Option<First>) {
    if matches!(level, Level::Os | Level::Oz) {
        return (true, None);
    }
    LOOPS_FOLDED.with(|one| one.set(false));
    let Ok(joined) = allocate_first(function.clone(), name, level) else { return (true, None) };
    if !LOOPS_FOLDED.with(|one| one.get()) {
        return (true, Some(joined));
    }
    HOT_LOOP_FOLD.with(|one| one.set(false));
    let apart = allocate_first(function.clone(), name, level);
    HOT_LOOP_FOLD.with(|one| one.set(true));
    let Ok(apart) = apart else { return (true, Some(joined)) };
    // the allocation kept is the first of the plain function in the chosen mode
    if matches!(
        (priced_in_loops(&joined.0, level), priced_in_loops(&apart.0, level)),
        (Some(now), Some(then)) if cheaper(level, then, now)
    ) {
        (false, Some(apart))
    } else {
        (true, Some(joined))
    }
}

/// What a first allocation leaves: the allocation, whether `strict` held a
/// byte back from `a`, and whether the other choices are worth exploring (not
/// after the fallback).
type First = (Allocated, bool, bool);

/// The first allocation of `function`; where it fails, once more with the
/// copies from the argument registers left unjoined.
fn allocate_first(
    function: MFunction,
    name: &str,
    level: Level,
) -> Result<First, String> {
    match allocate_with(None, true, function.clone(), name, level) {
        Ok((best, held)) => Ok((best, held, true)),
        Err(first) => allocate_with(Some(true), true, function, name, level)
            .map(|(done, _)| (done, false, false))
            .map_err(|_| first),
    }
}

fn allocate_with(
    unjoined: Option<bool>,
    strict: bool,
    function: MFunction,
    name: &str,
    level: Level,
) -> Result<(Allocated, bool), String> {
    crate::regalloc::UNJOINED.with(|one| one.set(unjoined));
    let done = allocate_once(function, name, strict, level);
    crate::regalloc::UNJOINED.with(|one| one.set(None));
    done
}

/// The other choices, allocated too, each stays if it is cheaper than the
/// best so far: the argument copies joined or not, and a byte wanted both in
/// `a` and in a register of its own loaded through `a` or through `hl`.
fn improved(
    (mut best, held, explore): First,
    function: MFunction,
    name: &str,
    level: Level,
) -> Allocated {
    if !explore {
        return best;
    }
    let has_loop = !Cfg::new(&function).loops().is_empty();
    let other_join = Some(!crate::abi::z80().pinned_copies_unjoined);
    let mut others = Vec::new();
    if has_loop {
        others.push((other_join, true));
    }
    if held {
        others.push((None, false));
    }
    for (unjoined, strict) in others {
        let Ok((other, _)) = allocate_with(unjoined, strict, function.clone(), name, level) else { continue };
        if let (Some(now), Some(then)) = (priced_in_loops(&best, level), priced_in_loops(&other, level)) {
            if cheaper(level, then, now) {
                best = other;
            }
        }
    }
    best
}

/// An ALU op or compare whose second source is a frame cell (`Target::fold_spill`).
fn reads_its_cell(inst: &Inst) -> bool {
    matches!(
        inst.kind,
        Kind::Op("ADD8r" | "SUB8r" | "AND8r" | "OR8r" | "XOR8r" | "CP8r" | "AND16" | "OR16" | "XOR16")
    ) && matches!(inst.operands.get(2), Some(Operand::Frame(_)))
        || matches!(inst.kind, Kind::Op("RELOAD8L"))
}

thread_local! {
    /// Whether `allocate_once` joins the guard and latch of the loops that
    /// repeat ([`repeating_loops`]).
    static HOT_LOOP_FOLD: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
    /// Whether the last allocations folded a repeating loop's guard and latch.
    static LOOPS_FOLDED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

thread_local! {
    /// Full allocations run since the last [`take_allocations`].
    static ALLOCATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// The request's `slot_by_conflicts`, for the allocation on this thread.
    static SLOT_BY_CONFLICTS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

thread_local! {
    static STACK_REFUSALS: std::cell::RefCell<Vec<(i32, StackRefusal)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The slots `stack_slots` refused since the last call, with the first reason
/// each.
pub fn take_stack_refusals() -> Vec<(i32, StackRefusal)> {
    STACK_REFUSALS.with(|one| std::mem::take(&mut *one.borrow_mut()))
}

/// The allocations run on this thread since the last call.
pub fn take_allocations() -> usize {
    ALLOCATIONS.with(|one| one.replace(0))
}

/// One allocation, and whether `strict` held a byte back from `a`.
fn allocate_once(
    mut function: MFunction,
    name: &str,
    strict: bool,
    level: Level,
) -> Result<(Allocated, bool), String> {
    if std::env::var("LLRM_NO_Z80_NARROW").is_err() {
        crate::narrow::narrow_low_only_pairs(&mut function);
    }
    two_address(&mut function);
    let held = read_bytes_through_their_pairs(&mut function, strict);
    use_the_shift_carry(&mut function);
    if std::env::var("LLRM_NO_Z80_ADC").is_err() {
        crate::carry::use_the_carry(&mut function);
    }
    if std::env::var("LLRM_NO_Z80_STEP").is_err() {
        crate::step::step_by_small_constants(&mut function);
    }
    if std::env::var("LLRM_NO_Z80_BUMP").is_err() {
        crate::bump::step_by_flags(&mut function);
    }
    sink_copies_before_allocation(&mut function);
    if std::env::var("LLRM_NO_Z80_FORWARD").is_err() {
        crate::forward::read_pairs_from_their_registers(&mut function);
        if std::env::var("LLRM_NO_Z80_CARRY").is_err() {
            crate::forward::carry_pairs_across_edges(&mut function);
        }
    }
    if std::env::var("LLRM_NO_Z80_COMMUTE").is_err() {
        crate::commute::commute_to_the_dying_source(&mut function);
    }
    if std::env::var("LLRM_Z80_DUMP").is_ok_and(|one| one == name) {
        dump(&function);
    }
    ALLOCATIONS.with(|one| one.set(one.get() + 1));
    let mut frequencies: Option<llrm_support::hash::HashMap<u32, f64>> = None;
    let (assignment, slots) = allocate_greedy::<Z80Registers>(&mut function, Regs(&REGISTER_INFO), false)?;
    let reads = expand::reads(&function);
    if std::env::var("LLRM_Z80_DUMP_RA").is_ok_and(|one| one == name) {
        dump(&function);
    }
    remove_identity_copies(&mut function, &assignment);
    tidy_copies(&mut function, &assignment);
    use_lanes_in_place(&mut function, &assignment);
    // DEVIATE Z80T1: tail merging crosses loops at Os/Oz, and elsewhere only
    // the loops `repeating_loops` names, kept where the function without it
    // is not cheaper (`improved`). The pin merges
    // across loops before block placement (BranchFolding.cpp L1173-1203 limit
    // it to after placement), and rotateLoop does not move the merged test
    // back (FallThrough2Top ties ExitFreq). The merged guard and latch of a
    // loop cost one entry jump: -3 B for +12 T per call (xs8 -O2 28/507/105
    // -> 25/524/106), a trade the z80 has no rate for. Exit: a rate for the
    // z80, or a form of the merge that costs no clocks. Pass:
    // `BranchFolder::OptimizeFunction` with `TailMergeBlocks`, its
    // `MBBFreqInfo` kept as blocks split and merge.
    let bytes_first = matches!(level, Level::Os | Level::Oz);
    if bytes_first {
        llrm_spike_mf::branch_fold_hoisting_across_loops::<Z80Registers>(&mut function, &assignment);
    } else {
        let hot = if HOT_LOOP_FOLD.with(|one| one.get()) { repeating_loops(&function) } else { Default::default() };
        let mut moved = expand::frequencies();
        let changed = llrm_spike_mf::branch_fold_hoisting_across_hot_loops::<Z80Registers>(
            &mut function,
            &assignment,
            &hot,
            &mut moved,
        );
        if changed && !hot.is_empty() {
            LOOPS_FOLDED.with(|one| one.set(true));
            frequencies = Some(moved);
        }
    }
    place_copies_where_needed(&mut function, &assignment);
    read_copies_at_their_source(&mut function, &assignment);
    if std::env::var("LLRM_NO_Z80_UPDATE").is_err() {
        crate::post_update::fold(&mut function, &assignment);
    }
    let size = |class| if Z80Registers::bytes_of(class) == 1 { 1 } else { 2 };
    let slots: Vec<(i32, usize)> = slots.into_iter().map(|(slot, class)| (slot, size(class))).collect();
    // the function with every slot in the frame, to keep if the push and pop
    // form is not cheaper
    let mut framed = function.clone();
    spills_first(&mut framed, &assignment);
    narrow_reload_for_byte_shift(&mut framed, &assignment);
    drop_repeated_slot_accesses(&mut framed, &assignment, &slots);
    let framed = (framed, assignment.clone(), slots.clone(), reads.clone(), frequencies.clone());
    // Pushes and pops are 1 B and 10-11 T where a slot through IX is 3 B and
    // 19 T a byte, but they change what the printer and the later choices
    // keep in a pair (fill_nest -Os: the constant 4660 left bc, +65% T): the
    // function is priced framed, with the slots that are plain stacks, and
    // with the slots read again peeked too, by the rule that picks the other
    // variants. More slots on the stack is not always cheaper
    // (logical_value -Os: +12 B).
    let mut best_price = priced_in_loops(&framed, level);
    let mut best = framed;
    // the base's whole-function pass (a frame of spills only is a stack, and
    // no frame) is the start to beat
    let mut frameless = function.clone();
    if frameless_slots_are_a_stack(&mut frameless, &assignment) {
        best = (frameless, assignment.clone(), Vec::new(), reads.clone(), frequencies.clone());
        best_price = priced_in_loops(&best, level);
    }
    // Z80-SLOT-CONFLICT is one more candidate, kept only where it prices
    // lower than the others
    let mut choices = vec![(false, false), (true, false)];
    if SLOT_BY_CONFLICTS.with(std::cell::Cell::get) {
        choices.push((true, true));
    }
    for (peek, by_conflicts) in choices {
        let mut candidate = function.clone();
        let report = stack_slot_report_by(&mut candidate, &assignment, peek, by_conflicts);
        if peek && !by_conflicts {
            if std::env::var_os("LLRM_Z80_STACK_WHY").is_some() {
                for (slot, (stores, loads)) in &report.accesses {
                    let verdict = match report.refused.iter().find(|(one, _)| one == slot) {
                        Some((_, why)) => format!("refused {why:?}"),
                        None => "pushed".to_owned(),
                    };
                    eprintln!("stack-slot {name} slot {slot} stores {stores} loads {loads} {verdict}");
                }
            }
            STACK_REFUSALS.with(|one| one.borrow_mut().extend(report.refused.iter().copied()));
        }
        let pushed = report.pushed;
        if pushed.is_empty() {
            continue;
        }
        let mut slots = slots.clone();
        slots.retain(|(slot, _)| !pushed.contains(slot));
        if !slots.is_empty() {
            spills_first(&mut candidate, &assignment);
            narrow_reload_for_byte_shift(&mut candidate, &assignment);
            drop_repeated_slot_accesses(&mut candidate, &assignment, &slots);
        }
        let stacked = (candidate, assignment.clone(), slots, reads.clone(), frequencies.clone());
        let now = priced_in_loops(&stacked, level);
        if std::env::var_os("LLRM_Z80_STACK_PRICE").is_some() {
            eprintln!(
                "stack-price {name} {level:?} peek {peek} conflicts {by_conflicts} best {best_price:?} stacked {now:?}"
            );
        }
        if keeps_stacked(level, best_price, now) {
            best = stacked;
            best_price = now;
        }
    }
    // A conditional return skips the epilogue, so a function with a frame keeps its jump.
    if !expand::needs_frame(&best.0, &best.2) {
        return_conditionally(&mut best.0);
    }
    Ok((best, held))
}

/// The headers of the loops that run more than once a time they are entered, by
/// the block frequencies of the selection (the header's against the busiest
/// block that enters it from outside). Tail merging joins their guard and
/// latch: the entry jump it leaves costs a loop's entry 12 T once and saves
/// each trip a `jp` (DEVIATE Z80T1: the pin merges across loops and leaves the
/// test's place to `rotateLoop`, which ties and does not rotate one entry and
/// one exit).
fn repeating_loops(function: &MFunction) -> llrm_support::hash::HashSet<u32> {
    let cfg = Cfg::new(function);
    cfg.loops()
        .iter()
        .filter_map(|(header, body)| {
            let id = function.blocks[*header].id;
            let entering = cfg.preds[*header]
                .iter()
                .filter(|&&pred| !body[pred])
                .filter_map(|&pred| expand::frequency_of(function.blocks[pred].id))
                .fold(0.0, f64::max);
            let inside = expand::frequency_of(id)?;
            (entering > 0.0 && inside / entering >= 2.0).then_some(id)
        })
        .collect()
}

/// The slots left in the frame lose the stores and reloads that repeat what a
/// register holds (`remove_redundant_slot_accesses`). Not the slots made a
/// stack: a `pop` and `push` that peeks is one byte each, and dropping the
/// `push` leaves the slot no stack.
fn drop_repeated_slot_accesses(
    function: &mut MFunction,
    assignment: &Assignment,
    slots: &[(i32, usize)],
) {
    if std::env::var("LLRM_NO_Z80_SLOTS").is_ok() {
        return;
    }
    let ids: Vec<i32> = slots.iter().map(|&(slot, _)| slot).collect();
    llrm_spike_mf::spill::remove_redundant_slot_accesses::<Z80Registers>(function, assignment, &REGISTER_INFO, &ids);
}

/// Whether the push and pop form of a function stays: where it prices lower
/// by the rule that picks the other variants (`prefers`: bytes first at the
/// size levels, the smaller relative regret at -O1, weighted clocks plus 3 a
/// byte otherwise), or where either price is missing.
fn keeps_stacked(
    level: Level,
    framed: Option<(u32, u64)>,
    stacked: Option<(u32, u64)>,
) -> bool {
    match (framed, stacked) {
        (Some(was), Some(now)) => prefers(level, now, was),
        _ => true,
    }
}

/// The sign of a word tested next to its shift left by one (`c & 0x8000 ? .. :
/// ..` beside `c << 1`) is the carry the shift left behind: `jr c` after `sla
/// e; rl d` (or `add hl, hl`). The test copied the high byte before the shift
/// and read its bit after (`ld h, d` ... `bit 7, h`), 5 B and 12 T on each of
/// crc16's 8 trips.
///
/// Only a test whose value is the one the shift read (the copy the shift is
/// tied to, unchanged since), read by branches in the block, with no flags set
/// between the shift and the test.
pub fn use_the_shift_carry(function: &mut MFunction) {
    for block in &mut function.blocks {
        let mut at = 0;
        while at < block.insts.len() {
            let found = (|| {
                let shift = &block.insts[at];
                let (
                    Kind::Op("SHL16"),
                    [Operand::Def(Reg::Virt(shifted)), Operand::Use(Reg::Virt(tied)), Operand::Imm(1)],
                ) = (shift.kind, &shift.operands[..])
                else {
                    return None;
                };
                if shifted != tied {
                    return None;
                }
                // the copy the shift's operand was made by, and the value it
                // copies
                let copy = block
                    .insts[..at]
                    .iter()
                    .rposition(
                        |inst| matches!(
                            &inst.operands[..],
                            [Operand::Def(Reg::Virt(d)), ..] if d == shifted
                        ),
                    )?;
                let [Operand::Def(_), Operand::Use(Reg::Virt(old))] = block.insts[copy].operands[..] else {
                    return None;
                };
                if block.insts[copy].kind != Kind::Copy {
                    return None;
                }
                let writes = |inst: &Inst, v: u32| {
                    inst.operands.iter().any(|o| matches!(o, Operand::Def(Reg::Virt(w)) if *w == v))
                };
                if block.insts[copy + 1..at].iter().any(|inst| writes(inst, old) || inst.call) {
                    return None;
                }
                // the test of the old value, after the shift with no flag set
                // between
                let mut test = None;
                for (k, inst) in block.insts.iter().enumerate().skip(at + 1) {
                    if inst.kind == Kind::Op("BIT7_16")
                        && matches!(
                            &inst.operands[..],
                            [_, Operand::Use(Reg::Virt(v))] if *v == old
                        )
                    {
                        test = Some(k);
                        break;
                    }
                    if inst.sets_flags || inst.call || writes(inst, old) {
                        return None;
                    }
                }
                let test = test?;
                // every reader is a branch of this block
                let readers: Vec<usize> =
                    (test + 1..block.insts.len()).filter(|&k| block.insts[k].reads_flags).collect();
                let flags_set_later = block.insts[test + 1..].iter().any(|inst| inst.sets_flags);
                let ends_here = block.insts.last().is_some_and(|inst| inst.terminator);
                let branches = readers.iter().all(|&k| matches!(block.insts[k].kind, Kind::Op("JPZ" | "JPNZ")));
                (!readers.is_empty() && branches && !flags_set_later && ends_here).then_some((test, readers))
            })();
            if let Some((test, readers)) = found {
                for k in readers {
                    block.insts[k].kind = match block.insts[k].kind {
                        Kind::Op("JPNZ") => Kind::Op("JPC"),
                        _ => Kind::Op("JPNC"),
                    };
                }
                block.insts.remove(test);
            }
            at += 1;
        }
    }
}

/// A byte loaded through `hl` that is wanted in `a` is loaded through
/// the pointer's own pair (`ld a, (bc)`), so the copy into `hl` is gone and
/// `hl` stays free for a pointer that has no other form (`cp (hl)`): scmp
/// copied both its pointers into `hl` on every trip. The use in `a` is
/// followed through copies, around loops.
fn read_bytes_through_their_pairs(
    function: &mut MFunction,
    strict: bool,
) -> bool {
    let copy_of = |inst: &Inst| match (inst.kind, &inst.operands[..]) {
        (Kind::Copy, [Operand::Def(to), Operand::Use(from)]) => Some((*to, *from)),
        _ => None,
    };
    // the values wanted in `a`: some use is a copy into `a` or into such a
    // value
    let mut in_a = llrm_support::hash::HashSet::<u32>::default();
    loop {
        let before = in_a.len();
        for inst in function.blocks.iter().flat_map(|block| &block.insts) {
            if let Some((to, Reg::Virt(from))) = copy_of(inst) {
                if to == Reg::Phys(reg::A) || matches!(to, Reg::Virt(w) if in_a.contains(&w)) {
                    in_a.insert(from);
                }
            }
        }
        if in_a.len() == before {
            break;
        }
    }
    // a byte some instruction other than a copy reads from its own register
    // is wanted there too: loaded in `a` it would be copied out again. Which of
    // the two is cheaper depends on the pressure on `hl`, so `strict` holds
    // such a byte back and the caller tries both
    let mut held = false;
    let read_elsewhere: llrm_support::hash::HashSet<u32> = function
        .blocks
        .iter()
        .flat_map(|block| &block.insts)
        .filter(|inst| inst.kind != Kind::Copy)
        .flat_map(|inst| &inst.operands)
        .filter_map(|operand| match operand {
            Operand::Use(Reg::Virt(v)) => Some(*v),
            _ => None,
        })
        .collect();
    for block in &mut function.blocks {
        for at in 0..block.insts.len().saturating_sub(1) {
            let (copy, load) = (&block.insts[at], &block.insts[at + 1]);
            let (
                Some((Reg::Phys(hl), _)),
                Kind::Op("LD8rm"),
                [Operand::Def(Reg::Virt(byte)), Operand::Use(Reg::Phys(pointer))],
            ) = (copy_of(copy), load.kind, &load.operands[..])
            else {
                continue;
            };
            if hl != reg::HL || *pointer != reg::HL || !in_a.contains(byte) {
                continue;
            }
            if strict && read_elsewhere.contains(byte) {
                held = true;
                continue;
            }
            let (Operand::Use(from), Operand::Def(to)) = (copy.operands[1].clone(), load.operands[0].clone()) else {
                continue;
            };
            let mut read = load.clone();
            read.kind = Kind::Op("LD8Arp");
            read.operands = vec![Operand::Def(Reg::Phys(reg::A)), Operand::Use(from)];
            let mut out = copy.clone();
            out.operands = vec![Operand::Def(to), Operand::Use(Reg::Phys(reg::A))];
            block.insts[at] = read;
            block.insts[at + 1] = out;
        }
    }
    held
}

/// The pair `register` is, or is half of, if `push` and `pop` can move it: not
/// `a`, whose pair holds the flags.
fn stack_pair(register: llrm_lir::registers::RegId) -> Option<llrm_lir::registers::RegId> {
    [reg::BC, reg::DE, reg::HL].into_iter().find(|pair| REGISTER_INFO.root(register) == *pair)
}

/// Which part of its pair `register` is: 0 the whole pair, 1 the low half, 2 the high half. A `pop` puts each
/// half back where it was pushed from, so a slot's spills and reloads must name the same part.
fn pair_lane(register: llrm_lir::registers::RegId) -> u8 {
    match (stack_pair(register), register) {
        (Some(pair), one) if pair == one => 0,
        (Some(pair), one) if expand::lanes(pair).is_ok_and(|(lo, _)| lo == one) => 1,
        _ => 2,
    }
}

/// For each instruction of each block, whether it lies inside the stack arguments of a call: after an
/// `ADJCALLSTACKDOWN` of bytes and before the matching `ADJCALLSTACKUP`.
fn call_areas(function: &MFunction) -> Vec<Vec<bool>> {
    let entry = expand::call_bytes_at_entry(function);
    function
        .blocks
        .iter()
        .enumerate()
        .map(|(b, block)| {
            let mut area = entry[b].0 != 0;
            block
                .insts
                .iter()
                .map(|inst| {
                    match (inst.kind, inst.operands.first()) {
                        (Kind::Op("ADJCALLSTACKDOWN"), Some(Operand::Imm(size))) if *size != 0 => area = true,
                        (Kind::Op("ADJCALLSTACKUP"), Some(Operand::Imm(size))) if *size != 0 => area = false,
                        _ => {}
                    }
                    area
                })
                .collect()
        })
        .collect()
}

/// The pass the push-and-pop conversion was before `stack_slots`, the start
/// the other forms must beat. Spill slots that nothing else needs a frame for
/// become `push` and `pop`, and the frame goes: `digits` kept a counter across
/// a call with `ld -1(ix), b` / `ld b, -1(ix)` and the 13 bytes of frame setup
/// and teardown SDCC does without.
///
/// Every path must treat the slots as a stack: a store with the slot empty, a
/// load of the slot on top, an empty stack at each return, one stack where
/// paths meet. A byte slot saves its pair, so the other half must not be live
/// after the `pop` (`PhysLiveness`). `a` is not saved this way: its pair is the
/// flags.
pub fn frameless_slots_are_a_stack(
    function: &mut MFunction,
    assignment: &Assignment,
) -> bool {
    use llrm_spike_mf::livephys::PhysLiveness;
    if !function.locals.is_empty() || !function.fixed.is_empty() {
        return false;
    }
    let register = |reg: Reg| match reg {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(id) => id,
    };
    let spill = |inst: &Inst| match (inst.kind, &inst.operands[..]) {
        (Kind::Op("SPILL8" | "SPILL16"), [Operand::Frame(slot), Operand::Use(reg)]) => {
            Some((true, *slot, register(*reg)))
        }
        (Kind::Op("RELOAD8" | "RELOAD16"), [Operand::Def(reg), Operand::Frame(slot)]) => {
            Some((false, *slot, register(*reg)))
        }
        _ => None,
    };
    let mut any = false;
    let mut lane_of: std::collections::BTreeMap<i32, u8> = std::collections::BTreeMap::new();
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        if inst.operands.iter().any(|one| matches!(one, Operand::Frame(_))) {
            match spill(inst) {
                Some((_, slot, register)) if stack_pair(register).is_some() => {
                    // a byte saved from `e` and reloaded into `d` came back as it was in `d`
                    if *lane_of.entry(slot).or_insert(pair_lane(register)) != pair_lane(register) {
                        return false;
                    }
                    any = true;
                }
                _ => return false,
            }
        }
    }
    if !any {
        return false;
    }
    // a `push` or `pop` between the pushes of a call's stack arguments moves them; such a spill stays a slot
    // (x87_arguments -O2: "a pushed spill inside the stack arguments of a call")
    let areas = call_areas(function);
    let succs = llrm_spike_mf::regalloc::successors(function);
    let mut entry: Vec<Option<Vec<i32>>> = vec![None; function.blocks.len()];
    entry[0] = Some(Vec::new());
    let mut work = vec![0usize];
    while let Some(b) = work.pop() {
        let mut stack = entry[b].clone().expect("a block on the list has a stack");
        for (at, inst) in function.blocks[b].insts.iter().enumerate() {
            if areas[b][at] && spill(inst).is_some() {
                return false;
            }
            match spill(inst) {
                Some((true, slot, _)) if !stack.contains(&slot) => stack.push(slot),
                Some((false, slot, _)) if stack.last() == Some(&slot) => {
                    stack.pop();
                }
                Some(_) => return false,
                None => {}
            }
        }
        if succs[b].is_empty() && !stack.is_empty() {
            return false;
        }
        for &next in &succs[b] {
            match &entry[next] {
                Some(seen) if *seen != stack => return false,
                Some(_) => {}
                None => {
                    entry[next] = Some(stack.clone());
                    work.push(next);
                }
            }
        }
    }
    // the other half of a pair a byte slot saves is dead once the pair is
    // restored
    let live = PhysLiveness::of::<Z80Registers>(function, assignment, &REGISTER_INFO);
    for (b, block) in function.blocks.iter().enumerate() {
        for (at, inst) in block.insts.iter().enumerate() {
            let Some((false, _, register)) = spill(inst) else { continue };
            let pair = stack_pair(register).expect("checked above");
            if register != pair {
                let after = live.live_before::<Z80Registers>(function, assignment, &REGISTER_INFO, b, at + 1);
                let (lo, hi) = expand::lanes(pair).expect("a pair has two halves");
                let mate = if register == lo { hi } else { lo };
                if after.is_live(&REGISTER_INFO, mate) {
                    return false;
                }
            }
        }
    }
    for inst in function.blocks.iter_mut().flat_map(|block| &mut block.insts) {
        match inst.kind {
            Kind::Op("SPILL8" | "SPILL16") => {
                inst.kind = Kind::Op("PUSHP");
                inst.operands.remove(0);
                inst.may_store = false;
            }
            Kind::Op("RELOAD8" | "RELOAD16") => {
                inst.kind = Kind::Op("POPP");
                inst.operands.remove(1);
                inst.may_load = false;
            }
            _ => {}
        }
    }
    true
}

/// Whether any spill slot of `function` became a `push` and `pop`: see
/// [`stack_slots`].
pub fn slots_are_a_stack(
    function: &mut MFunction,
    assignment: &Assignment,
) -> bool {
    !stack_slots(function, assignment).is_empty()
}

/// Spill slots that are used as a stack become `push` and `pop`: `digits` kept
/// a counter across a call with `ld -1(ix), b` / `ld b, -1(ix)` and the 13
/// bytes of frame setup and teardown SDCC does without, and a slot through IX
/// costs 3 B each way a byte where a pair is 1 B each way.
///
/// A slot is one when every access is the same lane (a word, or the same half
/// of a pair: `pop` puts the low byte back in a low half) and every path
/// treats it as such: a store with the slot not
/// on the stack, a load of the slot on top, an empty stack at each return, one
/// stack where paths meet. A byte slot saves its pair, so the other half must
/// not be live after the `pop` (`PhysLiveness`). `a` is not saved this way: its
/// pair is the flags. No spill sits between the reservation of a call's stack
/// arguments and the call: the arguments must lie lowest. A slot that fails
/// stays in the frame; the others are pushed below it, past the locals.
///
/// Returns the slots made pushes and pops.
pub fn stack_slots(
    function: &mut MFunction,
    assignment: &Assignment,
) -> Vec<i32> {
    stack_slot_report(function, assignment, true).pushed
}

/// Why a spill slot stays in the frame instead of becoming a `push` and `pop`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StackRefusal {
    /// A register that is not a pair half (`a`): its pair holds the flags.
    NoPair,
    /// A spill between the reservation of a call's stack arguments and the
    /// call.
    CallArea,
    /// A byte stored from both halves of a pair (a pop puts it back in one), or
    /// a slot read as a word and as a byte. A byte loaded into the other half
    /// than it was stored from is a pop and a copy (huffman_decode -O1); a word
    /// may be loaded into any pair.
    DifferentLane,
    /// A store with the slot already on the stack.
    StoreOnStack,
    /// A load of the slot with another slot on top.
    LoadNotOnTop,
    /// A return with the slot still on the stack.
    StackAtReturn,
    /// Two paths meet with different stacks.
    PathsDisagree,
    /// A byte slot whose pair's other half is live after the `pop`.
    ByteMateLive,
    /// A slot an instruction reads in place (`and a, d(ix)`): a pop cannot.
    ReadInPlace,
}

/// What [`stack_slots`] decided, slot by slot.
pub struct StackReport {
    /// The slots made pushes and pops.
    pub pushed: Vec<i32>,
    /// The slots kept in the frame and the first reason each was dropped.
    pub refused: Vec<(i32, StackRefusal)>,
    /// The stores and loads of every slot in play, before any was dropped.
    pub accesses: std::collections::BTreeMap<i32, (usize, usize)>,
}

/// [`stack_slots`] with the reason for every slot it refuses.
pub fn stack_slot_report(
    function: &mut MFunction,
    assignment: &Assignment,
    peek: bool,
) -> StackReport {
    stack_slot_report_by(function, assignment, peek, false)
}

/// [`stack_slot_report`] with the choice of the slot sent back to the frame:
/// `by_conflicts` false drops the slot a load found under others (the first
/// violation); true drops the slot in the most such conflicts.
pub fn stack_slot_report_by(
    function: &mut MFunction,
    assignment: &Assignment,
    peek: bool,
    by_conflicts: bool,
) -> StackReport {
    use llrm_spike_mf::livephys::PhysLiveness;
    let register = |reg: Reg| match reg {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(id) => id,
    };
    let spill = |inst: &Inst| match (inst.kind, &inst.operands[..]) {
        (Kind::Op("SPILL8" | "SPILL16"), [Operand::Frame(slot), Operand::Use(reg)]) => {
            Some((true, *slot, register(*reg)))
        }
        (Kind::Op("RELOAD8" | "RELOAD16"), [Operand::Def(reg), Operand::Frame(slot)]) => {
            Some((false, *slot, register(*reg)))
        }
        _ => None,
    };
    // the slots in play, less those no stack can hold
    let mut chosen: std::collections::BTreeSet<i32> = std::collections::BTreeSet::new();
    let mut barred: std::collections::BTreeMap<i32, StackRefusal> = std::collections::BTreeMap::new();
    let mut accesses: std::collections::BTreeMap<i32, (usize, usize)> = std::collections::BTreeMap::new();
    let mut lane_of: std::collections::BTreeMap<i32, u8> = std::collections::BTreeMap::new();
    let mut load_lanes: std::collections::BTreeMap<i32, std::collections::BTreeSet<u8>> =
        std::collections::BTreeMap::new();
    let areas = call_areas(function);
    for (b, block) in function.blocks.iter().enumerate() {
        for (at, inst) in block.insts.iter().enumerate() {
            let area = areas[b][at];
            if spill(inst).is_none() {
                for operand in &inst.operands {
                    if let Operand::Frame(slot) = operand
                        && inst.may_load
                    {
                        chosen.insert(*slot);
                        barred.entry(*slot).or_insert(StackRefusal::ReadInPlace);
                    }
                }
            }
            if let Some((store, slot, register)) = spill(inst) {
                chosen.insert(slot);
                let counts = accesses.entry(slot).or_default();
                if store {
                    counts.0 += 1;
                } else {
                    counts.1 += 1;
                }
                // only the stores say which half the pop puts the byte back in: a load
                // into the other half is a copy after the pop
                let lane = pair_lane(register);
                if store {
                    if *lane_of.entry(slot).or_insert(lane) != lane {
                        barred.entry(slot).or_insert(StackRefusal::DifferentLane);
                    }
                } else {
                    load_lanes.entry(slot).or_default().insert(lane);
                }
                if stack_pair(register).is_none() {
                    barred.entry(slot).or_insert(StackRefusal::NoPair);
                } else if area {
                    barred.entry(slot).or_insert(StackRefusal::CallArea);
                }
            }
        }
    }
    // a byte comes back in the other half through a copy; a word and a byte do not meet
    let mut crossing: std::collections::BTreeSet<i32> = std::collections::BTreeSet::new();
    for (slot, lanes) in &load_lanes {
        let Some(stored) = lane_of.get(slot) else { continue };
        if lanes.iter().any(|lane| lane != stored && (*lane == 0 || *stored == 0)) {
            barred.entry(*slot).or_insert(StackRefusal::DifferentLane);
        } else if lanes.iter().any(|lane| lane != stored) {
            crossing.insert(*slot);
        }
    }
    chosen.retain(|slot| !barred.contains_key(slot));
    // the slots that need a copy are tried after the others, one by one, and
    // kept only where they take no other slot from the stack
    let chosen_crossing: Vec<i32> = chosen.iter().copied().filter(|slot| crossing.contains(slot)).collect();
    chosen.retain(|slot| !crossing.contains(slot));
    let mut refused: Vec<(i32, StackRefusal)> = barred.into_iter().collect();
    if chosen.is_empty() && chosen_crossing.is_empty() {
        return StackReport { pushed: Vec::new(), refused, accesses };
    }
    let succs = llrm_spike_mf::regalloc::successors(function);
    let live = PhysLiveness::of::<Z80Registers>(function, assignment, &REGISTER_INFO);
    // the first slot that keeps `chosen` from being a stack, or none
    // per block and instruction: whether a load's slot is read again after it
    let read_again = |function: &MFunction, chosen: &std::collections::BTreeSet<i32>| -> Vec<Vec<bool>> {
        use std::collections::BTreeSet;
        let n = function.blocks.len();
        let (mut live_in, mut live_out) = (vec![BTreeSet::<i32>::new(); n], vec![BTreeSet::<i32>::new(); n]);
        let step = |inst: &Inst, now: &mut BTreeSet<i32>| match spill(inst) {
            Some((true, slot, _)) if chosen.contains(&slot) => {
                now.remove(&slot);
            }
            Some((false, slot, _)) if chosen.contains(&slot) => {
                now.insert(slot);
            }
            _ => {}
        };
        let mut changed = true;
        while changed {
            changed = false;
            for b in (0..n).rev() {
                live_out[b] = succs[b].iter().flat_map(|&next| live_in[next].iter().copied()).collect();
                let mut now = live_out[b].clone();
                function.blocks[b].insts.iter().rev().for_each(|inst| step(inst, &mut now));
                if now != live_in[b] {
                    live_in[b] = now;
                    changed = true;
                }
            }
        }
        (0..n)
            .map(|b| {
                let mut now = live_out[b].clone();
                let mut flags: Vec<bool> = function.blocks[b]
                    .insts
                    .iter()
                    .rev()
                    .map(|inst| {
                        let again = matches!(spill(inst), Some((false, slot, _)) if now.contains(&slot));
                        step(inst, &mut now);
                        again
                    })
                    .collect();
                flags.reverse();
                flags
            })
            .collect()
    };
    // DEVIATE Z80-SLOT-CONFLICT: on a load with other slots on top the pin has
    // no counterpart (StackSlotColoring.cpp only shares frame slots, by weight,
    // first fit); here the slot in the most such conflicts goes back to the
    // frame, not the slot loaded. Owner z80, exit 2026-11-20.
    let violation = |function: &MFunction,
                     chosen: &std::collections::BTreeSet<i32>,
                     peeks: &mut std::collections::BTreeSet<(usize, usize)>|
     -> Option<(i32, StackRefusal)> {
        peeks.clear();
        let mut conflicts: std::collections::BTreeMap<i32, usize> = std::collections::BTreeMap::new();
        let again = read_again(function, chosen);
        let mut entry: Vec<Option<Vec<i32>>> = vec![None; function.blocks.len()];
        entry[0] = Some(Vec::new());
        let mut work = vec![0usize];
        while let Some(b) = work.pop() {
            let mut stack = entry[b].clone().expect("a block on the list has a stack");
            for (at, inst) in function.blocks[b].insts.iter().enumerate() {
                match spill(inst) {
                    Some((true, slot, _)) if chosen.contains(&slot) => {
                        if stack.contains(&slot) {
                            return Some((slot, StackRefusal::StoreOnStack));
                        }
                        stack.push(slot);
                    }
                    Some((false, slot, _)) if chosen.contains(&slot) => {
                        if stack.last() != Some(&slot) {
                            if !by_conflicts {
                                return Some((slot, StackRefusal::LoadNotOnTop));
                            }
                            // count the slots in the way, and go on as if the
                            // load had found its slot on top
                            *conflicts.entry(slot).or_default() += 1;
                            match stack.iter().rposition(|held| *held == slot) {
                                Some(below) => {
                                    for blocker in stack.drain(below + 1..) {
                                        *conflicts.entry(blocker).or_default() += 1;
                                    }
                                }
                                None => continue,
                            }
                        }
                        // read again later: `pop` then `push` leaves it on top
                        if peek && again[b][at] {
                            peeks.insert((b, at));
                        } else {
                            stack.pop();
                        }
                    }
                    _ => {}
                }
            }
            if succs[b].is_empty() && !stack.is_empty() {
                return stack.last().map(|&slot| (slot, StackRefusal::StackAtReturn));
            }
            for &next in &succs[b] {
                match &entry[next] {
                    Some(seen) if *seen != stack => {
                        let at =
                            seen.iter().zip(&stack).position(|(a, b)| a != b).unwrap_or(seen.len().min(stack.len()));
                        return stack.get(at).or_else(|| seen.get(at)).map(|&slot| (slot, StackRefusal::PathsDisagree));
                    }
                    Some(_) => {}
                    None => {
                        entry[next] = Some(stack.clone());
                        work.push(next);
                    }
                }
            }
        }
        if let Some((&slot, _)) = conflicts.iter().max_by_key(|(slot, count)| (**count, std::cmp::Reverse(**slot))) {
            return Some((slot, StackRefusal::LoadNotOnTop));
        }
        // the other half of a pair a byte slot saves is dead once the pair is
        // restored
        for (b, block) in function.blocks.iter().enumerate() {
            for (at, inst) in block.insts.iter().enumerate() {
                let Some((false, slot, register)) = spill(inst) else { continue };
                if !chosen.contains(&slot) {
                    continue;
                }
                let pair = stack_pair(register).expect("checked above");
                if register != pair {
                    let after = live.live_before::<Z80Registers>(function, assignment, &REGISTER_INFO, b, at + 1);
                    let (lo, hi) = expand::lanes(pair).expect("a pair has two halves");
                    let mate = if register == lo { hi } else { lo };
                    if after.is_live(&REGISTER_INFO, mate) {
                        return Some((slot, StackRefusal::ByteMateLive));
                    }
                }
            }
        }
        None
    };
    let mut peeks = std::collections::BTreeSet::new();
    while let Some((slot, why)) = violation(function, &chosen, &mut peeks) {
        chosen.remove(&slot);
        refused.push((slot, why));
    }
    if !chosen_crossing.is_empty() {
        for slot in chosen_crossing {
            chosen.insert(slot);
            if let Some((_, why)) = violation(function, &chosen, &mut peeks) {
                chosen.remove(&slot);
                refused.push((slot, why));
            }
        }
        // the reads to peek are those of the stack as kept
        let settled = violation(function, &chosen, &mut peeks);
        debug_assert!(settled.is_none(), "{settled:?}");
    }
    for (b, block) in function.blocks.iter_mut().enumerate() {
        let mut rewritten = Vec::with_capacity(block.insts.len());
        for (at, mut inst) in std::mem::take(&mut block.insts).into_iter().enumerate() {
            let mut after = None;
            match spill(&inst) {
                Some((true, slot, _)) if chosen.contains(&slot) => {
                    inst.kind = Kind::Op("PUSHP");
                    inst.operands.remove(0);
                    inst.may_store = false;
                }
                Some((false, slot, loaded)) if chosen.contains(&slot) => {
                    inst.kind = Kind::Op(if peeks.contains(&(b, at)) { "PEEKP" } else { "POPP" });
                    inst.operands.remove(1);
                    inst.may_load = false;
                    // the pop put the byte back in the half it was stored from
                    if let (Some(&stored), Some(pair)) = (lane_of.get(&slot), stack_pair(loaded)) {
                        if stored != pair_lane(loaded) {
                            let (lo, hi) = expand::lanes(pair).expect("a pair has two halves");
                            let from = if stored == 1 { lo } else { hi };
                            after = Some(Inst::copy(Reg::Phys(loaded), Reg::Phys(from)));
                        }
                    }
                }
                _ => {}
            }
            rewritten.push(inst);
            rewritten.extend(after);
        }
        block.insts = rewritten;
    }
    refused.sort_by_key(|(slot, _)| *slot);
    StackReport { pushed: chosen.into_iter().collect(), refused, accesses }
}

/// Moves the entry block's spills of argument registers up to its start while
/// nothing before them writes the register or touches the slot, so the
/// prologue can `push` them: crc16 stored `n` and `p` with `ld -1(ix), a` /
/// `ld -3(ix), l` / `ld -2(ix), h` after a pair of `push af`, 9 B more than
/// `push af` / `push hl`.
pub fn spills_first(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    let register = |reg: Reg| match reg {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(id) => id,
    };
    if llrm_spike_mf::regalloc::successors(function).iter().any(|next| next.contains(&0)) {
        return;
    }
    let insts = &mut function.blocks[0].insts;
    let mut first = 0;
    for at in 0..insts.len() {
        let (Kind::Op("SPILL8" | "SPILL16"), [Operand::Frame(slot), Operand::Use(source)]) =
            (insts[at].kind, &insts[at].operands[..])
        else {
            continue;
        };
        let (slot, source) = (*slot, register(*source));
        let blocked = insts[first..at]
            .iter()
            .any(
                |before| before.call
                    || before.barrier
                    || before.terminator
                    || before.operands.iter().any(|operand| match operand {
                        Operand::Def(r) => Z80Registers::overlaps(register(*r), source),
                        Operand::Frame(other) => *other == slot,
                        _ => false,
                    }),
            );
        if !blocked {
            let moved = insts.remove(at);
            insts.insert(first, moved);
            first += 1;
        }
    }
}

/// A constant word stored from an immediate in a loop (`LD16mi`, `LD16fi`) is the constant and a store from a
/// register again, so [`hoist_constants`] can hold it across the loop: 6 T a trip cheaper, a byte or two more
/// (3 B once and 2 B a store against 4 B). The variant stays only where `priced_in_loops` says it is cheaper.
/// The selector chose the immediate form without a loop depth to see, as the pin's x86 does (`imm_su` counts
/// users only); this is the frequency-aware half, beyond the pin (row Z80-IMM-LOOP). Linear in the instructions.
/// Whether anything was split.
fn split_loop_constant_stores(function: &mut MFunction) -> bool {
    let mut changed = false;
    let cfg = Cfg::new(function);
    let in_loop: Vec<bool> = (0..function.blocks.len()).map(|b| cfg.loops().iter().any(|(_, body)| body[b])).collect();
    for (b, _) in in_loop.iter().enumerate().filter(|(_, in_loop)| **in_loop) {
        let mut at = 0;
        while at < function.blocks[b].insts.len() {
            let inst = &mut function.blocks[b].insts[at];
            let (store, Some(Operand::Imm(value))) = (
                match inst.kind {
                    Kind::Op("LD16mi") => "LD16mr",
                    Kind::Op("LD16fi") => "LD16fr",
                    _ => {
                        at += 1;
                        continue;
                    }
                },
                inst.operands.first().cloned(),
            ) else {
                at += 1;
                continue;
            };
            let constant = function.classes.len() as u32;
            function.classes.push(crate::mfunc::Class::Gr16);
            inst.kind = Kind::Op(store);
            inst.operands[0] = Operand::Use(Reg::Virt(constant));
            let mut made = Inst::new(Kind::Op("LD16ri"), vec![Operand::Def(Reg::Virt(constant)), Operand::Imm(value)]);
            made.rematerializable = true;
            made.cheap_as_move = true;
            function.blocks[b].insts.insert(at, made);
            changed = true;
            at += 2;
        }
    }
    changed
}

/// A constant word made in a block and stored there from a pair, with no other reader, is stored as immediates
/// (4 B and 38 T against 9 B and 48 T in the frame): the one case the choice needs no pricing. The selector took the
/// pair because the DAG's constant had a second user that became something else (an `adc` of zero, a counter), so
/// the pair serves one store. Nearest pin step `InlineSpiller::reMaterializeFor`; DEVIATE Z80-IMM-USES (owner
/// llrm-target-z80, exit 2026-11-30). Whether anything changed. Linear in the instructions.
pub fn merge_lone_constant_stores(function: &mut MFunction) -> bool {
    let mut users: llrm_support::hash::HashMap<u32, usize> = llrm_support::hash::HashMap::default();
    let mut defs: llrm_support::hash::HashMap<u32, usize> = llrm_support::hash::HashMap::default();
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        for operand in &inst.operands {
            match operand {
                Operand::Use(Reg::Virt(v)) => *users.entry(*v).or_default() += 1,
                Operand::Def(Reg::Virt(v)) => *defs.entry(*v).or_default() += 1,
                _ => {}
            }
        }
    }
    let mut changed = false;
    for block in &mut function.blocks {
        let mut made: llrm_support::hash::HashMap<u32, i64> = llrm_support::hash::HashMap::default();
        let mut gone: Vec<u32> = Vec::new();
        for inst in &mut block.insts {
            if let (Kind::Op("LD16ri"), [Operand::Def(Reg::Virt(v)), Operand::Imm(value)]) = (inst.kind, &inst.operands[..]) {
                if users.get(v) == Some(&1) && defs.get(v) == Some(&1) {
                    made.insert(*v, *value as i64);
                }
                continue;
            }
            let immediate = match inst.kind {
                Kind::Op("LD16mr") => "LD16mi",
                Kind::Op("LD16fr") => "LD16fi",
                _ => continue,
            };
            let Some(Operand::Use(Reg::Virt(v))) = inst.operands.first().cloned() else { continue };
            let Some(value) = made.remove(&v) else { continue };
            inst.kind = Kind::Op(immediate);
            inst.operands[0] = Operand::Imm(value as _);
            gone.push(v);
            changed = true;
        }
        block.insts.retain(|inst| {
            !matches!((inst.kind, &inst.operands[..]), (Kind::Op("LD16ri"), [Operand::Def(Reg::Virt(v)), Operand::Imm(_)]) if gone.contains(v))
        });
    }
    changed
}

/// A constant made in a loop is made once before it (`MachineLICM`'s hoisting
/// of a trivially rematerializable instruction): `find` made `ld hl, #buf` on
/// every trip. Where the registers run out the allocator makes the constant
/// again at its uses, as it does for any rematerializable value. Whether
/// anything moved.
fn hoist_constants(function: &mut MFunction) -> bool {
    let mut moved = false;
    let cfg = Cfg::new(function);
    let mut defs = llrm_support::hash::HashMap::<u32, usize>::default();
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        for operand in &inst.operands {
            if let Operand::Def(Reg::Virt(v)) = operand {
                *defs.entry(*v).or_default() += 1;
            }
        }
    }
    let constant = |inst: &Inst| {
        inst.rematerializable
            && matches!(
                &inst.operands[..],
                [Operand::Def(Reg::Virt(v)), Operand::Imm(_) | Operand::Symbol(..)] if defs[v] == 1
            )
    };
    // the innermost loop first, so a constant moves out as many loops as it can
    for (header, body) in cfg.loops().iter().rev() {
        let outside: Vec<usize> = cfg.preds[*header].iter().copied().filter(|p| !body[*p]).collect();
        let [pre] = outside[..] else { continue };
        if cfg.succs[pre] != [*header] {
            continue;
        }
        let mut hoisted = Vec::new();
        for (_, block) in function.blocks.iter_mut().enumerate().filter(|(b, _)| body[*b]) {
            let (out, keep): (Vec<Inst>, Vec<Inst>) = std::mem::take(&mut block.insts).into_iter().partition(&constant);
            block.insts = keep;
            hoisted.extend(out);
        }
        let made: Vec<u32> = hoisted
            .iter()
            .filter_map(|inst| match inst.operands[0] {
                Operand::Def(Reg::Virt(v)) => Some(v),
                _ => None,
            })
            .collect();
        // a commutable instruction that adds the constant to a fixed register
        // takes the other operand there
        for (_, block) in function.blocks.iter_mut().enumerate().filter(|(b, _)| body[*b]) {
            for at in 0..block.insts.len() {
                let inst = &block.insts[at];
                let uses: Vec<usize> = (0..inst.operands.len())
                    .filter(|&n| matches!(inst.operands[n], Operand::Use(Reg::Virt(_))))
                    .collect();
                let (true, [other]) = (inst.commutable, &uses[..]) else { continue };
                let Operand::Use(Reg::Virt(c)) = inst.operands[*other] else { continue };
                let Some(copy) = (0..at).rev().find(|&n| {
                    matches!(
                        &block.insts[n].operands[..],
                        [Operand::Def(Reg::Phys(_)), Operand::Use(Reg::Virt(_))]
                    )
                }) else {
                    continue;
                };
                let Operand::Use(Reg::Virt(x)) = block.insts[copy].operands[1] else { continue };
                if block.insts[copy].kind == Kind::Copy
                    && made.contains(&x)
                    && !made.contains(&c)
                    && function.classes[x as usize] == function.classes[c as usize]
                {
                    block.insts[copy].operands[1] = Operand::Use(Reg::Virt(c));
                    block.insts[at].operands[*other] = Operand::Use(Reg::Virt(x));
                }
            }
        }
        let insts = &mut function.blocks[pre].insts;
        let at = insts.iter().position(|inst| inst.terminator).unwrap_or(insts.len());
        moved |= !hoisted.is_empty();
        insts.splice(at..at, hoisted);
    }
    moved
}

/// Drops the copies the allocation gave one register.
///
/// DEVIATE from `llrm_spike_mf::remove_identity_copies`, which compares the
/// registers' roots: `b` and `c` are both `bc`, and `ld b, c` was dropped as an
/// identity (a select returned the wrong byte). Reason: that helper was written
/// for targets whose views of a root are one register. Exit: the shared helper
/// compares lanes.
pub fn remove_identity_copies(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    let register = |reg: Reg| match reg {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(id) => id,
    };
    for block in &mut function.blocks {
        block
            .insts
            .retain(
                |inst| match (inst.kind, &inst.operands[..]) {
                    (Kind::Copy, [Operand::Def(dst), Operand::Use(src)]) => register(*dst) != register(*src),
                    _ => true,
                },
            );
    }
}

/// The copies the allocation left, cleaned: the copies back first, since one
/// that reads a register stops the copy before it from moving down.
pub fn tidy_copies(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    remove_copies_back(function, assignment);
    sink_copies_to_the_branch(function, assignment);
}

/// Moves a copy down to the block's branches when nothing after it reads or
/// writes either register, so the compare before it stays last among the
/// instructions a branch hoist must move over: sum_arr's `ld b, a` sat ahead
/// of `or a, a`, and the `ld a, #0` both successors begin with could not be
/// hoisted past a compare that reads `a`.
pub fn sink_copies_to_the_branch(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    let register = |reg: Reg| match reg {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(id) => id,
    };
    sink_copies(function, |a, b| Z80Registers::overlaps(register(a), register(b)), |_, _| true);
}

/// The same before allocation, on the copies between virtual registers: a
/// value merged by a branch was copied in ahead of the compare that decides
/// the branch, so it lived beside the value it replaces and the two could not
/// share a register (minmax's `lo`: a register more, spilled).
pub fn sink_copies_before_allocation(function: &mut MFunction) {
    let mut defs = llrm_support::hash::HashMap::<u32, u32>::default();
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        for operand in &inst.operands {
            if let Operand::Def(Reg::Virt(v)) = operand {
                *defs.entry(*v).or_default() += 1;
            }
        }
    }
    // only a copy into a value several blocks define: what a branch merges
    sink_copies(
        function,
        |a, b| a == b,
        |dst, src| {
            matches!(
                (dst, src),
                (Reg::Virt(d), Reg::Virt(_)) if defs.get(&d).is_some_and(|n| *n > 1)
            )
        },
    );
}

/// Moves each copy `movable` accepts down to the block's branches when nothing
/// after it reads or writes either register (`conflicts` says when two
/// registers are one).
fn sink_copies(
    function: &mut MFunction,
    conflicts: impl Fn(Reg, Reg) -> bool,
    movable: impl Fn(Reg, Reg) -> bool,
) {
    for block in &mut function.blocks {
        let end = block.insts.iter().rposition(|inst| !inst.terminator).map_or(0, |last| last + 1);
        let mut at = 0;
        while at + 1 < end {
            let inst = &block.insts[at];
            let (Kind::Copy, [Operand::Def(dst), Operand::Use(src)]) = (inst.kind, &inst.operands[..]) else {
                at += 1;
                continue;
            };
            let (dst, src) = (*dst, *src);
            let blocked = |later: &Inst| {
                later.call
                    || later.barrier
                    || later.may_store
                    || later.side_effects
                    || later.operands.iter().any(|operand| match operand {
                        Operand::Def(r) | Operand::Use(r) => {
                            conflicts(*r, dst) || (matches!(operand, Operand::Def(_)) && conflicts(*r, src))
                        }
                        _ => false,
                    })
            };
            // only past an instruction that is not a copy (two copies that
            // touch neither register each other's would trade places for ever),
            // and only all the way: a copy stopped short sits between a reload
            // and the compare it folds into
            if !movable(dst, src)
                || (at + 1..end).all(|later| block.insts[later].kind == Kind::Copy)
                || (at + 1..end).any(|later| blocked(&block.insts[later]))
            {
                at += 1;
                continue;
            }
            let moved = block.insts.remove(at);
            block.insts.insert(end - 1, moved);
        }
    }
}

/// Drops a copy of what the register already holds: a copy back into the
/// register another copy read from (`ld b, a` ... `ld a, b`), or the same copy
/// again, while neither register is written in between. A block only one
/// earlier block enters starts from the copies that block ended with.
///
/// DEVIATE from `llrm_spike_mf::remove_redundant_copies`, which compares
/// roots (`ld a, c` after `ld b, a` is no copy back), stays in one block and
/// misses the repeat. Reason: `b` and `c` are lanes of one root. Exit: the
/// shared helper compares lanes.
pub fn remove_copies_back(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    copies_in_force(function, assignment, true);
}

/// The copies in force at the end of each block, as `(destination, source)`
/// registers; `drop` also removes the ones a copy repeats.
fn copies_in_force(
    function: &mut MFunction,
    assignment: &Assignment,
    drop: bool,
) -> Vec<Vec<(RegId, RegId)>> {
    let register = |reg: Reg| match reg {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(id) => id,
    };
    let successors = llrm_spike_mf::regalloc::successors(function);
    let mut entered = vec![0usize; function.blocks.len()];
    for to in successors.iter().flatten() {
        entered[*to] += 1;
    }
    // `(destination, source)` of the copies in force at the end of each block
    let mut ended: Vec<Vec<(RegId, RegId)>> = vec![Vec::new(); function.blocks.len()];
    for at in 0..function.blocks.len() {
        // the block's one entrance, if it came earlier in the layout
        let mut held = match successors.iter().position(|next| next.contains(&at)) {
            Some(from) if entered[at] == 1 && from < at => ended[from].clone(),
            _ => Vec::new(),
        };
        let mut keep = Vec::with_capacity(function.blocks[at].insts.len());
        for inst in std::mem::take(&mut function.blocks[at].insts) {
            let copy = match (inst.kind, &inst.operands[..]) {
                (Kind::Copy, [Operand::Def(dst), Operand::Use(src)]) => Some((register(*dst), register(*src))),
                _ => None,
            };
            let repeats = copy.is_some_and(|(dst, src)| held.contains(&(src, dst)) || held.contains(&(dst, src)));
            if drop && repeats {
                continue;
            }
            if inst.call {
                held.clear();
            }
            let written: Vec<RegId> = inst
                .operands
                .iter()
                .filter_map(|o| if let Operand::Def(r) = o { Some(register(*r)) } else { None })
                .collect();
            held.retain(|&(to, from)| {
                !written.iter().any(|&w| Z80Registers::overlaps(w, to) || Z80Registers::overlaps(w, from))
            });
            held.extend(copy);
            keep.push(inst);
        }
        function.blocks[at].insts = keep;
        ended[at] = held;
    }
    ended
}

/// Moves the copy a loop head starts with into the back edge, when every other
/// entrance has made it already: `ld a, b` at a loop head, where the entry
/// has just made it, runs every trip and once more at the exit; on the back
/// edge alone it runs once a trip. Elsewhere (a diamond's join) the move made
/// popcount and bsearch8 larger: the copy in a block of its own kept a jump.
/// The flow into the join is the sum of its entrances, so the count of copies
/// run never grows, and the code does not grow either: the copy moves, it is
/// not copied.
///
/// The pin has no such step: its allocator joins the copy out of existence
/// (`canJoinPhys`) or leaves it where selection put it. Here the join of an
/// argument's entry copy into its register is refused (`joins_pinned_copies`),
/// so the copy stays where selection put it.
pub fn place_copies_where_needed(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    let register = |reg: Reg| match reg {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(id) => id,
    };
    let successors = llrm_spike_mf::regalloc::successors(function);
    let mut changed = true;
    while changed {
        changed = false;
        let ended = copies_in_force(function, assignment, false);
        for head in 1..function.blocks.len() {
            let entrances: Vec<usize> =
                (0..function.blocks.len()).filter(|&from| successors[from].contains(&head)).collect();
            // the head's first copy, behind nothing that touches either
            // register (digits' call-stack marker came first)
            let Some((at, dst, src)) = function.blocks[head].insts.iter().enumerate().find_map(|(at, inst)| {
                let (Kind::Copy, [Operand::Def(dst), Operand::Use(src)]) = (inst.kind, &inst.operands[..]) else {
                    return None;
                };
                let (dst, src) = (register(*dst), register(*src));
                let clear = function.blocks[head].insts[..at].iter().all(|before| {
                    !(before.call
                        || before.barrier
                        || before.side_effects
                        || before.may_store
                        || before.may_load
                        || before.operands.iter().any(|operand| match operand {
                            Operand::Def(r) | Operand::Use(r) => {
                                Z80Registers::overlaps(register(*r), dst) || Z80Registers::overlaps(register(*r), src)
                            }
                            _ => false,
                        }))
                });
                clear.then_some((at, dst, src))
            }) else {
                continue;
            };
            if entrances.len() < 2 || dst == src {
                continue;
            }
            let made = |from: usize| ended[from].contains(&(dst, src)) || ended[from].contains(&(src, dst));
            let lacking: Vec<usize> = entrances.iter().copied().filter(|&from| !made(from)).collect();
            // one entrance lacks it, by a block that only leads here
            let [from] = lacking[..] else { continue };
            if successors[from] != [head] {
                continue;
            }
            let block = &function.blocks[from];
            let end = block.insts.iter().rposition(|inst| !inst.terminator).map_or(0, |last| last + 1);
            let touched = block
                .insts[end..]
                .iter()
                .any(
                    |inst| inst.operands.iter().any(|operand| match operand {
                        Operand::Def(r) | Operand::Use(r) => {
                            Z80Registers::overlaps(register(*r), dst) || Z80Registers::overlaps(register(*r), src)
                        }
                        _ => false,
                    }),
                );
            // a block of nothing but its jump is an edge a branch folds away;
            // the copy would keep it, and the jump with it
            if touched || end == 0 {
                continue;
            }
            let copy = function.blocks[head].insts.remove(at);
            function.blocks[from].insts.insert(end, copy);
            changed = true;
            break;
        }
    }
}

/// A word compared, or a register stored to the frame, right after a copy of
/// it reads the copy's source: digits' entry made `ld e, l` / `ld d, h` for the
/// compare of the dividend, and the loop head wanted it back in `hl`; crc16
/// copied `a` to `b` to store it (`ld b, a` / `ld -1(ix), b`). Only a `CP16i`
/// or a spill, which take any register of their size, is rewritten, when the
/// copy's destination is dead after the last of them and the source is not
/// written before it. The copy is then dropped.
pub fn read_copies_at_their_source(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    use llrm_spike_mf::livephys::PhysLiveness;
    let register = |reg: Reg| match reg {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(id) => id,
    };
    let pair = |r: RegId| [reg::BC, reg::DE, reg::HL].contains(&r);
    let byte = |r: RegId| [reg::A, reg::B, reg::C, reg::D, reg::E, reg::H, reg::L].contains(&r);
    'again: loop {
        let live = PhysLiveness::of::<Z80Registers>(function, assignment, &REGISTER_INFO);
        for b in 0..function.blocks.len() {
            for at in 0..function.blocks[b].insts.len() {
                let inst = &function.blocks[b].insts[at];
                let (Kind::Copy, [Operand::Def(dst), Operand::Use(src)]) = (inst.kind, &inst.operands[..]) else {
                    continue;
                };
                let (dst, src) = (register(*dst), register(*src));
                if dst == src || !(pair(dst) && pair(src) || byte(dst) && byte(src)) {
                    continue;
                }
                let mut compares = Vec::new();
                for (later, inst) in function.blocks[b].insts.iter().enumerate().skip(at + 1) {
                    let reads = inst
                        .operands
                        .iter()
                        .any(
                            |o| matches!(
                                o,
                                Operand::Use(r) if Z80Registers::overlaps(register(*r), dst)
                            ),
                        );
                    if reads {
                        // a compare of a pair, or a store of a register to the
                        // frame, takes any register of its size
                        let any = match inst.kind {
                            Kind::Op("CP16i" | "SPILL16") => pair(dst),
                            Kind::Op("SPILL8") => byte(dst),
                            _ => false,
                        };
                        let exact = any
                            && inst.operands.iter().all(|o| match o {
                                Operand::Use(r) if Z80Registers::overlaps(register(*r), dst) => register(*r) == dst,
                                _ => true,
                            });
                        if !exact {
                            compares.clear();
                            break;
                        }
                        compares.push(later);
                    }
                    let writes = |to: RegId| {
                        inst.operands
                            .iter()
                            .any(|o| matches!(o, Operand::Def(r) if Z80Registers::overlaps(register(*r), to)))
                    };
                    if inst.call || inst.barrier || writes(src) || writes(dst) {
                        break;
                    }
                }
                let Some(&last) = compares.last() else { continue };
                // the source stays as it is to the last compare (a write in the
                // compare's own instruction comes after its read)
                let after = live.live_before::<Z80Registers>(function, assignment, &REGISTER_INFO, b, last + 1);
                if after.is_live(&REGISTER_INFO, dst) {
                    continue;
                }
                let touched = function
                    .blocks[b]
                    .insts[at + 1..=last]
                    .iter()
                    .any(
                        |inst| inst.call
                            || inst
                                .operands
                                .iter()
                                .any(|o| matches!(
                                    o,
                                    Operand::Def(r) if Z80Registers::overlaps(register(*r), src) || Z80Registers::overlaps(register(*r), dst)
                                ) ),
                    );
                if touched {
                    continue;
                }
                for &later in &compares {
                    for operand in &mut function.blocks[b].insts[later].operands {
                        if matches!(operand, Operand::Use(r) if register(*r) == dst) {
                            *operand = Operand::Use(Reg::Phys(src));
                        }
                    }
                }
                function.blocks[b].insts.remove(at);
                continue 'again;
            }
        }
        break;
    }
}

/// A pair reloaded and shifted by a whole byte (`w >> 8`, `w << 8`) needs one
/// byte of the slot: the lane the shift overwrites is not loaded
/// (`ld e, -8(ix)` / `ld d, -7(ix)` / `ld e, d` / `ld d, #0` is `ld e,
/// -7(ix)` / `ld d, #0`). The pin folds a reload into its use where the
/// target has the form (`InlineSpiller::foldMemoryOperand`); this is that
/// form for the shift by 8 or more (the shift left over runs on the byte).
pub fn narrow_reload_for_byte_shift(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    let register = |reg: Reg| match reg {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(id) => id,
    };
    for block in &mut function.blocks {
        let mut at = 0;
        while at + 1 < block.insts.len() {
            let (first, second) = (&block.insts[at], &block.insts[at + 1]);
            let (
                Kind::Op("RELOAD16"),
                [Operand::Def(loaded), Operand::Frame(slot)],
                Kind::Op(shift @ ("SHL16" | "SRL16")),
                [Operand::Def(written), Operand::Use(read), Operand::Imm(count @ 8..=15)],
            ) = (first.kind, &first.operands[..], second.kind, &second.operands[..])
            else {
                at += 1;
                continue;
            };
            let rest = count - 8;
            let pair = register(*loaded);
            let Ok((lo, hi)) = expand::lanes(pair) else {
                at += 1;
                continue;
            };
            if register(*written) != pair || register(*read) != pair {
                at += 1;
                continue;
            }
            // `kept` takes the byte that moves across, read from its half of
            // the slot; `cleared` is zeroed
            let (kept, from, cleared) = if shift == "SRL16" { (lo, 1, hi) } else { (hi, 0, lo) };
            let load = Inst {
                may_load: true,
                ..Inst::new(
                    Kind::Op("RELOAD8L"),
                    vec![Operand::Def(Reg::Phys(kept)), Operand::Frame(*slot), Operand::Imm(from)],
                )
            };
            let clear = Inst::new(Kind::Op("LD8ri"), vec![Operand::Def(Reg::Phys(cleared)), Operand::Imm(0)]);
            let (a, b) = if shift == "SRL16" { (load, clear) } else { (clear, load) };
            let left_over = (rest > 0).then(|| {
                let mut one = second.clone();
                one.kind = Kind::Op(if shift == "SRL16" { "SRL8" } else { "SHL8" });
                one.operands = vec![Operand::Def(Reg::Phys(kept)), Operand::Use(Reg::Phys(kept)), Operand::Imm(rest)];
                one
            });
            block.insts[at] = a;
            block.insts[at + 1] = b;
            at += 2;
            if let Some(one) = left_over {
                block.insts.insert(at, one);
                at += 1;
            }
        }
    }
}

/// Reads the byte a `LO`/`HI` extracts straight from its half of the pair when
/// the pair keeps its value to the last use, instead of copying it to a byte
/// register first (`ld d, c` / `cp a, d` becomes `cp a, c`).
///
/// Only a byte defined once and used later in its own block, with no write to
/// that half and no call in between, is rewritten.
pub fn use_lanes_in_place(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    // a byte read in place frees the half it was copied into, so another byte
    // kept from that half may now be read in place too
    while use_lanes_once(function, assignment) {}
}

fn use_lanes_once(
    function: &mut MFunction,
    assignment: &Assignment,
) -> bool {
    let mut removed = false;
    let register = |reg: Reg| match reg {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(id) => id,
    };
    let mut defs = vec![0usize; function.classes.len()];
    let mut uses = vec![Vec::new(); function.classes.len()];
    for (b, block) in function.blocks.iter().enumerate() {
        for inst in &block.insts {
            for operand in &inst.operands {
                match operand {
                    Operand::Def(Reg::Virt(v)) => defs[*v as usize] += 1,
                    Operand::Use(Reg::Virt(v)) => uses[*v as usize].push(b),
                    _ => {}
                }
            }
        }
    }
    for b in 0..function.blocks.len() {
        let mut at = 0;
        while at < function.blocks[b].insts.len() {
            let inst = &function.blocks[b].insts[at];
            let (Kind::Op(op @ ("LO" | "HI")), [Operand::Def(Reg::Virt(byte)), Operand::Use(source)]) =
                (inst.kind, &inst.operands[..])
            else {
                at += 1;
                continue;
            };
            let Ok((lo, hi)) = expand::lanes(register(*source)) else {
                at += 1;
                continue;
            };
            let (byte, half) = (*byte, if op == "LO" { lo } else { hi });
            // a byte nothing virtual reads is read through its register by an
            // instruction that names it (a copy into `a` that allocation made
            // the identity is gone): it is still made
            if defs[byte as usize] != 1
                || uses[byte as usize].is_empty()
                || uses[byte as usize].iter().any(|one| *one != b)
            {
                at += 1;
                continue;
            }
            let insts = &function.blocks[b].insts;
            let mut last = at;
            for (k, later) in insts.iter().enumerate().skip(at + 1) {
                if later.operands.iter().any(|o| matches!(o, Operand::Use(Reg::Virt(v)) if *v == byte)) {
                    last = k;
                }
            }
            // a PACK that reads the byte writes the pair after reading both
            // halves (`pack`), so its own write to the half does
            // not clobber it: the low half of a pair kept while the
            // high half is replaced stays where it is
            let checked = if insts[last].kind == Kind::Op("PACK") { last } else { last + 1 };
            let clobbered = insts[at + 1..checked]
                .iter()
                .any(
                    |later| later.call
                        || later.operands.iter().any(|o| {
                            matches!(
                                o,
                                Operand::Def(r) if <Z80Registers as llrm_spike_mf::Target>::overlaps(register(*r), half)
                            )
                        }),
                );
            if clobbered {
                at += 1;
                continue;
            }
            // the byte's own register may be read by an instruction that names
            // it: a call's argument whose copy allocation made the identity,
            // or another value allocated to the same register (`x >> 31` of a
            // spilled long: the byte and the sign share `a`, the copy between
            // them is gone, and removing the load left `add a, a` on stale `a`)
            let own = register(Reg::Virt(byte));
            let overlaps = <Z80Registers as llrm_spike_mf::Target>::overlaps;
            let mut named = false;
            for later in &insts[at + 1..] {
                named |= later
                    .operands
                    .iter()
                    .any(|o| matches!(o, Operand::Use(r) if *r != Reg::Virt(byte) && overlaps(register(*r), own)));
                if named || later.operands.iter().any(|o| matches!(o, Operand::Def(r) if overlaps(register(*r), own))) {
                    break;
                }
            }
            if named {
                at += 1;
                continue;
            }
            for later in &mut function.blocks[b].insts[at + 1..=last] {
                for operand in &mut later.operands {
                    if matches!(operand, Operand::Use(Reg::Virt(v)) if *v == byte) {
                        *operand = Operand::Use(Reg::Phys(half));
                    }
                }
            }
            function.blocks[b].insts.remove(at);
            removed = true;
        }
    }
    removed
}

/// `function`, one instruction per line, on stderr (`LLRM_Z80_DUMP=name`).
fn dump(function: &MFunction) {
    for block in &function.blocks {
        eprintln!("bb{}:", block.id);
        for phi in &block.phis {
            eprintln!("  phi {:?} {:?}", phi.result, phi.incoming);
        }
        for inst in &block.insts {
            eprintln!(
                "  {:?} {:?}{}{}",
                inst.kind,
                inst.operands,
                if inst.sets_flags { " SF" } else { "" },
                if inst.reads_flags { " RF" } else { "" }
            );
        }
    }
}

/// The function `name` of `ir` as assembly lines.
pub fn lines(
    ir: &str,
    name: &str,
    number: usize,
    request: &Request,
) -> Result<Vec<Line>, String> {
    let (selected, alternatives) = cached_both(ir, request)?;
    lines_in(&selected, &alternatives, name, number, request)
}

/// The block frequencies of function `name`, relative to its entry.
fn frequencies_of(
    selected: &Selections,
    name: &str,
) -> Vec<(u32, f64)> {
    selected
        .iter()
        .find(|(one, ..)| one == name)
        .and_then(|(_, source, _)| source.as_ref().ok())
        .map(|selection| selection.frequencies.clone())
        .unwrap_or_default()
}

/// The lines of function `name` as [`lines`] gives them, each with the weight
/// its price counts its clocks by: the frequency of its block.
pub fn weighted_lines(
    ir: &str,
    name: &str,
    request: &Request,
) -> Result<Vec<(Line, f64)>, String> {
    let (selected, alternatives) = cached_both(ir, request)?;
    let lines = lines_in(&selected, &alternatives, name, 0, request)?;
    let frequencies = frequencies_of(&selected, name).into_iter().collect();
    Ok(expand::weighted(&lines, &frequencies))
}

/// The lines of function `name`. A function that calls `__mulsi3` for its long
/// multiplies is also built with them left as shifts and adds, and the form
/// the module's plan names stays ([`decomposes`]).
fn lines_in(
    selected: &Selections,
    alternatives: &Selections,
    name: &str,
    number: usize,
    request: &Request,
) -> Result<Vec<Line>, String> {
    if alternatives.iter().any(|(one, ..)| one == name) && decomposes(selected, alternatives, name, request) {
        return lines_of(alternatives, name, number, request);
    }
    lines_of(selected, name, number, request)
}

thread_local! {
    /// The functions of the module whose alternatives are `__mulsi3`'s
    /// (by the address of that list) that keep the shifts and adds.
    static DECOMPOSED: std::cell::RefCell<Option<(usize, std::rc::Rc<std::collections::HashSet<String>>)>> =
        const { std::cell::RefCell::new(None) };
}

/// Whether `name` keeps its long multiplies as shifts and adds. Whether the
/// call beats the copies depends on what the function keeps live across it,
/// which the selection cannot see (div_constants_long -Os: 28 calls, +177 B
/// over the copies; mul_constants the other way, -227 B), so each function is
/// built both ways and priced. The module is what is priced: its functions'
/// bytes and the runtime routines they reach, each linked once (`__mulsi3`
/// 57 B for the calls, the shift routines the copies call), so the functions
/// the call makes smaller keep it only where together they save what the
/// routines cost (switch_table_holes -Os: 40 B saved, 57 B routine).
/// DEVIATE z80-callinline-price (replaces z80-mul-function-price; see `prefers`): the pin's
/// `decomposeMulByConstant` has no such choice. Owner z80. Every runtime routine a variant reaches is already
/// priced (`runtime_size`), so no libcall is left out of the total.
fn decomposes(
    selected: &Selections,
    alternatives: &Selections,
    name: &str,
    request: &Request,
) -> bool {
    let key = std::rc::Rc::as_ptr(alternatives) as usize;
    let held = DECOMPOSED.with(|held| {
        held.borrow().as_ref().filter(|(was, _)| *was == key).map(|(_, set)| set.clone())
    });
    let set = held.unwrap_or_else(|| {
        let set = std::rc::Rc::new(decomposed_functions(selected, alternatives, request));
        DECOMPOSED.with(|held| *held.borrow_mut() = Some((key, set.clone())));
        set
    });
    set.contains(name)
}

/// The priced forms of the module's functions: those with one form, and
/// those that call `__mulsi3` with the form built with shifts and adds too
/// (bytes, runtime routines reached).
type Priced = (u32, u64, std::collections::HashSet<String>);
type Variants = (Vec<Priced>, Vec<(String, Priced, Option<Priced>)>);

fn variants(
    selected: &Selections,
    alternatives: &Selections,
    request: &Request,
) -> Variants {
    let speed = !matches!(request.level, Level::Os | Level::Oz);
    // the printed form of every function, and for those with an alternative
    // the form with the shifts
    let priced = |lines: Vec<Line>| {
        let (bytes, clocks) = expand::price_emitted(&lines, speed);
        (bytes, clocks, crate::runtime_size::referenced(&lines))
    };
    let mut fixed: Vec<Priced> = Vec::new();
    let mut either: Vec<(String, Priced, Option<Priced>)> = Vec::new();
    for (one, ..) in selected.iter() {
        let call = lines_of(selected, one, 0, request).ok().map(priced);
        let Some(call) = call else { continue };
        if !alternatives.iter().any(|(other, ..)| other == one) {
            fixed.push(call);
            continue;
        }
        let differs = {
            let (a, b) = (machine_function_with(selected, one, true), machine_function_with(alternatives, one, true));
            !matches!((a, b), (Ok(a), Ok(b)) if format!("{a:?}") == format!("{b:?}"))
        };
        let decomposed = if differs { lines_of(alternatives, one, 0, request).ok().map(priced) } else { None };
        either.push((one.clone(), call, decomposed));
    }
    (fixed, either)
}

/// The module's bytes with the functions of `decomposes` built with shifts
/// and adds: theirs and the runtime routines reached, each linked once.
fn module_price(
    (fixed, either): &Variants,
    decomposes: &std::collections::HashSet<usize>,
) -> (u32, u64) {
    use std::collections::HashSet;
    let mut reached: HashSet<String> = HashSet::new();
    let (mut bytes, mut clocks) = (0, 0);
    for (one_bytes, one_clocks, refs) in fixed {
        bytes += one_bytes;
        clocks += one_clocks;
        reached.extend(refs.iter().cloned());
    }
    for (at, (_, call, decomposed)) in either.iter().enumerate() {
        let (one_bytes, one_clocks, refs) = match decomposed {
            Some(decomposed) if decomposes.contains(&at) => decomposed,
            _ => call,
        };
        bytes += one_bytes;
        clocks += one_clocks;
        reached.extend(refs.iter().cloned());
    }
    (bytes + crate::runtime_size::reached(&reached), clocks)
}

/// The module's bytes with every function calling `__mulsi3` and with every
/// one that has the other form built with shifts and adds.
pub fn module_prices(
    ir: &str,
    request: &Request,
) -> Result<(u32, u32), String> {
    let (selected, alternatives) = cached_both(ir, request)?;
    let variants = variants(&selected, &alternatives, request);
    let every = (0..variants.1.len()).filter(|at| variants.1[*at].2.is_some()).collect();
    Ok((module_price(&variants, &std::collections::HashSet::new()).0, module_price(&variants, &every).0))
}

/// The price of function `at` alone in the form `decomposes` gives it: its bytes and clocks, and the bytes of the
/// runtime routines it reaches that no other function of the module reaches. The clocks of a function are
/// weighted by the loops inside it only, so against the whole module's clocks (dominated by a hot loop elsewhere)
/// a call that costs a hundredfold in a function the loop calls would not register.
fn own_price(
    (fixed, either): &Variants,
    decomposes: &std::collections::HashSet<usize>,
    at: usize,
) -> (u32, u64) {
    use std::collections::HashSet;
    let others: HashSet<String> = fixed
        .iter()
        .flat_map(|(.., refs)| refs.iter())
        .chain(
            either
                .iter()
                .enumerate()
                .filter(|(one, _)| *one != at)
                .flat_map(|(one, (_, call, decomposed))| match decomposed {
                    Some(decomposed) if decomposes.contains(&one) => decomposed.2.iter(),
                    _ => call.2.iter(),
                }),
        )
        .cloned()
        .collect();
    let (_, call, decomposed) = &either[at];
    let (bytes, clocks, refs) = match decomposed {
        Some(decomposed) if decomposes.contains(&at) => decomposed,
        _ => call,
    };
    let mut with = others.clone();
    with.extend(refs.iter().cloned());
    (bytes + crate::runtime_size::reached(&with) - crate::runtime_size::reached(&others), *clocks)
}

/// The functions of the module that call `__mulsi3`, built with the shifts
/// and adds where the module is smaller for it (bytes first at the size
/// levels).
fn decomposed_functions(
    selected: &Selections,
    alternatives: &Selections,
    request: &Request,
) -> std::collections::HashSet<String> {
    use std::collections::HashSet;
    let variants = variants(selected, alternatives, request);
    let either = &variants.1;
    let level = request.level;
    let total = |decomposes: &HashSet<usize>| module_price(&variants, decomposes);
    // descend from all alternatives and from none (two functions may together
    // pay for a routine neither pays for alone); a flip needs a strictly better
    // module price at the level, and the second start wins only if strictly
    // better than the first
    let every: HashSet<usize> = (0..either.len()).filter(|at| either[*at].2.is_some()).collect();
    // a function tuned for size (`optsize`/`minsize`) is priced bytes first at any level, as the pin's `ForCodeSize`
    // reads it; the others by the level
    let size_first: Vec<bool> = either
        .iter()
        .map(|(name, ..)| {
            matches!(level, Level::Os | Level::Oz)
                || selected.iter().any(|(one, selection, _)| one == name && selection.as_ref().is_ok_and(|s| s.sized))
        })
        .collect();
    // a function whose alternative is the speed routines or the shift loops (a long divide, a variable 64-bit
    // shift) is priced by the smaller relative regret at the size levels too: the size routines are a bit a trip
    // (a divide of a long 6.1k T, 4 to 6x the speed routine's), and bytes first took them where the module stayed
    // far under its bar in bytes and over it in clocks (loop_interchange, copy_runs, data_int64 -Os)
    let regret: Vec<bool> = either
        .iter()
        .map(|(name, ..)| {
            alternatives.iter().any(|(one, source, scheduled)| {
                one == name && [source, scheduled].into_iter().any(|form| form.as_ref().is_ok_and(|s| s.wide_alternative))
            })
        })
        .collect();
    let with_alternative = || (0..either.len()).filter(|at| either[*at].2.is_some());
    let all_sized = with_alternative().all(|at| size_first[at] && !regret[at]);
    let regretful = with_alternative().all(|at| size_first[at] && regret[at]);
    let descend = |start: HashSet<usize>| {
        let mut chosen = start;
        let mut best = total(&chosen);
        for _ in 0..3 {
            let mut moved = false;
            for at in 0..either.len() {
                if either[at].2.is_none() {
                    continue;
                }
                let mut other = chosen.clone();
                if !other.remove(&at) {
                    other.insert(at);
                }
                let price = total(&other);
                let better = if size_first[at] && regret[at] {
                    prefers(Level::O1, price, best)
                } else if size_first[at] {
                    prefers(Level::Os, price, best)
                } else if level == Level::O1 {
                    prefers(level, own_price(&variants, &other, at), own_price(&variants, &chosen, at))
                } else {
                    prefers(level, price, best)
                };
                if better {
                    (chosen, best, moved) = (other, price, true);
                }
            }
            if !moved {
                break;
            }
        }
        (best, chosen)
    };
    // a tie keeps the faster form: the alternative at the size levels is the
    // inline form, at the others the all-calls form
    let module_level = if all_sized {
        Level::Os
    } else if regretful {
        Level::O1
    } else {
        level
    };
    let (first, second) = if all_sized || regretful {
        (descend(every), descend(HashSet::new()))
    } else {
        (descend(HashSet::new()), descend(every))
    };
    let chosen = if prefers(module_level, second.0, first.0) { second.1 } else { first.1 };
    either
        .iter()
        .enumerate()
        .filter(|(at, (_, _, decomposed))| decomposed.is_some() && chosen.contains(at))
        .map(|(_, (one, ..))| one.clone())
        .collect()
}

/// Whether the module price `new` (bytes, clocks) is strictly better than `old` at `level`: bytes first at the
/// size levels; 1 B = 3 T at O2 and up; at O1, where no rate stands for the strict bar on both axes, the
/// smaller relative regret (a form's worse axis against the best of the two on that axis), so a form that gives
/// up a few percent of one axis for a large part of the other wins, and the reverse does not.
/// DEVIATE z80-callinline-price: the pin keys a call against inline on `optsize` alone (`ForCodeSize`) and
/// prices neither the routine's clocks nor the bytes against each other. It prices the multiply calls and the long
/// shift calls (`size_shift_call`) by the same model (`runtime_size::routine_clocks`). Owner z80, exit 2026-11-20.
fn prefers(
    level: Level,
    new: (u32, u64),
    old: (u32, u64),
) -> bool {
    match level {
        Level::Os | Level::Oz => new < old,
        Level::O1 => {
            let regret = |one: (u32, u64), other: (u32, u64)| {
                let (bytes, clocks) = (f64::from(one.0) / f64::from(one.0.min(other.0).max(1)), one.1 as f64 / other.1.min(one.1).max(1) as f64);
                bytes.max(clocks)
            };
            regret(new, old) < regret(old, new)
        }
        _ => cheaper(level, new, old),
    }
}

/// The function `name` allocated, in the cheaper of its two carry forms where it has both (`Selection::generic`,
/// DEVIATE Z80-CARRY-PRICE): the levels strict on both axes (O1, Os, Oz) take the generic add only where it is no
/// worse on either, the others the cheaper at 1 B = 3 T.
fn lines_of(
    selected: &Selections,
    name: &str,
    number: usize,
    request: &Request,
) -> Result<Vec<Line>, String> {
    let carry = lines_of_form(selected, name, number, request);
    let has_generic = selected.iter().any(|(one, source, scheduled)| {
        one == name && [source, scheduled].into_iter().any(|form| form.as_ref().is_ok_and(|s| s.generic.is_some()))
    });
    if !has_generic {
        return carry;
    }
    crate::dagisel::pipeline::GENERIC_FORM.with(|one| one.set(true));
    let generic = lines_of_form(selected, name, number, request);
    crate::dagisel::pipeline::GENERIC_FORM.with(|one| one.set(false));
    match (carry, generic) {
        (Ok(carry), Ok(generic)) => {
            let price = |lines: &[Line]| expand::price_emitted(lines, !matches!(request.level, Level::Os | Level::Oz));
            let (with, without) = (price(&carry), price(&generic));
            let strict = matches!(request.level, Level::O1 | Level::Os | Level::Oz);
            let take = if strict {
                without != with && without.0 <= with.0 && without.1 <= with.1
            } else {
                cheaper(request.level, without, with)
            };
            Ok(if take { generic } else { carry })
        }
        (Ok(one), Err(_)) | (Err(_), Ok(one)) => Ok(one),
        (Err(why), Err(_)) => Err(why),
    }
}

fn lines_of_form(
    selected: &Selections,
    name: &str,
    number: usize,
    request: &Request,
) -> Result<Vec<Line>, String> {
    let compile = |duplicate: bool| -> Result<Vec<Line>, String> {
        let (function, assignment, slots, reads, _) = allocated_with(selected, name, request, duplicate)?;
        finish(&function, &assignment, number, &slots, reads)
    };
    // Copying a return block into its predecessors saves a `jp` and often a
    // move, or costs a copy of the block: the smaller result, then the faster,
    // is kept.
    let scheduled = || {
        let copied = compile(true);
        // Where copying changes nothing (no small return block with an
        // unconditional predecessor) the two are one function: allocated once.
        let alike = || {
            let (one, other) =
                (machine_function_with(selected, name, true), machine_function_with(selected, name, false));
            matches!(
                (one, other),
                (Ok(one), Ok(other)) if format!("{one:?}") == format!("{other:?}")
            )
        };
        let kept = if copied.is_ok() && alike() { copied.clone() } else { compile(false) };
        match (copied, kept) {
            (Ok(one), Ok(two)) => Ok(if expand::price(&one) <= expand::price(&two) { one } else { two }),
            (Ok(one), Err(_)) | (Err(_), Ok(one)) => Ok(one),
            (Err(why), Err(_)) => Err(why),
        }
    };
    // The function is scheduled by `Source` and by `ILP` (DEVIATE Z80S1, in
    // `dagisel/pipeline.rs`); `ILP` replaces `Source` only where it is cheaper.
    let source = scheduled();
    crate::dagisel::pipeline::ILP.with(|one| one.set(true));
    let ilp = scheduled();
    crate::dagisel::pipeline::ILP.with(|one| one.set(false));
    match (source, ilp) {
        (Ok(source), Ok(ilp)) => {
            let price = |lines: &[Line]| expand::price_emitted(lines, !matches!(request.level, Level::Os | Level::Oz));
            Ok(if cheaper(request.level, price(&ilp), price(&source)) { ilp } else { source })
        }
        (Ok(one), Err(_)) | (Err(_), Ok(one)) => Ok(one),
        (Err(why), Err(_)) => Err(why),
    }
    .map(|lines| {
        let speed = !matches!(request.level, Level::Os | Level::Oz);
        let lines = if request.sp_frame { crate::sp_frame::without_ix(lines, speed) } else { lines };
        expand::relax(lines, speed)
    })
}

/// The function as text: its label, then one line per instruction.
pub fn text(
    ir: &str,
    name: &str,
    number: usize,
    request: &Request,
) -> Result<String, String> {
    let request = &effective(ir, request)?;
    let source = request.debug.then(|| crate::driver::source_of(ir, name)).flatten();
    let (selected, alternatives) = cached_both(ir, request)?;
    text_of(source, &selected, &alternatives, name, number, request)
}

/// [`text`] of a function of a [`Prepared`] module.
pub fn text_in(
    prepared: &Prepared,
    name: &str,
    number: usize,
    request: &Request,
) -> Result<String, String> {
    let source = prepared.sources.get(name).cloned();
    let request = &Request { level: prepared.level, ..request.clone() };
    text_of(source, &prepared.selections, &prepared.alternatives, name, number, request)
}

fn text_of(
    source: Option<(String, Option<u32>)>,
    selected: &Selections,
    alternatives: &Selections,
    name: &str,
    number: usize,
    request: &Request,
) -> Result<String, String> {
    let file = source.as_ref().map(|(file, _)| file.clone());
    let mut out = String::new();
    let mut rows = 0;
    let symbol = |out: &mut String, text: String| out.push_str(&format!("\t{text}\t= .\n\t.globl\t{text}\n"));
    // The function, and the line it starts at, stand ahead of its label.
    if let Some((file, line)) = &source {
        symbol(&mut out, format!("G${name}$0$0"));
        if let Some(line) = line {
            symbol(&mut out, format!("C${file}${line}$0_0${number}_0"));
        }
    }
    // an `internal` function is the module's own: `name:` is local to it, so
    // two modules may each define a helper of that name (`static inline`
    // `sf_clz64` in the f64 mul and div clashed at link)
    let local = selected
        .iter()
        .find(|(one, ..)| one == name)
        .is_some_and(|(_, source, _)| source.as_ref().is_ok_and(|s| s.local));
    out.push_str(&format!("{}{}\n", crate::data::symbol(name), if local { ":" } else { "::" }));
    let at = std::rc::Rc::as_ptr(selected) as usize;
    let kept = KEPT_LINES.with(|kept| kept.borrow_mut().remove(&(at, name.to_owned(), number)));
    let lines = match kept {
        Some(lines) => lines,
        None => lines_in(selected, alternatives, name, number, request)?,
    };
    for line in lines {
        match line {
            Line::Label(label) => out.push_str(&format!("{label}:\n")),
            Line::Inst(text) => out.push_str(&format!("\t{text}\n")),
            // SDCC's names for a line (`C$file$line$block_level$n`): a symbol
            // at the address, which sdld puts in the `.cdb`.
            Line::Loc(line) => {
                if let Some(file) = &file {
                    rows += 1;
                    symbol(&mut out, format!("C${file}${line}$0_0${number}_{rows}"));
                }
            }
        }
    }
    Ok(out)
}

/// The conditional jump for a `Z80::CondCode` (`NZ` 0, `Z` 1, `NC` 2, `C` 3).
fn branch_for(cond: i64) -> Option<&'static str> {
    Some(match cond {
        0 => "JPNZ",
        1 => "JPZ",
        2 => "JPNC",
        3 => "JPC",
        _ => return None,
    })
}

fn is_cmov(inst: &Inst) -> bool {
    matches!(inst.kind, Kind::Op("CMOV8" | "CMOV16"))
}

/// A `CMOV`'s result, false value, true value and condition.
fn cmov_parts(inst: &Inst) -> Result<(Reg, Reg, Reg, i64), String> {
    let [Operand::Def(result), Operand::Use(on_false), Operand::Use(on_true), Operand::Imm(cond), Operand::Use(_)] =
        inst.operands[..]
    else {
        return Err("a CMOV with unexpected operands".to_owned());
    };
    Ok((result, on_false, on_true, cond))
}

/// `CodeGenPrepare::sinkSelectOperand`: the instructions that only make one
/// side of the `CMOV` at `p` move out of the block into the arm that side
/// takes, so the other path does not run them. They come back in order, taken
/// out of `insts`; none when nothing but copies would move or a hazard shows.
///
/// DEVIATE: the pin sinks one-use IR operands that cost at least
/// `TCC_Expensive` before selection; this works on the selected instructions,
/// on any side that holds a non-copy. Reason: the select is expanded here, not
/// in CodeGenPrepare. Exit: the expansion at selection.
fn sunk_operand(
    function: &MFunction,
    block: usize,
    p: usize,
    value: Reg,
) -> Option<Vec<usize>> {
    let insts = &function.blocks[block].insts[..p];
    let mut uses: llrm_support::hash::HashMap<Reg, usize> = llrm_support::hash::HashMap::default();
    let mut defs: llrm_support::hash::HashMap<Reg, usize> = llrm_support::hash::HashMap::default();
    for one in &function.blocks {
        for inst in &one.insts {
            for operand in &inst.operands {
                match operand {
                    Operand::Use(reg @ Reg::Virt(_)) => *uses.entry(*reg).or_default() += 1,
                    Operand::Def(reg @ Reg::Virt(_)) => *defs.entry(*reg).or_default() += 1,
                    _ => {}
                }
            }
        }
    }
    let movable = |inst: &Inst| {
        !(inst.call
            || inst.terminator
            || inst.barrier
            || inst.reads_flags
            || inst.side_effects
            || inst.may_load
            || inst.may_store)
    };
    let touches = |inst: &Inst, reg: Reg| {
        inst.operands.iter().any(|o| matches!(o, Operand::Def(r) | Operand::Use(r) if *r == reg))
    };
    let cmov = &function.blocks[block].insts[p];
    let reads =
        |inst: &Inst, reg: Reg| inst.operands.iter().filter(|o| matches!(o, Operand::Use(r) if *r == reg)).count();
    // From the last instruction back, so every reader of a result is in the
    // slice before its maker is looked at. A value made here that anything
    // else reads is an input, not part of the slice; a machine register a
    // member reads must come from a member.
    let mut slice = std::collections::BTreeSet::new();
    let mut pending = vec![(value, insts.len(), false)];
    loop {
        let resolved: Vec<(usize, Reg, bool)> = pending
            .iter()
            .filter_map(|&(reg, before, must)| {
                insts[..before]
                    .iter()
                    .rposition(|inst| inst.operands.iter().any(|o| matches!(o, Operand::Def(r) if *r == reg)))
                    .map(|at| (at, reg, must))
                    .or(None)
            })
            .collect();
        if let Some(&(_, Reg::Phys(_), true)) = pending
            .iter()
            .map(|&(r, b, m)| (b, r, m))
            .find(|&(b, r, m)| {
                m && !insts[..b]
                    .iter()
                    .any(|inst| inst.operands.iter().any(|o| matches!(o, Operand::Def(x) if *x == r)))
            })
            .as_ref()
        {
            return None;
        }
        let Some(&(at, ..)) = resolved.iter().max_by_key(|one| one.0) else { break };
        let must = resolved.iter().any(|&(one, _, must)| one == at && must);
        pending.retain(|&(reg, before, _)| {
            !insts[..before]
                .iter()
                .rposition(|inst| inst.operands.iter().any(|o| matches!(o, Operand::Def(r) if *r == reg)))
                .is_some_and(|one| one == at)
        });
        if slice.contains(&at) {
            continue;
        }
        let inst = &insts[at];
        let alone = inst
            .operands
            .iter()
            .all(
                |operand| {
                    let Operand::Def(reg @ Reg::Virt(_)) = operand else { return true };
                    let inside: usize = slice.iter().map(|&one| reads(&insts[one], *reg)).sum();
                    let by_cmov = if *reg == value { reads(cmov, *reg) } else { 0 };
                    defs.get(reg) == Some(&1) && uses.get(reg).copied().unwrap_or(0) == inside + by_cmov
                },
            );
        if !movable(inst) || !alone {
            if must {
                return None;
            }
            continue;
        }
        slice.insert(at);
        for operand in &inst.operands {
            match operand {
                Operand::Use(reg @ Reg::Virt(_)) => pending.push((*reg, at, false)),
                Operand::Use(reg) if *reg != Reg::Phys(crate::reg::FLAGS) => pending.push((*reg, at, true)),
                _ => {}
            }
        }
    }
    if !slice.iter().any(|&at| matches!(insts[at].kind, Kind::Op(_))) {
        return None;
    }
    // a register of the machine the slice touches is touched by nothing left
    // between its first instruction and the `CMOV`
    let first = *slice.iter().next()?;
    for &at in &slice {
        for operand in &insts[at].operands {
            let (Operand::Def(reg @ Reg::Phys(_)) | Operand::Use(reg @ Reg::Phys(_))) = operand else { continue };
            if (first..insts.len()).any(|other| !slice.contains(&other) && touches(&insts[other], *reg)) {
                return None;
            }
        }
    }
    // the flags the branch reads are not made by the slice, and a flags reader
    // after the diamond sees what the slice leaves only if the slice leaves
    // none
    let flags = Reg::Phys(crate::reg::FLAGS);
    // an instruction that sets the flags may not name them (`XOR16i`)
    let sets = |inst: &Inst| inst.sets_flags || touches(inst, flags);
    let makes_flags = slice.iter().any(|&at| sets(&insts[at]));
    if makes_flags {
        if insts.iter().rposition(sets).is_some_and(|last| slice.contains(&last)) {
            return None;
        }
        let tail = &function.blocks[block].insts[p + 1..];
        let wanted = tail.iter().find(|inst| inst.reads_flags || sets(inst));
        if wanted.is_some_and(|inst| inst.reads_flags) {
            return None;
        }
    }
    Some(slice.into_iter().collect())
}

/// `EmitLoweredSelect`: a run of `CMOV` pseudos on one condition (or its
/// opposite) becomes a diamond. The block jumps to the join on the condition;
/// the empty block it falls through to is the other path; the join holds a phi
/// per `CMOV` and what followed the run. A later `CMOV` that reads an earlier
/// one's result is renamed to the value that result has on each path.
///
/// The arms are empty, and `ld` leaves the flags alone, so a copy the phis
/// become does not disturb a later reader of the compare. A lone `CMOV` whose
/// side is made by instructions nothing else reads has them moved into the
/// arm that side takes (`sunk_operand`).
///
/// DEVIATE: only `CMOV`s that follow one another are one diamond, and the
/// cascaded case is not translated. Reason: nothing selected it yet. Exit: when
/// a kernel needs it.
fn expand_cmov(function: &mut MFunction) -> Result<(), String> {
    let mut next_id = function.fresh_block_id();
    let mut at = 0;
    while at < function.blocks.len() {
        let Some(p) = function.blocks[at].insts.iter().position(is_cmov) else {
            at += 1;
            continue;
        };
        let alone = !function.blocks[at].insts.get(p + 1).is_some_and(is_cmov);
        let (_, on_false, on_true, first) = cmov_parts(&function.blocks[at].insts[p])?;
        // the side made in the block alone moves; the true side takes the
        // inverted jump to reach its arm
        let sunk = if alone {
            sunk_operand(function, at, p, on_false)
                .map(|slice| (false, slice))
                .or_else(|| sunk_operand(function, at, p, on_true).map(|slice| (true, slice)))
        } else {
            None
        };
        let (flip, moved) = match sunk {
            Some((flip, slice)) => {
                let mut taken = Vec::new();
                for &index in slice.iter().rev() {
                    taken.push(function.blocks[at].insts.remove(index));
                }
                taken.reverse();
                (flip, taken)
            }
            None => (false, Vec::new()),
        };
        let p = function.blocks[at].insts.iter().position(is_cmov).ok_or("the CMOV vanished")?;
        let mut tail = function.blocks[at].insts.split_off(p);
        let first = if flip { first ^ 1 } else { first };
        let same =
            |inst: &Inst| is_cmov(inst) && cmov_parts(inst).is_ok_and(|(.., cond)| cond == first || cond == first ^ 1);
        let taken = tail.iter().take_while(|inst| same(inst)).count();
        let cmovs: Vec<Inst> = tail.drain(..taken).collect();
        let branch = branch_for(first).ok_or_else(|| format!("a CMOV on condition {first}"))?;
        let mut head_value = llrm_support::hash::HashMap::default();
        let mut other_value = llrm_support::hash::HashMap::default();
        let mut phis = Vec::new();
        for cmov in &cmovs {
            let (result, on_false, on_true, cond) = cmov_parts(cmov)?;
            let (when_true, when_false) = if cond == first { (on_true, on_false) } else { (on_false, on_true) };
            let on_head = head_value.get(&when_true).copied().unwrap_or(when_true);
            let on_other = other_value.get(&when_false).copied().unwrap_or(when_false);
            head_value.insert(result, on_head);
            other_value.insert(result, on_other);
            phis.push(Phi { result, incoming: vec![(at, on_head), (at + 1, on_other)] });
        }
        let (otherwise, join) = (next_id, next_id + 1);
        next_id += 2;
        let mut jump =
            Inst::new(Kind::Op(branch), vec![Operand::Block(join), Operand::Use(Reg::Phys(crate::reg::FLAGS))]);
        jump.terminator = true;
        jump.reads_flags = true;
        function.blocks[at].insts.push(jump);
        function.blocks.insert(at + 1, Block { id: otherwise, insts: moved, phis: Vec::new() });
        function.blocks.insert(at + 2, Block { id: join, insts: tail, phis });
        // the join is where control leaves the original block, and every block
        // after it moved down by two
        for (index, block) in function.blocks.iter_mut().enumerate() {
            if index == at + 2 {
                continue;
            }
            for phi in &mut block.phis {
                for (pred, _) in &mut phi.incoming {
                    if *pred == at {
                        *pred = at + 2;
                    } else if *pred > at {
                        *pred += 2;
                    }
                }
            }
        }
        at += 1;
    }
    Ok(())
}

/// The most instructions, `ret` included, of a block that is copied into its
/// predecessors.
const TAIL_DUPLICATION_SIZE: usize = 4;

/// `TailDuplicator`: a small block that ends in `ret` is copied into each
/// predecessor that reaches it without a conditional branch, so `jp` (three
/// bytes) and the phi copy ahead of it become a `ret`. A predecessor that
/// branches to it conditionally keeps the original, with that edge's phi
/// inputs.
pub fn tail_duplicate_returns(function: &mut MFunction) {
    let mut j = 1;
    while j < function.blocks.len() {
        let block = &function.blocks[j];
        let small = block.insts.len() <= TAIL_DUPLICATION_SIZE
            && matches!(
                block.insts.last().map(|inst| inst.kind),
                Some(Kind::Op("RET"))
            )
            && block.insts.iter().filter(|inst| inst.terminator).count() == 1;
        if !small {
            j += 1;
            continue;
        }
        let id = block.id;
        let preds: Vec<usize> = (0..function.blocks.len())
            .filter(|&p| llrm_spike_mf::regalloc::successors(function)[p].contains(&j))
            .collect();
        let reaches_unconditionally = |function: &MFunction, p: usize| {
            let last = function.blocks[p].insts.last();
            match last {
                // a conditional branch ahead of it to the same block keeps the
                // original alive
                Some(inst) if inst.barrier => {
                    let insts = &function.blocks[p].insts;
                    matches!(inst.kind, Kind::Op("JP"))
                        && inst.operands == [Operand::Block(id)]
                        && !insts[..insts.len() - 1].iter().any(|one| one.operands.contains(&Operand::Block(id)))
                }
                // falls through: no branch at all, or a conditional one that
                // goes elsewhere
                last => {
                    p + 1 == j
                        && last.is_none_or(|inst| !inst.terminator || !inst.operands.contains(&Operand::Block(id)))
                }
            }
        };
        let copy_into: Vec<usize> = preds.iter().copied().filter(|&p| reaches_unconditionally(function, p)).collect();
        if copy_into.is_empty() {
            j += 1;
            continue;
        }
        let tail = function.blocks[j].insts.clone();
        // highest first: a split below a predecessor moves only the blocks after it
        for &p in copy_into.iter().rev() {
            // the phi inputs for this edge, as copies
            let mut moves = Vec::new();
            for phi in &mut function.blocks[j].phis {
                if let Some(at) = phi.incoming.iter().position(|(pred, _)| *pred == p) {
                    let (_, input) = phi.incoming.remove(at);
                    moves.push(Inst::copy(phi.result, input));
                }
            }
            let insts = &mut function.blocks[p].insts;
            if insts.last().is_some_and(|inst| inst.barrier) {
                insts.pop();
            }
            // a conditional branch elsewhere ahead of the copy ends the block (the pin's `analyzeBranch` reads
            // only the terminators at its end): the copy is the block it falls into
            let split = insts.iter().any(|inst| inst.terminator);
            if split {
                let id = function.blocks.iter().map(|block| block.id).max().map_or(0, |id| id + 1);
                function.blocks.insert(p + 1, Block { id, insts: Vec::new(), phis: Vec::new() });
                for block in &mut function.blocks {
                    for phi in &mut block.phis {
                        for (pred, _) in &mut phi.incoming {
                            if *pred > p {
                                *pred += 1;
                            }
                        }
                    }
                }
                if j > p {
                    j += 1;
                }
            }
            let insts = &mut function.blocks[if split { p + 1 } else { p }].insts;
            insts.extend(moves);
            insts.extend(tail.iter().cloned());
        }
        // the original goes when nothing branches to it now
        let others = preds.len() - copy_into.len();
        if others == 0 {
            function.blocks.remove(j);
            for block in &mut function.blocks {
                for phi in &mut block.phis {
                    for (pred, _) in &mut phi.incoming {
                        if *pred > j {
                            *pred -= 1;
                        }
                    }
                }
            }
        } else {
            j += 1;
        }
    }
}

/// `jp cc, T` over a bare `ret` that falls into `T` is `ret !cc`: the branch
/// returns on the opposite condition and the code after it is `T`.
fn return_conditionally(function: &mut MFunction) {
    use llrm_spike_mf::Jump;
    let mut i = 0;
    while i + 2 < function.blocks.len() {
        let (here, bare, after) = (&function.blocks[i], &function.blocks[i + 1], &function.blocks[i + 2]);
        let branch = here.insts.last().filter(|inst| inst.terminator && !inst.barrier);
        let to_after = branch.is_some_and(|inst| inst.operands.first() == Some(&Operand::Block(after.id)));
        let bare_ret = matches!(&bare.insts[..], [one] if one.kind == Kind::Op("RET")) && bare.phis.is_empty();
        let referenced = function
            .blocks
            .iter()
            .flat_map(|block| block.insts.iter())
            .any(|inst| inst.terminator && inst.operands.contains(&Operand::Block(bare.id)));
        if to_after && bare_ret && !referenced {
            let reversed = Z80Registers::reverse(branch.expect("a branch"));
            if let Some(Inst { kind: Kind::Op(name), .. }) = reversed {
                let returning = match name {
                    "JPNZ" => "RETNZ",
                    "JPZ" => "RETZ",
                    "JPNC" => "RETNC",
                    _ => "RETC",
                };
                let last = function.blocks[i].insts.len() - 1;
                let mut inst = Inst::new(Kind::Op(returning), vec![]);
                inst.reads_flags = true;
                function.blocks[i].insts[last] = inst;
                function.blocks.remove(i + 1);
                for block in &mut function.blocks {
                    for phi in &mut block.phis {
                        for (pred, _) in &mut phi.incoming {
                            if *pred > i + 1 {
                                *pred -= 1;
                            }
                        }
                    }
                }
                continue;
            }
        }
        i += 1;
    }
}

/// Whether `inst` leaves the Z flag set by its 8-bit result.
fn sets_zero_by_result(inst: &Inst) -> bool {
    matches!(
        inst.kind,
        Kind::Op(
            "INC8"
                | "DEC8"
                | "SHL8"
                | "SRL8"
                | "SRA8"
                | "ADD8r"
                | "ADD8i"
                | "SUB8r"
                | "SUB8i"
                | "AND8r"
                | "AND8i"
                | "OR8r"
                | "OR8i"
                | "XOR8r"
                | "XOR8i"
        )
    )
}

/// `optimizeCompareInstr`: the test of a byte against zero (`cp 0`) after the
/// instruction that made the byte and set the flags by it is dropped when only
/// the zero flag is read: `dec c; ld a, c; or a; jp nz` is `dec c; jp nz`.
/// Every `ld` between leaves the flags alone.
fn eliminate_tests(function: &mut MFunction) {
    for block in &mut function.blocks {
        let mut at = 1;
        while at < block.insts.len() {
            let compare = &block.insts[at];
            let is_test = compare.kind == Kind::Op("CP8i")
                && matches!(
                    compare.operands[..],
                    [_, Operand::Use(Reg::Phys(a)), Operand::Imm(0)] if a == crate::reg::A
                );
            let copy = &block.insts[at - 1];
            let tested = match (&copy.kind, &copy.operands[..]) {
                (Kind::Copy, [Operand::Def(Reg::Phys(a)), Operand::Use(v)]) if *a == crate::reg::A => Some(*v),
                _ => None,
            };
            let (true, Some(value)) = (is_test, tested) else {
                at += 1;
                continue;
            };
            // the instruction that made `value`: it defines it, or copies it
            // out of `a`
            let defines = |inst: &Inst, reg: Reg| inst.operands.contains(&Operand::Def(reg));
            let Some(made) = block.insts[..at - 1].iter().rposition(|inst| defines(inst, value)) else {
                at += 1;
                continue;
            };
            let setter = if block.insts[made].kind == Kind::Copy {
                // `v = COPY a` after an instruction that defines `a` as its
                // result
                made.checked_sub(1).filter(|&before| defines(&block.insts[before], Reg::Phys(crate::reg::A)))
            } else {
                Some(made)
            };
            let Some(setter) = setter.filter(|&one| sets_zero_by_result(&block.insts[one])) else {
                at += 1;
                continue;
            };
            // `optimizeCompareInstr` stops at an instruction between that sets the
            // flags. `add hl, rr` keeps Z, but every flag scan after this
            // (`eliminate_dead`, the expander's `flags_dead`) reads it as a full
            // def, so a zero kept across it was deleted with its setter.
            let flags_kept = block.insts[setter + 1..at - 1].iter().all(|inst| !inst.call && !inst.sets_flags);
            let readers = block.insts[at + 1..].iter().take_while(|inst| !inst.sets_flags);
            let only_zero = readers
                .filter(|inst| inst.reads_flags)
                .all(|inst| matches!(inst.kind, Kind::Op("JPZ" | "JPNZ" | "RETZ" | "RETNZ")));
            if flags_kept && only_zero {
                block.insts.drain(at - 1..=at);
                at -= 1;
            } else {
                at += 1;
            }
        }
    }
}
