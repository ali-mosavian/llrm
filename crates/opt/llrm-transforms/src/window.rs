//! A counted loop's pointers are split into windows, inside each of which the
//! loop runs on far pointers that never carry into the segment selector, or on
//! one narrow index.
//!
//! # How it works
//! A huge pointer's step carries into the selector, so each step costs the
//! carry. The target names a window (`Machine::huge_window`: the far address
//! space, and the number of bytes from a normalized far pointer that no
//! displacement carries out of); a target without one is left alone. On the
//! 16-bit x86 target it is 64K less one selector step's last byte.
//!
//! The loop must have one preheader and one latch, and a counted-loop proof
//! from `induction` whose test is the header's (before each trip) or the
//! latch's (after). Its walks are the huge pointer recurrences with a
//! constant step, read only by their own step and by loads and stores at
//! constant offsets from them. A walk's `fixed` is the span of bytes its
//! accesses reach beyond the steps; `most`, the trips one window holds, is
//! the least over the walks of `(window - fixed) / |step|`, and must be at
//! least 2.
//!
//! Where the trips provably fit one window (`count <= most`) nothing is
//! split: each walk is normalized once before the loop by the call
//! `llrm.ia16.window` (`Intrinsic::Window`), which returns the far pointer
//! with the lowest offset, and the loop keeps its own test but steps the far
//! pointer.
//!
//! Otherwise an outer loop walks the windows:
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef opt fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   pre["<b>preheader</b><br/><small>trips = count</small>"]:::mid
//!   w["<b>windows</b><br/><small>here = min(remaining, most)<br/>left = remaining - here<br/>origin = window(huge)</small>"]:::opt
//!   l["<b>loop</b><br/><small>far = phi [origin] [far + step]<br/>here trips</small>"]:::mid
//!   n["<b>window.next</b><br/><small>huge = far, less the<br/>accesses#39; lowest byte<br/>br left != 0</small>"]:::mid
//!   x["<b>exit</b>"]:::mid
//!   pre --> w
//!   pre -->|"no trips"| x
//!   w --> l
//!   l -->|"next trip"| l
//!   l --> n
//!   n -->|"left != 0"| w
//!   n -->|"left = 0"| x
//! ```
//!
//! *The windows: one far-pointer loop inside each window*
//!
//! Every other header phi is carried across windows by a phi in `windows`.
//! The exit's phis read the outer latch; values the loop computes may be read
//! outside only by the exit's phis. A walk down is normalized at its last
//! trip's lowest byte.
//!
//! The split is priced. Each window normalizes every walk (three adds each)
//! and past the first steps it on as a huge pointer, besides its count and
//! test; it saves one carry step per walk per trip. For time, trips times the
//! saving must exceed the windows (`trips / most`, rounded up) times the setup,
//! where an unknown count is taken as its bound or `profit::UNKNOWN_TRIPS`.
//! For size (-Os), the saving must exceed one setup. A loop that fails is
//! left alone. The new branches carry `branch_weights`.
//!
//! # The index form
//! A target with no huge window but an index narrower than its pointers
//! (`Machine::index_register_bits`; the 6502's `lda (base),y`) gets the same
//! windows with another inner loop: the walks step by one constant, upward, and
//! one `n`-bit index counts every walk and the window's trips alike, from
//! `-bytes` up to the wrap at zero. Each walk's base is fixed for the window,
//! `start + bytes - 2^n`, and the next window starts at `start + bytes`. A
//! window holds `2^n / step` trips. A pointer read after the loop is the last
//! window's end. The count and the sums a window makes are folded by `combine`
//! at the end, where no later pass would. Priced in the index's words: a
//! pointer step costs its words, the shared index step one.
//!
//! DEVIATE LP-4 (no pin pass: huge-pointer windows are the DOS exception, LLVM
//! has no huge pointer; exit: none planned, kept; to migrate off the old
//! analyses it reads; owner w-6; review 2026-11-27).
//!
//! DEVIATE LP-5 (the index form: LLVM's LSR keeps one pointer per access and
//! leaves the carry of an 8-bit index to the target; exit: when the greedy
//! allocator (#6259) can hold a pointer pair and an index across the loop, a
//! translation of LSR's `LSRUse` formulae for a narrow index replaces this;
//! owner 6502 session; review 2026-11-27).
//!
//! # Example
//! Before:
//! ```mir
//! define ... @"_clear@3"(ptr addrspace(3) nocapture writeonly %0, i16 %1) ... {
//! b1:
//!   %2 = sub i16 0, %1
//!   %3 = icmp ule i16 %1, 0
//!   br i1 %3, label %b3, label %5
//!
//! b3:
//!   ret void
//!
//! b4:
//!   %lsr.iv2 = phi ptr addrspace(3) [ %lsr.iv.next, %b4 ], [ %0, %5 ]
//!   %lsr.iv11 = phi i16 [ %lsr.iv.next1, %b4 ], [ %2, %5 ]
//!   store i8 0, ptr addrspace(3) %lsr.iv2, !tbaa !7
//!   %lsr.iv.next = getelementptr i8, ptr addrspace(3) %lsr.iv2, i32 1
//!   %lsr.iv.next1 = add i16 %lsr.iv11, 1
//!   %4 = icmp ne i16 %lsr.iv.next1, 0
//!   br i1 %4, label %b4, label %6
//!
//! 5:
//!   br label %b4
//!
//! 6:
//!   br label %b3
//! }
//! ```
//! After:
//! ```mir
//! define ... @"_clear@3"(ptr addrspace(3) nocapture writeonly %0, i16 %1) ... {
//!   ; ...
//! b4:
//!   %window.trips = phi i16 [ %14, %windows ], [ %5, %b4 ]
//!   %window = phi ptr addrspace(1) [ %4, %b4 ], [ %15, %windows ]
//!   store i8 0, ptr addrspace(1) %window, !tbaa !7
//!   %4 = getelementptr i8, ptr addrspace(1) %window, i16 1
//!   %5 = sub i16 %window.trips, 1
//!   %6 = icmp ne i16 %5, 0
//!   br i1 %6, label %b4, label %window.next, !prof !24
//!
//! window.next:
//!   %7 = addrspacecast ptr addrspace(1) %4 to ptr addrspace(3)
//!   %8 = icmp ne i16 %16, 0
//!   br i1 %8, label %windows, label %17, !prof !25
//!   ; ...
//! windows:
//!   %remaining = phi i16 [ %12, %9 ], [ %16, %window.next ]
//!   %carried = phi ptr addrspace(3) [ %0, %9 ], [ %7, %window.next ]
//!   %13 = icmp ult i16 %remaining, -16
//!   %14 = select i1 %13, i16 %remaining, i16 -16
//!   %15 = call ptr addrspace(1) @llrm.ia16.window.p1.p3(ptr addrspace(3) %carried)
//!   %16 = sub i16 %remaining, %14
//!   br label %b4
//!   ; ...
//! ```
//! The huge pointer `%lsr.iv2` becomes a far pointer `%window` that steps
//! inside `b4`. Each trip of `windows` normalizes the pointer with
//! `llrm.ia16.window` and runs at most 65520 trips (`-16`) before the next
//! window.
//!
//! From `--dump` of `clear` (`char huge *p; for (i = 0; i < n; i++) p[i] = 0;`)
//! at -O2 -m16, stages `differences` and `window`.
//!
//! # Runs
//! Once per function, after LSR chose the pointers (the windows are made over
//! its recurrences) and before the final `rotate`. Off at -O0 and by
//! `-fno-strength-reduce`; a no-op on a target with no huge window.
//!
//! # Needs
//! `induction` (`counted`, `pointers`, `trips`), `Registers` and `cfg::Shape`
//! (`llrm_analysis`); [`crate::profit`] for the target's prices;
//! [`crate::counting`] for the skip test and constants.
//!
//! # Keeps
//! Nothing: a change invalidates every analysis.
//!
//! # See also
//! LLVM's IRCE (inductive range check elimination), whose check the carry
//! into the selector stands in for.

use llrm_analysis::graph::loops::Loop;
use llrm_analysis::induction::{self, CountedLoop, PointerRecurrence};
use llrm_analysis::manager::Registers;
use llrm_analysis::num::Num as BigInt;
use llrm_analysis::{cfg, memory};
use llrm_mir::context::{Constant, ConstantKind, Context, GlobalId};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, MetadataNode, MetadataOperand, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CallInfo, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, Declared, FunctionPass, Outer, PreservedAnalyses, Unit};
use llrm_mir::target::OperationCosts;
use llrm_mir::types::{Type, TypeId};
use num_traits::ToPrimitive;

use crate::counting::{self, Seeds};
use crate::profit;

pub struct Window {
    /// Priced in bytes, not time.
    pub size: bool,
}

impl FunctionPass for Window {
    fn name(&self) -> &'static str {
        "window"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        if windowed(unit, analyses, &outer, self.size) { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// A huge recurrence the windows carry, its accesses, and the bytes they
/// reach from the phi's value, `low` inclusive.
struct Walk {
    recurrence: PointerRecurrence,
    /// Each access through the phi or a constant step from it, by that step.
    reads: Vec<(InstId, usize, i64)>,
    low: i64,
    high: i64,
    /// Where the stepped value is read after the loop: it is the pointer past
    /// the last trip (`Form::Index` only).
    last: Vec<(InstId, usize)>,
}

impl Walk {
    fn step(&self) -> i64 {
        self.recurrence.step.to_i64().expect("a checked step")
    }

    /// What `n` trips reach beyond `n` steps. The far recurrence follows
    /// the lowest byte the accesses read; its `n + 1` values name bytes of
    /// the window too, so the last one, carried to the next window, never
    /// wraps. A window holds `(window - fixed) / |step|` trips.
    fn fixed(&self) -> i64 {
        let (step, span) = (self.step(), self.high - self.low);
        if step > 0 { (span - step).max(1) } else { span }
    }
}

/// A loop to split.
struct Found {
    preheader: BlockId,
    header: BlockId,
    latch: BlockId,
    /// The header's successor in the loop, and its exit.
    body: BlockId,
    exit: BlockId,
    proof: CountedLoop,
    walks: Vec<Walk>,
    /// The most whole trips one window holds for every walk.
    most: i64,
    /// The trips provably fit one window.
    one: bool,
    /// The index-wide words of a walked pointer (`Form::Index`).
    words: i64,
}

/// How a window's inner loop addresses its walks, by what the target states.
#[derive(Clone, Copy)]
enum Form {
    /// Far pointers that never carry into a selector (`Machine::huge_window`):
    /// their space, and the bytes of a window.
    Far { space: u32, window: i64 },
    /// One narrow index shared by the walks, added to a base fixed for the
    /// window (`Machine::index_register_bits`): its width in bits.
    Index { bits: u32 },
}

impl Form {
    fn of(machine: &dyn llrm_mir::target::Machine) -> Option<Form> {
        machine
            .huge_window()
            .map(|(space, window)| Form::Far { space, window })
            .or_else(|| machine.index_register_bits().map(|bits| Form::Index { bits }))
    }
}

/// Each loop's huge recurrences split into windows, where that pays.
pub fn windowed(
    unit: &mut Unit,
    analyses: &Analyses,
    outer: &Outer,
    size: bool,
) -> bool {
    let Some(form) = Form::of(outer.target()) else { return false };
    let costs = if size { outer.target().size_costs() } else { outer.target().costs() };
    let mut done = std::collections::BTreeSet::new();
    let mut changed = false;
    loop {
        let mut fresh = analyses.fresh();
        let facts = fresh.get::<Registers>(unit.context, unit.layout, unit.function);
        let shape = fresh.get::<llrm_analysis::cfg::Shape>(unit.context, unit.layout, unit.function);
        let found = {
            let view = memory::Unit::within(unit.context, unit.layout, unit.function, outer)
                .with_registers(&facts)
                .with_shape(&shape);
            let loops = shape.loops.clone();
            let mut found = None;
            for one in &loops {
                if done.insert(one.header) {
                    found = _found(&view, one, form).filter(|found| _pays(found, form, &costs, size));
                    if found.is_some() {
                        break;
                    }
                }
            }
            found
        };
        let Some(found) = found else { break };
        _split(unit, form, &found);
        crate::dead::dead(unit.context, outer.callees(), unit.function);
        changed = true;
    }
    // The trips and the sums a window makes are exact but unfolded (`0 - (0 -
    // n)`), and an index of a few bits has no later pass to fold them.
    if changed && matches!(form, Form::Index { .. }) {
        crate::combine::combine(
            unit.context,
            &outer.globals,
            outer.callees(),
            &outer.metadata,
            unit.layout,
            unit.function,
            unit.declared,
        );
    }
    changed
}

fn _preheader(
    function: &Function,
    loop_: &Loop,
) -> Option<BlockId> {
    let header = cfg::block(loop_.header);
    let outside = function
        .predecessors(header)
        .into_iter()
        .filter(|&one| !loop_.body.contains(&cfg::id(one)))
        .collect::<Vec<_>>();
    match outside[..] {
        [one] if function.successors(one) == [header] => Some(one),
        _ => None,
    }
}

/// `recurrence` and its accesses, where it is read only by them and its step.
fn _walk(
    view: &memory::Unit,
    loop_: &Loop,
    recurrence: PointerRecurrence,
    live_out: bool,
) -> Option<Walk> {
    let function = view.function;
    let within = |inst: InstId| function.parent(inst).is_some_and(|block| loop_.body.contains(&cfg::id(block)));
    recurrence.step.to_i64()?;
    let stepped = function.instruction(recurrence.stepping).result?;
    let mut last = Vec::new();
    for one in function.users(stepped) {
        if one.user == recurrence.phi {
            continue;
        }
        if !live_out || within(one.user) {
            return None;
        }
        last.push((one.user, one.index as usize));
    }
    let mut reads = Vec::new();
    let (mut low, mut high) = (i64::MAX, i64::MIN);
    let mut read = |user: InstId, index: usize, by: i64, reads: &mut Vec<(InstId, usize, i64)>| -> Option<()> {
        let op = function.instruction(user);
        let ty = match (&op.opcode, index) {
            (Opcode::Load { .. }, 0) => op.ty,
            (Opcode::Store { .. }, 1) => function.operand_type(view.context, op.operands[0])?,
            _ => return None,
        };
        let width = i64::try_from(view.layout.store_size(&view.context.types, ty)).ok()?;
        (low, high) = (low.min(by), high.max(by + width));
        reads.push((user, index, by));
        Some(())
    };
    for one in function.users(recurrence.value) {
        if one.user == recurrence.stepping {
            continue;
        }
        if !within(one.user) {
            return None;
        }
        let op = function.instruction(one.user);
        match op.opcode {
            Opcode::GetElementPtr { source } if one.index == 0 => {
                let indices = op.operands[1..]
                    .iter()
                    .map(|&index| {
                        view.int_constant(index)
                            .map(|bits| llrm_mir::context::signed(bits, view.int_bits(index).unwrap_or(128)))
                    })
                    .collect::<Vec<_>>();
                let (by, variable) = view.layout.collect_offset(&view.context.types, source, &indices);
                if !variable.is_empty() || indices.iter().any(Option::is_none) {
                    return None;
                }
                let address = op.result?;
                for access in function.users(address) {
                    read(access.user, access.index as usize, i64::try_from(by).ok()?, &mut reads)?;
                }
            }
            _ => read(one.user, one.index as usize, 0, &mut reads)?,
        }
    }
    (!reads.is_empty()).then_some(Walk { recurrence, reads, low, high, last })
}

fn _found(
    view: &memory::Unit,
    loop_: &Loop,
    form: Form,
) -> Option<Found> {
    let function = view.function;
    let preheader = _preheader(function, loop_)?;
    let header = cfg::block(loop_.header);
    let latches: Vec<i64> = loop_.latches.iter().copied().collect();
    let [latch] = latches[..] else { return None };
    let latch = cfg::block(latch);
    let inside = |block: BlockId| loop_.body.contains(&cfg::id(block));
    let walks: Vec<Walk> = induction::pointers(view, loop_)
        .into_iter()
        .filter(|one| match (form, view.space(Operand::Value(one.value))) {
            (Form::Far { .. }, Some(space)) => view.layout.carries(space),
            (Form::Index { .. }, Some(space)) => !view.layout.carries(space),
            _ => false,
        })
        .filter_map(|one| _walk(view, loop_, one, matches!(form, Form::Index { .. })))
        .collect();
    if walks.is_empty() {
        return None;
    }
    // One index steps every walk alike, upward.
    if matches!(form, Form::Index { .. }) {
        let step = walks[0].step();
        if step <= 0 || walks.iter().any(|walk| walk.step() != step) {
            return None;
        }
    }
    // The proof's test leaves: the header's before each trip, or the latch's
    // after.
    let proofs = induction::counted(view, loop_, Some(&view.registers()), false);
    let tested = |one: &CountedLoop, block: BlockId| function.terminator(block) == Some(one.branch);
    let proof = proofs
        .into_iter()
        .find(|one| !one.stops && (tested(one, header) && !one.posttested || tested(one, latch) && one.posttested))?;
    let words = match (form, view.space(Operand::Value(walks[0].recurrence.value))) {
        (Form::Index { bits }, Some(space)) => i64::from(view.layout.pointer(space).index_bits.div_ceil(bits)),
        _ => 1,
    };
    let most = match form {
        Form::Far { window, .. } => {
            walks.iter().map(|walk| (window - walk.fixed()).div_euclid(walk.step().abs())).min()?
        }
        Form::Index { bits } => (1i64 << bits) / walks[0].step(),
    };
    if most < 2 {
        return None;
    }
    // One window changes no block: the loop keeps its own test.
    if matches!(form, Form::Far { .. })
        && proof.count.as_ref().and_then(ToPrimitive::to_i64).is_some_and(|count| count <= most)
    {
        return Some(Found {
            preheader,
            header,
            latch,
            body: header,
            exit: header,
            proof,
            walks,
            most,
            one: true,
            words,
        });
    }
    // The trips must be placeable before the loop.
    if proof.count.is_none() && induction::trips(&proof, &mut |_, args| args[0].clone()).is_none() {
        return None;
    }
    // The test's block alone leaves, to one exit.
    let test = if proof.posttested { latch } else { header };
    let (body, exit) = match function.successors(test)[..] {
        [one, other] if inside(one) && !inside(other) => (one, other),
        [one, other] if inside(other) && !inside(one) => (other, one),
        _ => return None,
    };
    for &block in &loop_.body {
        let block = cfg::block(block);
        if block != test && function.successors(block).into_iter().any(|one| !inside(one)) {
            return None;
        }
    }
    // A pointer read past the loop is the one past the last trip: the exit
    // takes it from the test after a trip, through a phi of its own.
    let lives_out = walks.iter().any(|walk| !walk.last.is_empty());
    if lives_out
        && (!proof.posttested
            || function.predecessors(exit) != [test]
            || walks.iter().flat_map(|walk| &walk.last).any(|&(user, _)| function.parent(user) == Some(exit)))
    {
        return None;
    }
    // What the loop computes is read outside only by the exit's phis. A
    // window leaves from its latch: before each trip, only a header phi's
    // value is known there; after, anything the latch reaches.
    for &block in &loop_.body {
        for inst in function.iter_block(cfg::block(block)) {
            let Some(result) = function.instruction(inst).result else { continue };
            if walks.iter().any(|walk| walk.recurrence.stepping == inst) {
                continue;
            }
            for one in function.users(result) {
                let Some(at) = function.parent(one.user) else { continue };
                if inside(at) {
                    continue;
                }
                let phi = function.instruction(one.user);
                let header_phi =
                    function.parent(inst) == Some(header) && function.instruction(inst).opcode == Opcode::Phi;
                if at != exit
                    || phi.opcode != Opcode::Phi
                    || phi.operands.get(one.index as usize + 1) != Some(&Operand::Block(test))
                    || !(header_phi || proof.posttested)
                {
                    return None;
                }
            }
        }
    }
    Some(Found { preheader, header, latch, body, exit, proof, walks, most, one: false, words })
}

/// Whether the carries a split saves outweigh its windows' setup: each
/// window normalizes every walk and, past the first, steps it on as a huge
/// pointer, besides its count. Time is weighed over the trips, or the loop's
/// estimate where they are unknown; size once.
fn _pays(
    found: &Found,
    form: Form,
    costs: &OperationCosts,
    size: bool,
) -> bool {
    let walks = found.walks.len() as i64;
    let (saved, setup) = match form {
        Form::Far { .. } => {
            let normalize = walks * 3 * costs.add;
            let setup = if found.one {
                normalize
            } else {
                normalize + walks * costs.carry_step + 4 * costs.add + 2 * costs.branch
            };
            (walks * costs.carry_step, setup)
        }
        // Each walk's pointer step, in the index's words, less the one index
        // step that replaces them; each window sums two pointers per walk
        // besides its count.
        Form::Index { .. } => {
            let words = found.words;
            (walks * words * costs.add - costs.add, 4 * costs.add + 2 * costs.branch + walks * 2 * words * costs.add)
        }
    };
    if size {
        return saved > setup;
    }
    let trips = _expected(found);
    let windows = (trips + found.most - 1) / found.most;
    trips * saved > windows.max(1) * setup
}

/// The trips the loop makes, or is expected to.
fn _expected(found: &Found) -> i64 {
    let known = found.proof.count.as_ref().and_then(ToPrimitive::to_i64);
    known.unwrap_or_else(|| {
        found.proof.maximum.as_ref().and_then(ToPrimitive::to_i64).unwrap_or(i64::MAX).min(profit::UNKNOWN_TRIPS)
    })
}

/// `branch` taken to its first target `staying` times for each time to its
/// second.
fn _weighed(
    context: &mut Context,
    declared: &mut Declared,
    function: &mut Function,
    branch: InstId,
    staying: i64,
) {
    let word = context.types.int(32);
    let weight = |context: &mut Context, n: i64| MetadataOperand::Constant(context.int(word, i128::from(n.max(0))));
    let operands =
        vec![MetadataOperand::String("branch_weights".to_owned()), weight(context, staying), weight(context, 1)];
    let node = declared.node(MetadataNode { distinct: false, operands });
    function.annotate(branch, "prof", node);
}

fn _placed(
    function: &mut Function,
    opcode: Opcode,
    ty: TypeId,
    operands: Vec<Operand>,
    at: Position,
) -> Operand {
    let inst = function.create_instruction(opcode, ty, operands, Flags::default(), None);
    function.insert(inst, at).expect("a placed position");
    Operand::Value(function.instruction(inst).result.expect("a value"))
}

fn _phi(
    function: &mut Function,
    ty: TypeId,
    block: BlockId,
    name: &str,
) -> (InstId, Operand) {
    let phi = function.create_instruction(Opcode::Phi, ty, Vec::new(), Flags::default(), Some(name));
    let at = function.block(block).first().map_or(Position::End(block), Position::Before);
    function.insert(phi, at).expect("a placed phi");
    (phi, Operand::Value(function.instruction(phi).result.expect("a phi's value")))
}

/// `found` split: an outer loop over windows around the loop on far pointers,
/// or, for one window, the start normalized before it.
fn _split(
    unit: &mut Unit,
    form: Form,
    found: &Found,
) {
    let (context, declared, function, layout) =
        (&mut *unit.context, &mut *unit.declared, &mut *unit.function, unit.layout);
    let entering = function.terminator(found.preheader).expect("a preheader's branch");
    if found.one {
        let Form::Far { space, .. } = form else { unreachable!("only a far window holds the trips whole") };
        let far = context.types.ptr(space);
        let far_index = layout.pointer(space).index_bits;
        // One window: each walk from its start, the loop's own test kept.
        let count = found.proof.count.clone().expect("one window has a count");
        for walk in &found.walks {
            let huge = function.value(walk.recurrence.value).ty;
            let Type::Pointer(huge_space) = *context.types.get(huge) else { unreachable!("a pointer recurrence") };
            let trips = counting::constant(context, &count, layout.pointer(huge_space).index_bits);
            let origin = _windowed(
                context,
                declared,
                function,
                layout,
                far,
                walk,
                walk.recurrence.start,
                trips,
                Position::Before(entering),
            );
            let _ = _far(context, function, walk, origin, found.preheader, found.header, far, far_index);
        }
        _erased(function, found);
        return;
    }
    let width = found.proof.width();
    let count_ty = context.types.int(width);
    let word = context.types.int(16);
    // The trips, before the loop: none skips it.
    let mut seeds = Seeds { context: &mut *context, function: &mut *function, at: entering, width };
    let trips = match &found.proof.count {
        Some(count) => induction::AffineOperand::constant(count.clone(), width),
        None => {
            induction::trips(&found.proof, &mut |kind, args| seeds.computed(kind, args)).expect("trips _found placed")
        }
    };
    let trips = seeds.operand(&trips);
    let skip = if found.proof.count.is_none() && !found.proof.entry_guarded {
        counting::skip_guard(&mut seeds, &found.proof)
    } else {
        None
    };
    // Outer header, and its latch.
    let windows = function.create_block(Some("windows"));
    function.insert_block(windows, Some(found.preheader)).expect("a new block");
    let next_window = function.create_block(Some("window.next"));
    function.insert_block(next_window, Some(found.latch)).expect("a new block");
    // Every header phi is carried across windows; a walk's as its huge pointer.
    let header_phis: Vec<InstId> =
        function.iter_block(found.header).take_while(|&one| function.instruction(one).opcode == Opcode::Phi).collect();
    let walked: Vec<InstId> = found.walks.iter().map(|walk| walk.recurrence.phi).collect();
    let mut carried = Vec::new();
    for &phi in &header_phis {
        let ty = function.value(function.instruction(phi).result.expect("a phi's value")).ty;
        let (outer, value) = _phi(function, ty, windows, "carried");
        carried.push((phi, outer, value));
    }
    let (remaining_phi, remaining) = _phi(function, count_ty, windows, "remaining");
    let at =
        |function: &Function, block: BlockId| Position::Before(function.terminator(block).expect("a terminated block"));
    // This window's trips: those left, or the most a window holds.
    let most = counting::constant(context, &BigInt::from(found.most), width);
    let bit = context.types.int(1);
    function.set_operands(entering, vec![Operand::Block(windows)]);
    let jump = function.create_instruction(
        Opcode::Br,
        context.types.void(),
        vec![Operand::Block(found.header)],
        Flags::default(),
        None,
    );
    function.insert(jump, Position::End(windows)).expect("a new block");
    let fewer = _placed(function, Opcode::ICmp(IntPredicate::Ult), bit, vec![remaining, most], Position::Before(jump));
    let trips_here = _placed(function, Opcode::Select, count_ty, vec![fewer, remaining, most], Position::Before(jump));
    let mut stepped_on = Vec::new();
    let latch_branch = function.terminator(found.latch).expect("a latch's branch");
    if let Form::Far { space, .. } = form {
        let far = context.types.ptr(space);
        let far_index = layout.pointer(space).index_bits;
        let counted16 = _resized(context, function, trips_here, width, 16, false, Position::Before(jump));
        let counted32 = _resized(context, function, trips_here, width, 32, false, Position::Before(jump));
        // Each walk normalized where this window starts.
        for (walk, &(_, _, outer_value)) in found
            .walks
            .iter()
            .map(|walk| (walk, carried.iter().find(|(phi, ..)| *phi == walk.recurrence.phi).expect("a carried walk")))
        {
            let origin = _windowed(
                context,
                declared,
                function,
                layout,
                far,
                walk,
                outer_value,
                counted32,
                Position::Before(jump),
            );
            let last = _far(context, function, walk, origin, windows, found.header, far, far_index);
            // The next window's start: the far recurrence past the last trip,
            // which the window holds, less the accesses' lowest byte. The huge
            // pointer is not held across the window.
            let huge = function.value(walk.recurrence.value).ty;
            let reached =
                _placed(function, Opcode::Cast(CastOp::AddrSpaceCast), huge, vec![last], Position::End(next_window));
            let next = if walk.low == 0 {
                reached
            } else {
                let back = counting::constant(context, &BigInt::from(-walk.low), 32);
                let byte = context.types.int(8);
                _placed(
                    function,
                    Opcode::GetElementPtr { source: byte },
                    huge,
                    vec![reached, back],
                    Position::End(next_window),
                )
            };
            stepped_on.push((walk.recurrence.phi, next));
        }
        // The inner loop counts this window's trips, in place of its own test:
        // a window runs one at least, so it tests after each.
        let (count_phi, count) = _phi(function, word, found.header, "window.trips");
        let one = counting::constant(context, &BigInt::from(1), 16);
        let counted_down =
            _placed(function, Opcode::Binary(BinaryOp::Sub), word, vec![count, one], at(function, found.latch));
        function.set_operands(
            count_phi,
            vec![counted16, Operand::Block(windows), counted_down, Operand::Block(found.latch)],
        );
        let zero16 = counting::constant(context, &BigInt::from(0), 16);
        let goes_on = _placed(
            function,
            Opcode::ICmp(IntPredicate::Ne),
            bit,
            vec![counted_down, zero16],
            at(function, found.latch),
        );
        function.set_operands(latch_branch, vec![goes_on, Operand::Block(found.header), Operand::Block(next_window)]);
    } else if let Form::Index { bits } = form {
        // One index counts every walk, from where it must stop (zero) back by
        // the window's bytes; each walk's base is fixed at the window's start
        // less the index's range, so the index wraps as the window ends.
        let step = &found.walks[0].recurrence.step;
        let narrow = context.types.int(bits);
        let here = _resized(context, function, trips_here, width, bits, false, Position::Before(jump));
        let bytes = _scaled(context, function, here, step, bits, Position::Before(jump));
        let zero = counting::constant(context, &BigInt::from(0), bits);
        let start = _placed(function, Opcode::Binary(BinaryOp::Sub), narrow, vec![zero, bytes], Position::Before(jump));
        let (index_phi, index) = _phi(function, narrow, found.header, "window.index");
        let stepper = counting::constant(context, step, bits);
        let next =
            _placed(function, Opcode::Binary(BinaryOp::Add), narrow, vec![index, stepper], at(function, found.latch));
        function.set_operands(index_phi, vec![start, Operand::Block(windows), next, Operand::Block(found.latch)]);
        let goes_on =
            _placed(function, Opcode::ICmp(IntPredicate::Ne), bit, vec![next, zero], at(function, found.latch));
        function.set_operands(latch_branch, vec![goes_on, Operand::Block(found.header), Operand::Block(next_window)]);
        let first = function
            .iter_block(found.header)
            .find(|&one| function.instruction(one).opcode != Opcode::Phi)
            .expect("a header's body");
        let byte = context.types.int(8);
        let gep = Opcode::GetElementPtr { source: byte };
        for walk in &found.walks {
            let &(_, _, outer_value) =
                carried.iter().find(|(phi, ..)| *phi == walk.recurrence.phi).expect("a carried walk");
            let pointer = function.value(walk.recurrence.value).ty;
            let Type::Pointer(space) = *context.types.get(pointer) else { unreachable!("a pointer recurrence") };
            let wide = layout.pointer(space).index_bits;
            // The window's end is the next window's start; its base is one
            // range before, and each access's offset on from there.
            let counted = _resized(context, function, trips_here, width, wide, false, Position::Before(jump));
            let moved = _scaled(context, function, counted, step, wide, Position::Before(jump));
            let end = _placed(function, gep.clone(), pointer, vec![outer_value, moved], Position::Before(jump));
            let range = counting::constant(context, &BigInt::from(-(1i64 << bits)), wide);
            let base = _placed(function, gep.clone(), pointer, vec![end, range], Position::Before(jump));
            let reach = _resized(context, function, index, bits, wide, false, Position::Before(first));
            let mut bases = std::collections::BTreeMap::new();
            for &(user, operand, by) in &walk.reads {
                let origin = *bases
                    .entry(by)
                    .or_insert_with(
                        || if by == 0 {
                            base
                        } else {
                            let by = counting::constant(context, &BigInt::from(by), wide);
                            _placed(function, gep.clone(), pointer, vec![base, by], Position::Before(jump))
                        },
                    );
                let address = _placed(function, gep.clone(), pointer, vec![origin, reach], Position::Before(user));
                function.set_operand(user, operand, address);
            }
            stepped_on.push((walk.recurrence.phi, end));
        }
    }
    // What the branches are known to do, for the blocks' frequencies.
    let expected = _expected(found);
    let windows_run = ((expected + found.most - 1) / found.most).max(1);
    _weighed(context, declared, function, latch_branch, (expected + windows_run - 1) / windows_run - 1);
    if !found.proof.posttested {
        let branch = function.terminator(found.header).expect("a header's branch");
        function.set_operands(branch, vec![Operand::Block(found.body)]);
    }
    // The outer latch: the trips left, and the next window or the exit.
    // Taken before the window runs, so its trips need not outlive it.
    let left =
        _placed(function, Opcode::Binary(BinaryOp::Sub), count_ty, vec![remaining, trips_here], Position::Before(jump));
    let zero = counting::constant(context, &BigInt::from(0), width);
    let more = _placed(function, Opcode::ICmp(IntPredicate::Ne), bit, vec![left, zero], Position::End(next_window));
    let back = function.create_instruction(
        Opcode::Br,
        context.types.void(),
        vec![more, Operand::Block(windows), Operand::Block(found.exit)],
        Flags::default(),
        None,
    );
    function.insert(back, Position::End(next_window)).expect("a new block");
    _weighed(context, declared, function, back, windows_run - 1);
    function
        .set_operands(remaining_phi, vec![trips, Operand::Block(found.preheader), left, Operand::Block(next_window)]);
    // The carried values: a header phi's start, then its value as a window
    // ends, its latch's; a walk's, its huge pointer stepped on.
    let arm_of = |function: &Function, phi: InstId, block: BlockId| {
        function.instruction(phi).operands.chunks(2).find(|pair| pair[1] == Operand::Block(block)).expect("an arm")[0]
    };
    let mut endings = Vec::new();
    for &(phi, _, _) in &carried {
        let ending = stepped_on
            .iter()
            .find(|(one, _)| *one == phi)
            .map_or_else(|| arm_of(function, phi, found.latch), |(_, next)| *next);
        endings.push((Operand::Value(function.instruction(phi).result.expect("a value")), ending));
    }
    for (&(phi, outer, outer_value), &(_, ending)) in carried.iter().zip(&endings) {
        let start = arm_of(function, phi, found.preheader);
        function.set_operands(outer, vec![start, Operand::Block(found.preheader), ending, Operand::Block(next_window)]);
        if !walked.contains(&phi) {
            let arms = function.instruction(phi).operands.clone();
            let arms = arms
                .chunks(2)
                .flat_map(|pair| {
                    if pair[1] == Operand::Block(found.preheader) {
                        [outer_value, Operand::Block(windows)]
                    } else {
                        [pair[0], pair[1]]
                    }
                })
                .collect();
            function.set_operands(phi, arms);
        }
    }
    // The exit is left from the outer latch, with what the header held then;
    // a walk's huge pointer, past its last window. A skipped loop leaves with
    // the starts.
    let test = if found.proof.posttested { found.latch } else { found.header };
    let exit_phis: Vec<InstId> =
        function.iter_block(found.exit).take_while(|&one| function.instruction(one).opcode == Opcode::Phi).collect();
    for phi in exit_phis {
        let mut arms = function.instruction(phi).operands.clone();
        let mut skipped = None;
        for pair in arms.chunks_mut(2) {
            if pair[1] != Operand::Block(test) {
                continue;
            }
            skipped = Some(_entry_value(function, pair[0], &carried, found.preheader));
            pair[1] = Operand::Block(next_window);
            // Tested before each trip, the header held the next trip's values:
            // the latch's.
            if let Some(&(_, ending)) =
                endings.iter().find(|(value, _)| *value == pair[0]).filter(|_| !found.proof.posttested)
            {
                pair[0] = ending;
            }
        }
        if let (Some(_), Some(value)) = (skip, skipped) {
            arms.extend([value, Operand::Block(found.preheader)]);
        }
        function.set_operands(phi, arms);
    }
    if let Some(skip) = skip {
        function
            .set_operands(entering, vec![Operand::Value(skip), Operand::Block(found.exit), Operand::Block(windows)]);
    }
    for walk in found.walks.iter().filter(|walk| !walk.last.is_empty()) {
        let pointer = function.value(walk.recurrence.value).ty;
        let end = stepped_on.iter().find(|(phi, _)| *phi == walk.recurrence.phi).expect("a walk's end").1;
        let (phi, value) = _phi(function, pointer, found.exit, "window.last");
        let mut arms = vec![end, Operand::Block(next_window)];
        if skip.is_some() {
            arms.extend([walk.recurrence.start, Operand::Block(found.preheader)]);
        }
        function.set_operands(phi, arms);
        for &(user, index) in &walk.last {
            function.set_operand(user, index, value);
        }
    }
    _erased(function, found);
}

/// What an exit value is where the loop runs no trip: a header phi's
/// start, or itself where it is not the loop's.
fn _entry_value(
    function: &Function,
    value: Operand,
    carried: &[(InstId, InstId, Operand)],
    preheader: BlockId,
) -> Operand {
    let start_of = |phi: InstId| {
        function
            .instruction(phi)
            .operands
            .chunks(2)
            .find(|pair| pair[1] == Operand::Block(preheader))
            .map(|pair| pair[0])
    };
    let Operand::Value(id) = value else { return value };
    let ValueDef::Instruction(inst) = function.value(id).def else { return value };
    carried.iter().find(|(phi, ..)| *phi == inst).and_then(|&(_, outer, _)| start_of(outer)).unwrap_or(value)
}

/// `value` times the constant `by`, `bits` wide.
fn _scaled(
    context: &Context,
    function: &mut Function,
    value: Operand,
    by: &BigInt,
    bits: u32,
    at: Position,
) -> Operand {
    if *by == BigInt::from(1) {
        return value;
    }
    let ty = context.types.int(bits);
    let by = counting::constant(context, by, bits);
    _placed(function, Opcode::Binary(BinaryOp::Mul), ty, vec![value, by], at)
}

/// `value`, `from` bits wide, as `to` bits.
fn _resized(
    context: &Context,
    function: &mut Function,
    value: Operand,
    from: u32,
    to: u32,
    signed: bool,
    at: Position,
) -> Operand {
    let ty = context.types.int(to);
    match from.cmp(&to) {
        std::cmp::Ordering::Equal => value,
        std::cmp::Ordering::Greater => _placed(function, Opcode::Cast(CastOp::Trunc), ty, vec![value], at),
        std::cmp::Ordering::Less => {
            _placed(function, Opcode::Cast(if signed { CastOp::SExt } else { CastOp::ZExt }), ty, vec![value], at)
        }
    }
}

/// Where `walk` starts a window from its huge pointer `start`: the lowest
/// byte its `trips` (of its huge index's width) reach, normalized, then on
/// to the first trip's lowest byte, far. A walk up reaches its lowest on its
/// first trip; a walk down, on its last.
#[allow(clippy::too_many_arguments)]
fn _windowed(
    context: &Context,
    declared: &mut Declared,
    function: &mut Function,
    layout: &llrm_mir::datalayout::DataLayout,
    far: TypeId,
    walk: &Walk,
    start: Operand,
    trips: Operand,
    at: Position,
) -> Operand {
    let huge = function.value(walk.recurrence.value).ty;
    let Type::Pointer(huge_space) = *context.types.get(huge) else { unreachable!("a pointer recurrence") };
    let Type::Pointer(far_space) = *context.types.get(far) else { unreachable!("a far pointer") };
    let (huge_index, far_index) = (layout.pointer(huge_space).index_bits, layout.pointer(far_space).index_bits);
    let byte = context.types.int(8);
    let gep = Opcode::GetElementPtr { source: byte };
    let step = walk.step();
    // The lowest byte reached, from `start`: `low`, and for a backward walk its
    // trips less one on. Walking down, the far recurrence passes below the
    // last trip's bytes: one step more.
    let lowest = if step < 0 {
        let dword = context.types.int(huge_index);
        let by = counting::constant(context, &BigInt::from(step), huge_index);
        let bytes = _placed(function, Opcode::Binary(BinaryOp::Mul), dword, vec![trips, by], at);
        let low = counting::constant(context, &BigInt::from(walk.low), huge_index);
        Some(_placed(function, Opcode::Binary(BinaryOp::Add), dword, vec![bytes, low], at))
    } else {
        (walk.low != 0).then(|| counting::constant(context, &BigInt::from(walk.low), huge_index))
    };
    let base = lowest.map_or(start, |lowest| _placed(function, gep.clone(), huge, vec![start, lowest], at));
    let (callee, function_type) = _declared(context, declared, far, huge);
    let ty = context.types.ptr(0);
    let callee = Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Global(callee) }));
    let info = CallInfo {
        function_type,
        calling_convention: 0,
        return_attrs: Vec::new(),
        argument_attrs: vec![Vec::new()],
        attrs: Vec::new(),
        tail: Default::default(),
    };
    let origin = _placed(function, Opcode::Call(Box::new(info)), far, vec![base, callee], at);
    // The far recurrence starts at the first trip's lowest byte: the origin
    // itself walking up, its trips on walking down.
    if step > 0 {
        return origin;
    }
    let lowest = _resized(context, function, lowest.expect("a walk down's lowest"), huge_index, far_index, true, at);
    let index = context.types.int(far_index);
    let low = counting::constant(context, &BigInt::from(walk.low), far_index);
    let back = _placed(function, Opcode::Binary(BinaryOp::Sub), index, vec![low, lowest], at);
    _placed(function, gep, far, vec![origin, back], at)
}

/// `walk`'s accesses on a far recurrence from `origin` that follows their
/// lowest byte, entered from `entry`, stepped where the huge one was; its
/// stepped value.
#[allow(clippy::too_many_arguments)]
fn _far(
    context: &Context,
    function: &mut Function,
    walk: &Walk,
    origin: Operand,
    entry: BlockId,
    header: BlockId,
    far: TypeId,
    far_index: u32,
) -> Operand {
    let recurrence = &walk.recurrence;
    let byte = context.types.int(8);
    let gep = Opcode::GetElementPtr { source: byte };
    let (phi, value) = _phi(function, far, header, "window");
    let step = counting::constant(context, &recurrence.step, far_index);
    let next = _placed(function, gep.clone(), far, vec![value, step], Position::Before(recurrence.stepping));
    let arms = function.instruction(recurrence.phi).operands.clone();
    let operands = arms
        .chunks(2)
        .flat_map(|pair| {
            if function.parent(recurrence.stepping).is_some_and(|_| {
                pair[0] == Operand::Value(function.instruction(recurrence.stepping).result.expect("a step"))
            }) {
                [next, pair[1]]
            } else {
                [origin, Operand::Block(entry)]
            }
        })
        .collect();
    function.set_operands(phi, operands);
    for &(user, index, by) in &walk.reads {
        let by = by - walk.low;
        let address = if by == 0 {
            value
        } else {
            let by = counting::constant(context, &BigInt::from(by), far_index);
            _placed(function, gep.clone(), far, vec![value, by], Position::Before(user))
        };
        function.set_operand(user, index, address);
    }
    next
}

/// Each walk's huge recurrence, read by nothing now.
fn _erased(
    function: &mut Function,
    found: &Found,
) {
    for walk in &found.walks {
        // The constant steps its accesses read through, left unread.
        let value = function.instruction(walk.recurrence.phi).result.expect("a phi's value");
        let steps: Vec<InstId> =
            function.users(value).iter().map(|one| one.user).filter(|&user| user != walk.recurrence.stepping).collect();
        for step in steps {
            function.erase(step).expect("an unread step");
        }
        function.set_operands(walk.recurrence.phi, Vec::new());
        function.erase(walk.recurrence.stepping).expect("its phi was emptied");
        function.erase(walk.recurrence.phi).expect("its readers were replaced");
    }
}

/// `Intrinsic::Window` from `huge` pointers to `far` ones, declared where the
/// module has none.
fn _declared(
    context: &Context,
    declared: &mut Declared,
    far: TypeId,
    huge: TypeId,
) -> (GlobalId, TypeId) {
    let ty = context.types.intern(Type::Function { returns: far, parameters: vec![huge], variadic: false });
    (declared.declare(&llrm_mir::intrinsics::window_name(&context.types, far, huge), ty), ty)
}

#[cfg(test)]
#[path = "window_tests.rs"]
mod tests;
