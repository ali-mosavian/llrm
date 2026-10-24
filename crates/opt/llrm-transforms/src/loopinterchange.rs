//! Loop interchange: a loop of a perfect nest swaps places with the loop
//! around it when the dependences allow and the access order says the inner
//! loop should walk the other index (`LoopInterchange.cpp` at `d1106deb71cc`).
//!
//! # How it works
//! Each perfect nest (a chain of loops, each the only child of its parent, two
//! deep or more) is a `LoopList`. The dependence matrix of the whole nest comes
//! from [`DependenceInfo`](llrm_analysis::depanalysis::DependenceInfo): one row
//! for each distinct direction vector of a pair of memory accesses with a
//! write, one column for each loop and a last one saying whether the
//! dependence is known to point forward. Then a bubble sort moves the
//! innermost loop outwards as far as it pays, then the next, and so on
//! (`processLoopList`).
//!
//! A pair of adjacent loops is interchanged when
//! - every row stays lexicographically non-negative with the two columns
//!   swapped (`isLegalToInterChangeLoops`);
//! - the loops hold no instruction that touches memory but a simple load or
//!   store, the header phis of the inner loops are inductions with a step
//!   invariant in the outer loop, or reductions that cross both loops (add,
//!   mul, and/or/xor, min/max, and the float ones; an `ninf` or `nsw`/`nuw` of
//!   one is dropped), the loops are tightly nested (a header, a latch and an
//!   exit block with nothing unsafe), the inner exit condition compares inner
//!   induction values with a value the outer loop leaves invariant, and the
//!   LCSSA phis at the exits are the kind the transform moves
//!   (`LoopInterchangeLegality`);
//! - the rules of `-loop-interchange-profitabilities` say so in order, the
//!   first to decide wins: `instorder` counts the base pointers whose access
//!   steps less in the inner loop than the outer one, against those that step
//!   more, and interchanges when the bad ones outnumber the good; `vectorize`
//!   interchanges when only the inner loop is blocked by dependences
//!   (`LoopInterchangeProfitability`).
//!
//! The transform ([`transform`](crate::loopinterchange_transform)) moves the
//! inner latch's compare and induction updates into a new latch, makes the
//! inner header branch to a single block, moves the inner preheader's code
//! into the outer header, rewires the branches so the old inner loop encloses
//! the old outer one, swaps the contents of the two preheaders, and rebuilds
//! LCSSA.
//!
//! # Deviations from LLVM
//! - D1: `LoopInfo`, the dominator tree and SCEV are made again after each
//!   interchange (`restructureLoops`, `DT->applyUpdates` and `forgetLoop` are
//!   not translated). The pass asks `LoopInfo::analyze` after the branches are
//!   rewired, which gives the nest `restructureLoops` builds.
//! - D2: no `LoopCacheAnalysis`: the `cache` rule answers "cannot tell", as
//!   LLVM's does when the analysis fails; `-cache-line-size` has no use.
//! - D3: no optimization remarks (`ORE`); `LLRM_DEBUG=interchange` says why a
//!   nest is refused.
//! - D4: loops are named by their header block across interchanges, as LLVM
//!   keeps the `Loop *`; a loop's header is the block its phis sit in.
//! - D5: the loop pass manager's loop-simplify and LCSSA come from this pass:
//!   it runs [`loopsimplify`](crate::loopsimplify) and [`lcssa`](crate::lcssa)
//!   first.
//! - D6: DA runs on the `basic-aa` + `tbaa` pipeline, as `looploadelim` does.
//!
//! # Example
//! `a[j][i] = a[j][i] + 1` with `i` the outer loop and `j` the inner: the inner
//! step is the row size, the outer step 4 bytes, so the loops swap and `j`
//! becomes the outer loop.
//!
//! # Runs
//! -O3 and -Omax, once per body, before the vectorizer's rounds.
//!
//! # Needs
//! `depanalysis`, `ivdescriptors`, `scev`.
//!
//! # Keeps
//! Nothing: a change invalidates every analysis.
//!
//! # See also
//! `LoopInterchange.cpp`: `processLoopList` (L751), `populateDependencyMatrix`
//! (L171), `LoopInterchangeLegality::canInterchangeLoops` (L1649),
//! `LoopInterchangeProfitability::isProfitable` (L2007),
//! `LoopInterchangeTransform::transform` (L2237), `adjustLoopBranches` (L2560).

use llrm_analysis::aggregate::AggregateAA;
use llrm_analysis::basicaa::BasicAA;
use llrm_analysis::depanalysis::{DependenceInfo, Options as DaOptions, dir};
use llrm_analysis::ivdescriptors::{self, InductionStep, RecurKind};
use llrm_analysis::memory;
use llrm_analysis::scev::analysis::ScevSession;
use llrm_analysis::scev::backedge::ExitCountKind;
use llrm_analysis::scev::expr::{Kind, Scev};
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_analysis::typebasedaa::TypeBasedAA;
use llrm_mir::loopinfo::Loop;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};
use llrm_mir::valuetracking::guarantee::{may_read_from_memory, may_throw, may_write_to_memory, will_return};
use llrm_support::hash::HashMap;

use crate::loopinterchange_transform::{InnerReduction, Plan, form_lcssa_recursively, transform};

/// `RuleTy`: a profitability rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rule {
    PerLoopCacheAnalysis,
    PerInstrOrderCost,
    ForVectorization,
    Ignore,
}

/// The `cl::opt`s of the pass.
#[derive(Clone, Debug)]
pub struct Options {
    /// `-loop-interchange-threshold`.
    pub cost_threshold: i32,
    /// `-loop-interchange-max-mem-instr-ratio`.
    pub max_mem_instr_ratio: u32,
    /// `-loop-interchange-min-loop-nest-depth`.
    pub min_loop_nest_depth: u32,
    /// `-loop-interchange-max-loop-nest-depth`.
    pub max_loop_nest_depth: u32,
    /// `-loop-interchange-profitabilities`.
    pub profitabilities: Vec<Rule>,
    /// `-loop-interchange-reduction-to-mem`.
    pub reduction_to_mem: bool,
    /// `-da-*`: the options of the dependence analysis.
    pub da: DaOptions,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            cost_threshold: 0,
            max_mem_instr_ratio: 4,
            min_loop_nest_depth: 2,
            max_loop_nest_depth: 10,
            profitabilities: vec![Rule::PerInstrOrderCost, Rule::ForVectorization],
            reduction_to_mem: false,
            da: DaOptions::default(),
        }
    }
}

impl Options {
    /// The `-loop-interchange-*` flags of `arguments`, taken out of it.
    pub fn take_flags(arguments: &mut Vec<String>) -> Result<Self, String> {
        let mut options = Self::default();
        let mut kept = Vec::new();
        for argument in std::mem::take(arguments) {
            let Some(flag) = argument.strip_prefix('-').map(|one| one.trim_start_matches('-')) else {
                kept.push(argument);
                continue;
            };
            let number = |value: &str| value.parse::<i64>().map_err(|_| format!("{argument}: not a number"));
            if let Some(value) = flag.strip_prefix("loop-interchange-threshold=") {
                options.cost_threshold = number(value)? as i32;
            } else if let Some(value) = flag.strip_prefix("loop-interchange-max-mem-instr-ratio=") {
                options.max_mem_instr_ratio = number(value)? as u32;
            } else if let Some(value) = flag.strip_prefix("loop-interchange-min-loop-nest-depth=") {
                options.min_loop_nest_depth = number(value)? as u32;
            } else if let Some(value) = flag.strip_prefix("loop-interchange-max-loop-nest-depth=") {
                options.max_loop_nest_depth = number(value)? as u32;
            } else if let Some(value) = flag.strip_prefix("loop-interchange-profitabilities=") {
                options.profitabilities = value
                    .split(',')
                    .map(|one| match one {
                        "cache" => Ok(Rule::PerLoopCacheAnalysis),
                        "instorder" => Ok(Rule::PerInstrOrderCost),
                        "vectorize" => Ok(Rule::ForVectorization),
                        "ignore" => Ok(Rule::Ignore),
                        other => Err(format!("{argument}: unknown rule {other}")),
                    })
                    .collect::<Result<_, _>>()?;
            } else if flag == "loop-interchange-reduction-to-mem" {
                options.reduction_to_mem = true;
            } else if flag == "da-disable-delinearization-checks" {
                options.da.disable_delinearization_checks = true;
            } else if flag == "da-enable-dependence-test=all" {
                options.da.enable_test = llrm_analysis::depanalysis::TestType::All;
            } else {
                kept.push(argument);
            }
        }
        *arguments = kept;
        Ok(options)
    }
}

pub struct Interchange {
    pub options: Options,
}

impl Default for Interchange {
    fn default() -> Self {
        Self { options: Options::default() }
    }
}

/// A list of direction vectors, one row each; each of N + 1 entries, N the
/// depth of the nest: the first N the direction of each loop, the last
/// whether the entry is a forward dependence (`<`) or not (`*`).
type CharMatrix = Vec<Vec<u8>>;

/// `Instruction::mayHaveSideEffects`.
fn may_have_side_effects(
    cx: &Cx,
    inst: InstId,
) -> bool {
    let query = cx.query(None);
    may_write_to_memory(&query, inst) || may_throw(&query, inst) || !will_return(&query, inst)
}

fn is_load(
    function: &Function,
    inst: InstId,
) -> bool {
    matches!(function.instruction(inst).opcode, Opcode::Load { .. })
}

fn is_store(
    function: &Function,
    inst: InstId,
) -> bool {
    matches!(function.instruction(inst).opcode, Opcode::Store { .. })
}

fn is_simple(
    function: &Function,
    inst: InstId,
) -> bool {
    matches!(
        function.instruction(inst).opcode,
        Opcode::Load { volatile: false, .. } | Opcode::Store { volatile: false, .. }
    )
}

/// `getLoadStorePointerOperand`.
fn pointer_operand(
    function: &Function,
    inst: InstId,
) -> Operand {
    let instruction = function.instruction(inst);
    match instruction.opcode {
        Opcode::Load { .. } => instruction.operands[0],
        _ => instruction.operands[1],
    }
}

/// `populateDependencyMatrix`: the rows for the nest of `outermost`, `level`
/// loops deep; false where legality cannot be proven.
fn populate_dependency_matrix<A: llrm_analysis::aa::AAResults>(
    matrix: &mut CharMatrix,
    level: usize,
    outermost: Loop,
    info: &mut DependenceInfo<A>,
    se: &mut ScalarEvolution,
    cx: &Cx,
    options: &Options,
) -> bool {
    let mut mem_instr: Vec<InstId> = Vec::new();
    let mut num_insts = 0u64;
    for &block in outermost.get_blocks() {
        for inst in cx.function.iter_block(block) {
            num_insts += 1;
            if is_load(cx.function, inst) || is_store(cx.function, inst) {
                if !is_simple(cx.function, inst) {
                    return false;
                }
                mem_instr.push(inst);
            }
        }
    }

    // The tests are O(NumMemInstr^2): a loop of mostly memory instructions is
    // too dear.
    let num_mem = mem_instr.len() as u64;
    if u64::from(options.max_mem_instr_ratio) * num_insts < num_mem * num_mem {
        llrm_support::debug!("interchange", "number of loads/stores exceeded");
        return false;
    }

    // Direction vectors already seen, each to its index in the matrix.
    let mut seen: HashMap<Vec<u8>, usize> = HashMap::default();
    let depth = outermost.get_loop_depth() as usize;
    for (at, &src) in mem_instr.iter().enumerate() {
        for &dst in &mem_instr[at..] {
            // Ignore input dependencies.
            if is_load(cx.function, src) && is_load(cx.function, dst) {
                continue;
            }
            // Track output, flow and anti dependencies.
            let Some(mut d) = info.depends(se, src, dst, false) else { continue };
            // A negative direction vector is made non-negative.
            d.normalize(se, cx);
            let levels = d.levels();
            let mut dep: Vec<u8> = Vec::new();
            for ii in 1..=levels {
                // `LE` is `<` or `=`, for which there is no representation,
                // so it is `*`, and so is `GE`.
                dep.push(match d.direction(ii, false) {
                    dir::LT => b'<',
                    dir::GT => b'>',
                    dir::EQ => b'=',
                    _ => b'*',
                });
            }
            // A dependence with no information is `*` for every loop.
            if d.is_confused() {
                debug_assert!(dep.is_empty());
                dep = vec![b'*'; depth + level - 1];
            }
            while dep.len() < depth + level - 1 {
                dep.push(b'I');
            }
            // DA reports the levels of the whole enclosing nest: keep the
            // suffix of the selected perfect subnest.
            if dep.len() > level {
                dep.drain(..dep.len() - level);
            }
            // When every entry is `*`, legality cannot be proven.
            if dep.iter().all(|&one| one == b'*') {
                llrm_support::debug!("interchange", "all loops have dependencies in all directions");
                return false;
            }

            // Whether the dependence is forward.
            let known_forward = if cx.function.parent(src) != cx.function.parent(dst) {
                // In different blocks the order within one iteration is not
                // known.
                false
            } else {
                // Normalizing reversed it: it is a backward dependence.
                d.src() == src
            };

            // The last entry is assumed forward until a dependence is not.
            dep.push(b'<');
            let key = dep[..dep.len() - 1].to_vec();
            let index = *seen
                .entry(key)
                .or_insert_with(
                    || {
                        matrix.push(dep.clone());
                        matrix.len() - 1
                    },
                );
            // `[... *]` includes `[... <]`: change the entry there is.
            if !known_forward {
                *matrix[index].last_mut().expect("a row") = b'*';
            }
        }
    }
    true
}

/// `interChangeDependencies`: the columns of the two loops swap.
fn interchange_dependencies(
    matrix: &mut CharMatrix,
    from: usize,
    to: usize,
) {
    for row in matrix {
        row.swap(to, from);
    }
}

/// `isLexicographicallyPositive`: true if the vector is positive in
/// `begin..end`, none if it is zero, else false.
fn is_lexicographically_positive(
    dv: &[u8],
    begin: usize,
    end: usize,
) -> Option<bool> {
    for &direction in &dv[begin..end] {
        if direction == b'<' {
            return Some(true);
        }
        if direction == b'>' || direction == b'*' {
            return Some(false);
        }
    }
    None
}

/// `isLegalToInterChangeLoops`.
fn is_legal_to_interchange_loops(
    matrix: &CharMatrix,
    inner: usize,
    outer: usize,
) -> bool {
    for row in matrix {
        let mut cur = row.clone();
        // The surrounding loops already make the vector positive: nothing
        // within can break the dependence.
        if is_lexicographically_positive(&cur, 0, outer) == Some(true) {
            continue;
        }
        // Positive (or zero) before and after exchanging; the last element
        // does not affect legality.
        let end = cur.len() - 1;
        if is_lexicographically_positive(&cur, outer, end) == Some(false) {
            return false;
        }
        cur.swap(inner, outer);
        if is_lexicographically_positive(&cur, outer, end) == Some(false) {
            return false;
        }
    }
    true
}

/// `LoopNest::skipEmptyBlockUntil`.
fn skip_empty_block_until(
    function: &Function,
    from: BlockId,
    end: BlockId,
) -> BlockId {
    let unique_successor = |block: BlockId| {
        let successors = function.successors(block);
        (successors.len() == 1).then(|| successors[0])
    };
    if from == end || unique_successor(from).is_none() {
        return from;
    }
    let is_empty = |block: BlockId| function.block(block).len() == 1;
    let mut visited: Vec<BlockId> = Vec::new();
    let mut block = unique_successor(from);
    let mut pred = from;
    while let Some(current) = block {
        if current == end || !is_empty(current) || visited.contains(&current) {
            break;
        }
        visited.push(current);
        pred = current;
        block = unique_successor(current);
    }
    if block == Some(end) { end } else { pred }
}

/// The only child loop of `l`, none where it has none.
fn child_loop<'a>(l: Loop<'a>) -> Option<Loop<'a>> {
    debug_assert!(l.get_sub_loops().count() <= 1, "Expect at most one child loop for now.");
    l.get_sub_loops().next()
}

/// `followLCSSA`: the value an LCSSA phi (or a chain of them) passes on.
fn follow_lcssa(
    function: &Function,
    mut value: Operand,
) -> Operand {
    loop {
        let Operand::Value(v) = value else { return value };
        let llrm_mir::module::ValueDef::Instruction(inst) = function.value(v).def else { return value };
        let instruction = function.instruction(inst);
        if instruction.opcode != Opcode::Phi || instruction.operands.len() != 2 {
            return value;
        }
        value = instruction.operands[0];
    }
}

fn instruction_of(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(v) = operand else { return None };
    match function.value(v).def {
        llrm_mir::module::ValueDef::Instruction(inst) => Some(inst),
        llrm_mir::module::ValueDef::Argument(_) => None,
    }
}

fn phis(
    function: &Function,
    block: BlockId,
) -> Vec<InstId> {
    function.iter_block(block).take_while(|&inst| function.instruction(inst).opcode == Opcode::Phi).collect()
}

fn phi_incoming(
    function: &Function,
    phi: InstId,
) -> Vec<(Operand, BlockId)> {
    function
        .instruction(phi)
        .operands
        .chunks(2)
        .map(|pair| {
            let Operand::Block(block) = pair[1] else { unreachable!("a phi names blocks") };
            (pair[0], block)
        })
        .collect()
}

fn incoming_for(
    function: &Function,
    phi: InstId,
    block: BlockId,
) -> Option<Operand> {
    phi_incoming(function, phi).into_iter().find(|&(_, from)| from == block).map(|(value, _)| value)
}

/// `Value::users()` of an instruction, as instructions.
fn users_of(
    function: &Function,
    inst: InstId,
) -> Vec<InstId> {
    let Some(result) = function.instruction(inst).result else { return Vec::new() };
    function.users(result).iter().map(|one| one.user).collect()
}

fn is_constant(operand: Operand) -> bool {
    matches!(operand, Operand::Constant(_))
}

/// The coefficient of `l` in an expression: `getAddRecCoefficient`'s
/// `nullptr`, a step, or `nullopt`.
enum Coefficient {
    Invariant,
    Step(Scev),
    Unknown,
}

/// `getAddRecCoefficient`.
fn add_rec_coefficient(
    se: &mut ScalarEvolution,
    cx: &Cx,
    s: Scev,
    l: Loop,
) -> Coefficient {
    let Kind::AddRec(operands, header) = se.node(s).kind.clone() else {
        return if se.is_loop_invariant(cx, s, Some(l.get_header())) {
            Coefficient::Invariant
        } else {
            Coefficient::Unknown
        };
    };
    if operands.len() != 2 {
        return Coefficient::Unknown;
    }
    let coefficient = add_rec_coefficient(se, cx, operands[0].scev, l);
    if matches!(coefficient, Coefficient::Unknown) {
        return Coefficient::Unknown;
    }
    if header == l.get_header() {
        debug_assert!(matches!(coefficient, Coefficient::Invariant), "more than one addrec for the same loop");
        return Coefficient::Step(se.get_step_recurrence(cx, s));
    }
    coefficient
}

/// What `canInterchangeLoops` found, for the transform.
struct Legality {
    outer_inner_reductions: Vec<InstId>,
    inner_loop_inductions: Vec<InstId>,
    has_no_wrap_reductions: Vec<InstId>,
    has_no_inf_insts: Vec<InstId>,
    inner_reductions: Vec<InnerReduction>,
}

/// `LoopInterchangeLegality`.
struct Checker<'a, 'b> {
    cx: &'a Cx<'b>,
    options: &'a Options,
    outer: Loop<'a>,
    inner: Loop<'a>,
    found: Legality,
}

impl<'a, 'b> Checker<'a, 'b> {
    fn function(&self) -> &'b Function {
        self.cx.function
    }

    /// `containsUnsafeInstructions`.
    fn contains_unsafe_instructions(
        &self,
        block: BlockId,
        skip: Option<InstId>,
    ) -> bool {
        let query = self.cx.query(None);
        self.function()
            .iter_block(block)
            .any(
                |inst| {
                    if Some(inst) == skip {
                        return false;
                    }
                    may_have_side_effects(self.cx, inst) || may_read_from_memory(&query, inst)
                },
            )
    }

    /// `tightlyNested`.
    fn tightly_nested(&self) -> bool {
        let function = self.function();
        let outer_header = self.outer.get_header();
        let Some(inner_preheader) = self.inner.get_loop_preheader(function) else { return false };
        let Some(outer_latch) = self.outer.get_loop_latch(function) else { return false };

        // A perfect nest's outer header branches only into the inner loop. If
        // it can also reach the outer latch, it guards the inner loop, which
        // then runs on a subset of the outer iterations.
        for succ in function.successors(outer_header) {
            if succ != inner_preheader && succ != self.inner.get_header() {
                return false;
            }
        }

        // When the inner reduction is stored in the outer latch, skip that
        // store.
        debug_assert!(self.found.inner_reductions.len() <= 1, "So far we only support at most one reduction.");
        let skip = self.found.inner_reductions.first().map(|one| one.lcssa_store);

        // The outer header and latch hold no unsafe instruction.
        if self.contains_unsafe_instructions(outer_header, skip) || self.contains_unsafe_instructions(outer_latch, skip)
        {
            return false;
        }
        // All the inner preheader's instructions move to the outer header.
        if inner_preheader != outer_header && self.contains_unsafe_instructions(inner_preheader, skip) {
            return false;
        }

        let Some(inner_exit) = self.inner.get_exit_block(function) else { return false };
        // The inner exit flows to the outer latch, possibly through empty
        // blocks.
        if skip_empty_block_until(function, inner_exit, outer_latch) != outer_latch {
            llrm_support::debug!("interchange", "the inner exit does not lead to the outer latch");
            return false;
        }
        // It moves into the new inner loop: it holds only safe instructions.
        !self.contains_unsafe_instructions(inner_exit, skip)
    }

    /// `isLoopStructureUnderstood`.
    fn is_loop_structure_understood(
        &self,
        se: &mut ScalarEvolution,
    ) -> bool {
        let function = self.function();
        let inner_preheader = self.inner.get_loop_preheader(function);
        for &induction in &self.found.inner_loop_inductions {
            for (value, from) in phi_incoming(function, induction) {
                if is_constant(value) {
                    continue;
                }
                let Some(inst) = instruction_of(function, value) else { return false };
                // TODO: triangular loops: `for (j = i; ...)`.
                if Some(from) == inner_preheader
                    && !self.outer.is_loop_invariant(
                        function,
                        Operand::Value(function.instruction(inst).result.expect("a value")),
                    )
                {
                    return false;
                }
            }
        }

        // TODO: triangular loops of another form: `j < i`, `j * i < N`.
        let Some(inner_latch) = self.inner.get_loop_latch(function) else { return false };
        let Some(terminator) = function.terminator(inner_latch) else { return false };
        let branch = function.instruction(terminator);
        if branch.opcode != Opcode::Br || branch.operands.len() != 3 {
            return false;
        }
        let Some(cmp) = instruction_of(function, branch.operands[0]) else { return false };
        if !matches!(
            function.instruction(cmp).opcode,
            Opcode::ICmp(_) | Opcode::FCmp(_)
        ) {
            return false;
        }
        let (op0, op1) = (function.instruction(cmp).operands[0], function.instruction(cmp).operands[1]);

        // `V` involves only the inner induction variable: it is one, a
        // constant, a cast of such, or a binary operator on such.
        fn is_path_to_inner_ind_var(
            function: &Function,
            inductions: &[InstId],
            value: Operand,
        ) -> bool {
            if is_constant(value) {
                return true;
            }
            let Some(inst) = instruction_of(function, value) else { return false };
            if inductions.contains(&inst) {
                return true;
            }
            let instruction = function.instruction(inst);
            match instruction.opcode {
                Opcode::Cast(_) => is_path_to_inner_ind_var(function, inductions, instruction.operands[0]),
                Opcode::Binary(_) => {
                    is_path_to_inner_ind_var(function, inductions, instruction.operands[0])
                        && is_path_to_inner_ind_var(function, inductions, instruction.operands[1])
                }
                _ => false,
            }
        }
        let path = |value| is_path_to_inner_ind_var(function, &self.found.inner_loop_inductions, value);
        if path(op0) && path(op1) {
            return true;
        }

        // Otherwise an inner indvar related value (Left) is compared with an
        // outer loop invariant (Right).
        let right = if path(op0) && !is_constant(op0) {
            op1
        } else if path(op1) && !is_constant(op1) {
            op0
        } else {
            return false;
        };
        let s = se.get_scev(self.cx, right);
        se.is_loop_invariant(self.cx, s, Some(self.outer.get_header()))
    }

    /// `checkReductionKind`.
    fn check_reduction_kind(
        &self,
        l: Loop,
        phi: InstId,
        has_no_wrap: &mut Vec<InstId>,
        has_no_inf: &mut Vec<InstId>,
    ) -> bool {
        let function = self.function();
        // Reductions of the inner loop are found with no SCEV, as the pin's
        // `isReductionPHI(PHI, L, RD)` does.
        let Some(rd) = ivdescriptors::is_reduction_phi(self.cx, None, phi, l) else { return false };
        // A floating point reduction only where it can be reordered.
        if rd.exact_fp_math_inst().is_some() {
            return false;
        }
        // Extra uses of the phi outside its chain make the order the elements
        // are visited in observable.
        if rd.has_uses_outside_reduction_chain() {
            return false;
        }
        match rd.recurrence_kind() {
            RecurKind::Or
            | RecurKind::And
            | RecurKind::Xor
            | RecurKind::SMin
            | RecurKind::SMax
            | RecurKind::UMin
            | RecurKind::UMax => true,
            // An AnyOf whose result is not invariant in the outer loop changes
            // with the order.
            RecurKind::AnyOf => false,
            // Reordering floating-point operations may change the results; an
            // `ninf` makes the reordering able to produce poison, so it is
            // dropped.
            RecurKind::FAdd
            | RecurKind::FMul
            | RecurKind::FMin
            | RecurKind::FMax
            | RecurKind::FMinimum
            | RecurKind::FMaximum
            | RecurKind::FMinimumNum
            | RecurKind::FMaximumNum
            | RecurKind::FMulAdd => {
                for inst in rd.reduction_op_chain(self.cx, phi, l) {
                    let instruction = function.instruction(inst);
                    if is_fp_math_operator(self.cx, inst) && instruction.flags.contains(Flags::NINF) {
                        has_no_inf.push(inst);
                    }
                }
                true
            }
            // Reordering integer additions or multiplications may overflow:
            // the `nuw`/`nsw` are dropped.
            RecurKind::Add | RecurKind::Mul => {
                let ops = rd.reduction_op_chain(self.cx, phi, l);
                // Bail out when the chain is not collected.
                if ops.is_empty() {
                    return false;
                }
                for inst in ops {
                    let flags = function.instruction(inst).flags;
                    if flags.contains(Flags::NSW) || flags.contains(Flags::NUW) {
                        has_no_wrap.push(inst);
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// `findInnerReductionPhi`: a user of `value` that is a reduction in `l`.
    fn find_inner_reduction_phi(
        &self,
        l: Loop,
        value: Operand,
        has_no_wrap: &mut Vec<InstId>,
        has_no_inf: &mut Vec<InstId>,
    ) -> Option<InstId> {
        // Reduction variables cannot be constants.
        if is_constant(value) {
            return None;
        }
        let function = self.function();
        let inst = instruction_of(function, value)?;
        for user in users_of(function, inst) {
            if function.instruction(user).opcode == Opcode::Phi {
                if function.instruction(user).operands.len() == 2 {
                    continue;
                }
                return self.check_reduction_kind(l, user, has_no_wrap, has_no_inf).then_some(user);
            }
        }
        None
    }

    /// `isInnerReduction`.
    fn is_inner_reduction(
        &mut self,
        l: Loop,
        phi: InstId,
        has_no_wrap: &mut Vec<InstId>,
        has_no_inf: &mut Vec<InstId>,
    ) -> bool {
        let function = self.function();
        // Only the innermost loop of the two innermost.
        if !l.is_innermost() {
            return false;
        }
        if function.instruction(phi).operands.len() != 4 {
            return false;
        }
        let (Some(preheader), Some(latch)) = (l.get_loop_preheader(function), l.get_loop_latch(function)) else {
            return false;
        };
        let (Some(init), Some(next)) = (incoming_for(function, phi, preheader), incoming_for(function, phi, latch))
        else {
            return false;
        };
        // So far only a constant initial value.
        if !is_constant(init) {
            return false;
        }
        // The reduction result lives in the inner loop.
        if let Some(inst) = instruction_of(function, next)
            && !l.contains_instruction(function, inst)
        {
            return false;
        }
        // One user.
        if users_of(function, phi).len() != 1 {
            return false;
        }
        if !self.check_reduction_kind(l, phi, has_no_wrap, has_no_inf) {
            return false;
        }
        // The LCSSA phi in the outer loop's latch.
        let Some(exit_block) = l.get_exit_block(function) else { return false };
        let Some(next_inst) = instruction_of(function, next) else { return false };
        let mut lcssa: Option<InstId> = None;
        for user in users_of(function, next_inst) {
            if function.instruction(user).opcode != Opcode::Phi {
                return false;
            }
            if user == phi {
                continue;
            }
            if lcssa.is_none()
                && function.parent(user) == Some(exit_block)
                && incoming_for(function, user, latch) == Some(next)
            {
                lcssa = Some(user);
            } else {
                return false;
            }
        }
        let Some(lcssa) = lcssa else { return false };
        let lcssa_users = users_of(function, lcssa);
        if lcssa_users.len() != 1 {
            return false;
        }
        let store = lcssa_users[0];
        if !is_store(function, store) || function.parent(store) != Some(exit_block) {
            return false;
        }
        let mem_ref = function.instruction(store).operands[1];
        let stored = function.instruction(store).operands[0];
        let elem_ty = function.operand_type(self.cx.context, stored).expect("a typed value");
        // The memory reference is read in the inner loop's header: it dominates
        // it.
        if let Some(def) = instruction_of(function, mem_ref) {
            let def_block = function.parent(def).expect("a placed instruction");
            if def_block == l.get_header() || !self.cx.tree.dominates(def_block, l.get_header()) {
                return false;
            }
        }
        self.found
            .inner_reductions
            .push(
                InnerReduction {
                    reduction: phi,
                    init,
                    next,
                    lcssa_phi: lcssa,
                    lcssa_store: store,
                    mem_ref,
                    elem_ty,
                },
            );
        true
    }

    /// `checkInductionsAndReductions`.
    fn check_inductions_and_reductions(
        &mut self,
        se: &mut ScalarEvolution,
    ) -> bool {
        let function = self.function();
        let inner_loop = child_loop(self.outer);
        let mut cur_loop = Some(self.outer);
        while let Some(cur) = cur_loop {
            for phi in phis(function, cur.get_header()) {
                if let Some(id) = ivdescriptors::is_induction_phi(self.cx, se, phi, cur) {
                    if Some(cur) == inner_loop {
                        let InductionStep::Scev(step) = id.step else { unreachable!("an integer step") };
                        if !se.is_loop_invariant(self.cx, step, Some(self.outer.get_header())) {
                            return false;
                        }
                        self.found.inner_loop_inductions.push(phi);
                    }
                    continue;
                }

                if cur == self.outer {
                    // A phi of the outer loop must be a reduction across both
                    // loops.
                    if function.instruction(phi).operands.len() != 4 {
                        return false;
                    }
                    // The phi has the inner loop's reduction result as an
                    // incoming value.
                    let Some(outer_latch) = self.outer.get_loop_latch(function) else { return false };
                    let Some(incoming) = incoming_for(function, phi, outer_latch) else { return false };
                    let value = follow_lcssa(function, incoming);
                    let Some(inner) = inner_loop else { return false };
                    let mut no_wrap = std::mem::take(&mut self.found.has_no_wrap_reductions);
                    let mut no_inf = std::mem::take(&mut self.found.has_no_inf_insts);
                    let inner_red_phi = self.find_inner_reduction_phi(inner, value, &mut no_wrap, &mut no_inf);
                    self.found.has_no_wrap_reductions = no_wrap;
                    self.found.has_no_inf_insts = no_inf;

                    // The phi has no user but the inner reduction phi.
                    let phi_value = Operand::Value(function.instruction(phi).result.expect("a phi"));
                    let Some(inner_red_phi) = inner_red_phi else {
                        llrm_support::debug!("interchange", "failed to recognize a phi as an induction or reduction");
                        return false;
                    };
                    if !phi_incoming(function, inner_red_phi).iter().any(|&(one, _)| one == phi_value)
                        || !users_of(function, phi).iter().all(|&user| user == inner_red_phi)
                    {
                        llrm_support::debug!("interchange", "failed to recognize a phi as an induction or reduction");
                        return false;
                    }
                    self.found.outer_inner_reductions.push(phi);
                    self.found.outer_inner_reductions.push(inner_red_phi);
                } else if self.found.outer_inner_reductions.contains(&phi) {
                    // A reduction across the outer loop.
                } else if self.options.reduction_to_mem && {
                    let mut no_wrap = std::mem::take(&mut self.found.has_no_wrap_reductions);
                    let mut no_inf = std::mem::take(&mut self.found.has_no_inf_insts);
                    let found = self.is_inner_reduction(cur, phi, &mut no_wrap, &mut no_inf);
                    self.found.has_no_wrap_reductions = no_wrap;
                    self.found.has_no_inf_insts = no_inf;
                    found
                } {
                    // A reduction in the inner loop.
                } else {
                    return false;
                }
            }

            // So far only one reduction.
            if self.found.inner_reductions.len() > 1 {
                return false;
            }
            cur_loop = child_loop(cur);
        }
        !self.found.inner_loop_inductions.is_empty()
    }

    /// `currentLimitations`: whether the transform cannot do this nest.
    fn current_limitations(
        &self,
        se: &mut ScalarEvolution,
    ) -> bool {
        let function = self.function();
        let Some(inner_latch) = self.inner.get_loop_latch(function) else { return true };
        let Some(outer_latch) = self.outer.get_loop_latch(function) else { return true };
        let is_conditional = |block: BlockId| {
            function
                .terminator(block)
                .is_some_and(
                    |term| {
                        let instruction = function.instruction(term);
                        instruction.opcode == Opcode::Br && instruction.operands.len() == 3
                    },
                )
        };
        // The transform expects the latches to be the exiting blocks.
        if self.inner.get_exiting_block(function) != Some(inner_latch)
            || self.outer.get_exiting_block(function) != Some(outer_latch)
            || !is_conditional(inner_latch)
            || !is_conditional(outer_latch)
        {
            llrm_support::debug!("interchange", "a latch is not the exiting block");
            return true;
        }

        // TODO: triangular loops.
        if !self.is_loop_structure_understood(se) {
            llrm_support::debug!("interchange", "the loop structure is not understood");
            return true;
        }

        // The inner loop header has no duplicate successors.
        let header = self.inner.get_header();
        let term = function.terminator(header).expect("a terminated header");
        let mut seen: Vec<BlockId> = Vec::new();
        for operand in &function.instruction(term).operands {
            if let Operand::Block(succ) = operand {
                if seen.contains(succ) {
                    return true;
                }
                seen.push(*succ);
            }
        }
        false
    }

    /// `areInnerLoopExitPHIsSupported`: the LCSSA phis of the inner exit have
    /// users that are reduction phis, phis outside the outer loop, or phis of
    /// the outer latch: only the final value after the inner loop matters.
    fn are_inner_loop_exit_phis_supported(
        &self,
        lcssa_reduction: Option<InstId>,
    ) -> bool {
        let function = self.function();
        let Some(inner_exit) = self.inner.get_unique_exit_block(function) else { return false };
        let outer_latch = self.outer.get_loop_latch(function);
        for phi in phis(function, inner_exit) {
            // The reduction LCSSA phi has one incoming block, the loop latch.
            if function.instruction(phi).operands.len() > 2 {
                return false;
            }
            // Its store user is rewritten by `reduction2Memory`.
            if Some(phi) == lcssa_reduction {
                continue;
            }
            let bad = users_of(function, phi)
                .into_iter()
                .any(
                    |user| {
                        if function.instruction(user).opcode != Opcode::Phi {
                            return true;
                        }
                        if self.found.outer_inner_reductions.contains(&user) {
                            return false;
                        }
                        let parent = function.parent(user).expect("a placed user");
                        if !self.outer.contains(parent) {
                            return false;
                        }
                        Some(parent) != outer_latch
                    },
                );
            if bad {
                return false;
            }
        }
        true
    }

    /// `areOuterLoopExitPHIsSupported`: the LCSSA phis of the nest's exit read
    /// the outer latch only where it has a single predecessor.
    fn are_outer_loop_exit_phis_supported(&self) -> bool {
        let function = self.function();
        let Some(nest_exit) = self.outer.get_unique_exit_block(function) else { return false };
        let Some(outer_latch) = self.outer.get_loop_latch(function) else { return false };
        for phi in phis(function, nest_exit) {
            for (incoming, _) in phi_incoming(function, phi) {
                let Some(inst) = instruction_of(function, incoming) else { continue };
                if function.parent(inst) != Some(outer_latch) {
                    continue;
                }
                // Defined in the outer latch: only if it has one
                // predecessor, so it runs if and only if the inner loop does.
                if function.sole_predecessor(outer_latch).is_none() {
                    return false;
                }
            }
        }
        true
    }

    /// `areInnerLoopLatchPHIsSupported`: no phi but the inductions feeds the
    /// instructions the transform clones into the new latch.
    fn are_inner_loop_latch_phis_supported(&self) -> bool {
        let function = self.function();
        let Some(inner_latch) = self.inner.get_loop_latch(function) else { return false };
        let inductions = &self.found.inner_loop_inductions;
        let mut worklist: Vec<InstId> = Vec::new();
        let push = |worklist: &mut Vec<InstId>, inst: InstId| {
            if !worklist.contains(&inst) {
                worklist.push(inst);
            }
        };
        if let Some(term) = function.terminator(inner_latch) {
            let instruction = function.instruction(term);
            if instruction.opcode == Opcode::Br
                && instruction.operands.len() == 3
                && let Some(cond) = instruction_of(function, instruction.operands[0])
            {
                push(&mut worklist, cond);
            }
        }
        for &induction in inductions {
            if let Some(incoming) = incoming_for(function, induction, inner_latch)
                && let Some(inst) = instruction_of(function, incoming)
                && !inductions.contains(&inst)
            {
                push(&mut worklist, inst);
            }
        }
        let mut at = 0;
        while at < worklist.len() {
            let cur = worklist[at];
            at += 1;
            if function.instruction(cur).opcode == Opcode::Phi && !inductions.contains(&cur) {
                return false;
            }
            for operand in &function.instruction(cur).operands {
                if let Some(inst) = instruction_of(function, *operand)
                    && self.inner.contains_instruction(function, inst)
                {
                    push(&mut worklist, inst);
                }
            }
        }
        true
    }

    /// `findFreezeInReNestedBlocks`.
    fn find_freeze_in_re_nested_blocks(&self) -> bool {
        let function = self.function();
        // `adjustLoopBranches` swaps the preheader bodies after changing their
        // loop roles, so the original outer-preheader body stays outside the
        // new outer loop.
        let blocks = [
            Some(self.outer.get_header()),
            self.outer.get_loop_latch(function),
            self.inner.get_loop_preheader(function),
            self.inner.get_exit_block(function),
        ];
        blocks
            .into_iter()
            .flatten()
            .any(|block| function.iter_block(block).any(|inst| function.instruction(inst).opcode == Opcode::Freeze))
    }

    /// `findFreezeInInnerLatchCloneSet`: a freeze among the instructions the
    /// transform clones into the new inner latch.
    fn find_freeze_in_inner_latch_clone_set(&self) -> bool {
        let function = self.function();
        let inductions = &self.found.inner_loop_inductions;
        let Some(inner_latch) = self.inner.get_loop_latch(function) else { return false };
        let is_direct_inner_loop_block =
            |block: BlockId| self.inner.contains(block) && !self.inner.get_sub_loops().any(|sub| sub.contains(block));
        let mut worklist: Vec<InstId> = Vec::new();
        if let Some(term) = function.terminator(inner_latch) {
            let instruction = function.instruction(term);
            if instruction.opcode == Opcode::Br
                && instruction.operands.len() == 3
                && let Some(cond) = instruction_of(function, instruction.operands[0])
            {
                worklist.push(cond);
            }
        }
        for &induction in inductions {
            if let Some(incoming) = incoming_for(function, induction, inner_latch)
                && let Some(inst) = instruction_of(function, incoming)
                && !inductions.contains(&inst)
                && !worklist.contains(&inst)
            {
                worklist.push(inst);
            }
        }
        let mut at = 0;
        while at < worklist.len() {
            let cur = worklist[at];
            at += 1;
            if function.instruction(cur).opcode == Opcode::Freeze {
                return true;
            }
            for operand in &function.instruction(cur).operands {
                let Some(inst) = instruction_of(function, *operand) else { continue };
                if !is_direct_inner_loop_block(function.parent(inst).expect("a placed instruction"))
                    || inductions.contains(&inst)
                    || worklist.contains(&inst)
                {
                    continue;
                }
                worklist.push(inst);
            }
        }
        false
    }

    /// `canInterchangeLoops`.
    fn can_interchange_loops(
        &mut self,
        se: &mut ScalarEvolution,
        inner_id: usize,
        outer_id: usize,
        matrix: &CharMatrix,
    ) -> bool {
        let function = self.function();
        if !is_legal_to_interchange_loops(matrix, inner_id, outer_id) {
            llrm_support::debug!("interchange", "cannot interchange: dependences");
            return false;
        }
        // The outer loop holds only legal instructions: loads and stores are
        // checked apart.
        let query = self.cx.query(None);
        for &block in self.outer.get_blocks() {
            for inst in function.iter_block(block) {
                if is_load(function, inst) || is_store(function, inst) {
                    continue;
                }
                // A call may read memory.
                if !may_have_side_effects(self.cx, inst) && !may_read_from_memory(&query, inst) {
                    continue;
                }
                llrm_support::debug!("interchange", "instructions that cannot be safely interchanged");
                return false;
            }
        }

        if !self.check_inductions_and_reductions(se) {
            llrm_support::debug!("interchange", "no inner induction or an unsupported reduction");
            return false;
        }
        if !self.are_inner_loop_latch_phis_supported() {
            llrm_support::debug!("interchange", "unsupported phis in the inner latch");
            return false;
        }
        if self.find_freeze_in_re_nested_blocks() || self.find_freeze_in_inner_latch_clone_set() {
            llrm_support::debug!("interchange", "a freeze would be re-nested or duplicated");
            return false;
        }
        if self.current_limitations(se) {
            llrm_support::debug!("interchange", "a limitation of the transform");
            return false;
        }
        if !self.tightly_nested() {
            llrm_support::debug!("interchange", "not tightly nested");
            return false;
        }

        // The reduction's LCSSA phi passed the checks: its user is a store.
        debug_assert!(self.found.inner_reductions.len() <= 1, "So far we only support at most one reduction.");
        let lcssa_reduction = self.found.inner_reductions.first().map(|one| one.lcssa_phi);
        if !self.are_inner_loop_exit_phis_supported(lcssa_reduction) {
            llrm_support::debug!("interchange", "unsupported phis in the inner loop exit");
            return false;
        }
        if !self.are_outer_loop_exit_phis_supported() {
            llrm_support::debug!("interchange", "unsupported phis in the outer loop exit");
            return false;
        }
        let Some(outer_latch) = self.outer.get_loop_latch(function) else { return false };
        if phis(function, outer_latch).into_iter().any(|phi| function.instruction(phi).operands.len() != 2) {
            llrm_support::debug!("interchange", "outer latch phis with several incoming values");
            return false;
        }

        // From an LCSSA phi of the inner exit to the outer latch only phis
        // are supported; `tightlyNested` leaves any other user in the latch.
        if self.inner.get_exit_block(function) != Some(outer_latch) {
            for phi in phis(function, outer_latch) {
                if users_of(function, phi).into_iter().any(|user| function.instruction(user).opcode != Opcode::Phi) {
                    llrm_support::debug!("interchange", "an outer latch phi has a non-phi user");
                    return false;
                }
            }
        }
        true
    }
}

/// `isa<FPMathOperator>`.
fn is_fp_math_operator(
    cx: &Cx,
    inst: InstId,
) -> bool {
    let instruction = cx.function.instruction(inst);
    let float = |ty| matches!(cx.context.types.get(ty), llrm_mir::types::Type::Float(_));
    match instruction.opcode {
        Opcode::Binary(op) => {
            matches!(
                op,
                BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul | BinaryOp::FDiv | BinaryOp::FRem
            )
        }
        Opcode::FNeg | Opcode::FCmp(_) | Opcode::Select | Opcode::Phi | Opcode::Call(_) => float(instruction.ty),
        _ => false,
    }
}

/// `getInstrOrderCost`.
fn instr_order_cost(
    se: &mut ScalarEvolution,
    cx: &Cx,
    outer: Loop,
    inner: Loop,
) -> i32 {
    let mut good: Vec<Scev> = Vec::new();
    let mut bad: Vec<Scev> = Vec::new();
    for &block in inner.get_blocks() {
        for inst in cx.function.iter_block(block) {
            if !is_load(cx.function, inst) && !is_store(cx.function, inst) {
                continue;
            }
            let access = se.get_scev(cx, pointer_operand(cx.function, inst));
            let base = se.get_pointer_base(access);
            let outer_coefficient = add_rec_coefficient(se, cx, access, outer);
            let inner_coefficient = add_rec_coefficient(se, cx, access, inner);
            let (Coefficient::Step(outer_coefficient), Coefficient::Step(inner_coefficient)) =
                (outer_coefficient, inner_coefficient)
            else {
                continue;
            };
            // A smaller step suggests the induction variable indexes the
            // inner dimension of the array: the loop is better inner.
            let outer_step = se.get_abs_expr(cx, outer_coefficient, false);
            let inner_step = se.get_abs_expr(cx, inner_coefficient, false);
            // Base pointers are counted once.
            if se.is_known_predicate_scev(cx, IntPredicate::Slt, inner_step, outer_step) {
                if !good.contains(&base) {
                    good.push(base);
                }
            } else if se.is_known_predicate_scev(cx, IntPredicate::Slt, outer_step, inner_step) && !bad.contains(&base)
            {
                bad.push(base);
            }
        }
    }
    good.len() as i32 - bad.len() as i32
}

/// `canVectorize`: whether the loop `id` has no dependence it carries
/// backward or unknown.
fn can_vectorize(
    matrix: &CharMatrix,
    id: usize,
) -> bool {
    for dep in matrix {
        let direction = dep[id];
        let dep_type = *dep.last().expect("a row");
        debug_assert!(dep_type == b'<' || dep_type == b'*', "Unexpected element in dependency vector");
        // No loop-carried dependence.
        if direction == b'=' || direction == b'I' {
            continue;
        }
        // A forward dependence can still be vectorized.
        if direction == b'<' && dep_type == b'<' {
            continue;
        }
        return false;
    }
    true
}

/// `isProfitableForVectorization`.
fn is_profitable_for_vectorization(
    inner_id: usize,
    outer_id: usize,
    matrix: &CharMatrix,
) -> Option<bool> {
    // The outer loop cannot be vectorized: no point making it inner.
    if !can_vectorize(matrix, outer_id) {
        return Some(false);
    }
    // Only the outer loop can be vectorized: interchange to enable it.
    if !can_vectorize(matrix, inner_id) {
        return Some(true);
    }
    // Both can: there is no cost model to tell.
    None
}

/// `LoopInterchangeProfitability::isProfitable`.
fn is_profitable(
    se: &mut ScalarEvolution,
    cx: &Cx,
    options: &Options,
    outer: Loop,
    inner: Loop,
    inner_id: usize,
    outer_id: usize,
    matrix: &CharMatrix,
) -> bool {
    // A loop whose backedge is not taken (an unconditional branch) is no
    // candidate.
    for l in [inner, outer] {
        let btc = se.get_backedge_taken_count(cx, l.get_header(), ExitCountKind::Exact);
        if se.as_constant(btc).is_some_and(|value| value.is_zero()) {
            llrm_support::debug!("interchange", "a back-edge that is not taken, rejecting");
            return false;
        }
    }

    // Interchange is forced.
    if options.profitabilities.len() == 1 && options.profitabilities[0] == Rule::Ignore {
        return true;
    }

    // The first rule that can decide does; the others are not asked.
    let mut should_interchange: Option<bool> = None;
    for rule in &options.profitabilities {
        should_interchange = match rule {
            // D2: LoopCacheAnalysis fails, so no cost map and no answer.
            Rule::PerLoopCacheAnalysis => None,
            Rule::PerInstrOrderCost => {
                // The legacy cost model: bad orders against good ones.
                let cost = instr_order_cost(se, cx, outer, inner);
                llrm_support::debug!("interchange", "cost = {cost}");
                (cost < 0 && cost < options.cost_threshold).then_some(true)
            }
            Rule::ForVectorization => is_profitable_for_vectorization(inner_id, outer_id, matrix),
            Rule::Ignore => unreachable!("Option 'ignore' is not supported with other options"),
        };
        if should_interchange.is_some() {
            break;
        }
    }
    should_interchange.unwrap_or(false)
}

/// `isComputableLoopNest`.
fn is_computable_loop_nest(
    se: &mut ScalarEvolution,
    cx: &Cx,
    list: &[BlockId],
) -> bool {
    for &header in list {
        let l = cx.loops.get_loop_for(header).expect("a loop header");
        let exit_count = se.get_backedge_taken_count(cx, header, ExitCountKind::Exact);
        if se.is_could_not_compute(exit_count) {
            return false;
        }
        if l.get_num_back_edges(cx.function) != 1 {
            return false;
        }
        if l.get_exiting_block(cx.function).is_none() {
            return false;
        }
    }
    true
}

/// `collectPerfectNests`: for each innermost loop of the nest of `top`, the
/// chain of loops that are each the only child of their parent, outermost
/// first, if it is two or more deep.
fn collect_perfect_nests(top: Loop) -> Vec<Vec<BlockId>> {
    let mut lists = Vec::new();
    let mut loops = vec![top];
    top.get_inner_loops_in_preorder(&mut loops);
    for l in loops {
        if !l.is_innermost() {
            continue;
        }
        let mut list = Vec::new();
        let mut current = l;
        loop {
            list.push(current.get_header());
            let Some(parent) = current.get_parent_loop() else { break };
            if parent.get_sub_loops().count() != 1 {
                break;
            }
            current = parent;
        }
        list.reverse();
        if list.len() >= 2 {
            lists.push(list);
        }
    }
    lists
}

/// What the pass reads and edits.
struct Run<'a, 'u> {
    unit: &'a mut passes::Unit<'u>,
    outer: std::rc::Rc<passes::Outer>,
    options: &'a Options,
    session: ScevSession,
}

impl Run<'_, '_> {
    /// The analyses again, after the function changed.
    fn refresh(&mut self) {
        self.session = ScevSession::of(self.unit.context, self.unit.function, &self.outer);
    }

    /// `processLoopList`.
    fn process_loop_list(
        &mut self,
        list: &mut [BlockId],
    ) -> bool {
        let mut changed = false;
        let depth = list.len();
        let mut matrix: CharMatrix = Vec::new();

        // The dependence matrix of the whole nest.
        {
            let (se, env) = self.session.split(self.unit.layout, &self.outer.globals, &self.outer.metadata);
            let cx = env.cx(&*self.unit.context, &*self.unit.function);
            let outermost = cx.loops.get_loop_for(list[0]).expect("a loop");
            let memory_unit =
                memory::Unit::within(&*self.unit.context, self.unit.layout, &*self.unit.function, &self.outer);
            let mut aa = AggregateAA::new();
            aa.add_aa_result(BasicAA::new(&memory_unit));
            aa.add_aa_result(TypeBasedAA::new(&memory_unit));
            let mut info = DependenceInfo::new(&mut aa, &memory_unit, &cx, self.options.da);
            if !populate_dependency_matrix(&mut matrix, depth, outermost, &mut info, se, &cx, self.options) {
                llrm_support::debug!("interchange", "populating the dependency matrix failed");
                return false;
            }
            // The nest needs a unique exit block.
            if outermost.get_exit_block(cx.function).is_none() {
                llrm_support::debug!("interchange", "the outermost loop needs a unique exit block");
                return false;
            }
        }

        // A bubble sort: from the innermost loop, move it outwards to the best
        // position, and repeat.
        let selected = depth - 1;
        for j in (1..=selected).rev() {
            let mut changed_per_iter = false;
            for i in ((selected - j + 1)..=selected).rev() {
                let interchanged = self.process_loop(list, i, i - 1, &mut matrix);
                changed_per_iter |= interchanged;
                changed |= interchanged;
            }
            // No interchange in a whole round: stop.
            if !changed_per_iter {
                break;
            }
        }
        changed
    }

    /// `processLoop`.
    fn process_loop(
        &mut self,
        list: &mut [BlockId],
        inner_id: usize,
        outer_id: usize,
        matrix: &mut CharMatrix,
    ) -> bool {
        let plan = {
            let (se, env) = self.session.split(self.unit.layout, &self.outer.globals, &self.outer.metadata);
            let cx = env.cx(&*self.unit.context, &*self.unit.function);
            let outer = cx.loops.get_loop_for(list[outer_id]).expect("a loop");
            let inner = cx.loops.get_loop_for(list[inner_id]).expect("a loop");
            let mut checker = Checker {
                cx: &cx,
                options: self.options,
                outer,
                inner,
                found: Legality {
                    outer_inner_reductions: Vec::new(),
                    inner_loop_inductions: Vec::new(),
                    has_no_wrap_reductions: Vec::new(),
                    has_no_inf_insts: Vec::new(),
                    inner_reductions: Vec::new(),
                },
            };
            if !checker.can_interchange_loops(se, inner_id, outer_id, matrix) {
                llrm_support::debug!("interchange", "cannot prove legality, not interchanging");
                return false;
            }
            if !is_profitable(se, &cx, self.options, outer, inner, inner_id, outer_id, matrix) {
                llrm_support::debug!("interchange", "not profitable");
                return false;
            }
            Plan::of(
                &cx,
                outer,
                inner,
                checker.found.outer_inner_reductions,
                checker.found.inner_loop_inductions,
                checker.found.has_no_wrap_reductions,
                checker.found.has_no_inf_insts,
                checker.found.inner_reductions,
            )
        };

        transform(self.unit.function, self.unit.context, self.unit.layout, &plan);
        llrm_support::debug!("interchange", "loops interchanged");
        form_lcssa_recursively(self.unit.function, self.unit.context, self.unit.layout, plan.outer_header);
        self.refresh();

        // Loops interchanged: the list and the matrix follow.
        list.swap(outer_id, inner_id);
        interchange_dependencies(matrix, inner_id, outer_id);
        true
    }
}

impl FunctionPass for Interchange {
    fn name(&self) -> &'static str {
        "interchange"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        // Only a function with a nest of two loops that are each the only child
        // of its parent is looked at.
        let has_nest = {
            let tree = llrm_mir::domtree::DomTree::of(unit.function);
            let loops = llrm_mir::loopinfo::LoopInfo::analyze(unit.function, || &tree);
            loops.top_level_loops().any(|top| !collect_perfect_nests(top).is_empty())
        };
        if !has_nest {
            return PreservedAnalyses::all();
        }
        // D5: the loop pass manager's canonical form, on the function as it is
        // kept if no loops are interchanged.
        let saved = unit.function.clone();
        crate::loopsimplify::simplified(unit.function);
        let _ = crate::lcssa::formed(unit.function);

        let mut session = ScevSession::of(unit.context, unit.function, &outer);
        let tops: Vec<BlockId> = {
            let (_, env) = session.split(unit.layout, &outer.globals, &outer.metadata);
            env.loops.top_level_loops().map(|one| one.get_header()).collect()
        };
        let options = self.options.clone();
        let mut changed = false;
        {
            let mut run = Run { unit: &mut *unit, outer: std::rc::Rc::clone(&outer), options: &options, session };

            // The loop nests, last first: the pass manager's worklist is popped
            // from the back.
            for top in tops.into_iter().rev() {
                let lists = {
                    let (_, env) = run.session.split(run.unit.layout, &outer.globals, &outer.metadata);
                    match env.loops.get_loop_for(top) {
                        Some(found) => collect_perfect_nests(found),
                        None => continue,
                    }
                };
                for mut list in lists {
                    // The depth of the nest.
                    let depth = list.len() as u32;
                    if depth < options.min_loop_nest_depth || depth > options.max_loop_nest_depth {
                        continue;
                    }
                    // A computable loop nest.
                    let computable = {
                        let (se, env) = run.session.split(run.unit.layout, &outer.globals, &outer.metadata);
                        let cx = env.cx(&*run.unit.context, &*run.unit.function);
                        is_computable_loop_nest(se, &cx, &list)
                    };
                    if !computable {
                        continue;
                    }
                    changed |= run.process_loop_list(&mut list);
                }
            }
        }
        if !changed {
            *unit.function = saved;
            return PreservedAnalyses::all();
        }
        PreservedAnalyses::none()
    }
}

#[cfg(test)]
#[path = "loopinterchange_tests.rs"]
mod tests;
