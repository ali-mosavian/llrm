//! Loop idiom recognition and store merging: a counted loop that fills or
//! copies memory becomes one `llvm.memset`, `llvm.memcpy` or `llvm.memmove`
//! (LLVM's LoopIdiomRecognize; gcc's loop distribution patterns), and
//! straight-line adjacent stores become one memset or wider stores (LLVM's
//! MemCpyOpt `tryMergingIntoMemset`, MergeConsecutiveStores; gcc's store
//! merging).
//!
//! # How it works
//! `LoopIdioms` rewrites one loop at a time, then looks again, with two shapes
//! of loop: tested before its trips, or tested after them (a loop entered
//! behind a copy of its test, `-ftree-ch`). The loop has one latch, a
//! straight-line body, and a trip count `induction::counted` proves; what it
//! does besides arithmetic is stores, loads and memsets that follow one of
//! these forms:
//!
//! - A fill: a store, or a memset, of a loop-invariant value that is one byte
//!   repeated, through an address `induction` derives as a GEP off an invariant
//!   pointer, striding by the element's size. A cell whose bytes do not repeat
//!   is `llvm.experimental.memset.pattern` (DEVIATE: the pin forms none off
//!   Darwin,
//!   [L1214](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L1214);
//!   here it is priced from the target's table, so a word fill is `rep stosw`
//!   where its clocks beat the loop's at speed, and its bytes at `-Os`; m16
//!   `fillw` of 100 words is 407 clocks against the loop's 500, break-even
//!   near 16 trips with the segment setup; owner w-5). A loop with several
//!   fills to bytes no other reaches is a memset each; fills that tile adjacent
//!   bytes with the same byte are one.
//! - A copy: a load and a store of one cell each trip, the stored value being
//!   the loaded one, from one pointer to another with the same step. Where the
//!   ranges are apart it is `llvm.memcpy`; where they overlap but each trip
//!   reads what the earlier trips left untouched (writes trail the reads going
//!   up, or going down) it is `llvm.memmove`, tagged with its direction. Any
//!   other overlap is a smear and stays.
//!
//! The count in bytes must not wrap the index: a byte's trips never do, and
//! wider cells need `inbounds` GEPs or the proof's `maximum`. The counter must
//! not be wider than the index. A huge pointer, which carries into its
//! selector, is taken like any other: the intrinsic carries its address space,
//! and instruction selection splits the fill at each 64K segment
//! (`isel/huge.rs`; Owner: clib-huge-fill-2256). A volatile access, a call, or
//! a value the loop changes keeps the loop.
//!
//! A loop with one fill of constant bytes is rewritten always. A pattern, a
//! copy or a loop of several fills is priced (`_pays`, `_cheaper_by`) against
//! what the loop costs for its trips (the proven count, or the profit model's
//! estimate): the target's string-instruction setup and per-cell cost against
//! the loop's operations, and a few cells are left as stores or loads and
//! stores. At -Os the comparison is in code bytes of the loop against the
//! call's.
//!
//! ```text
//!   header: %i = phi [0,pre],[%n,body]      pre:   %cnt = trips (or n)
//!           br (%i < n), body, exit                call memset(&a[0], 0, %cnt)
//!   body:   store i8 0, &a[%i]       ==>     header: (one trip kept)
//!           %n = add %i, 1                   exit:   phis read the counters'
//!           br header                                exit values
//! ```
//!
//! The loop is kept as one trip (the latch goes to the exit, the header's phis
//! are replaced by their starts), guarded by the header's test unless the count
//! is proven positive, and its counters leave with their exit values computed
//! in the preheader (`counting::Seeds`). Later passes delete what is left. The
//! memset is declared in the module where it has none, through the pass
//! manager's `Declared`.
//!
//! `Merge` is a separate pass over straight-line code in each block. A run of
//! at least two non-volatile stores of one repeated byte to adjacent bytes of
//! one object (no other memory access between them) is one memset placed where
//! the last stood, if profitable (LLVM's `isProfitableToUseMemset`: four stores
//! or 16 bytes, or fewer only where the memset needs fewer widest-integer
//! stores). The other stores, of constants and of loaded values, to adjacent
//! cells are wider ones (`mergestores`). `Merge` runs
//! after the scalar passes have settled: no pass asks a memset what a cell
//! holds, so merging sooner would lose a store's value to the forwarding after
//! it.
//!
//! # Example
//! Before:
//!
//! ```mir
//! b1:
//!   %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
//!   %c = icmp slt i16 %i, 10
//!   br i1 %c, label %b2, label %b3
//!
//! b2:
//!   %p = getelementptr inbounds [64 x i16], ptr @buf, i16 0, i16 %i
//!   store i16 0, ptr %p
//!   %next = add i16 %i, 1
//!   br label %b1
//!
//! b3:
//!   %e = phi i16 [ %i, %b1 ]
//!   ; ...
//! ```
//!
//! After:
//!
//! ```mir
//! b1:
//!   %c = icmp slt i16 0, 10
//!   br label %b2
//!
//! b2:
//!   %p = getelementptr inbounds [64 x i16], ptr @buf, i16 0, i16 0
//!   %0 = add i16 0, 10
//!   call void @llvm.memset.p0.i16(ptr %p, i8 0, i16 20, i1 false)
//!   %next = add i16 0, 1
//!   br label %b3
//!
//! b3:
//!   %e = phi i16 [ %0, %b2 ]
//!   ; ...
//! ```
//!
//! Ten word stores are one memset of 20 bytes; the loop stays as one trip and
//! the counter leaves with its exit value 10.
//!
//! From unit test `a_counted_word_loop_is_one_memset_of_its_bytes` in
//! `loopidioms_tests.rs` (first case; the printed text was taken from a run).
//!
//! # Runs
//! Last in the function fixed point: `LoopIdioms`, then `Merge`, after the
//! unroller. On with `-ftree-loop-distribute-patterns`; off at -O0 and -Og,
//! on from -O1. `size` is set at -Os and -Oz.
//!
//! # Needs
//! `induction` (`counted`, `trips`, `recurrences`, `derived` addresses),
//! `consts` facts through `Standing`, aliasing (`MemRef`) and the target's
//! costs (`profit`, `OperationCosts`).
//!
//! # Keeps
//! Blocks and edges change (a loop becomes one trip): every analysis is
//! invalidated when anything was rewritten.
//!
//! # See also
//! [`crate::counting`] builds the count and exit values;
//! [`crate::loopunrollpass`], [`crate::looppeel`] precede it.

use std::collections::BTreeSet;

use llrm_analysis::cfg;
use llrm_analysis::graph::loops::Loop;
use llrm_analysis::induction::{self, AffineOperand, CountedLoop};
use llrm_analysis::memory::{MemRef, Unit};
use llrm_analysis::num::Num as BigInt;
use llrm_mir::context::{Constant, ConstantKind, Context, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::memory::{self, Callees};
use llrm_mir::module::{BlockId, Function, InstId, MetadataNode, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, CallInfo, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, Declared, FunctionPass, Outer, PreservedAnalyses};
use llrm_mir::types::{Type, TypeId};
use llrm_support::hash::IndexMap;
use num_traits::ToPrimitive;

use crate::counting::{self, Seeds};
use crate::edges;
use crate::lcssa::{arms, from_arms, operations};
use crate::profit::{self, OperationCosts};

/// `size`: priced in code bytes, as under `-Os`.
pub struct LoopIdioms {
    pub size: bool,
}

impl FunctionPass for LoopIdioms {
    fn name(&self) -> &'static str {
        "loopidioms"
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let registers = analyses.get::<llrm_analysis::manager::Registers>(unit.context, unit.layout, unit.function);
        let changed = filled_with(
            unit.context,
            unit.layout,
            analyses.outer().callees(),
            unit.function,
            analyses.outer(),
            unit.declared,
            self.size,
            &mut llrm_analysis::memory::Standing::held(&registers),
            &|_| true,
        );
        if changed { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// The straight-line fills, once the scalar passes have settled: no pass
/// asks a memset what a cell holds, so merged sooner a store's value is
/// lost to the forwarding after it. LLVM merges stores in codegen, too.
/// `size`: not priced in bytes where a register already holds the value, so
/// that a pair of constants is not made one dword store under `-Os`.
pub struct Merge {
    pub size: bool,
}

impl FunctionPass for Merge {
    fn name(&self) -> &'static str {
        "merge"
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let memset =
            crate::memcpyoptimizer::run_impl(unit.context, unit.layout, unit.function, analyses.outer(), unit.declared);
        let bytes = analyses.outer().target().size_costs();
        let copies = !llrm_support::env_flag!("LLRM_NO_MERGE_COPIES")
            && crate::mergestores::merged_copies(
                unit.context,
                unit.layout,
                analyses.outer().callees(),
                unit.function,
                analyses.outer(),
                self.size.then(|| (&mut *unit.declared, &bytes)),
            );
        let constants = crate::mergestores::merged_constants(
            unit.context,
            unit.layout,
            analyses.outer().callees(),
            unit.function,
            analyses.outer(),
            self.size,
        );
        if constants | memset | copies { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// The byte `value`, `width` bits of one byte repeated, is made of.
fn _repeated(
    unit: &Unit,
    value: Operand,
    width: u32,
) -> Option<u128> {
    if width % 8 != 0 || width == 0 {
        return None;
    }
    let bits = unit.int_constant(value)?;
    let byte = bits & 0xFF;
    (0..width / 8).all(|at| (bits >> (8 * at)) & 0xFF == byte).then_some(byte)
}

/// `function` with every such loop's body made one fill; whether any was.
pub fn filled(
    context: &mut Context,
    layout: &DataLayout,
    callees: &Callees,
    function: &mut Function,
    outer: &Outer,
    declared: &mut Declared,
    size: bool,
) -> bool {
    filled_with(
        context,
        layout,
        callees,
        function,
        outer,
        declared,
        size,
        &mut llrm_analysis::memory::Standing::underived(),
        &|_| true,
    )
}

/// `filled`, what is known of the body without memory given as `standing` says:
/// derived once for each state of the body, not once for each loop, and of the
/// loops `wanted` names.
#[allow(clippy::too_many_arguments)]
pub fn filled_with(
    context: &mut Context,
    layout: &DataLayout,
    callees: &Callees,
    function: &mut Function,
    outer: &Outer,
    declared: &mut Declared,
    size: bool,
    standing: &mut llrm_analysis::memory::Standing,
    wanted: &dyn Fn(&Loop) -> bool,
) -> bool {
    // `"no-builtins"` turns every library function off in the pin's TLI.
    if function.attrs.iter().any(|attr| {
        matches!(
            attr,
            llrm_mir::opcode::Attribute::Str(key, _) if key == "no-builtins"
        )
    }) {
        return false;
    }
    let costs = if size { outer.target().size_costs() } else { outer.target().costs() };
    let bytes = outer.target().size_costs();
    let mut changed = false;
    'again: loop {
        let shape = cfg::Shape::of(function);
        for loop_ in shape.loops.iter().filter(|loop_| wanted(loop_)).cloned().collect::<Vec<_>>() {
            let found = {
                let unit = Unit::within(context, layout, function, outer).with_shape(&shape);
                let facts = standing.of(&unit);
                _fill(&unit.with_registers(facts), callees, &loop_, &costs, &bytes, size)
            };
            if let Some(found) = found {
                _filled(context, layout, declared, function, &found);
                standing.changed();
                changed = true;
                continue 'again;
            }
        }
        return changed;
    }
}

/// What makes a loop one fill.
struct _Found {
    proof: CountedLoop,
    header: BlockId,
    first: BlockId,
    latch: BlockId,
    exit: BlockId,
    /// The store or memset each trip makes.
    effect: InstId,
    pointer: Operand,
    stored: Stored,
    /// Bytes each trip fills.
    bytes: BigInt,
    /// The other fills of the loop, where it makes several that touch apart
    /// bytes: each a memset of its own (LLVM's `LoopIdiomRecognize` takes each
    /// store by itself; gcc's loop distribution splits the loop by them).
    extras: Vec<Extra>,
    /// Fills the one memset also makes: the loop's other streams, where they
    /// tile the bytes after the first's with the same byte (gcc's loop
    /// distribution, then its store merging).
    absorbed: Vec<InstId>,
    /// The last of the loop's effects: where the calls go, after every
    /// address and count they read.
    anchor: InstId,
    /// What the exit reads of the counters, by the value it reads: the counter,
    /// its step, and the trips more than the loop's count the value has
    /// taken (a tested-after loop's exit takes the counter of its last trip:
    /// one fewer, or its next value: none).
    left: Vec<(ValueId, ValueId, BigInt, i64)>,
    /// The loop is tested after its trips, entered behind a copy of its test
    /// (`-ftree-ch`).
    posttested: bool,
    /// The memset's pointer space and length width.
    memset: (u32, u32),
}

/// A fill of a loop that makes several.
struct Extra {
    effect: InstId,
    pointer: Operand,
    byte: Byte,
    bytes: BigInt,
}

/// What a loop's trips make of the bytes they reach.
enum Stored {
    Fill(Byte),
    Copy(Copy),
}

/// A load and a store of one cell each trip, the stored value the loaded:
/// where it reads, how it may overlap what it writes, and whether it runs down.
struct Copy {
    load: InstId,
    source: Operand,
    how: How,
    descending: bool,
    /// The source's pointer space.
    space: u32,
}

/// What lets one copy stand for the loop's trips in order.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum How {
    /// The two ranges are apart: `llvm.memcpy`.
    Apart,
    /// They overlap, but what each trip reads is as the trips before it left it
    /// untouched: `llvm.memmove`, running the loop's way.
    Overlapping,
}

/// What a fill sets each byte to.
enum Byte {
    Operand(Operand),
    Number(u128),
    /// A cell whose bytes do not repeat: `memset.pattern`.
    Pattern(Operand),
}

/// The fill `loop_` is, if it is one.
fn _fill(
    unit: &Unit,
    callees: &Callees,
    loop_: &Loop,
    costs: &OperationCosts,
    bytes_costs: &OperationCosts,
    size: bool,
) -> Option<_Found> {
    let function = unit.function;
    let header = cfg::block(loop_.header);
    if loop_.latches.len() != 1 {
        return None;
    }
    // Tested before its trips: the header tests and the body is a straight line
    // back to it, in whatever order its blocks lie. Tested after
    // them: the body is a straight line from the header to the latch, which
    // tests (a loop entered behind a copy of its test).
    let successors = function.successors(header);
    let posttested = loop_.body.len() == 1 || !successors.iter().any(|to| !loop_.body.contains(&cfg::id(*to)));
    let (chain, latch, test) = if posttested {
        let chain = _chain_after(function, header, loop_)?;
        let latch = *chain.last().expect("a chain has blocks");
        (chain, latch, latch)
    } else {
        let chain = _chain(function, header, loop_)?;
        let latch = *chain.last().expect("a chain has blocks");
        (chain, latch, header)
    };
    let exit = *function.successors(test).iter().find(|to| !loop_.body.contains(&cfg::id(**to)))?;

    // A loop is a fill only if what it does besides arithmetic is stores, loads
    // and memsets that `_stored` or `_copied` take: asked before its trips are
    // proved, which most loops are not worth (27 tries, 5.3 M of x_radix's
    // compile at -O1).
    let plain = |inst: InstId| memory::speculatable(unit.context, callees, function, inst);
    let effects: Vec<InstId> = chain
        .iter()
        .flat_map(|&block| {
            operations(function, block).into_iter().filter(move |&inst| function.terminator(block) != Some(inst))
        })
        .filter(|&inst| !plain(inst))
        .collect();
    let still = induction::invariant(function, &loop_.body);
    let takes = match effects[..] {
        [] => false,
        [effect] => _stored(unit, callees, effect, &still).is_some(),
        [one, other] => {
            _copied(unit, one, other).is_some()
                || (_stored(unit, callees, one, &still).is_some() && _stored(unit, callees, other, &still).is_some())
        }
        _ => effects.iter().all(|&effect| _stored(unit, callees, effect, &still).is_some()),
    };
    if !takes {
        return None;
    }
    // How many trips is `induction`'s to prove, whatever the counter's step or
    // test.
    let tested = operations(function, test);
    let proof = induction::counted(unit, loop_, None, true)
        .into_iter()
        .find(
            |proof| proof.posttested == posttested
                && tested.last() == Some(&proof.branch)
                && tested.contains(&proof.compare)
                && (posttested || tested.iter().all(|&one| one == proof.branch || one == proof.compare || plain(one))),
        )?;
    // Its trips, placed before the loop, are what the fill is counted by.
    if posttested && induction::trips(&proof, &mut |_, args| args[0].clone()).is_none() {
        return None;
    }
    let counters = induction::basics(unit, loop_);
    let phis = edges::phis(function, header);
    if counters.len() != phis.len() {
        return None;
    }

    let work = chain
        .iter()
        .flat_map(|&block| {
            operations(function, block).into_iter().filter(move |&inst| function.terminator(block) != Some(inst))
        })
        .collect::<Vec<_>>();
    let steps = work.iter().copied().filter_map(|inst| _stepped(unit, &phis, latch, inst)).collect::<BTreeSet<_>>();
    if steps.len() != phis.len() {
        return None;
    }
    let walk = induction::recurrences(unit, loop_, &counters);
    let (effect, pointer, mut stored, bytes, extras, absorbed) = match effects[..] {
        [effect] => {
            let (pointer, byte, bytes) = _stored(unit, callees, effect, &still)?;
            (effect, pointer, Stored::Fill(byte), bytes, Vec::new(), Vec::new())
        }
        [one, other] => match _copied(unit, one, other) {
            Some((effect, pointer, stored, bytes)) => (effect, pointer, stored, bytes, Vec::new(), Vec::new()),
            None => _several(unit, callees, &effects, &still, &walk, &proof)?,
        },
        _ => _several(unit, callees, &effects, &still, &walk, &proof)?,
    };

    let facts = unit.registers();
    let Operand::Value(address) = pointer else { return None };
    let formula = walk.values.get(&address).filter(|one| one.pointer.is_some())?;
    let width = formula.width();
    let stride = formula.step.known()?;
    let descending = matches!(stored, Stored::Copy(_)) && stride == -bytes.clone();
    // Each stream strides by its own cell; the merged fill is all of them.
    let each = &bytes / BigInt::from(1 + absorbed.len());
    if (stride != each && !descending)
        || proof.width() > width
        || unit.layout.pointer(unit.space(pointer)?).index_bits != width
    {
        return None;
    }
    let pattern = match &stored {
        Stored::Fill(byte) => matches!(byte, Byte::Pattern(..)),
        Stored::Copy(_) => false,
    };
    if let Stored::Copy(copy) = &mut stored {
        copy.descending = descending;
        let Operand::Value(source) = copy.source else { return None };
        let from = walk.values.get(&source).filter(|one| one.pointer.is_some())?;
        let (bits, to) = (unit.layout.pointer(unit.space(copy.source)?).index_bits, formula);
        if from.step != to.step || from.width() != width || bits != width {
            return None;
        }
        copy.how = _overlap(unit, copy.load, effect, from, to, &proof, &bytes, descending)?;
    }
    // DEVIATE, for `loopidiom`: a fill, copy or move through a huge pointer
    // is a routine laid down at the site (`isel/huge.rs`), which a loop of a
    // few instructions undercuts under -Os (fillw -Os 86 -> 176 bytes, copyw
    // 321 -> 470, against main).
    let routine = if matches!(stored, Stored::Fill(_)) { bytes_costs.huge_fill } else { bytes_costs.huge_copy };
    if size
        && routine > 0
        && unit.layout.carries(unit.space(pointer)?)
        && trip_cost(
            unit.context,
            unit.layout,
            unit.function,
            callees,
            std::iter::once(&header).chain(&chain),
            bytes_costs,
            true,
        ) <= routine
    {
        return None;
    }
    // A pattern is `rep stosw` or `stosd`: priced, as a short loop beats its
    // setup.
    let moved = match &stored {
        Stored::Copy(copy) => Some((bytes.to_i64()?, copy.descending && copy.how == How::Overlapping)),
        Stored::Fill(_) => None,
    };
    // A loop of several fills is as many memsets: priced against the loop like
    // a pattern's, since a few cells are as many stores each (bench: lru
    // -O2 grew 78 bytes in 16-bit code where its 7-trip loop of two stores
    // was 20).
    let fills = 1 + extras.len() as i64;
    let merged = !absorbed.is_empty();
    // What each memset fills, for the price of its stores or its string.
    let filled = match (&stored, proof.count.as_ref().and_then(ToPrimitive::to_i64), bytes.to_i64()) {
        (Stored::Fill(Byte::Number(number)), Some(count), Some(bytes)) => Some((count * bytes, *number & 0xFF == 0)),
        _ => None,
    };
    let body = if posttested { &chain[1..] } else { &chain };
    if (pattern || moved.is_some() || fills > 1 || merged)
        && !_pays(unit, callees, body, header, &proof, costs, size, moved, fills, filled)
    {
        return None;
    }
    // And no larger than the loop, which a speed price does not say: in 16-bit
    // code a dword store takes the operand-size prefix and its address, and
    // seven of them are more bytes than the loop that made them (bench: lru
    // -O2, 408 -> 486).
    if (fills > 1 || merged) && !_pays(unit, callees, body, header, &proof, bytes_costs, true, moved, fills, filled) {
        return None;
    }
    // Trips of a byte never wrap the index; wider cells need a promise.
    let modulus = BigInt::from(1) << width;
    let bounded = proof.maximum.as_ref().is_some_and(|maximum| maximum * &bytes < modulus);
    if bytes != BigInt::from(1) && !MemRef::at(unit, pointer, 1).inbounds && !bounded {
        return None;
    }

    // Nothing after the loop may read what it computed, but a counter the
    // exit's phis take from the test's block.
    let mut left = Vec::new();
    for &at in &loop_.body {
        for inst in function.iter_block(cfg::block(at)) {
            let Some(result) = function.instruction(inst).result else { continue };
            for one in function.users(result) {
                let user = function.instruction(one.user);
                if function.parent(one.user).is_some_and(|block| loop_.body.contains(&cfg::id(block))) {
                    continue;
                }
                let from = user.operands.get(one.index as usize + 1);
                if user.opcode != Opcode::Phi
                    || function.parent(one.user) != Some(exit)
                    || from != Some(&Operand::Block(test))
                {
                    return None;
                }
                // The counter itself, or the value of it that a trip steps to.
                let (base, taken) = match counters.get(&result).filter(|counter| counter.start.width() == proof.width())
                {
                    Some(_) => (result, -i64::from(posttested)),
                    None if posttested => (_stepped(unit, &phis, latch, inst)?, 0),
                    None => return None,
                };
                let AffineOperand::Const(step) =
                    &counters.get(&base).filter(|counter| counter.start.width() == proof.width())?.step
                else {
                    return None;
                };
                left.push((
                    result,
                    base,
                    induction::_signed(&AffineOperand::Const(step.clone()), &facts, proof.width())?,
                    taken,
                ));
            }
        }
    }
    left.dedup();
    let memset = (unit.space(pointer)?, width);
    Some(_Found {
        proof,
        header,
        first: chain[0],
        latch,
        exit,
        effect,
        pointer,
        stored,
        bytes,
        extras,
        absorbed,
        anchor: *effects.last().expect("an effect"),
        left,
        posttested,
        memset,
    })
}

/// The first of a loop's fills as `_fill` takes the only one, and the others as
/// `Extra`s, where every effect of the loop is a fill of a plain byte, each
/// strides by its cell over bytes no other reaches. The loop's trips then fill
/// the same bytes in any order of the fills.
fn _several(
    unit: &Unit,
    callees: &Callees,
    effects: &[InstId],
    still: &induction::Invariant,
    walk: &induction::Users,
    proof: &CountedLoop,
) -> Option<(InstId, Operand, Stored, BigInt, Vec<Extra>, Vec<InstId>)> {
    let mut fills = Vec::new();
    for &effect in effects {
        let (pointer, byte, bytes) = _stored(unit, callees, effect, still)?;
        if matches!(byte, Byte::Pattern(..)) {
            return None;
        }
        let Operand::Value(address) = pointer else { return None };
        let formula = walk.values.get(&address).filter(|one| one.pointer.is_some())?;
        if formula.step.known()? != bytes {
            return None;
        }
        fills.push((effect, pointer, byte, bytes));
    }
    if fills.len() < 2 {
        return None;
    }
    // At most the trips the loop can make, from each cell.
    let trips = proof.count.clone().or_else(|| proof.maximum.clone())?;
    for (at, one) in fills.iter().enumerate() {
        let (first, other) = (MemRef::at(unit, one.1, 1), &fills[at + 1..]);
        for two in other {
            let second = MemRef::at(unit, two.1, 1);
            let apart = match (&first.root, &second.root) {
                (Some(left), Some(right)) if first.object && second.object && left != right => true,
                (Some(left), Some(right)) if first.object && second.object => {
                    first.base.is_some()
                        && first.base == second.base
                        && first.scale == second.scale
                        && one.3 == two.3
                        && BigInt::from((first.disp - second.disp).abs()) >= &one.3 * &trips
                        && left == right
                }
                _ => false,
            };
            if !apart {
                return None;
            }
        }
    }
    if let Some(count) = proof.count.clone() {
        if let Some((effect, pointer, byte, bytes, absorbed)) = _tiled(unit, &fills, &count) {
            return Some((effect, pointer, Stored::Fill(byte), bytes, Vec::new(), absorbed));
        }
    }
    let mut fills = fills.into_iter();
    let (effect, pointer, byte, bytes) = fills.next()?;
    let extras = fills.map(|(effect, pointer, byte, bytes)| Extra { effect, pointer, byte, bytes }).collect();
    Some((effect, pointer, Stored::Fill(byte), bytes, extras, Vec::new()))
}

/// The fills as one, where they set one number to the bytes of one object, each
/// the same bytes a trip, and each starts where the one before it ends after
/// all the trips: the lowest effect and pointer, the bytes a trip of them all,
/// and the other effects. gcc distributes such a loop into memsets and merges
/// the ones that are adjacent with the same byte.
fn _tiled(
    unit: &Unit,
    fills: &[(InstId, Operand, Byte, BigInt)],
    count: &BigInt,
) -> Option<(InstId, Operand, Byte, BigInt, Vec<InstId>)> {
    let Byte::Number(number) = fills[0].2 else { return None };
    let bytes = &fills[0].3;
    let mut placed = Vec::new();
    for (effect, pointer, byte, each) in fills {
        let Byte::Number(this) = byte else { return None };
        let at = MemRef::at(unit, *pointer, 1);
        if *this != number || each != bytes || !at.object || at.root.is_none() {
            return None;
        }
        placed.push((at.disp, (at.root, at.base, at.scale), *effect, *pointer));
    }
    placed.sort_by_key(|one| one.0);
    let span = bytes * count;
    for pair in placed.windows(2) {
        if pair[0].1 != pair[1].1 || BigInt::from(pair[1].0 - pair[0].0) != span {
            return None;
        }
    }
    let (_, _, effect, pointer) = placed[0].clone();
    let absorbed = placed[1..].iter().map(|one| one.2).collect();
    Some((effect, pointer, Byte::Number(number), bytes * BigInt::from(placed.len()), absorbed))
}

/// Whether the one fill is cheaper than the loop for its trips, the proven
/// count or the profit model's estimate: `rep stos` pays its setup and each
/// cell, and a count of few cells is stores. Under `size` it is the code bytes
/// of the loop against the fill's.
fn _pays(
    unit: &Unit,
    callees: &Callees,
    chain: &[BlockId],
    header: BlockId,
    proof: &CountedLoop,
    costs: &OperationCosts,
    size: bool,
    moved: Option<(i64, bool)>,
    fills: i64,
    stored: Option<(i64, bool)>,
) -> bool {
    let each = trip_cost(
        unit.context,
        unit.layout,
        unit.function,
        callees,
        std::iter::once(&header).chain(chain),
        costs,
        size,
    );
    let known = proof.count.as_ref().and_then(ToPrimitive::to_i64);
    let most = proof.maximum.as_ref().and_then(ToPrimitive::to_i64);
    _cheaper_by(each, known, most, costs, size, moved, fills, stored)
}

/// What one trip of the loop through `blocks` costs, in `costs`' units. In
/// bytes the loop's counter work is the step and its branch: the phi is a
/// register, the address an operand, the test the step's flags.
fn trip_cost<'a>(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    callees: &Callees,
    blocks: impl Iterator<Item = &'a BlockId>,
    costs: &OperationCosts,
    size: bool,
) -> i64 {
    blocks
        .flat_map(|&block| operations(function, block))
        .filter(|&inst| {
            !size
                || !matches!(
                    function.instruction(inst).opcode,
                    Opcode::Phi | Opcode::GetElementPtr { .. } | Opcode::ICmp(_)
                )
        })
        .map(|inst| profit::operation(context, layout, function, callees, inst, costs).unwrap_or(costs.add))
        .sum()
}

/// DEVIATE, for `loopidiom`: whether `rep movs` over a length known only at
/// run time is fewer code bytes than the loop over `blocks` it replaces. The
/// pin lowers that copy to a libc call, which llrm has none of: the string
/// instruction takes ES, three registers, the count split in dwords and odd
/// bytes, and a second `rep movs` (x_mergesort msort at m16 -Os: three sites,
/// 16 bytes over the loop each).
pub(crate) fn variable_copy_pays(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    callees: &Callees,
    blocks: &[BlockId],
    costs: &OperationCosts,
) -> bool {
    trip_cost(context, layout, function, callees, blocks.iter(), costs, true) > costs.copy + costs.copy_variable
}

/// DEVIATE, for `loopidiom`: whether `fills` fills, for the `stores` stores of
/// the loop over `blocks`, are fewer code bytes than the loop. A loop of one
/// store is always filled; of several, each fill is a `rep stos` with ES set
/// around it, and the stores of one object with one value merge into one.
pub(crate) fn fills_pay(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    callees: &Callees,
    blocks: &[BlockId],
    costs: &OperationCosts,
    stores: i64,
    fills: i64,
    kept: i64,
    at_speed: bool,
    clocks: Option<(&OperationCosts, i64, Option<i64>, Option<i64>)>,
) -> bool {
    // `clocks`' last entry is the cells the fill stores in all: a cell is a
    // `stos` of a dword, or the pattern's own cell.
    // The loop stays for the stores of computed values: a fill saves one
    // store, less than itself.
    if kept > 0 {
        return false;
    }
    // The table doubles an operation that becomes more than one instruction;
    // a fill's three immediate loads are 2 to 3 bytes each, not 6: three
    // quarters of `fill` is its real size (12 to 13 of 18 bytes at m16).
    let loop_bytes = trip_cost(context, layout, function, callees, blocks.iter(), costs, true);
    // At speed under a clock price (m16 -O2: 1 clock = 2 B) the bytes are in
    // the price: a fill larger than the loop still pays for the clocks it
    // saves. At -Os the bytes decide first.
    (stores < 2 || (at_speed && clocks.is_some()) || fills_cheaper(loop_bytes, fills, costs.fill))
        && clocks.is_none_or(|(clocks, rate, trips, width)| {
            // The clocks the fill adds against the bytes it saves, as the
            // backend prices a memset's fill against its stores. The loop
            // stays when its extra bytes buy fewer clocks than that.
            let (Some(trips), Some(cells)) = (trips, width) else { return true };
            let each = trip_cost(context, layout, function, callees, blocks.iter(), clocks, true);
            let fill = fills * clocks.fill + cells * clocks.fill_cell;
            let saved = loop_bytes - 3 * fills * costs.fill / 4;
            clocks_pay(saved, rate, fill, trips * each)
        })
}

/// Whether the `saved` bytes of the fill are worth the clocks it adds over
/// the loop, at `rate` thousandths of a clock a byte.
fn clocks_pay(
    saved: i64,
    rate: i64,
    fill: i64,
    looped: i64,
) -> bool {
    saved * rate >= (fill - looped) * 1000
}

/// Whether a loop of `loop_bytes` is more than `fills` fills of `fill` bytes by
/// the table, three quarters of which is their real size.
fn fills_cheaper(
    loop_bytes: i64,
    fills: i64,
    fill: i64,
) -> bool {
    4 * loop_bytes > 3 * fills * fill
}

/// What a copy of `length` bytes costs: the loads and stores isel expands it to
/// (`MEMCPY_MOVES` of them, a dword each, the odd bytes by their set bits),
/// else the string instruction.
pub(crate) fn copy_price(
    costs: &OperationCosts,
    length: i64,
    backward: bool,
) -> i64 {
    let pairs = length / 4 + i64::from((length % 4).count_ones());
    if pairs <= 8 { pairs * (costs.load + costs.store) } else { string_copy(costs, length, backward) }
}

fn string_copy(
    costs: &OperationCosts,
    length: i64,
    backward: bool,
) -> i64 {
    costs.copy + (length + 3) / 4 * costs.copy_cell + if backward { costs.direction } else { 0 }
}

/// Whether a fill beats a loop of `each` per trip, over `known` trips or, where
/// there are none, up to `most` of them. A copy of `moved` bytes a trip, `rep
/// movs` in dwords, `.1` where it runs down, beats one a few loads and stores.
fn _cheaper(
    each: i64,
    known: Option<i64>,
    most: Option<i64>,
    costs: &OperationCosts,
    size: bool,
    moved: Option<(i64, bool)>,
) -> bool {
    _cheaper_by(each, known, most, costs, size, moved, 1, None)
}

/// `_cheaper`, for a loop whose trips are `fills` fills, each its own memset.
fn _cheaper_by(
    each: i64,
    known: Option<i64>,
    most: Option<i64>,
    costs: &OperationCosts,
    size: bool,
    moved: Option<(i64, bool)>,
    fills: i64,
    stored: Option<(i64, bool)>,
) -> bool {
    let trips = if size { 1 } else { known.unwrap_or_else(|| most.unwrap_or(i64::MAX).min(profit::UNKNOWN_TRIPS)) };
    // Isel expands a few cells to stores, as gcc's by-pieces rule says, and a
    // few bytes of a copy to loads and stores.
    let Some((bytes, backward)) = moved else {
        let string = costs.fill + trips * costs.fill_cell;
        let fill = match (stored, known) {
            // The fill's length known: stores while they are under the CPU's
            // ratio, as `memset` expands it (a CPU with none states keeps the
            // cells it has always taken).
            (Some((length, zero)), _) if costs.pieces(size, zero) > 0 => {
                let pieces = length / 4 + i64::from((length % 4).count_ones());
                if pieces < costs.pieces(size, zero) {
                    pieces * costs.store
                } else {
                    costs.fill + (length + 3) / 4 * costs.fill_cell
                }
            }
            (_, Some(count)) if count <= 16 => count * costs.store,
            _ => string,
        };
        return trips * each > fills * fill;
    };
    let copy = match known {
        Some(count) => copy_price(costs, count * bytes, backward),
        None => string_copy(costs, trips * bytes, backward) + if size { costs.copy_variable } else { 0 },
    };
    trips * each > copy
}

/// `llvm.memset` for pointers of `space` and lengths `width` bits wide,
/// declared where the module has none.
pub(crate) fn _memset(
    context: &mut Context,
    declared: &mut Declared,
    space: u32,
    width: u32,
) -> (GlobalId, TypeId) {
    let types = &mut context.types;
    let (void, pointer, byte, length, flag) =
        (types.void(), types.ptr(space), types.int(8), types.int(width), types.int(1));
    let ty =
        types.intern(Type::Function { returns: void, parameters: vec![pointer, byte, length, flag], variadic: false });
    (declared.declare(&format!("llvm.memset.p{space}.i{width}"), ty), ty)
}

/// `llvm.experimental.memset.pattern` of cells of type `cell` for pointers of
/// `space` and counts `width` bits wide, declared where the module has none.
fn _pattern(
    context: &mut Context,
    declared: &mut Declared,
    space: u32,
    cell: TypeId,
    width: u32,
) -> (GlobalId, TypeId) {
    let bits = context.types.int_bits(cell).expect("an integer cell");
    let types = &mut context.types;
    let (void, pointer, count, flag) = (types.void(), types.ptr(space), types.int(width), types.int(1));
    let ty =
        types.intern(Type::Function { returns: void, parameters: vec![pointer, cell, count, flag], variadic: false });
    (declared.declare(&format!("llvm.experimental.memset.pattern.p{space}.i{bits}.i{width}"), ty), ty)
}

/// `llvm.memcpy`, or `llvm.memmove` where the ranges overlap, from pointers of
/// space `from` to ones of `to`, in lengths `width` bits wide, declared where
/// the module has none.
pub(crate) fn _copy(
    context: &mut Context,
    declared: &mut Declared,
    how: How,
    to: u32,
    from: u32,
    width: u32,
) -> (GlobalId, TypeId) {
    let types = &mut context.types;
    let (void, destination, source, length, flag) =
        (types.void(), types.ptr(to), types.ptr(from), types.int(width), types.int(1));
    let ty = types.intern(Type::Function {
        returns: void,
        parameters: vec![destination, source, length, flag],
        variadic: false,
    });
    let name = if how == How::Apart { "memcpy" } else { "memmove" };
    (declared.declare(&format!("llvm.{name}.p{to}.p{from}.i{width}"), ty), ty)
}

/// The loop made one trip that fills.
fn _filled(
    context: &mut Context,
    layout: &DataLayout,
    declared: &mut Declared,
    function: &mut Function,
    found: &_Found,
) {
    let counter = found.proof.width();
    let mut seeds = Seeds { context, function, at: found.effect, width: counter };
    let trips =
        induction::trips(&found.proof, &mut |kind, args| seeds.computed(kind, args)).expect("a pre-tested proof");
    // The bytes are counted in the pointer's index: a narrower counter's trips,
    // never wrapped, widen.
    let width = found.memset.1;
    let counted = trips.clone();
    let trips = seeds.widened(&trips, width);
    seeds.width = width;
    let cells = matches!(found.stored, Stored::Fill(Byte::Pattern(..)));
    let count = if found.bytes == BigInt::from(1) || cells {
        trips.clone()
    } else {
        seeds.computed(BinaryOp::Mul, vec![trips.clone(), AffineOperand::constant(found.bytes.clone(), width)])
    };
    let extra_counts = found
        .extras
        .iter()
        .map(|extra| {
            if extra.bytes == BigInt::from(1) {
                trips.clone()
            } else {
                seeds.computed(BinaryOp::Mul, vec![trips.clone(), AffineOperand::constant(extra.bytes.clone(), width)])
            }
        })
        .collect::<Vec<_>>();
    seeds.width = counter;
    let finals = found
        .left
        .iter()
        .map(|(value, base, step, taken)| {
            let trips = if *taken == 0 {
                counted.clone()
            } else {
                seeds.computed(BinaryOp::Add, vec![counted.clone(), AffineOperand::constant(*taken, counter)])
            };
            let moved = if *step == BigInt::from(1) {
                trips
            } else {
                seeds.computed(BinaryOp::Mul, vec![trips, AffineOperand::constant(step.clone(), counter)])
            };
            (*value, seeds.computed(BinaryOp::Add, vec![AffineOperand::Value(*base, counter), moved]))
        })
        .collect::<IndexMap<_, _>>();
    seeds.width = width;
    let count = seeds.operand(&count);
    let extra_counts = extra_counts.iter().map(|count| seeds.operand(count)).collect::<Vec<_>>();
    let finals = finals.into_iter().map(|(value, sum)| (value, seeds.operand(&sum))).collect::<IndexMap<_, _>>();
    let void = seeds.context.types.void();
    let off = counting::constant(seeds.context, &BigInt::from(0), 1);
    let (callee, function_type, mut operands, direction) = match &found.stored {
        Stored::Fill(byte) => {
            let (callee, function_type) = match byte {
                Byte::Pattern(value) => {
                    let cell = seeds.function.operand_type(seeds.context, *value).expect("a typed cell");
                    _pattern(seeds.context, declared, found.memset.0, cell, found.memset.1)
                }
                _ => _memset(seeds.context, declared, found.memset.0, found.memset.1),
            };
            let byte = match *byte {
                Byte::Operand(byte) => byte,
                Byte::Number(byte) => counting::constant(seeds.context, &BigInt::from(byte), 8),
                Byte::Pattern(value) => value,
            };
            (callee, function_type, vec![found.pointer, byte, count], None)
        }
        Stored::Copy(copy) => {
            let (callee, function_type) =
                _copy(seeds.context, declared, copy.how, found.memset.0, copy.space, found.memset.1);
            // A loop that runs down starts at its last cell: the copy starts at
            // its first.
            let (to, from) = if copy.descending {
                let last = seeds.computed(BinaryOp::Sub, vec![trips.clone(), AffineOperand::constant(1, width)]);
                let back =
                    seeds.computed(BinaryOp::Mul, vec![last, AffineOperand::constant(-found.bytes.clone(), width)]);
                let back = seeds.operand(&back);
                let i8 = seeds.context.types.int(8);
                let mut lowered = |pointer: Operand| {
                    let ty = seeds.function.operand_type(seeds.context, pointer).expect("a typed pointer");
                    let gep = seeds
                        .function
                        .create_instruction(
                            Opcode::GetElementPtr { source: i8 },
                            ty,
                            vec![pointer, back],
                            Flags::default(),
                            None,
                        );
                    seeds.function.insert(gep, Position::Before(found.effect)).expect("a placed effect");
                    Operand::Value(seeds.function.instruction(gep).result.expect("a pointer"))
                };
                (lowered(found.pointer), lowered(copy.source))
            } else {
                (found.pointer, copy.source)
            };
            let direction = (copy.how == How::Overlapping).then_some(if copy.descending {
                llrm_mir::intrinsics::BACKWARD
            } else {
                llrm_mir::intrinsics::FORWARD
            });
            (callee, function_type, vec![to, from, count], direction)
        }
    };
    let pointer = seeds.context.types.ptr(0);
    operands.extend([
        off,
        Operand::Constant(seeds.context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee) })),
    ]);
    // A memset's destination keeps the alignment its stores had, the least of
    // them: without it the backend fills by bytes (`rep stosb`).
    let dest_align = matches!(found.stored, Stored::Fill(..))
        .then(|| {
            std::iter::once(found.effect)
                .chain(found.absorbed.iter().copied())
                .map(|store| match function.instruction(store).opcode {
                    // No `align` is the type's ABI alignment.
                    Opcode::Store { align, .. } => align.or_else(|| {
                        let ty = function.operand_type(context, function.instruction(store).operands[0])?;
                        Some(layout.align(&context.types, ty))
                    }),
                    _ => None,
                })
                .try_fold(u64::MAX, |least, align| align.map(|align| least.min(align)))
        })
        .flatten()
        .filter(|align| *align != u64::MAX);
    let mut argument_attrs = vec![Vec::new(); 4];
    if let Some(align) = dest_align {
        argument_attrs[0].push(llrm_mir::opcode::Attribute::Int("align".to_owned(), align));
    }
    let info = CallInfo {
        function_type,
        calling_convention: 0,
        return_attrs: Vec::new(),
        argument_attrs,
        attrs: Vec::new(),
        tail: Default::default(),
    };
    let info_again = info.clone();
    let call = function.create_instruction(Opcode::Call(Box::new(info)), void, operands, Flags::default(), None);
    function.insert(call, Position::Before(found.anchor)).expect("a placed effect");
    if let Some(kind) = direction {
        let node = declared.node(MetadataNode { distinct: false, operands: Vec::new() });
        function.annotate(call, kind, node);
    }
    function.erase(found.effect).expect("an effect has no result");
    for &one in &found.absorbed {
        function.erase(one).expect("an effect has no result");
    }
    // The loop's other fills, each a memset of its own by the same trips.
    for (extra, count) in found.extras.iter().zip(extra_counts) {
        let (callee, function_type) = _memset(context, declared, found.memset.0, found.memset.1);
        let byte = match extra.byte {
            Byte::Operand(byte) => byte,
            Byte::Number(byte) => counting::constant(context, &BigInt::from(byte), 8),
            Byte::Pattern(value) => value,
        };
        let pointer_type = context.types.ptr(0);
        let callee =
            Operand::Constant(context.constant(Constant { ty: pointer_type, kind: ConstantKind::Global(callee) }));
        let off = counting::constant(context, &BigInt::from(0), 1);
        let info = CallInfo { function_type, ..info_again.clone() };
        let call = function.create_instruction(
            Opcode::Call(Box::new(info)),
            void,
            vec![extra.pointer, byte, count, off, callee],
            Flags::default(),
            None,
        );
        function.insert(call, Position::Before(found.anchor)).expect("a placed effect");
        function.erase(extra.effect).expect("an effect has no result");
    }
    if let Stored::Copy(copy) = &found.stored {
        function.erase(copy.load).expect("its only user is the store");
    }

    // The exit's phis take each counter's exit value from the one trip.
    for phi in edges::phis(function, found.exit) {
        let mut incoming = arms(function, phi);
        let test = if found.posttested { found.latch } else { found.header };
        let Some(&(value, _)) = incoming.iter().find(|(_, from)| *from == test) else { continue };
        let value = match value {
            Operand::Value(one) => finals.get(&one).copied().unwrap_or(value),
            _ => value,
        };
        if found.posttested {
            for arm in incoming.iter_mut().filter(|(_, from)| *from == found.latch) {
                arm.0 = value;
            }
        } else {
            incoming.push((value, found.latch));
        }
        function.set_operands(phi, from_arms(&incoming));
    }
    let back = function.terminator(found.latch).expect("a latch branch");
    function.set_operands(back, vec![Operand::Block(found.exit)]);
    for phi in edges::phis(function, found.header) {
        let [(start, _)] =
            arms(function, phi).into_iter().filter(|(_, from)| *from != found.latch).collect::<Vec<_>>()[..]
        else {
            unreachable!("one preheader")
        };
        let result = function.instruction(phi).result.expect("a phi's value");
        function.replace_all_uses_with(result, start);
        function.set_operands(phi, Vec::new());
        function.erase(phi).expect("its uses were replaced");
    }
    // A proven positive count means the header's test passes on entry: it
    // guards nothing.
    if !found.posttested && found.proof.count.as_ref().is_some_and(|count| *count != BigInt::from(0)) {
        let test = function.terminator(found.header).expect("a header branch");
        function.set_operands(test, vec![Operand::Block(found.first)]);
        for phi in edges::phis(function, found.exit) {
            let kept = arms(function, phi).into_iter().filter(|(_, from)| *from != found.header).collect::<Vec<_>>();
            function.set_operands(phi, from_arms(&kept));
        }
    }
}

/// The loop's blocks after its header, when each has one way in and one out and
/// the last goes back.
fn _chain(
    function: &Function,
    header: BlockId,
    loop_: &Loop,
) -> Option<Vec<BlockId>> {
    let mut chain = Vec::new();
    let mut at = function.successors(header).into_iter().find(|to| loop_.body.contains(&cfg::id(*to)));
    while let Some(here) = at.filter(|here| *here != header) {
        let successors = function.successors(here);
        if chain.contains(&here) || successors.len() != 1 || !edges::phis(function, here).is_empty() {
            return None;
        }
        chain.push(here);
        at = Some(successors[0]);
    }
    (!chain.is_empty() && chain.len() + 1 == loop_.body.len()).then_some(chain)
}

/// The blocks of a loop tested after its trips: the header, then each block
/// with one way on, to the latch, which tests.
fn _chain_after(
    function: &Function,
    header: BlockId,
    loop_: &Loop,
) -> Option<Vec<BlockId>> {
    let mut chain = vec![header];
    loop {
        let here = *chain.last().expect("a chain has blocks");
        let inside: Vec<BlockId> =
            function.successors(here).into_iter().filter(|to| loop_.body.contains(&cfg::id(*to))).collect();
        match inside[..] {
            [next] if next == header => break,
            [next]
                if function.successors(here).len() == 1
                    && !chain.contains(&next)
                    && edges::phis(function, next).is_empty() =>
            {
                chain.push(next)
            }
            _ => return None,
        }
    }
    (chain.len() == loop_.body.len() && function.successors(*chain.last().expect("a chain has blocks")).len() == 2)
        .then_some(chain)
}

/// Whether `stores`, all the stores some loop `loop_` would make were it split
/// to give them a loop of their own, are what `loopidioms` takes for the whole
/// of a loop: each a fill of a value the loop does not change, or the one store
/// of a copy. What loop distribution asks of a part before it parts it.
pub fn takes(
    unit: &Unit,
    callees: &Callees,
    loop_: &Loop,
    stores: &[InstId],
) -> bool {
    let function = unit.function;
    let still = induction::invariant(function, &loop_.body);
    let loads = || {
        loop_
            .body
            .iter()
            .flat_map(|&at| function.iter_block(cfg::block(at)))
            .filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::Load { .. }))
    };
    match stores {
        [] => false,
        [store] => {
            _stored(unit, callees, *store, &still).is_some()
                || loads().any(|load| _copied(unit, load, *store).is_some())
        }
        _ => stores.iter().all(|&store| _stored(unit, callees, store, &still).is_some()),
    }
}

/// A store's or a memset's pointer, the byte it fills with, and its bytes,
/// where its value is invariant and one byte repeated.
fn _stored(
    unit: &Unit,
    callees: &Callees,
    effect: InstId,
    still: &induction::Invariant,
) -> Option<(Operand, Byte, BigInt)> {
    let function = unit.function;
    let op = function.instruction(effect);
    if let Some((pointer, byte, length)) = memory::memset(unit.context, callees, function, effect) {
        let bytes = unit.int_constant(length)?;
        let volatile = unit.int_constant(op.operands[3])?;
        return (volatile == 0 && still.operand(byte)).then(|| (pointer, Byte::Operand(byte), BigInt::from(bytes)));
    }
    let Opcode::Store { volatile: false, .. } = op.opcode else { return None };
    let (value, pointer) = (op.operands[0], op.operands[1]);
    let width = unit.int_bits(value)?;
    if !still.operand(value) || width % 8 != 0 {
        return None;
    }
    let bytes = BigInt::from(width / 8);
    if width == 8 {
        return Some((pointer, Byte::Operand(value), bytes));
    }
    if let Some(byte) = _repeated(unit, value, width) {
        return Some((pointer, Byte::Number(byte), bytes));
    }
    // LLVM's memset_pattern16: the cell is a word or dword; the stored value
    // need not be a constant. Not over a pointer whose index is wider than the
    // default space's (a huge array): the count would be wider than the
    // target's `rep stos` takes, and isel cannot lower it.
    let counted = unit.layout.pointer(unit.space(pointer)?).index_bits <= unit.layout.pointer(0).index_bits;
    (matches!(width, 16 | 32) && counted).then_some((pointer, Byte::Pattern(value), bytes))
}

/// The store and load of `one` and `other`, where the store writes the loaded
/// cell and the load has no other user: its pointer, the copy, and the bytes of
/// a cell.
fn _copied(
    unit: &Unit,
    one: InstId,
    other: InstId,
) -> Option<(InstId, Operand, Stored, BigInt)> {
    let function = unit.function;
    let (load, store) =
        if matches!(function.instruction(one).opcode, Opcode::Load { .. }) { (one, other) } else { (other, one) };
    let (Opcode::Load { volatile: false, .. }, Opcode::Store { volatile: false, .. }) =
        (&function.instruction(load).opcode, &function.instruction(store).opcode)
    else {
        return None;
    };
    let loaded = function.instruction(load).result?;
    if function.instruction(store).operands[0] != Operand::Value(loaded) || !function.users(loaded).has_one_use() {
        return None;
    }
    let bits = unit.int_bits(Operand::Value(loaded))?;
    if bits % 8 != 0 || bits == 0 {
        return None;
    }
    let (source, pointer) = (function.instruction(load).operands[0], function.instruction(store).operands[1]);
    let copy = Copy { load, source, how: How::Apart, descending: false, space: unit.space(source)? };
    Some((store, pointer, Stored::Copy(copy), BigInt::from(bits / 8)))
}

/// How a loop's two walks, `from` read and `to` written, `bytes` a trip, let
/// one copy stand for the trips in order; none where they do not.
///
/// The alias analysis owns what is apart: two objects, or a `restrict`. Of
/// one object, two walks of one stride a constant `d` apart bytes (to less
/// from) are apart past the trips' span, and else run in order only when the
/// writes trail the reads: `d <= 0` going up, `d >= 0` going down. Any other
/// overlap makes each trip read what an earlier one wrote: a smear, no copy.
fn _overlap(
    unit: &Unit,
    load: InstId,
    store: InstId,
    from: &induction::Recurrence,
    to: &induction::Recurrence,
    proof: &CountedLoop,
    bytes: &BigInt,
    descending: bool,
) -> Option<How> {
    // Two objects (or a `restrict`, or two `!tbaa` types) are apart in every
    // trip. `regions::overlapping` is not asked: for two accesses off one index
    // it answers about one trip (a constant 7 past a one-byte cell is apart),
    // where the ranges of all the trips overlap.
    let references = unit.annotated().ok();
    if let Some((read, written)) = references.as_ref().and_then(|found| found.get(&load).zip(found.get(&store)))
        && (llrm_analysis::regions::typed_apart(read, written)
            || !llrm_analysis::regions::may_alias(read, written, None, None, unit.program).unwrap_or(true))
    {
        return Some(How::Apart);
    }
    if from.pointer != to.pointer {
        return None;
    }
    let apart = to.start.minus(&from.start).known()?;
    let trips = proof.count.as_ref().or(proof.maximum.as_ref());
    if trips.is_some_and(|trips| apart.magnitude() >= (trips * bytes).magnitude()) {
        return Some(How::Apart);
    }
    let zero = BigInt::from(0);
    ((!descending && apart <= zero) || (descending && apart >= zero)).then_some(How::Overlapping)
}

/// The header phi `inst` steps, if it is one's step: a constant added, the
/// phi's value from the latch.
fn _stepped(
    unit: &Unit,
    phis: &[InstId],
    latch: BlockId,
    inst: InstId,
) -> Option<ValueId> {
    let function = unit.function;
    let op = function.instruction(inst);
    let (AffineOperand::Value(stepped, _), AffineOperand::Const(_)) = induction::stepping(unit, op)? else {
        return None;
    };
    let phi = phis.iter().copied().find(|&phi| function.instruction(phi).result == Some(stepped))?;
    (arms(function, phi).iter().any(|&(value, from)| from == latch && Some(value) == op.result.map(Operand::Value)))
        .then_some(stepped)
}

#[cfg(test)]
#[path = "loopidioms_tests.rs"]
mod tests;
